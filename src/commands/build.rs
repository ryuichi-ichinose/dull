use crate::build_graph::BuildGraph;
use crate::cli::BuildArgs;
use crate::config::{LockImageProfile, ProjectContext};
use crate::engine::{BuildOptions, BuildResult, ContainerEngine};
use anyhow::{Result, bail};
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

pub fn run_build(
    engine: &impl ContainerEngine,
    ctx: &mut ProjectContext,
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

    let graph = BuildGraph::new(&ctx.config);
    let plan = graph.create_plan(&targets)?;

    if plan.stages.is_empty() {
        println!("Nothing to build.");
        return Ok(());
    }

    let new_locks = Mutex::new(BTreeMap::new());

    for (i, stage) in plan.stages.iter().enumerate() {
        println!("\n-- Stage {} --", i + 1);

        let results: Vec<Result<(String, BuildResult)>> = stage
            .par_iter()
            .map(|name| {
                let profile = &ctx.config.images[name];
                let tag = args.tag.as_deref().unwrap_or(&profile.local_tag);

                let opts = BuildOptions {
                    dockerfile_path: &ctx.absolute_path(&profile.path),
                    context_dir: &ctx.absolute_path(&profile.context),
                    target_tag: tag,
                };

                engine.build(name, opts).map(|res| (name.clone(), res))
            })
            .collect();

        for res in results {
            let (name, build_res) = res?;
            new_locks.lock().unwrap().insert(
                name,
                LockImageProfile {
                    digest: build_res.digest,
                    local_ref: build_res.local_ref,
                },
            );
        }
    }

    if !args.no_lock {
        ctx.lock.images.extend(new_locks.into_inner().unwrap());
        ctx.lock
            .images
            .retain(|k, _| ctx.config.images.contains_key(k));
        ctx.save_lock()?;
        println!("\n🔒 deck.lock updated.");
    }

    Ok(())
}
