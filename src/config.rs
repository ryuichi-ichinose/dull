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
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProjectConfig {
    pub name: Option<String>,
    #[serde(default)]
    pub registries: BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DeckConfig {
    pub project: Option<ProjectConfig>,
    #[serde(default)]
    pub images: BTreeMap<String, ImageProfile>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LockImageProfile {
    pub digest: String,
    pub local_ref: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct DeckLock {
    pub images: BTreeMap<String, LockImageProfile>,
}

// 配管に流すためのデータパック
pub struct ProjectContext {
    pub root: PathBuf,
    pub config: DeckConfig,
    pub lock: DeckLock,
}

// --- I/O ヘルパー (境界) ---

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

    let config_content = fs::read_to_string(root.join("dull.toml"))?;
    let config: DeckConfig = toml::from_str(&config_content)?;

    let lock_path = root.join("dull.lock");
    let lock = if lock_path.exists() {
        let content = fs::read_to_string(&lock_path)?;
        serde_json::from_str(&content).context("Failed to parse dull.lock")?
    } else {
        DeckLock::default()
    };

    Ok(ProjectContext { root, config, lock })
}

fn write_atomic(path: &Path, content: &str) -> Result<()> {
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
    save_config_file(&ctx.root.join("dull.toml"), &ctx.config)
}

pub fn save_lock(ctx: &ProjectContext) -> Result<()> {
    let content = serde_json::to_string_pretty(&ctx.lock)?;
    write_atomic(&ctx.root.join("dull.lock"), &content)
}

// initなどコンテキスト外でも使用する低レベルヘルパー
pub fn save_config_file(p: &Path, c: &DeckConfig) -> Result<()> {
    let content = toml::to_string_pretty(c)?;
    write_atomic(p, &content)
}
