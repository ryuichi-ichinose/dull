use crate::cli::PushArgs;
use crate::config::{ProjectContext, save_lock};
use crate::engine::ContainerEngine;
use anyhow::{Context, Result};
use std::sync::Arc;

pub async fn run_push(
    engine: Arc<impl ContainerEngine + 'static>,
    mut ctx: ProjectContext,
    args: PushArgs,
) -> Result<()> {
    let project = ctx
        .config
        .project
        .as_ref()
        .context("Project configuration missing")?;
    let registry_base = project
        .registries
        .get("default")
        .context("Default registry not configured")?;

    // Determine target images
    let target_names: Vec<String> = if args.all {
        ctx.lock.images.keys().cloned().collect()
    } else {
        args.images.clone()
    };

    println!(
        "Pushing {} images to {}...",
        target_names.len(),
        registry_base
    );

    let mut handles = Vec::new();

    for name in target_names {
        let lock_info = match ctx.lock.images.get(&name) {
            Some(info) => info.clone(),
            None => {
                eprintln!("Warning: No lock entry for {}. Build it first.", name);
                continue;
            }
        };

        let local_tag = ctx
            .config
            .images
            .get(&name)
            .map(|p| p.local_tag.as_str())
            .unwrap_or("latest")
            .to_string();

        let destination = format!("docker://{}/{}:{}", registry_base, name, local_tag);
        let engine = Arc::clone(&engine);
        let name_clone = name.clone();

        handles.push(tokio::spawn(async move {
            println!("-> Pushing {}...", name_clone);
            match engine.push(&lock_info.local_ref, &destination).await {
                Ok(registry_digest) => {
                    println!("DONE: {} pushed. Digest: {}", name_clone, registry_digest);
                    Ok((name_clone, registry_digest))
                }
                Err(e) => {
                    eprintln!("FAIL: Failed to push {}: {}", name_clone, e);
                    Err(e)
                }
            }
        }));
    }

    let mut updated_count = 0;
    for handle in handles {
        if let Ok(Ok((name, digest))) = handle.await
            && let Some(info) = ctx.lock.images.get_mut(&name)
        {
            info.registry_digests.insert("default".to_string(), digest);
            updated_count += 1;
        }
    }

    if updated_count > 0 {
        save_lock(&ctx)?;
        println!("dull.lock updated with {} registry digests.", updated_count);
    } else {
        println!("No images were successfully pushed.");
    }

    Ok(())
}
