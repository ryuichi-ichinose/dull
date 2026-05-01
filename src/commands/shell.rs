use crate::config::ProjectContext;
use anyhow::Result;
use std::env;
use std::os::unix::process::CommandExt;
use std::process::Command;

pub fn run_shell(ctx: ProjectContext) -> Result<()> {
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

    println!(
        "Entering dull shell for project: {}",
        &ctx.config.project.name
    );

    let mut cmd = Command::new(&shell);
    cmd.env("CONTAINERS_STORAGE_CONF", &ctx.paths.storage_conf)
        .env("TMPDIR", &ctx.paths.tmp_dir)
        .env("DULL_PROJECT_NAME", &ctx.config.project.name);

    let err = cmd.exec();

    anyhow::bail!("Failed to exec shell: {}", err);
}
