use crate::build_graph::compute_plan;
use crate::build_lock::BuildLock;
use crate::cli::{BuildArgs, LogsMode};
use crate::config::{ProjectContext, save_state};
use crate::engine::{BuildOptions, BuildahEngine};
use crate::image_reference::ImageReference;
use anyhow::{Context, Result, bail};
use futures::future::{FutureExt, Shared};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;

type SharedTaskFuture = Shared<futures::future::BoxFuture<'static, Result<ImageReference, String>>>;

pub async fn run_build(
    engine: Arc<BuildahEngine>,
    mut ctx: ProjectContext,
    args: BuildArgs,
) -> Result<()> {
    let _build_lock = BuildLock::acquire(&ctx.paths.build_lock_file)?;

    let targets: BTreeSet<String> = if args.all {
        ctx.config.images.keys().cloned().collect()
    } else if let Some(group_name) = &args.group {
        let group = ctx.config.build_groups.get(group_name).with_context(|| {
            format!("Build group '{}' is not defined in dull.toml.", group_name)
        })?;
        let mut set: BTreeSet<String> = group.images.iter().cloned().collect();
        set.extend(group.base.clone());
        set
    } else if args.images.is_empty() {
        bail!("Specify image names, use --all, or use --group <name>");
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

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    let session_log_dir = ctx.paths.dull_dir.join("logs").join(&timestamp);
    std::fs::create_dir_all(&session_log_dir)?;

    let semaphore = Arc::new(Semaphore::new(concurrency));
    let multi = Arc::new(MultiProgress::new());
    let mut task_futures: HashMap<String, SharedTaskFuture> = HashMap::new();
    let mut log_paths: HashMap<String, PathBuf> = HashMap::new();

    let base_root = ctx.paths.root.clone();

    for task in plan.tasks {
        let name = task.name;
        let profile = task.profile;
        let build_tag = args.tag.clone().unwrap_or(profile.local_tag);

        let reference = ImageReference {
            registry: Some("localhost".into()),
            name: name.clone(),
            tag: Some(build_tag),
            digest: None,
        };

        let safe_tag_name = reference
            .to_tag_string()
            .replace('/', "_")
            .replace(':', "_");
        let log_path = session_log_dir.join(format!("{}.log", safe_tag_name));
        log_paths.insert(name.clone(), log_path.clone());

        let engine = Arc::clone(&engine);
        let semaphore = Arc::clone(&semaphore);
        let multi = Arc::clone(&multi);
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

            let (log_tx, mut log_rx) = tokio::sync::mpsc::channel::<String>(100);
            let pb_clone = pb.clone();

            tokio::spawn(async move {
                if let Ok(mut log_file) = tokio::fs::File::create(&log_path).await {
                    while let Some(line) = log_rx.recv().await {
                        pb_clone.set_message(line.clone());
                        let _ = log_file.write_all(format!("{}\n", line).as_bytes()).await;
                    }
                    let _ = log_file.sync_all().await;
                } else {
                    while let Some(line) = log_rx.recv().await {
                        pb_clone.set_message(line);
                    }
                }
            });

            let opts = BuildOptions {
                dockerfile_path: &root.join(&profile.path),
                context_dir: &root.join(&profile.context),
                build_args: &profile.args,
            };

            match engine.build(&reference, opts, Some(log_tx)).await {
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

    let mut failed_names: Vec<String> = Vec::new();
    for (name, fut) in task_futures {
        match fut.await {
            Ok(completed_ref) => {
                ctx.state.images.insert(name, completed_ref);
            }
            Err(_) => failed_names.push(name),
        }
    }

    drop(multi);

    if let Some(mode) = &args.logs {
        let names_to_dump: Vec<&String> = match mode {
            LogsMode::Errors => failed_names.iter().collect(),
            LogsMode::All => {
                let mut names: Vec<&String> = log_paths.keys().collect();
                names.sort();
                names
            }
        };

        for name in names_to_dump {
            if let Some(log_path) = log_paths.get(name) {
                println!("=== {} ===", name);
                match std::fs::read_to_string(log_path) {
                    Ok(contents) => println!("{}", contents),
                    Err(e) => println!("(failed to read log: {})", e),
                }
            }
        }
    }

    ctx.state
        .images
        .retain(|k, _| ctx.config.images.contains_key(k));
    save_state(&ctx)?;

    if !failed_names.is_empty() {
        bail!("Build completed with errors.");
    }

    println!(
        "Successfully updated {}",
        ctx.paths.state_file.to_string_lossy()
    );

    Ok(())
}
