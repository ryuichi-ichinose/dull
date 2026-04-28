use crate::config::ProjectContext;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::process::Command;

pub fn run_gc(ctx: ProjectContext) -> Result<()> {
    let dull_dir = ctx.root.join(".dull");
    let storage_conf = dull_dir.join("storage.conf");

    if !storage_conf.exists() {
        println!("No storage configured. Run 'dull init' first.");
        return Ok(());
    }

    let mut active_hashes = HashSet::new();
    for profile in ctx.lock.images.values() {
        active_hashes.insert(profile.digest.clone());
    }

    println!("Found {} active images in dull.lock", active_hashes.len());

    let output = Command::new("buildah")
        .env("CONTAINERS_STORAGE_CONF", &storage_conf)
        .args(["images", "--no-trunc", "--format", "{{.ID}}@{{.Digest}}"])
        .output()
        .context("Failed to get images from buildah")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to execute buildah images:\n{}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut garbage_ids = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('@').collect();
        if parts.len() == 2 {
            let id = parts[0];
            let digest = parts[1];

            let id_with_prefix = if id.starts_with("sha256:") {
                id.to_string()
            } else {
                format!("sha256:{}", id)
            };

            if !active_hashes.contains(id)
                && !active_hashes.contains(&id_with_prefix)
                && !active_hashes.contains(digest)
            {
                garbage_ids.push(id.to_string());
            }
        }
    }

    if garbage_ids.is_empty() {
        println!("No garbage images found. The system is in perfect order.");
        return Ok(());
    }

    println!(
        "Found {} orphan images. Collecting garbage...",
        garbage_ids.len()
    );

    let mut args = vec!["rmi", "-f"];
    args.extend(garbage_ids.iter().map(|s| s.as_str()));

    let status = Command::new("buildah")
        .env("CONTAINERS_STORAGE_CONF", &storage_conf)
        .args(&args)
        .status()
        .context("Failed to execute buildah rmi")?;

    if status.success() {
        for id in &garbage_ids {
            let naked_id = id.strip_prefix("sha256:").unwrap_or(id);
            let display_id = if naked_id.len() > 12 {
                &naked_id[..12]
            } else {
                naked_id
            };
            println!("- Removed: {}", display_id);
        }
        println!(
            "Garbage collection complete. Swept {} images.",
            garbage_ids.len()
        );
    } else {
        anyhow::bail!("Buildah failed to remove some or all garbage images.");
    }

    Ok(())
}
