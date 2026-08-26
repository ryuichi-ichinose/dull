use crate::config::ProjectContext;
use crate::engine::BuildahEngine;
use anyhow::Result;

pub fn run_shell(engine: &BuildahEngine, ctx: ProjectContext) -> Result<()> {
    let project_name = ctx
        .paths
        .root
        .file_name()
        .and_then(|os_str| os_str.to_str())
        .unwrap_or("dull");

    println!("Entering dull shell for project: {}", project_name);

    // exec()-based; never returns on success.
    engine.shell(project_name)
}
