use crate::config::{ProjectContext, save_lock};
use crate::engine::SkopeoEngine;
use crate::image_reference::{ImageReference, parse_oci_reference};
use anyhow::{Context, Result, bail};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

async fn lock_single_image(
    engine: Arc<SkopeoEngine>,
    multi: Arc<MultiProgress>,
    semaphore: Arc<Semaphore>,
    reference_str: String,
) -> Result<(String, ImageReference)> {
    let _permit = semaphore.acquire().await.map_err(|e| anyhow::anyhow!(e))?;

    let reference = parse_oci_reference(&reference_str)
        .with_context(|| format!("Failed to parse dependency: {}", reference_str))?;

    if reference.registry.is_none() {
        bail!(
            "Short-name '{}' is forbidden for reproducibility.\n\
         Please specify a Fully Qualified Image Name (FQIN) (e.g., 'docker.io/library/ubuntu:latest' or 'quay.io/fedora/fedora:latest').",
            reference_str
        );
    }

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

pub async fn run_lock(engine: Arc<SkopeoEngine>, mut ctx: ProjectContext) -> Result<()> {
    if ctx.config.dependencies.is_empty() {
        println!("No dependencies to lock.");
        return Ok(());
    }

    println!("Locking {} dependencies...", ctx.config.dependencies.len());

    let concurrency = std::thread::available_parallelism().map_or(4, |n| n.get());
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let multi = Arc::new(MultiProgress::new());

    let mut handles = Vec::new();

    for reference_str in ctx.config.dependencies.clone() {
        let semaphore_clone = Arc::clone(&semaphore);
        let engine_clone = Arc::clone(&engine);
        let multi_clone = Arc::clone(&multi);

        handles.push(tokio::spawn(async move {
            lock_single_image(engine_clone, multi_clone, semaphore_clone, reference_str).await
        }));
    }

    let mut locked_images = BTreeMap::new();
    let mut errors = false;

    for handle in handles {
        match handle.await {
            Ok(Ok((ref_str, locked_ref))) => {
                locked_images.insert(ref_str, locked_ref);
            }
            _ => errors = true,
        }
    }

    if errors {
        anyhow::bail!("Locking completed with errors.");
    }

    ctx.lock.images = locked_images;
    save_lock(&ctx)?;

    drop(multi);
    println!("Lock file updated: {:?}", ctx.paths.lock_file);

    Ok(())
}
