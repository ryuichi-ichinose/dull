use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

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

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProjectConfig {
    pub name: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct RegistryProfile {
    pub url: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DullConfig {
    pub project: ProjectConfig,
    #[serde(default)]
    pub registries: BTreeMap<String, RegistryProfile>,
    #[serde(default)]
    pub images: BTreeMap<String, ImageProfile>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StateImageProfile {
    pub digest: String,
    pub local_ref: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub registry_digests: BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct DullState {
    pub images: BTreeMap<String, StateImageProfile>,
}

#[derive(Debug, Clone)]
pub struct ProjectPaths {
    pub root: PathBuf,
    pub config_file: PathBuf,
    pub state_file: PathBuf,
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
            state_file: root.join(".dull").join("dull_state.json"),
            storage_dir: containers_dir.join("storage"),
            storage_conf: containers_dir.join("storage.conf"),
            tmp_dir: containers_dir.join("tmp"),
            containers_dir,
            dull_dir,
            root,
        }
    }
}

pub struct ProjectContext {
    pub paths: ProjectPaths,
    pub config: DullConfig,
    pub state: DullState,
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
        serde_json::from_str(&content).context("Failed to parse dull.state")?
    } else {
        DullState::default()
    };

    Ok(ProjectContext {
        paths,
        config,
        state,
    })
}

pub fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let tmp_path = path.with_extension("tmp");

    let mut file = File::create(&tmp_path)
        .with_context(|| format!("Failed to create temp file {:?}", tmp_path))?;
    file.write_all(content.as_bytes())?;

    file.sync_all()?;

    fs::rename(&tmp_path, path)
        .with_context(|| format!("Failed to atomic rename {:?} to {:?}", tmp_path, path))?;

    Ok(())
}

pub fn save_config(ctx: &ProjectContext) -> Result<()> {
    let content = toml::to_string_pretty(&ctx.config)?;
    write_atomic(&ctx.paths.config_file, &content)
}

pub fn save_state(ctx: &ProjectContext) -> Result<()> {
    let content = serde_json::to_string_pretty(&ctx.state)?;
    write_atomic(&ctx.paths.state_file, &content)
}
