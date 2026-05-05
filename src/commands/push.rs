use crate::cli::PushArgs;
use crate::config::{ProjectContext, save_state};
use crate::engine::BuildahEngine;
use anyhow::{Context, Result};
use std::sync::Arc;

pub async fn run_push(
    engine: Arc<BuildahEngine>,
    mut ctx: ProjectContext,
    args: PushArgs,
) -> Result<()> {
    let target_reg_name = args.registry.as_deref().unwrap_or("default");

    let registry_profile = ctx
        .config
        .registries
        .get(target_reg_name)
        .with_context(|| format!("Registry '{}' not configured", target_reg_name))?;

    let registry_base = &registry_profile.url;
    // Determine target images
    let target_names: Vec<String> = if args.all {
        ctx.state.images.keys().cloned().collect()
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
        let state_info = match ctx.state.images.get(&name) {
            Some(info) => info.clone(),
            None => {
                eprintln!("Warning: No state entry for {}. Build it first.", name);
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
            match engine.push(&state_info.local_ref, &destination).await {
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
            && let Some(info) = ctx.state.images.get_mut(&name)
        {
            info.registry_digests.insert("default".to_string(), digest);
            updated_count += 1;
        }
    }

    if updated_count > 0 {
        save_state(&ctx)?;
        println!(
            "{} updated with {} registry digests.",
            ctx.paths.state_file.to_string_lossy(),
            updated_count
        );
    } else {
        println!("No images were successfully pushed.");
    }

    Ok(())
}
