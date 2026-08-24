mod build_graph;
mod build_lock;
mod cli;
mod commands;
mod config;
mod engine;
mod image_reference;
mod output;

use crate::cli::{Cli, Commands};
use crate::engine::{BuildahEngine, SkopeoEngine};
use anyhow::Result;
use clap::Parser;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Context-less boundary
    // `init` creates the initial dull.toml, so it runs completely independent of context.
    if let Commands::Init = cli.command {
        return commands::init::run_init();
    }

    let ctx = config::load_project_context()?;

    // Dispatch to subcommands
    match cli.command {
        Commands::Init => unreachable!(),
        Commands::Venv(args) => commands::venv::run_venv(args),

        // Context-only boundary
        Commands::Scan(args) => commands::scan::run_scan(ctx, args),
        Commands::Registry(args) => commands::registry::run_registry(ctx, args.action),

        // Engine-required boundary
        // All other commands require the physical .dull/ environment to be fully initialized.
        cmd => {
            let buildah_engine = Arc::new(BuildahEngine::new(ctx.clone())?);
            let skopeo_engine = Arc::new(SkopeoEngine::new(ctx.clone())?);

            match cmd {
                Commands::Login(args) => {
                    commands::login::run_login(&skopeo_engine.as_ref(), ctx, args).await
                }
                Commands::Lock(args) => commands::lock::run_lock(skopeo_engine, ctx, args).await,
                Commands::Fetch(args) => commands::fetch::run_fetch(skopeo_engine, ctx, args).await,
                Commands::Build(args) => {
                    commands::build::run_build(buildah_engine, ctx, args).await
                }
                Commands::Push(args) => commands::push::run_push(skopeo_engine, ctx, args).await,
                Commands::Export(args) => {
                    commands::export::run_export(skopeo_engine, ctx, args).await
                }
                Commands::Shell => commands::shell::run_shell(buildah_engine.as_ref(), ctx),
                Commands::Images(args) => {
                    commands::images::run_images(buildah_engine.as_ref(), ctx, args).await
                }
                Commands::Rmi(args) => {
                    commands::rmi::run_rmi(buildah_engine.as_ref(), ctx, args).await
                }
                _ => unreachable!(),
            }
        }
    }
}
