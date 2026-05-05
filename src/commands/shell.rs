use crate::config::ProjectContext;
use crate::engine::BuildahEngine;
use anyhow::Result;

pub fn run_shell(engine: &BuildahEngine, ctx: ProjectContext) -> Result<()> {
    println!(
        "Entering dull shell for project: {}",
        &ctx.config.project.name
    );

    // Delegate process replacement to the engine. Will not return.
    engine.shell(&ctx.config.project.name)
}
