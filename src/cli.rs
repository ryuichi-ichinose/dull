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
    Venv(VenvArgs),
    Scan(ScanArgs),
    Login(LoginArgs),
    Registry(RegistryArgs),
    Lock(LockArgs),
    Fetch(FetchArgs),
    Build(BuildArgs),
    Push(PushArgs),
    Shell,
    Rmi(RmiArgs),
    Images(ImagesArgs),
    Export(ExportArgs),
}

#[derive(Args)]
pub struct VenvArgs {
    /// Use the host's global/default container storage instead of an isolated
    /// project-local one. Generates an empty storage.conf.
    #[arg(
        long,
        conflicts_with_all = [
            "driver", "mountopt", "composefs", "size", "inodes",
            "convert_images", "enable_partial_images", "use_hard_links",
        ]
    )]
    pub global: bool,

    #[arg(long, default_value = "overlay")]
    pub driver: String,

    #[arg(long, default_value = "nodev")]
    pub mountopt: String,

    /// storage.options.overlay.use_composefs
    #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
    pub composefs: bool,

    /// storage.options.overlay.size — max size of a read/write layer (e.g. "10G")
    #[arg(long)]
    pub size: Option<String>,

    /// storage.options.overlay.inodes — max inodes in a read/write layer
    #[arg(long)]
    pub inodes: Option<String>,

    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub convert_images: bool,

    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub enable_partial_images: bool,

    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub use_hard_links: bool,
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
    pub registry: String,
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

#[derive(clap::ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum LogsMode {
    Errors,
    All,
}

#[derive(Args)]
pub struct BuildArgs {
    #[arg(short, long, conflicts_with_all = ["images", "group"])]
    pub all: bool,
    #[arg(num_args = 1.., conflicts_with = "group")]
    pub images: Vec<String>,
    #[arg(short, long, conflicts_with_all = ["all", "images"])]
    pub group: Option<String>,
    #[arg(short, long)]
    pub tag: Option<String>,
    #[arg(short = 'j', long)]
    pub jobs: Option<usize>,
    #[arg(long, value_enum)]
    pub logs: Option<LogsMode>,
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
    pub registry: String,
    #[arg(short = 'j', long)]
    pub jobs: Option<usize>,
    #[arg(long)]
    pub insecure: bool,
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
pub struct RmiArgs {
    /// Image names (dull.toml [images.X] keys) to remove from state and local storage
    #[arg(num_args = 0.., conflicts_with_all = ["all", "keep_tracked", "keep_state", "keep_lock"])]
    pub images: Vec<String>,

    /// Wipe all images in project storage and reset dull_state
    #[arg(long, conflicts_with_all = ["keep_tracked", "keep_state", "keep_lock"])]
    pub all: bool,

    /// Keep everything tracked by state or lock; remove the rest
    #[arg(long, conflicts_with = "all")]
    pub keep_tracked: bool,

    /// Keep images tracked by state (built images)
    #[arg(long, conflicts_with = "all")]
    pub keep_state: bool,

    /// Keep images tracked by lock (fetched base images)
    #[arg(long, conflicts_with = "all")]
    pub keep_lock: bool,
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
