mod build_graph;
mod cli;
mod commands;
mod config;
mod engine;

use crate::cli::{Cli, Commands};
use crate::engine::BuildahEngine;
use anyhow::Result;
use clap::Parser;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Specialized handling for init command (root-less)
    if let Commands::Init = cli.command {
        return commands::init::run_init();
    }

    // Single point of project context loading
    let ctx = config::load_project_context()?;
    let engine = Arc::new(BuildahEngine::new(&ctx.root)?);

    // Dispatch to subcommands
    match cli.command {
        Commands::Init => unreachable!(),
        Commands::Scan => commands::scan::run_scan(ctx),
        Commands::Login(args) => commands::registry::run_login(engine.as_ref(), ctx, args).await,
        Commands::Registry(args) => commands::registry::run_registry(ctx, args.action),
        Commands::Build(args) => commands::build::run_build(engine, ctx, args).await,
        Commands::Push(args) => commands::push::run_push(engine, ctx, args).await,
        Commands::Shell => commands::shell::run_shell(ctx),
        Commands::Clean => commands::clean::run_clean(ctx),
        Commands::Gc => commands::gc::run_gc(ctx),
    }
}
