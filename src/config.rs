use crate::image_reference::ImageReference;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use tempfile::Builder;

fn default_tag() -> String {
    "latest".to_string()
}
fn default_context() -> String {
    ".".to_string()
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ImageProfile {
    pub path: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default = "default_tag")]
    pub local_tag: String,
    #[serde(default = "default_context")]
    pub context: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct RegistryProfile {
    pub url: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DullConfig {
    #[serde(default)]
    pub registries: BTreeMap<String, RegistryProfile>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub images: BTreeMap<String, ImageProfile>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct DullState {
    pub images: BTreeMap<String, ImageReference>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct DullLock {
    pub images: BTreeMap<String, ImageReference>,
}

#[derive(Debug, Clone)]
pub struct ProjectPaths {
    pub root: PathBuf,
    pub config_file: PathBuf,
    pub state_file: PathBuf,
    pub lock_file: PathBuf,
    pub dull_dir: PathBuf,
    pub containers_dir: PathBuf,
    pub storage_dir: PathBuf,
    pub storage_conf: PathBuf,
    pub tmp_dir: PathBuf,
}

impl ProjectPaths {
    pub fn new(root: PathBuf) -> Self {
        let dull_dir = root.join(".dull");
        let containers_dir = dull_dir.join("containers");

        Self {
            config_file: root.join("dull.toml"),
            state_file: root.join(".dull").join("dull_state"),
            lock_file: root.join("dull.lock"),
            storage_dir: containers_dir.join("storage"),
            storage_conf: containers_dir.join("storage.conf"),
            tmp_dir: containers_dir.join("tmp"),
            containers_dir,
            dull_dir,
            root,
        }
    }
}
#[derive(Clone)]
pub struct ProjectContext {
    pub paths: ProjectPaths,
    pub config: DullConfig,
    pub state: DullState,
    pub lock: DullLock,
}

pub fn load_project_context() -> Result<ProjectContext> {
    let mut curr = env::current_dir()?;
    let root = loop {
        if curr.join("dull.toml").exists() {
            break curr;
        }
        if !curr.pop() {
            bail!("No dull.toml found. Run 'dull init' first.");
        }
    };

    let paths = ProjectPaths::new(root);

    let config_content = fs::read_to_string(&paths.config_file)?;
    let config: DullConfig = toml::from_str(&config_content)?;

    let state = if paths.state_file.exists() {
        let content = fs::read_to_string(&paths.state_file)?;
        toml::from_str(&content).context("Failed to parse dull.state")?
    } else {
        DullState::default()
    };

    let lock = if paths.lock_file.exists() {
        let content = fs::read_to_string(&paths.lock_file)?;
        toml::from_str(&content).context("Failed to parse dull.lock")?
    } else {
        DullLock::default()
    };

    Ok(ProjectContext {
        paths,
        config,
        state,
        lock,
    })
}

pub fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));

    let mut tmp_file = Builder::new()
        .prefix(&format!(".tmp_{}_", process::id()))
        .tempfile_in(parent)
        .context("Failed to create temp file")?;

    tmp_file.write_all(content.as_bytes())?;
    tmp_file.as_file().sync_all()?;

    tmp_file
        .persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("Failed to atomic rename to {:?}", path))?;

    let parent_dir = File::open(parent)
        .with_context(|| format!("Failed to open parent directory {:?}", parent))?;
    parent_dir
        .sync_all()
        .context("Failed to sync parent directory")?;

    Ok(())
}

pub fn save_config(ctx: &ProjectContext) -> Result<()> {
    let content = toml::to_string_pretty(&ctx.config)?;
    write_atomic(&ctx.paths.config_file, &content)
}

pub fn save_state(ctx: &ProjectContext) -> Result<()> {
    let content = toml::to_string_pretty(&ctx.state)?;
    write_atomic(&ctx.paths.state_file, &content)
}

pub fn save_lock(ctx: &ProjectContext) -> Result<()> {
    let content = toml::to_string_pretty(&ctx.lock)?;
    write_atomic(&ctx.paths.lock_file, &content)
}
