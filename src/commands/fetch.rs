use crate::cli::FetchArgs;
use crate::config::ProjectContext;
use crate::engine::SkopeoEngine;
use crate::image_reference::{ImageReference, shorten_digest};
use crate::output::multi_println;
use anyhow::{Result, bail};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

async fn fetch_single_image(
    engine: Arc<SkopeoEngine>,
    multi: Arc<MultiProgress>,
    semaphore: Arc<Semaphore>,
    name: String,
    reference: ImageReference,
    is_insecure: bool,
) -> Result<()> {
    let _permit = semaphore.acquire().await.map_err(|e| anyhow::anyhow!(e))?;

    let pb = multi.add(ProgressBar::new_spinner());
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} [{elapsed_precise}] {msg}")
            .unwrap(),
    );
    pb.set_message(format!("Fetching {}...", name));
    pb.enable_steady_tick(Duration::from_millis(100));

    match engine.fetch(&reference, is_insecure).await {
        Ok(digest) => {
            pb.finish_with_message(format!(
                "{} fetched ({}@{})",
                name,
                reference.to_tag_string(),
                shorten_digest(&digest)
            ));
            Ok(())
        }
        Err(e) => {
            multi_println(&multi, format!("Error fetching {}:\n{}", name, e));
            pb.abandon_with_message(format!("{} failed", name));
            Err(e)
        }
    }
}

pub async fn run_fetch(
    engine: Arc<SkopeoEngine>,
    ctx: ProjectContext,
    args: FetchArgs,
) -> Result<()> {
    if ctx.lock.images.is_empty() {
        println!("No dependencies found in lock file. Run 'dull lock' first.");
        return Ok(());
    }

    let mut to_fetch = Vec::new();
    if args.images.is_empty() {
        for (name, reference) in ctx.lock.images.clone() {
            to_fetch.push((name, reference));
        }
    } else {
        for name in &args.images {
            if let Some(reference) = ctx.lock.images.get(name) {
                to_fetch.push((name.clone(), reference.clone()));
            } else {
                bail!("Image '{}' is not found in the lock file.", name);
            }
        }
    }

    if to_fetch.is_empty() {
        println!("Nothing to fetch.");
        return Ok(());
    }

    let fetch_count = to_fetch.len();
    println!("Fetching {} dependencies...", fetch_count);

    let concurrency = std::thread::available_parallelism().map_or(4, |n| n.get());
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let multi = Arc::new(MultiProgress::new());
    let mut handles = Vec::new();

    for (name, reference) in to_fetch {
        let semaphore_clone = Arc::clone(&semaphore);
        let engine_clone = Arc::clone(&engine);
        let multi_clone = Arc::clone(&multi);
        let is_insecure = args.insecure;

        handles.push(tokio::spawn(async move {
            fetch_single_image(
                engine_clone,
                multi_clone,
                semaphore_clone,
                name,
                reference,
                is_insecure,
            )
            .await
        }));
    }

    let mut errors = false;
    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => {}
            _ => errors = true,
        }
    }

    if errors {
        bail!("Fetching completed with errors.");
    }

    drop(multi);
    println!("Fetched {} dependencies.", fetch_count);

    Ok(())
}
