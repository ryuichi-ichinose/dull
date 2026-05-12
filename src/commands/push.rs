use crate::cli::PushArgs;
use crate::config::ProjectContext;
use crate::engine::BuildahEngine;
use crate::image_reference::ImageReference;
use anyhow::{Context, Result};
use std::sync::Arc;

pub async fn run_push(
    engine: Arc<BuildahEngine>,
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

    println!(
        "Pushing {} images to {}...",
        target_names.len(),
        registry_base
    );

    let mut handles = Vec::new();
    let project_namespace = ctx.config.project.name.clone();

    for name in target_names {
        let local_reference = match ctx.state.images.get(&name) {
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

        let dest_reference = ImageReference {
            registry: Some(registry_base.clone()),
            namespace: Some(project_namespace.clone()),
            name: name.clone(),
            tag: Some(local_tag),
            digest: None,
        };

        let engine = Arc::clone(&engine);
        let name_clone = name.clone();

        handles.push(tokio::spawn(async move {
            println!("-> Pushing {}...", name_clone);

            match engine.push(&local_reference, &dest_reference).await {
                Ok(remote_digest) => {
                    println!(
                        "DONE: {} pushed. Remote Digest: {}",
                        name_clone, remote_digest
                    );
                    Ok(name_clone)
                }
                Err(e) => {
                    eprintln!("FAIL: Failed to push {}: {}", name_clone, e);
                    Err(e)
                }
            }
        }));
    }

    let mut pushed_count = 0;
    for handle in handles {
        if let Ok(Ok(_)) = handle.await {
            pushed_count += 1;
        }
    }

    println!("{} images were successfully pushed.", pushed_count);
    Ok(())
}
