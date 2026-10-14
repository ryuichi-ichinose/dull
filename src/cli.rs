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
    /// Create a new dull.toml in the current directory
    Init,
    /// Set up this project's isolated container storage
    Venv(VenvArgs),
    /// Discover Containerfiles and add them to dull.toml
    Scan(ScanArgs),
    /// Log in to a container registry
    Login(LoginArgs),
    /// Manage registry aliases used by push/pull
    Registry(RegistryArgs),
    /// Resolve dependencies to digests and write dull.lock
    Lock(LockArgs),
    /// Pull locked dependencies into local storage
    Fetch(FetchArgs),
    /// Build images defined in dull.toml
    Build(BuildArgs),
    /// Push built images to a registry
    Push(PushArgs),
    /// Open a shell configured for this project's isolated storage
    Shell,
    /// Remove images from local storage and/or tracking
    Rmi(RmiArgs),
    /// List tracked (state/lock) and local images
    Images(ImagesArgs),
    /// Export built images to an OCI layout on disk
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

    /// storage.driver
    #[arg(long, default_value = "overlay")]
    pub driver: String,

    /// storage.options.overlay.mountopt
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

    /// storage.options.pull_options.convert_images
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub convert_images: bool,

    /// storage.options.pull_options.enable_partial_images
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub enable_partial_images: bool,

    /// storage.options.pull_options.use_hard_links
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub use_hard_links: bool,
}

#[derive(Args)]
pub struct ScanArgs {
    /// Directories to scan (default: the whole project)
    #[arg(num_args = 0..)]
    pub directories: Vec<String>,
    /// Override the build context for every image found this run
    #[arg(short, long)]
    pub context: Option<String>,
}

#[derive(Args)]
pub struct LoginArgs {
    /// Registry to log in to (e.g. docker.io, ghcr.io)
    pub registry: String,
    #[arg(short, long)]
    pub username: Option<String>,
    #[arg(short, long, conflicts_with = "password_stdin")]
    pub password: Option<String>,
    /// Read the password from stdin instead of passing it on the command line
    #[arg(long)]
    pub password_stdin: bool,
    /// Allow HTTP / untrusted TLS certificates for this registry
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
    /// Add or update a registry alias
    Add { name: String, url: String },
    /// Remove a registry alias
    Rm { name: String },
    /// List configured registry aliases
    Ls,
}

#[derive(clap::ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum LogsMode {
    Errors,
    All,
}

#[derive(Args)]
pub struct BuildArgs {
    /// Build every image defined in dull.toml
    #[arg(short, long, conflicts_with_all = ["images", "group"])]
    pub all: bool,
    /// Image names (dull.toml [images.X] keys) to build
    #[arg(num_args = 1.., conflicts_with = "group")]
    pub images: Vec<String>,
    /// Build a named [build_groups.X] entry instead of individual images
    #[arg(short, long, conflicts_with_all = ["all", "images"])]
    pub group: Option<String>,
    /// Override every built image's tag (default: each image's local_tag)
    #[arg(short, long)]
    pub tag: Option<String>,
    /// Max number of images to build concurrently
    #[arg(short = 'j', long)]
    pub jobs: Option<usize>,
    /// After the run, print full per-image logs in addition to the default
    /// (a failed image's own log is always shown inline regardless of this)
    #[arg(long, value_enum)]
    pub logs: Option<LogsMode>,
    /// Don't abort other builds when one fails; run everything to
    /// completion and report all failures at the end
    #[arg(long)]
    pub keep_going: bool,
    /// Pull base images missing from local storage by name, instead of
    /// requiring them to have been fetched from dull.lock first
    #[arg(long)]
    pub ignore_lock: bool,
}

#[derive(Args)]
pub struct PushArgs {
    /// Push every image tracked in dull_state
    #[arg(short, long, conflicts_with = "images")]
    pub all: bool,
    /// Image names (dull.toml [images.X] keys) to push
    #[arg(num_args = 1..)]
    pub images: Vec<String>,
    /// Override the destination tag (default: each image's local tag)
    #[arg(short, long)]
    pub tag: Option<String>,
    /// Registry alias (see 'dull registry ls') to push to
    #[arg(short = 'r', long)]
    pub registry: String,
    /// Max number of images to push concurrently
    #[arg(short = 'j', long)]
    pub jobs: Option<usize>,
    /// Allow HTTP / untrusted TLS certificates for this registry
    #[arg(long)]
    pub insecure: bool,
    /// Sign each pushed image with this GPG key ID
    #[arg(long)]
    pub sign_by: Option<String>,
    /// Sign each pushed image with this sigstore private key file
    #[arg(long, conflicts_with = "sign_by")]
    pub sign_by_sigstore_private_key: Option<PathBuf>,
    /// File containing the passphrase for --sign-by-sigstore-private-key
    #[arg(long)]
    pub sign_passphrase_file: Option<PathBuf>,
}

#[derive(Args)]
pub struct LockArgs {
    /// Re-resolve digests even for dependencies that are already locked
    #[arg(short, long)]
    pub upgrade: bool,
    /// Dependencies (FQINs from dull.toml) to lock (default: all)
    #[arg(num_args = 0..)]
    pub images: Vec<String>,
    /// Allow HTTP / untrusted TLS certificates
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
    /// Print as JSON instead of a table
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct FetchArgs {
    /// Dependencies (FQINs from dull.lock) to fetch (default: all)
    #[arg(num_args = 0..)]
    pub images: Vec<String>,
    /// Allow HTTP / untrusted TLS certificates
    #[arg(long)]
    pub insecure: bool,
}

#[derive(clap::Args, Debug)]
pub struct ExportArgs {
    /// Export every image tracked in dull_state
    #[arg(short, long)]
    pub all: bool,
    /// Output directory (default: the project root)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Image names (dull.toml [images.X] keys) to export
    pub images: Vec<String>,
}
