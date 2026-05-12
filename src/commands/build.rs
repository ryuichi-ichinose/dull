use crate::build_graph::compute_plan;
use crate::cli::BuildArgs;
use crate::config::{ProjectContext, save_state};
use crate::engine::{BuildOptions, BuildahEngine};
use crate::image_reference::ImageReference;
use anyhow::{Result, bail};
use futures::future::{BoxFuture, FutureExt, Shared};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

pub async fn run_build(
    engine: Arc<BuildahEngine>,
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
    if plan.tasks.is_empty() {
        println!("Nothing to build.");
        return Ok(());
    }

    let concurrency_limit = args.jobs.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    });

    println!("Starting build with max concurrency: {}", concurrency_limit);
    let semaphore = Arc::new(Semaphore::new(concurrency_limit));
    let multi = Arc::new(MultiProgress::new());
    let root = Arc::new(ctx.paths.root.clone());
    let build_tag = Arc::new(args.tag.clone());
    let project_name = Arc::new(ctx.config.project.name.clone());

    // Futures now resolve directly to the completed ImageReference
    let mut task_futures: HashMap<
        String,
        Shared<BoxFuture<'static, Result<ImageReference, String>>>,
    > = HashMap::new();

    for task in plan.tasks {
        let name = task.name.clone();
        let profile = task.profile;
        let engine = Arc::clone(&engine);
        let root = Arc::clone(&root);
        let multi = Arc::clone(&multi);
        let build_tag = Arc::clone(&build_tag);
        let semaphore = Arc::clone(&semaphore);
        let project_name = Arc::clone(&project_name);

        let dep_futures: Vec<_> = task
            .dependencies
            .iter()
            .map(|d| task_futures.get(d).unwrap().clone())
            .collect();

        let task_name = name.clone();
        let task_handle: JoinHandle<Result<ImageReference, String>> = tokio::spawn(async move {
            for dep_fut in dep_futures {
                if let Err(e) = dep_fut.await {
                    return Err(format!("Dependency failed: {}", e));
                }
            }

            let _permit = semaphore
                .acquire()
                .await
                .map_err(|_| "Semaphore closed".to_string())?;

            let pb = multi.add(ProgressBar::new_spinner());
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} [{elapsed_precise}] {msg}")
                    .unwrap(),
            );
            pb.set_message(format!("Building {}...", task_name));
            pb.enable_steady_tick(std::time::Duration::from_millis(100));

            let tag_owned = build_tag
                .as_ref()
                .as_ref()
                .cloned()
                .unwrap_or_else(|| profile.local_tag.clone());

            let dockerfile_path = root.join(&profile.path);
            let context_dir = root.join(&profile.context);

            // Construct strictly typed OCI reference at the system boundary
            let reference = ImageReference {
                registry: Some("localhost".to_string()),
                namespace: Some((*project_name).clone()),
                name: task_name.clone(),
                tag: Some(tag_owned),
                digest: None,
            };

            let pb_inner = pb.clone();

            let res = engine
                .build(
                    &reference,
                    BuildOptions {
                        dockerfile_path: &dockerfile_path,
                        context_dir: &context_dir,
                        build_args: &profile.args,
                        progress_bar: Some(pb_inner),
                    },
                )
                .await;

            match res {
                Ok(completed_ref) => {
                    pb.finish_with_message(format!("{} built", task_name));
                    Ok(completed_ref)
                }
                Err(e) => {
                    let _ = multi.println(format!("\nError building {}:\n{}", task_name, e));
                    pb.abandon_with_message(format!("{} failed", task_name));
                    Err(e.to_string())
                }
            }
        });

        task_futures.insert(
            name,
            task_handle
                .map(|r| r.unwrap_or_else(|e| Err(e.to_string())))
                .boxed()
                .shared(),
        );
    }

    let mut build_errors = Vec::new();
    let mut new_states = BTreeMap::new();
    let task_names: Vec<String> = task_futures.keys().cloned().collect();

    for name in task_names {
        match task_futures[&name].clone().await {
            Ok(completed_ref) => {
                new_states.insert(name, completed_ref);
            }
            Err(e) => build_errors.push(e),
        }
    }

    if !build_errors.is_empty() {
        bail!("Build completed with errors");
    }

    ctx.state.images.extend(new_states);
    ctx.state
        .images
        .retain(|k, _| ctx.config.images.contains_key(k));
    save_state(&ctx)?;

    println!("{} updated.", ctx.paths.state_file.to_string_lossy());
    Ok(())
}
