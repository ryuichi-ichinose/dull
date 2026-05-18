use crate::build_graph::compute_plan;
use crate::cli::BuildArgs;
use crate::config::{ProjectContext, save_state};
use crate::engine::{BuildOptions, BuildahEngine};
use crate::image_reference::ImageReference;
use anyhow::{Result, bail};
use futures::future::{FutureExt, Shared};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

type SharedTaskFuture = Shared<futures::future::BoxFuture<'static, Result<ImageReference, String>>>;

pub async fn run_build(
    engine: Arc<BuildahEngine>,
    mut ctx: ProjectContext,
    args: BuildArgs,
) -> Result<()> {
    let targets: BTreeSet<String> = if args.all {
        ctx.config.images.keys().cloned().collect()
    } else if args.images.is_empty() {
        bail!("Specify image names or use --all");
    } else {
        args.images.iter().cloned().collect()
    };

    let plan = compute_plan(&ctx.config, &targets)?;
    if plan.tasks.is_empty() {
        println!("Nothing to build.");
        return Ok(());
    }

    let concurrency = args
        .jobs
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get()));

    println!("Starting build with max concurrency: {}", concurrency);

    let semaphore = Arc::new(Semaphore::new(concurrency));
    let multi = Arc::new(MultiProgress::new());
    let mut task_futures: HashMap<String, SharedTaskFuture> = HashMap::new();

    let base_project_name = ctx.config.project.name.clone();
    let base_root = ctx.paths.root.clone();

    for task in plan.tasks {
        let name = task.name;
        let profile = task.profile;
        let build_tag = args.tag.clone().unwrap_or(profile.local_tag);

        let engine = Arc::clone(&engine);
        let semaphore = Arc::clone(&semaphore);
        let multi = Arc::clone(&multi);
        let project_name = base_project_name.clone();
        let root = base_root.clone();

        let dep_futures: Vec<_> = task
            .dependencies
            .iter()
            .map(|d| {
                task_futures
                    .get(d)
                    .expect("Dependency missing in graph")
                    .clone()
            })
            .collect();

        let name_key = name.clone();

        let task_future = tokio::spawn(async move {
            for dep in dep_futures {
                dep.await.map_err(|e| format!("Dependency failed: {}", e))?;
            }

            let _permit = semaphore.acquire().await.map_err(|_| "Semaphore closed")?;

            let pb = multi.add(ProgressBar::new_spinner());
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} [{elapsed_precise}] {msg}")
                    .unwrap(),
            );
            pb.set_message(format!("Building {}...", name));
            pb.enable_steady_tick(Duration::from_millis(100));

            let reference = ImageReference {
                registry: Some("localhost".into()),
                namespace: Some(project_name),
                name: name,
                tag: Some(build_tag),
                digest: None,
            };

            let opts = BuildOptions {
                dockerfile_path: &root.join(&profile.path),
                context_dir: &root.join(&profile.context),
                build_args: &profile.args,
                progress_bar: Some(pb.clone()),
            };

            match engine.build(&reference, opts).await {
                Ok(completed_ref) => {
                    pb.finish_with_message(format!("{} built", reference.name));
                    Ok(completed_ref)
                }
                Err(e) => {
                    let _ = multi.println(format!("Error building {}:\n{}", reference.name, e));
                    pb.abandon_with_message(format!("{} failed", reference.name));
                    Err(e.to_string())
                }
            }
        });

        task_futures.insert(
            name_key,
            task_future
                .map(|r| r.unwrap_or_else(|e| Err(e.to_string())))
                .boxed()
                .shared(),
        );
    }

    let mut build_errors = false;
    for (name, fut) in task_futures {
        match fut.await {
            Ok(completed_ref) => {
                ctx.state.images.insert(name, completed_ref);
            }
            Err(_) => build_errors = true,
        }
    }

    if build_errors {
        bail!("Build completed with errors.");
    }

    ctx.state
        .images
        .retain(|k, _| ctx.config.images.contains_key(k));
    save_state(&ctx)?;

    drop(multi);
    println!(
        "Successfully updated {}",
        ctx.paths.state_file.to_string_lossy()
    );

    Ok(())
}
