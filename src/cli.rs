use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "deck", version, about = "Container build orchestrator.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Init,
    Scan,
    Login {
        registry: Option<String>,
        #[arg(short, long)]
        username: Option<String>,
        #[arg(short, long)]
        password: Option<String>,
        #[arg(last = true)]
        extra_args: Vec<String>,
    },
    Registry {
        #[command(subcommand)]
        action: RegistryAction,
    },
    Build {
        #[arg(short, long, conflicts_with = "images")]
        all: bool,
        #[arg(num_args = 1..)]
        images: Vec<String>,
        #[arg(long)]
        no_lock: bool,
        #[arg(long)]
        tag: Option<String>,
    },
    Push {
        #[arg(short, long, conflicts_with = "images")]
        all: bool,
        #[arg(num_args = 1..)]
        images: Vec<String>,
        #[arg(short, long, default_value = "latest")]
        tag: String,
        #[arg(short = 'r', long)]
        registry: Option<String>,
        #[arg(long)]
        insecure: bool,
    },
}

#[derive(Subcommand)]
pub enum RegistryAction {
    Add { name: String, url: String },
    Rm { name: String },
    Ls,
}
