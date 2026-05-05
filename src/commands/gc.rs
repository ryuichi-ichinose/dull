use crate::config::ProjectContext;
use crate::engine::BuildahEngine;
use anyhow::Result;
use std::collections::HashSet;

pub async fn run_gc(engine: &BuildahEngine, ctx: ProjectContext) -> Result<()> {
    let mut active_hashes = HashSet::new();
    let mut active_refs = HashSet::new();

    for profile in ctx.state.images.values() {
        active_hashes.insert(profile.digest.clone());
        active_refs.insert(profile.local_ref.clone());
    }

    println!(
        "Found {} active images in {}",
        active_hashes.len(),
        ctx.paths.state_file.to_string_lossy()
    );

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
