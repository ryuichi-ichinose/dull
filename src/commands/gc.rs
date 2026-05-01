// src/commands/gc.rs
use crate::config::ProjectContext;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashSet;
use std::process::Command;

// Defines the expected JSON structure from `buildah images --json`
#[derive(Deserialize, Debug)]
struct BuildahImage {
    id: String,
    digest: String,
    #[serde(default)]
    names: Vec<String>,
}

pub fn run_gc(ctx: ProjectContext) -> Result<()> {
    if !ctx.paths.storage_conf.exists() {
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
        .env("CONTAINERS_STORAGE_CONF", &ctx.paths.storage_conf)
        .args(["images", "--json"])
        .output()
        .context("Failed to get images from buildah")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to execute buildah images:\n{}", stderr);
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stdout_str = stdout_str.trim();

    if stdout_str.is_empty() || stdout_str == "null" {
        println!("No images found in local storage. Nothing to clean.");
        return Ok(());
    }

    let buildah_images: Vec<BuildahImage> =
        serde_json::from_str(stdout_str).context("Failed to parse buildah images JSON output")?;

    let mut garbage_ids = HashSet::new();
    let mut garbage_tags = HashSet::new();

    for img in buildah_images {
        // Normalize ID to handle inconsistencies in buildah output across versions
        let id_full = if img.id.starts_with("sha256:") {
            img.id.clone()
        } else {
            format!("sha256:{}", img.id)
        };

        let is_known_entity = active_hashes.contains(&img.id)
            || active_hashes.contains(&id_full)
            || active_hashes.contains(&img.digest);

        if !is_known_entity {
            garbage_ids.insert(img.id);
        } else {
            // The underlying image is active, but we must check for stray tags.
            // Using JSON, untagged images simply have empty `names` arrays.
            // We firmly filter out any unexpected placeholders just in case.
            for name in img.names {
                if !name.is_empty() && !name.contains("<none>") && !active_refs.contains(&name) {
                    garbage_tags.insert(name);
                }
            }
        }
    }

    if garbage_ids.is_empty() && garbage_tags.is_empty() {
        println!("No garbage images or stray tags found. The system is in perfect order.");
        return Ok(());
    }

    // Phase 1: Untag stray tags first.
    if !garbage_tags.is_empty() {
        println!("Found {} stray tags. Untagging...", garbage_tags.len());

        let mut args = vec!["rmi"];
        args.extend(garbage_tags.iter().map(|s| s.as_str()));

        let status = Command::new("buildah")
            .env("CONTAINERS_STORAGE_CONF", &ctx.paths.storage_conf)
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
            .env("CONTAINERS_STORAGE_CONF", &ctx.paths.storage_conf)
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
