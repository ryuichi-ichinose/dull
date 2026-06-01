use crate::config::ProjectContext;
use crate::engine::SkopeoEngine;
use crate::image_reference::ImageReference;
use anyhow::Result;
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

    match engine.fetch(&reference, false).await {
        Ok(_) => {
            pb.finish_with_message(format!("{} fetched", name));
            Ok(())
        }
        Err(e) => {
            let _ = multi.println(format!("Error fetching {}:\n{}", name, e));
            pb.abandon_with_message(format!("{} failed", name));
            Err(e)
        }
    }
}

pub async fn run_fetch(engine: Arc<SkopeoEngine>, ctx: ProjectContext) -> Result<()> {
    if ctx.lock.images.is_empty() {
        println!("No dependencies found in lock file. Run 'dull lock' first.");
        return Ok(());
    }

    println!("Fetching {} dependencies...", ctx.lock.images.len());

    let concurrency = std::thread::available_parallelism().map_or(4, |n| n.get());
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let multi = Arc::new(MultiProgress::new());

    let mut handles = Vec::new();

    for (name, reference) in ctx.lock.images.clone() {
        let semaphore_clone = Arc::clone(&semaphore);
        let engine_clone = Arc::clone(&engine);
        let multi_clone = Arc::clone(&multi);

        handles.push(tokio::spawn(async move {
            fetch_single_image(engine_clone, multi_clone, semaphore_clone, name, reference).await
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
        anyhow::bail!("Fetching completed with errors.");
    }

    drop(multi);
    println!("All dependencies fetched.");

    Ok(())
}
