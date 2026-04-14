mod cli;
mod commands;
mod config;
mod engine;

use crate::cli::{Cli, Commands};
use crate::engine::BuildahEngine;
use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let engine = BuildahEngine::new()?;

    match cli.command {
        Commands::Init => commands::init::run_init(),
        Commands::Scan => commands::scan::run_scan(),
        Commands::Login(args) => commands::registry::run_login(&engine, args),
        Commands::Registry(args) => commands::registry::run_registry(&args.action),
        Commands::Build(args) => commands::build::run_build(&engine, args),
        Commands::Push(args) => commands::push::run_push(&engine, args),
    }
}
