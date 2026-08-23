use crate::cli::RmiArgs;
use crate::config::{DullState, ProjectContext, save_state};
use crate::engine::BuildahEngine;
use anyhow::{Result, bail};
use std::collections::HashSet;

pub async fn run_rmi(engine: &BuildahEngine, mut ctx: ProjectContext, args: RmiArgs) -> Result<()> {
    if args.all {
        return run_clean(engine, ctx).await;
    }
    if args.keep_tracked || args.keep_state || args.keep_lock {
        let keep_state = args.keep_tracked || args.keep_state;
        let keep_lock = args.keep_tracked || args.keep_lock;
        return run_gc(engine, ctx, keep_state, keep_lock).await;
    }
    if args.images.is_empty() {
        bail!("Specify an image name, or use --keep-tracked / --keep-state / --keep-lock / --all");
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
    mut ctx: ProjectContext,
    keep_state: bool,
    keep_lock: bool,
) -> Result<()> {
    let mut active_hashes = HashSet::new();
    let mut active_refs = HashSet::new();

    if keep_state {
        for reference in ctx.state.images.values() {
            if let Some(ref d) = reference.digest {
                active_hashes.insert(d.clone());
            }
            active_refs.insert(reference.to_tag_string());
        }
    }

    if keep_lock {
        for reference in ctx.lock.images.values() {
            if let Some(ref d) = reference.digest {
                active_hashes.insert(d.clone());
            }
            active_refs.insert(reference.to_tag_string());
        }
    }

    if keep_state && keep_lock {
        println!(
            "Tracking {} built images (state) and {} base images (lock)...",
            ctx.state.images.len(),
            ctx.lock.images.len()
        );
    } else if keep_state {
        println!(
            "Keeping {} built images (state) only. Base images (lock) will be removed...",
            ctx.state.images.len()
        );
    } else {
        println!(
            "Keeping {} base images (lock) only. Built images (state) will be removed...",
            ctx.lock.images.len()
        );
    }

    let report = engine.gc(&active_hashes, &active_refs).await?;

    if !keep_state {
        let local_images = engine.list_images().await?;
        let existing_names: HashSet<String> = local_images
            .into_iter()
            .flat_map(|img| img.names.unwrap_or_default())
            .collect();

        let before = ctx.state.images.len();
        ctx.state
            .images
            .retain(|_, reference| existing_names.contains(&reference.to_tag_string()));
        let pruned = before - ctx.state.images.len();

        if pruned > 0 {
            save_state(&ctx)?;
            println!("Pruned {} stale entries from dull_state.", pruned);
        }
    }

    if report.untagged.is_empty() && report.removed_ids.is_empty() {
        println!("Nothing to garbage collect.");
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
