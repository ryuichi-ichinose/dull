use crate::config::{DullLock, ProjectContext, save_lock};
use crate::engine::BuildahEngine;
use anyhow::Result;

pub async fn run_clean(engine: &BuildahEngine, mut ctx: ProjectContext) -> Result<()> {
    if !ctx.paths.storage_conf.exists() {
        println!("Nothing to clean. (.dull/storage.conf not found)");
        return Ok(());
    }

    println!("Cleaning up containers and images in project storage...");
    // Delegate physical destruction to the engine
    engine.clean().await?;

    ctx.lock = DullLock::default();
    save_lock(&ctx)?;

    println!("Clean completed.");
    Ok(())
}
