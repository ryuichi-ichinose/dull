use crate::cli::PushArgs;
use crate::config::ProjectContext;
use crate::engine::SkopeoEngine;
use crate::image_reference::ImageReference;
use anyhow::{Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::Path;
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
    sign_by: Option<&str>,
    sign_by_sigstore_private_key: Option<&Path>,
    sign_passphrase_file: Option<&Path>,
) -> Result<String> {
    let _permit = semaphore.acquire().await.expect("Semaphore closed");

    let pb = multi.add(ProgressBar::new_spinner());
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} [{elapsed_precise}] {msg}")
            .unwrap(),
    );

    pb.set_message(format!("Pushing {}...", name));
    pb.enable_steady_tick(Duration::from_millis(100));

    match engine
        .push(
            &local_reference,
            &dest_reference,
            is_insecure,
            sign_by,
            sign_by_sigstore_private_key,
            sign_passphrase_file,
        )
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

    let requires_interactive_prompt = (args.sign_by.is_some()
        || args.sign_by_sigstore_private_key.is_some())
        && args.sign_passphrase_file.is_none();

    let concurrency = if requires_interactive_prompt {
        println!(
            "Interactive signature requested: Falling back to sequential execution to prevent TTY conflicts."
        );
        1
    } else {
        args.jobs
            .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get()))
    };

    println!("Starting push with max concurrency: {}", concurrency);

    let multi = Arc::new(MultiProgress::new());
    let semaphore = Arc::new(Semaphore::new(concurrency));

    let mut handles = Vec::new();
    let base_namespace = ctx.config.namespace.clone();

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

        let target_tag = match args.tag.clone().or_else(|| local_reference.tag.clone()) {
            Some(tag) => tag,
            None => {
                let _ = multi.println(format!(
                    "Warning: No tag specified or found in build state for {}. Push aborted.",
                    name
                ));
                continue;
            }
        };

        let dest_reference = ImageReference {
            registry: Some(registry_base.clone()),
            namespace: base_namespace.clone(),
            name: name.clone(),
            tag: Some(target_tag),
            digest: None,
        };

        let engine_clone = Arc::clone(&engine);
        let multi_clone = Arc::clone(&multi);
        let semaphore_clone = Arc::clone(&semaphore);
        let name_clone = name.clone();
        let is_insecure = args.insecure;
        let sign_by_clone = args.sign_by.clone();
        let sign_by_sigstore_private_key_clone = args.sign_by_sigstore_private_key.clone();
        let sign_passphrase_file_clone = args.sign_passphrase_file.clone();

        handles.push(tokio::spawn(async move {
            push_single_image(
                engine_clone,
                multi_clone,
                semaphore_clone,
                local_reference,
                dest_reference,
                name_clone,
                is_insecure,
                sign_by_clone.as_deref(),
                sign_by_sigstore_private_key_clone.as_deref(),
                sign_passphrase_file_clone.as_deref(),
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
