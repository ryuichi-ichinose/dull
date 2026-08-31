use crate::cli::ExportArgs;
use crate::config::ProjectContext;
use crate::engine::SkopeoEngine;
use crate::image_reference::ImageReference;
use crate::output::{self, multi_println};
use anyhow::{Context, Result, bail};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

async fn export_single_image(
    engine: Arc<SkopeoEngine>,
    multi: Arc<MultiProgress>,
    semaphore: Arc<Semaphore>,
    local_reference: ImageReference,
    name: String,
    output_dir: PathBuf,
) -> Result<(String, String)> {
    let _permit = semaphore.acquire().await.expect("Semaphore closed");

    let pb = multi.add(ProgressBar::new_spinner());
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.magenta} [{elapsed_precise}] {msg}")
            .unwrap(),
    );

    pb.set_message(format!("Exporting {} to OCI format...", name));
    pb.enable_steady_tick(Duration::from_millis(100));

    match engine.export_to_oci(&local_reference, &output_dir).await {
        Ok(_) => {
            let local_str = local_reference.to_tag_string();
            let output_path = output_dir.display().to_string();
            pb.finish_with_message(format!("{} exported", local_str));
            Ok((local_str, output_path))
        }
        Err(e) => {
            multi_println(&multi, format!("Error exporting {}:\n{}", name, e));
            pb.abandon_with_message(format!("{} export failed", name));
            Err(e)
        }
    }
}

pub async fn run_export(
    engine: Arc<SkopeoEngine>,
    ctx: ProjectContext,
    args: ExportArgs,
) -> Result<()> {
    let target_names: Vec<String> = if args.all {
        ctx.state.images.keys().cloned().collect()
    } else {
        args.images.clone()
    };

    if target_names.is_empty() {
        println!("No images to export. Run 'dull build' first.");
        return Ok(());
    }

    let base_dir = args.output.unwrap_or_else(|| ctx.paths.root.clone());

    if !base_dir.exists() {
        std::fs::create_dir_all(&base_dir).context(format!(
            "Failed to create output directory: {}",
            base_dir.display()
        ))?;
    }

    println!(
        "Exporting {} images to {}: {}",
        target_names.len(),
        base_dir.display(),
        target_names.join(", ")
    );

    let concurrency = std::thread::available_parallelism().map_or(4, |n| n.get());
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

        let engine_clone = Arc::clone(&engine);
        let multi_clone = Arc::clone(&multi);
        let semaphore_clone = Arc::clone(&semaphore);
        let name_clone = name.clone();

        let safe_dir_name = local_reference.to_tag_string().replace(['/', ':'], "_");

        let output_dir = base_dir.join(&safe_dir_name);

        let name_for_result = name.clone();
        handles.push((
            name_for_result,
            tokio::spawn(async move {
                export_single_image(
                    engine_clone,
                    multi_clone,
                    semaphore_clone,
                    local_reference,
                    name_clone,
                    output_dir,
                )
                .await
            }),
        ));
    }

    let mut successes: Vec<(String, String)> = Vec::new();
    let mut failed_names: Vec<String> = Vec::new();

    for (name, handle) in handles {
        match handle.await {
            Ok(Ok((local_str, output_path))) => successes.push((local_str, output_path)),
            Ok(Err(_)) => failed_names.push(name), // skopeo failed
            Err(_) => failed_names.push(name),     // task panicked/cancelled
        }
    }

    output::finish_multi(multi);

    successes.sort();
    for (local_str, output_path) in &successes {
        println!("+ {} -> {}", local_str, output_path);
    }
    failed_names.sort();
    for name in &failed_names {
        println!("- {} failed", name);
    }

    if !failed_names.is_empty() {
        bail!("Exporting completed with errors.");
    }

    println!("Exported {} images.", successes.len());
    Ok(())
}
