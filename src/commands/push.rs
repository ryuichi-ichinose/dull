use crate::cli::PushArgs;
use crate::config::ProjectContext;
use crate::engine::SkopeoEngine;
use crate::image_reference::ImageReference;
use crate::output::{self, multi_println};
use anyhow::{Context, Result, bail};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

#[allow(clippy::too_many_arguments)]
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
    interactive: bool,
) -> Result<(String, String)> {
    let _permit = semaphore.acquire().await.expect("Semaphore closed");

    let pb = multi.add(ProgressBar::new_spinner());
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} [{elapsed_precise}] {msg}")
            .unwrap(),
    );

    pb.set_message(format!("Pushing {}...", name));
    if interactive {
        // A steadily redrawing spinner would fight with gpg/pinentry for the
        // terminal while it prompts for a passphrase/PIN. Leave it static.
        multi
            .println(format!(
                "Pushing {} (may prompt for a passphrase/PIN)...",
                name
            ))
            .ok();
    } else {
        pb.enable_steady_tick(Duration::from_millis(100));
    }

    match engine
        .push(
            &local_reference,
            &dest_reference,
            is_insecure,
            sign_by,
            sign_by_sigstore_private_key,
            sign_passphrase_file,
            interactive,
        )
        .await
    {
        Ok(_) => {
            let local_str = local_reference.to_tag_string();
            let dest_str = dest_reference.to_tag_string();
            pb.finish_with_message(format!("{} -> {}", local_str, dest_str));
            Ok((local_str, dest_str))
        }
        Err(e) => {
            multi_println(&multi, format!("Error pushing {}:\n{}", name, e));
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
    let registry_profile = ctx
        .config
        .registries
        .get(&args.registry)
        .with_context(|| format!("Registry '{}' not configured", args.registry))?;

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
        "Pushing {} images to {}: {}",
        target_names.len(),
        registry_base,
        target_names.join(", ")
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

    for name in target_names {
        let local_reference = match ctx.state.images.get(&name) {
            Some(info) => info.clone(),
            None => {
                multi_println(
                    &multi,
                    format!("Warning: {} not tracked; build it first.", name),
                );
                continue;
            }
        };

        let target_tag = match args.tag.clone().or_else(|| local_reference.tag.clone()) {
            Some(tag) => tag,
            None => {
                multi_println(&multi, format!("Warning: {} has no tag; skipping.", name));
                continue;
            }
        };

        let dest_reference = ImageReference {
            registry: Some(registry_base.clone()),
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

        let name_for_result = name.clone();
        handles.push((
            name_for_result,
            tokio::spawn(async move {
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
                    requires_interactive_prompt,
                )
                .await
            }),
        ));
    }

    let mut successes: Vec<(String, String)> = Vec::new();
    let mut failed_names: Vec<String> = Vec::new();

    for (name, handle) in handles {
        match handle.await {
            Ok(Ok((local_str, dest_str))) => successes.push((local_str, dest_str)),
            Ok(Err(_)) => failed_names.push(name), // skopeo failed
            Err(_) => failed_names.push(name),     // task panicked/cancelled
        }
    }

    output::finish_multi(multi);

    successes.sort();
    for (local_str, dest_str) in &successes {
        println!("+ {} -> {}", local_str, dest_str);
    }
    failed_names.sort();
    for name in &failed_names {
        println!("- {} failed", name);
    }

    if !failed_names.is_empty() {
        bail!("Pushing completed with errors.");
    }

    println!("Pushed {} images.", successes.len());
    Ok(())
}
