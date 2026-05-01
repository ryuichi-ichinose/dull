use crate::config::ProjectContext;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::process::Command;

pub fn run_gc(ctx: ProjectContext) -> Result<()> {
    let dull_dir = ctx.root.join(".dull");
    let containers_dir = dull_dir.join("containers");
    let storage_conf = containers_dir.join("storage.conf");

    if !storage_conf.exists() {
        println!("No storage configured. Run 'dull init' first.");
        return Ok(());
    }

    // Collect both digests and local_refs to prevent deleting images
    // that share the same ID but have different tags (e.g., base images).
    let mut active_hashes = HashSet::new();
    let mut active_refs = HashSet::new();

    for profile in ctx.lock.images.values() {
        active_hashes.insert(profile.digest.clone());
        active_refs.insert(profile.local_ref.clone());
    }

    println!("Found {} active images in dull.lock", active_hashes.len());

    let output = Command::new("buildah")
        .env("CONTAINERS_STORAGE_CONF", &storage_conf)
        .args([
            "images",
            "--no-trunc",
            "--format",
            "{{.ID}}@{{.Digest}}@{{.Name}}:{{.Tag}}",
        ])
        .output()
        .context("Failed to get images from buildah")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to execute buildah images:\n{}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut garbage_ids = HashSet::new();
    let mut garbage_tags = HashSet::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('@').collect();
        if parts.len() >= 3 {
            let id = parts[0];
            let digest = parts[1];
            let local_ref = parts[2];

            // Normalize ID to handle inconsistencies in buildah output across versions
            let id_with_prefix = if id.starts_with("sha256:") {
                id.to_string()
            } else {
                format!("sha256:{}", id)
            };

            let is_known_entity = active_hashes.contains(id)
                || active_hashes.contains(&id_with_prefix)
                || active_hashes.contains(digest);

            if !is_known_entity {
                garbage_ids.insert(id.to_string());
            } else if local_ref != "<none>:<none>" && !active_refs.contains(local_ref) {
                // The underlying image is active, but this specific tag is not tracked.
                garbage_tags.insert(local_ref.to_string());
            }
        }
    }

    if garbage_ids.is_empty() && garbage_tags.is_empty() {
        println!("No garbage images or stray tags found. The system is in perfect order.");
        return Ok(());
    }

    // Phase 1: Untag stray tags first.
    // Buildah does not have an `untag` command; we use `rmi` with the tag name.
    // If the underlying image has other tags, `rmi` simply removes this specific tag.
    if !garbage_tags.is_empty() {
        println!("Found {} stray tags. Untagging...", garbage_tags.len());

        let mut args = vec!["rmi"];
        args.extend(garbage_tags.iter().map(|s| s.as_str()));

        let status = Command::new("buildah")
            .env("CONTAINERS_STORAGE_CONF", &storage_conf)
            .args(&args)
            .status()
            .context("Failed to execute buildah rmi for untagging")?;

        if status.success() {
            for tag in &garbage_tags {
                println!("- Untagged: {}", tag);
            }
        } else {
            eprintln!("Warning: Buildah failed to untag some images.");
        }
    }
    // Phase 2: Physically remove orphan images.
    if !garbage_ids.is_empty() {
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
        } else {
            anyhow::bail!("Buildah failed to remove some or all garbage images.");
        }
    }

    println!("Garbage collection complete.");
    Ok(())
}
