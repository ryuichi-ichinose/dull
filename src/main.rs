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

    match &cli.command {
        Commands::Init => commands::init::run_init(),
        Commands::Scan => commands::scan::run_scan(),
        Commands::Login {
            registry,
            username,
            password,
            extra_args,
        } => commands::registry::run_login(
            &engine,
            registry.as_deref(),
            username.as_deref(),
            password.as_deref(),
            extra_args,
        ),
        Commands::Registry { action } => commands::registry::run_registry(action),
        Commands::Build {
            all,
            images,
            no_lock,
            tag,
        } => commands::build::run_build(&engine, *all, images, *no_lock, tag),
        Commands::Push {
            all,
            images,
            tag,
            registry,
            insecure,
        } => commands::push::run_push(&engine, *all, images, tag, registry.as_deref(), *insecure),
    }
}
