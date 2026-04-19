use crate::build_graph::compute_plan; // 関数化したグラフ計算
use crate::cli::BuildArgs;
use crate::config::{LockImageProfile, ProjectContext, save_lock};
use crate::engine::{BuildOptions, BuildResult, ContainerEngine};
use anyhow::{Context, Result, bail};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

pub fn run_build(
    engine: &impl ContainerEngine,
    mut ctx: ProjectContext,
    args: BuildArgs,
) -> Result<()> {
    let targets: BTreeSet<String> = if args.all {
        ctx.config.images.keys().cloned().collect()
    } else {
        if args.images.is_empty() {
            bail!("Specify image names or use --all");
        }
        args.images.iter().cloned().collect()
    };

    let plan = compute_plan(&ctx.config, &targets)?;

    if plan.stages.is_empty() {
        println!("Nothing to build.");
        return Ok(());
    }

    let threads = args.jobs.unwrap_or(0);
    let pool = ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .context("Failed to build rayon thread pool")?;

    let new_locks = Mutex::new(BTreeMap::new());
    let multi = MultiProgress::new();

    pool.install(|| -> Result<()> {
        for (i, stage) in plan.stages.iter().enumerate() {
            println!("\n-- Stage {} --", i + 1);

            let results: Vec<Result<(String, BuildResult)>> = stage
                .par_iter()
                .map(|name| {
                    let profile = &ctx.config.images[name];
                    let tag = args.tag.as_deref().unwrap_or(&profile.local_tag);

                    let pb = multi.add(ProgressBar::new_spinner());
                    pb.set_style(
                        ProgressStyle::default_spinner()
                            .template("{spinner:.green} [{elapsed_precise}] {msg}")
                            .unwrap(),
                    );
                    pb.set_message(format!("Building {}...", name));
                    pb.enable_steady_tick(std::time::Duration::from_millis(100));

                    let opts = BuildOptions {
                        dockerfile_path: &ctx.root.join(&profile.path),
                        context_dir: &ctx.root.join(&profile.context),
                        target_tag: tag,
                        progress_bar: Some(pb.clone()),
                    };

                    let res = engine.build(name, opts).map(|res| (name.clone(), res));

                    if res.is_ok() {
                        pb.finish_with_message(format!("{} built", name));
                    } else if let Err(ref e) = res {
                        let _ = multi.println(format!("\nError building {}:\n{}", name, e));
                        pb.abandon_with_message(format!("{} failed", name));
                    }

                    res
                })
                .collect();

            let mut success_count = 0;
            let mut errors = Vec::new();

            for res in results {
                match res {
                    Ok((name, build_res)) => {
                        new_locks.lock().unwrap().insert(
                            name,
                            LockImageProfile {
                                digest: build_res.digest,
                                local_ref: build_res.local_ref,
                                registry_digests: BTreeMap::new(),
                            },
                        );
                        success_count += 1;
                    }
                    Err(e) => {
                        // エラーは ProgressBar が表示済みなのでここでは eprintln しない
                        errors.push(e);
                    }
                }
            }

            if !errors.is_empty() {
                bail!(
                    "Stage {} failed with {} errors ({} succeeded)",
                    i + 1,
                    errors.len(),
                    success_count
                );
            }
        }
        Ok(())
    })?;

    ctx.lock.images.extend(new_locks.into_inner().unwrap());

    ctx.lock
        .images
        .retain(|k, _| ctx.config.images.contains_key(k));
    save_lock(&ctx)?;
    println!("\ndull.lock updated.");

    Ok(())
}
