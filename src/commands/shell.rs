use crate::config::ProjectContext;
use anyhow::Result;
use std::env;
use std::os::unix::process::CommandExt;
use std::process::Command;

pub fn run_shell(ctx: ProjectContext) -> Result<()> {
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

    let dull_dir = ctx.root.join(".dull");
    let containers_dir = dull_dir.join("containers");
    let storage_conf = containers_dir.join("storage.conf");
    let tmp_dir = containers_dir.join("tmp");

    let project_name = ctx.config.project.name;

    println!("Entering dull shell for project: {}", project_name);

    let mut cmd = Command::new(&shell);
    cmd.env("CONTAINERS_STORAGE_CONF", &storage_conf)
        .env("TMPDIR", &tmp_dir)
        .env("DULL_PROJECT_NAME", &project_name);

    let err = cmd.exec();

    anyhow::bail!("Failed to exec shell: {}", err);
}
