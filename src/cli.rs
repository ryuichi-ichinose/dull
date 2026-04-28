use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "dull", version, about = "Container build orchestrator.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Init,
    Scan,
    Login(LoginArgs),
    Registry(RegistryArgs),
    Build(BuildArgs),
    Push(PushArgs),
    Shell,
    Clean,
    Gc,
}

#[derive(Args)]
pub struct LoginArgs {
    pub registry: Option<String>,
    #[arg(short, long)]
    pub username: Option<String>,
    #[arg(short, long)]
    pub password: Option<String>,
    #[arg(last = true)]
    pub extra_args: Vec<String>,
}

#[derive(Args)]
pub struct RegistryArgs {
    #[command(subcommand)]
    pub action: RegistryAction,
}

#[derive(Subcommand)]
pub enum RegistryAction {
    Add { name: String, url: String },
    Rm { name: String },
    Ls,
}

#[derive(Args)]
pub struct BuildArgs {
    #[arg(short, long, conflicts_with = "images")]
    pub all: bool,
    #[arg(num_args = 1..)]
    pub images: Vec<String>,
    #[arg(long)]
    pub no_lock: bool,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(short = 'j', long)]
    pub jobs: Option<usize>,
}

#[derive(Args)]
pub struct PushArgs {
    #[arg(short, long, conflicts_with = "images")]
    pub all: bool,
    #[arg(num_args = 1..)]
    pub images: Vec<String>,
    #[arg(short, long, default_value = "latest")]
    pub tag: String,
    #[arg(short = 'r', long)]
    pub registry: Option<String>,
    #[arg(long)]
    pub insecure: bool,
}
