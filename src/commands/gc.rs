use crate::cli::GcArgs;
use crate::config::ProjectContext;
use crate::engine::BuildahEngine;
use anyhow::Result;
use std::collections::HashSet;

pub async fn run_gc(engine: &BuildahEngine, ctx: ProjectContext, args: GcArgs) -> Result<()> {
    let mut active_hashes = HashSet::new();
    let mut active_refs = HashSet::new();

    if !args.lock_only {
        for reference in ctx.state.images.values() {
            if let Some(ref d) = reference.digest {
                active_hashes.insert(d.clone());
            }
            active_refs.insert(reference.to_tag_string());
        }
    }

    if !args.state_only {
        for reference in ctx.lock.images.values() {
            if let Some(ref d) = reference.digest {
                active_hashes.insert(d.clone());
            }
            active_refs.insert(reference.to_tag_string());
        }
    }

    if args.state_only {
        println!(
            "STATE-ONLY MODE: Tracking {} built images. Fetched base images will be removed...",
            ctx.state.images.len()
        );
    } else if args.lock_only {
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
