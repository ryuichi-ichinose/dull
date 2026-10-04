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
use anyhow::{Result, bail};
use clap::Parser;
use std::sync::Arc;
use tokio::signal::unix::{SignalKind, signal};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // `init` creates dull.toml itself, so it must run before any context exists.
    if let Commands::Init = cli.command {
        return commands::init::run_init();
    }

    let ctx = config::load_project_context()?;

    match cli.command {
        Commands::Init => unreachable!(),
        Commands::Venv(args) => commands::venv::run_venv(args),

        // Don't need the .dull/ engine environment, just the config.
        Commands::Scan(args) => commands::scan::run_scan(ctx, args),
        Commands::Registry(args) => commands::registry::run_registry(ctx, args.action),

        cmd => {
            let buildah_engine = Arc::new(BuildahEngine::new(ctx.clone())?);
            let skopeo_engine = Arc::new(SkopeoEngine::new(ctx.clone())?);

            let work = async {
                match cmd {
                    Commands::Login(args) => {
                        commands::login::run_login(skopeo_engine.as_ref(), ctx, args).await
                    }
                    Commands::Lock(args) => {
                        commands::lock::run_lock(skopeo_engine, ctx, args).await
                    }
                    Commands::Fetch(args) => {
                        commands::fetch::run_fetch(skopeo_engine, ctx, args).await
                    }
                    Commands::Build(args) => {
                        commands::build::run_build(buildah_engine, ctx, args).await
                    }
                    Commands::Push(args) => {
                        commands::push::run_push(skopeo_engine, ctx, args).await
                    }
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
            };

            tokio::select! {
                result = work => result,
                () = shutdown_signal() => bail!("Interrupted."),
            }
        }
    }
}

async fn shutdown_signal() {
    let mut terminate = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
}
