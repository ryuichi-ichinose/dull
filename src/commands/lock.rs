use crate::cli::LockArgs;
use crate::config::{ProjectContext, save_lock};
use crate::engine::SkopeoEngine;
use crate::image_reference::{ImageReference, parse_oci_reference};
use anyhow::{Context, Result, bail};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

async fn lock_single_image(
    engine: Arc<SkopeoEngine>,
    multi: Arc<MultiProgress>,
    semaphore: Arc<Semaphore>,
    reference_str: String,
    reference: ImageReference,
) -> Result<(String, ImageReference)> {
    let _permit = semaphore.acquire().await.map_err(|e| anyhow::anyhow!(e))?;

    let pb = multi.add(ProgressBar::new_spinner());
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap(),
    );
    pb.set_message(format!("Locking {}...", reference.to_tag_string()));
    pb.enable_steady_tick(Duration::from_millis(100));

    match engine.inspect_remote(&reference, false).await {
        Ok(Some(inspected)) => {
            pb.finish_with_message(format!("{} locked", reference.to_tag_string()));
            Ok((reference_str, inspected))
        }
        Ok(None) => {
            let _ = multi.println(format!("Error: Image not found: {}", reference_str));
            pb.abandon_with_message(format!("{} not found", reference.to_tag_string()));
            Err(anyhow::anyhow!("Image not found: {}", reference_str))
        }
        Err(e) => {
            let _ = multi.println(format!("Error locking {}:\n{}", reference_str, e));
            pb.abandon_with_message(format!("{} failed", reference.to_tag_string()));
            Err(e)
        }
    }
}

pub async fn run_lock(
    engine: Arc<SkopeoEngine>,
    mut ctx: ProjectContext,
    args: LockArgs,
) -> Result<()> {
    if ctx.config.dependencies.is_empty() {
        println!("No dependencies defined in dull.toml.");
        return Ok(());
    }
    let mut parsed_deps = Vec::new();
    for dep_str in &ctx.config.dependencies {
        let mut reference = parse_oci_reference(dep_str)
            .with_context(|| format!("Failed to parse dependency: {}", dep_str))?;

        if reference.registry.is_none() {
            bail!(
                "Short-name '{}' is forbidden for reproducibility.\n\
                 Please specify a Fully Qualified Image Name (FQIN) (e.g., 'docker.io/library/ubuntu:latest' or 'quay.io/fedora/fedora:latest').",
                dep_str
            );
        }

        if reference.tag.is_none() && reference.digest.is_none() {
            reference.tag = Some("latest".to_string());
        }

        parsed_deps.push((dep_str.clone(), reference));
    }

    let mut to_resolve = Vec::new();
    let mut final_locks = ctx.lock.images.clone();

    if args.upgrade {
        if args.images.is_empty() {
            to_resolve = parsed_deps;
            final_locks.clear();
        } else {
            for target in &args.images {
                if let Some(pos) = parsed_deps.iter().position(|(s, _)| s == target) {
                    to_resolve.push(parsed_deps[pos].clone());
                    final_locks.remove(target);
                } else {
                    bail!("Dependency '{}' is not defined in dull.toml.", target);
                }
            }
        }
    } else {
        if args.images.is_empty() {
            for (dep_str, reference) in parsed_deps {
                if !ctx.lock.images.contains_key(&dep_str) {
                    to_resolve.push((dep_str, reference));
                }
            }
            if to_resolve.is_empty() {
                bail!(
                    "All dependencies are already locked.\n\
                     Use 'dull lock --upgrade' to update all, or 'dull lock --upgrade <IMAGE>' to update a specific one."
                );
            }
        } else {
            for target in &args.images {
                if let Some(pos) = parsed_deps.iter().position(|(s, _)| s == target) {
                    if ctx.lock.images.contains_key(target) {
                        bail!(
                            "Dependency '{}' is already locked.\n\
                             To overwrite it with the latest digest, use 'dull lock --upgrade {}'",
                            target,
                            target
                        );
                    }
                    to_resolve.push(parsed_deps[pos].clone());
                } else {
                    bail!("Dependency '{}' is not defined in dull.toml.", target);
                }
            }
        }
    }

    println!("Locking {} dependencies...", to_resolve.len());

    let concurrency = std::thread::available_parallelism().map_or(4, |n| n.get());
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let multi = Arc::new(MultiProgress::new());

    let mut handles = Vec::new();

    for (reference_str, reference) in to_resolve {
        let semaphore_clone = Arc::clone(&semaphore);
        let engine_clone = Arc::clone(&engine);
        let multi_clone = Arc::clone(&multi);

        handles.push(tokio::spawn(async move {
            lock_single_image(
                engine_clone,
                multi_clone,
                semaphore_clone,
                reference_str,
                reference,
            )
            .await
        }));
    }

    let mut error_messages = Vec::new();

    for handle in handles {
        match handle.await {
            Ok(Ok((ref_str, locked_ref))) => {
                final_locks.insert(ref_str, locked_ref);
            }
            Ok(Err(e)) => {
                error_messages.push(e.to_string());
            }
            Err(e) => {
                error_messages.push(format!("Task panicked: {}", e));
            }
        }
    }

    if !error_messages.is_empty() {
        drop(multi);
        for err in error_messages {
            eprintln!("Error: {}", err);
        }
        bail!("Locking completed with errors.");
    }

    ctx.lock.images = final_locks;
    save_lock(&ctx)?;

    drop(multi);
    println!("Lock file updated: {:?}", ctx.paths.lock_file);

    Ok(())
}
