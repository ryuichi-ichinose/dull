mod build_graph;
mod cli;
mod commands;
mod config;
mod engine;

use crate::cli::{Cli, Commands};
use crate::config::ProjectContext;
use crate::engine::BuildahEngine;
use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let engine = BuildahEngine::new()?;

    if let Commands::Init = cli.command {
        return commands::init::run_init();
    }

    let mut ctx = ProjectContext::load()?;

    match cli.command {
        Commands::Init => unreachable!(),
        Commands::Scan => commands::scan::run_scan(&mut ctx),
        Commands::Login(args) => commands::registry::run_login(&engine, &ctx, args),
        Commands::Registry(args) => commands::registry::run_registry(&mut ctx, &args.action),
        Commands::Build(args) => commands::build::run_build(&engine, &mut ctx, args),
        Commands::Push(args) => commands::push::run_push(&engine, &ctx, args),
    }
}
