use crate::cli::PushArgs;
use crate::config::ProjectContext;
use crate::engine::SkopeoEngine;
use crate::image_reference::ImageReference;
use anyhow::{Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

async fn push_single_image(
    engine: Arc<SkopeoEngine>,
    multi: Arc<MultiProgress>,
    semaphore: Arc<Semaphore>,
    local_reference: ImageReference,
    dest_reference: ImageReference,
    name: String,
    is_insecure: bool,
    is_force: bool,
) -> Result<String> {
    let _permit = semaphore.acquire().await.expect("Semaphore closed");

    let pb = multi.add(ProgressBar::new_spinner());
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} [{elapsed_precise}] {msg}")
            .unwrap(),
    );

    pb.set_message(format!(
        "Checking remote {}...",
        dest_reference.to_tag_string()
    ));
    pb.enable_steady_tick(Duration::from_millis(100));

    match engine.inspect_remote(&dest_reference, is_insecure).await {
        Ok(Some(remote_ref)) => {
            if !is_force {
                let remote_tag = remote_ref.to_tag_string();
                let err_msg = format!(
                    "{} already exists.\nUse --force (-f) to overwrite it.",
                    remote_tag
                );
                let _ = multi.println(format!("Conflict for {}:\n{}", name, err_msg));
                pb.abandon_with_message(format!("{} blocked", name));
                return Err(anyhow::anyhow!("Conflict: Tag already exists"));
            } else {
                let _ = multi.println(format!(
                    "Warning: Force overwriting {}...",
                    remote_ref.to_tag_string()
                ));
            }
        }
        Ok(None) => {}
        Err(e) => {
            let _ = multi.println(format!("Error checking {}:\n{}", name, e));
            pb.abandon_with_message(format!("{} check failed", name));
            return Err(e);
        }
    }

    pb.set_message(format!("Pushing {}...", name));

    match engine
        .push(&local_reference, &dest_reference, is_insecure)
        .await
    {
        Ok(_) => {
            let local_str = local_reference.to_tag_string();
            let dest_str = dest_reference.to_tag_string();
            pb.finish_with_message(format!("{} → {}", local_str, dest_str));
            Ok(name)
        }
        Err(e) => {
            let _ = multi.println(format!("Error pushing {}:\n{}", name, e));
            pb.abandon_with_message(format!("{} failed", name));
            Err(e)
        }
    }
}

pub async fn run_push(
    engine: Arc<SkopeoEngine>,
    ctx: ProjectContext,
    args: PushArgs,
) -> Result<()> {
    let target_reg_name = args.registry.as_deref().unwrap_or("default");

    let registry_profile = ctx
        .config
        .registries
        .get(target_reg_name)
        .with_context(|| format!("Registry '{}' not configured", target_reg_name))?;

    let registry_base = &registry_profile.url;

    let target_names: Vec<String> = if args.all {
        ctx.state.images.keys().cloned().collect()
    } else {
        args.images.clone()
    };

    if target_names.is_empty() {
        println!("No images to push.");
        return Ok(());
    }

    println!(
        "Pushing {} images to {}...",
        target_names.len(),
        registry_base
    );

    let concurrency = args
        .jobs
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get()));

    println!("Starting push with max concurrency: {}", concurrency);

    let multi = Arc::new(MultiProgress::new());
    let semaphore = Arc::new(Semaphore::new(concurrency));

    let mut handles = Vec::new();
    let base_namespace = ctx.config.project.namespace.clone();

    for name in target_names {
        let local_reference = match ctx.state.images.get(&name) {
            Some(info) => info.clone(),
            None => {
                let _ = multi.println(format!(
                    "Warning: No state entry for {}. Build it first.",
                    name
                ));
                continue;
            }
        };

        let target_tag = args.tag.clone().unwrap_or_else(|| {
            local_reference
                .tag
                .clone()
                .unwrap_or_else(|| "latest".to_string())
        });

        let dest_reference = ImageReference {
            registry: Some(registry_base.clone()),
            namespace: Some(base_namespace.clone()),
            name: name.clone(),
            tag: Some(target_tag),
            digest: None,
        };

        let engine_clone = Arc::clone(&engine);
        let multi_clone = Arc::clone(&multi);
        let semaphore_clone = Arc::clone(&semaphore);
        let name_clone = name.clone();
        let is_insecure = args.insecure;
        let is_force = args.force;

        handles.push(tokio::spawn(async move {
            push_single_image(
                engine_clone,
                multi_clone,
                semaphore_clone,
                local_reference,
                dest_reference,
                name_clone,
                is_insecure,
                is_force,
            )
            .await
        }));
    }

    let mut pushed_count = 0;
    for handle in handles {
        if let Ok(Ok(_)) = handle.await {
            pushed_count += 1;
        }
    }

    drop(multi);

    println!("{} images were successfully pushed.", pushed_count);
    Ok(())
}
