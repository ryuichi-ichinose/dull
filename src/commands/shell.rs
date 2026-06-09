use crate::config::ProjectContext;
use crate::engine::BuildahEngine;
use anyhow::Result;

pub fn run_shell(engine: &BuildahEngine, ctx: ProjectContext) -> Result<()> {
    let namespace = ctx.config.namespace.as_deref().unwrap_or("dull");

    println!("Entering dull shell for project: {}", namespace);

    // Delegate process replacement to the engine. Will not return.
    engine.shell(namespace)
}
