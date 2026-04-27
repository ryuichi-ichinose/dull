// src/commands/clean.rs
use crate::config::ProjectContext;
use anyhow::{Context, Result};
use std::process::Command;

pub fn run_clean(ctx: ProjectContext) -> Result<()> {
    let dull_dir = ctx.root.join(".dull");
    let storage_conf = dull_dir.join("storage.conf");

    if !storage_conf.exists() {
        println!("Nothing to clean. (.dull/storage.conf not found)");
        return Ok(());
    }

    println!("Cleaning up containers and images in project storage...");

    let rm_status = Command::new("buildah")
        .env("CONTAINERS_STORAGE_CONF", &storage_conf)
        .args(["rm", "-a"])
        .status()
        .context("Failed to execute buildah rm")?;

    if !rm_status.success() {
        eprintln!("Warning: Failed to remove some containers.");
    }

    let rmi_status = Command::new("buildah")
        .env("CONTAINERS_STORAGE_CONF", &storage_conf)
        .args(["rmi", "-a", "-f"])
        .status()
        .context("Failed to execute buildah rmi")?;

    if rmi_status.success() {
        println!("Clean completed.");
    } else {
        anyhow::bail!("Clean failed while removing images.");
    }

    Ok(())
}
