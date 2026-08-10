use crate::cli::RmiArgs;
use crate::config::{DullState, ProjectContext, save_state};
use crate::engine::BuildahEngine;
use anyhow::{Result, bail};
use std::collections::HashSet;

pub async fn run_rmi(engine: &BuildahEngine, mut ctx: ProjectContext, args: RmiArgs) -> Result<()> {
    if args.clean {
        return run_clean(engine, ctx).await;
    }
    if args.gc {
        return run_gc(engine, ctx, args.state_only, args.lock_only).await;
    }
    if args.images.is_empty() {
        bail!("Specify an image name, or use --gc / --clean");
    }

    let mut tags = Vec::new();
    for name in &args.images {
        match ctx.state.images.get(name) {
            Some(reference) => tags.push(reference.to_tag_string()),
            None => {
                println!("Warning: No state entry for {}. Skipping.", name);
            }
        }
    }

    if tags.is_empty() {
        println!("Nothing to remove.");
        return Ok(());
    }

    engine.remove_images(&tags).await?;

    ctx.state
        .images
        .retain(|name, _| !args.images.contains(name));
    save_state(&ctx)?;

    for name in &args.images {
        println!("- Removed: {}", name);
    }
    Ok(())
}

async fn run_clean(engine: &BuildahEngine, mut ctx: ProjectContext) -> Result<()> {
    if !ctx.paths.storage_conf.exists() {
        println!("Nothing to clean. (.dull/storage.conf not found)");
        return Ok(());
    }

    println!("Cleaning up containers and images in project storage...");
    // Delegate physical destruction to the engine
    engine.clean().await?;

    ctx.state = DullState::default();
    save_state(&ctx)?;

    println!("Clean completed.");
    Ok(())
}

async fn run_gc(
    engine: &BuildahEngine,
    ctx: ProjectContext,
    state_only: bool,
    lock_only: bool,
) -> Result<()> {
    let mut active_hashes = HashSet::new();
    let mut active_refs = HashSet::new();

    if !lock_only {
        for reference in ctx.state.images.values() {
            if let Some(ref d) = reference.digest {
                active_hashes.insert(d.clone());
            }
            active_refs.insert(reference.to_tag_string());
        }
    }

    if !state_only {
        for reference in ctx.lock.images.values() {
            if let Some(ref d) = reference.digest {
                active_hashes.insert(d.clone());
            }
            active_refs.insert(reference.to_tag_string());
        }
    }

    if state_only {
        println!(
            "STATE-ONLY MODE: Tracking {} built images. Fetched base images will be removed...",
            ctx.state.images.len()
        );
    } else if lock_only {
        println!(
            "LOCK-ONLY MODE: Tracking {} base images. Built images will be removed...",
            ctx.lock.images.len()
        );
    } else {
        println!(
            "Tracking {} built images (state) and {} base images (lock)...",
            ctx.state.images.len(),
            ctx.lock.images.len()
        );
    }

    let report = engine.gc(&active_hashes, &active_refs).await?;

    if report.untagged.is_empty() && report.removed_ids.is_empty() {
        println!("All local images are actively tracked. The system is in perfect order.");
        return Ok(());
    }

    for tag in report.untagged {
        println!("- Untagged: {}", tag);
    }
    for id in report.removed_ids {
        let naked_id = id.strip_prefix("sha256:").unwrap_or(&id);
        println!("- Removed: {}", &naked_id[..12.min(naked_id.len())]);
    }

    println!("Garbage collection complete.");
    Ok(())
}
