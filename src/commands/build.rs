use crate::build_graph::compute_plan;
use crate::build_lock::BuildLock;
use crate::cli::{BuildArgs, LogsMode};
use crate::config::{ProjectContext, save_state};
use crate::engine::{BuildOptions, BuildahEngine};
use crate::image_reference::ImageReference;
use crate::output::{self, multi_println};
use anyhow::{Context, Result, bail};
use futures::future::{FutureExt, Shared};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;
use tokio::task::AbortHandle;

#[derive(Clone)]
enum BuildFailure {
    Failed,
    Skipped(String),
}

type SharedTaskFuture =
    Shared<futures::future::BoxFuture<'static, Result<ImageReference, BuildFailure>>>;

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

    let mut target_names: Vec<&String> = plan.tasks.iter().map(|t| &t.name).collect();
    target_names.sort();
    println!(
        "Building {} images: {}",
        target_names.len(),
        target_names
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
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
    let abort_handles: Arc<Mutex<Vec<AbortHandle>>> = Arc::new(Mutex::new(Vec::new()));
    let logger_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>> =
        Arc::new(Mutex::new(Vec::new()));
    let aborting = Arc::new(AtomicBool::new(false));
    let keep_going = args.keep_going;
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

        let safe_tag_name = reference.to_tag_string().replace(['/', ':'], "_");
        let log_path = session_log_dir.join(format!("{}.log", safe_tag_name));
        log_paths.insert(name.clone(), log_path.clone());

        let engine = Arc::clone(&engine);
        let semaphore = Arc::clone(&semaphore);
        let multi = Arc::clone(&multi);
        let abort_handles_for_task = Arc::clone(&abort_handles);
        let logger_handles_for_task = Arc::clone(&logger_handles);
        let aborting = Arc::clone(&aborting);
        let root = base_root.clone();

        let dep_futures: Vec<_> = task
            .dependencies
            .iter()
            .map(|d| {
                (
                    d.clone(),
                    task_futures
                        .get(d)
                        .expect("Dependency missing in graph")
                        .clone(),
                )
            })
            .collect();

        let name_key = name.clone();

        let task_future = tokio::spawn(async move {
            for (dep_name, dep) in dep_futures {
                dep.await.map_err(|_| {
                    BuildFailure::Skipped(format!("dependency '{}' failed", dep_name))
                })?;
            }

            let _permit = semaphore
                .acquire()
                .await
                .map_err(|_| BuildFailure::Skipped("Semaphore closed".to_string()))?;

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

            let logger_handle = tokio::spawn(async move {
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
            logger_handles_for_task.lock().unwrap().push(logger_handle);

            let opts = BuildOptions {
                dockerfile_path: &root.join(&profile.path),
                context_dir: &root.join(&profile.context),
                build_args: &profile.args,
                secrets: &profile.secrets,
            };

            match engine.build(&reference, opts, Some(log_tx)).await {
                Ok(completed_ref) => {
                    pb.finish_with_message(format!("{} built", reference.name));
                    Ok(completed_ref)
                }
                Err(e) => {
                    if !keep_going && !aborting.swap(true, Ordering::SeqCst) {
                        multi_println(
                            &multi,
                            "Aborting remaining builds due to an earlier failure...",
                        );
                        for handle in abort_handles_for_task.lock().unwrap().iter() {
                            handle.abort();
                        }
                    }
                    multi_println(&multi, format!("Error building {}:\n{}", reference.name, e));
                    pb.abandon_with_message(format!("{} failed", reference.name));
                    Err(BuildFailure::Failed)
                }
            }
        });

        abort_handles
            .lock()
            .unwrap()
            .push(task_future.abort_handle());

        task_futures.insert(
            name_key,
            task_future
                .map(|r| {
                    r.unwrap_or_else(|join_err| {
                        if join_err.is_cancelled() {
                            Err(BuildFailure::Skipped(
                                "aborted after an earlier failure".to_string(),
                            ))
                        } else {
                            Err(BuildFailure::Failed)
                        }
                    })
                })
                .boxed()
                .shared(),
        );
    }

    let mut successes: Vec<(String, ImageReference)> = Vec::new();
    let mut failed_names: Vec<String> = Vec::new();
    let mut skipped: Vec<(String, String)> = Vec::new();
    for (name, fut) in task_futures {
        match fut.await {
            Ok(completed_ref) => {
                successes.push((name.clone(), completed_ref.clone()));
                ctx.state.images.insert(name, completed_ref);
            }
            Err(BuildFailure::Failed) => failed_names.push(name),
            Err(BuildFailure::Skipped(reason)) => skipped.push((name, reason)),
        }
    }

    let pending_loggers: Vec<_> = logger_handles.lock().unwrap().drain(..).collect();
    for handle in pending_loggers {
        let _ = handle.await;
    }

    output::finish_multi(multi);

    successes.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, reference) in &successes {
        println!("+ {}", reference.to_tag_string());
    }
    failed_names.sort();
    for name in &failed_names {
        println!("- {} failed", name);
    }
    skipped.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, reason) in &skipped {
        println!("- {} skipped ({})", name, reason);
    }

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

    if !failed_names.is_empty() || !skipped.is_empty() {
        bail!("Build completed with errors.");
    }

    println!("Built {} images.", successes.len());

    Ok(())
}
