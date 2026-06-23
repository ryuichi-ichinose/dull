use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "dull", version, about = "Container build orchestrator.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Init,
    Venv,
    Scan(ScanArgs),
    Login(LoginArgs),
    Registry(RegistryArgs),
    Lock(LockArgs),
    Fetch(FetchArgs),
    Build(BuildArgs),
    Push(PushArgs),
    Shell,
    Clean,
    Gc(GcArgs),
    Images(ImagesArgs),
    Export(ExportArgs),
}

#[derive(Args)]
pub struct ScanArgs {
    #[arg(num_args = 0..)]
    pub directories: Vec<String>,
    #[arg(short, long)]
    pub context: Option<String>,
}

#[derive(Args)]
pub struct LoginArgs {
    pub registry: Option<String>,
    #[arg(short, long)]
    pub username: Option<String>,
    #[arg(short, long)]
    pub password: Option<String>,
    #[arg(long)]
    pub insecure: bool,
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
    #[arg(short, long)]
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
    #[arg(short, long)]
    pub tag: Option<String>,
    #[arg(short = 'r', long)]
    pub registry: Option<String>,
    #[arg(short = 'j', long)]
    pub jobs: Option<usize>,
    #[arg(long)]
    pub insecure: bool,
    #[arg(short = 'f', long)]
    pub force: bool,
    #[arg(long)]
    pub sign_by: Option<String>,
    #[arg(long, conflicts_with = "sign_by")]
    pub sign_by_sigstore_private_key: Option<PathBuf>,
    #[arg(long)]
    pub sign_passphrase_file: Option<PathBuf>,
}

#[derive(Args)]
pub struct LockArgs {
    #[arg(short, long)]
    pub upgrade: bool,
    #[arg(num_args = 0..)]
    pub images: Vec<String>,
    #[arg(long)]
    pub insecure: bool,
}

#[derive(Args)]
pub struct GcArgs {
    #[arg(long, conflicts_with = "lock_only")]
    pub state_only: bool,
    #[arg(long, conflicts_with = "state_only")]
    pub lock_only: bool,
}

#[derive(Args)]
pub struct ImagesArgs {
    /// Show ONLY images from dull_state.json (built images)
    #[arg(long, conflicts_with = "lock")]
    pub state: bool,

    /// Show ONLY images from dull.lock (fetched base images)
    #[arg(long, conflicts_with = "state")]
    pub lock: bool,

    /// Show ALL images in local storage, including untracked ones
    #[arg(short, long)]
    pub all: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct FetchArgs {
    #[arg(num_args = 0..)]
    pub images: Vec<String>,
    #[arg(long)]
    pub insecure: bool,
}

#[derive(clap::Args, Debug)]
pub struct ExportArgs {
    #[arg(short, long)]
    pub all: bool,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    pub images: Vec<String>,
}
