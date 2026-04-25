// src/commands/shell.rs
use crate::config::ProjectContext;
use anyhow::{Context, Result};
use std::env;
use std::process::Command;

pub fn run_shell(ctx: ProjectContext) -> Result<()> {

    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let dull_dir = ctx.root.join(".dull");
    let storage_conf = dull_dir.join("storage.conf");
    let tmp_dir = dull_dir.join("tmp");

    let project_name = ctx
        .config
        .project
        .as_ref()
        .and_then(|p| p.name.clone())
        .unwrap_or_else(|| {
            ctx.root
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "dull-project".to_string())
        });

    println!("Spawning dull shell for project: {}", project_name);

    let mut cmd = Command::new(&shell);

    cmd.env("CONTAINERS_STORAGE_CONF", &storage_conf)
        .env("TMPDIR", &tmp_dir)
        .env("DULL_PROJECT_NAME", &project_name);


    let mut child = cmd.spawn().context("Failed to spawn shell")?;
    child.wait().context("Shell process encountered an error")?;

    println!("Exited dull shell.");
    Ok(())
}
