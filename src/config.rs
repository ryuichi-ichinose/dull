use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
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

/// 低レベルなファイル保存ヘルパー。initなどコンテキスト外でも使用する。
pub fn save_config_file(p: &Path, c: &DeckConfig) -> Result<()> {
    let content = toml::to_string_pretty(c)?;
    fs::write(p, content).with_context(|| format!("Failed to write config to {:?}", p))?;
    Ok(())
}

pub struct ProjectContext {
    pub root: PathBuf,
    pub config: DeckConfig,
    pub lock: DeckLock,
}

impl ProjectContext {
    pub fn load() -> Result<Self> {
        let mut curr = env::current_dir()?;
        let root = loop {
            if curr.join("deck.toml").exists() {
                break curr;
            }
            if !curr.pop() {
                bail!("No deck.toml found. Run 'deck init' first.");
            }
        };

        let config_content = fs::read_to_string(root.join("deck.toml"))?;
        let config: DeckConfig = toml::from_str(&config_content)?;

        let lock_path = root.join("deck.lock");
        let lock = if lock_path.exists() {
            let content = fs::read_to_string(&lock_path)?;
            serde_json::from_str(&content).context("Failed to parse deck.lock")?
        } else {
            DeckLock::default()
        };

        Ok(Self { root, config, lock })
    }

    pub fn save_config(&self) -> Result<()> {
        save_config_file(&self.root.join("deck.toml"), &self.config)
    }

    pub fn save_lock(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.lock)?;
        fs::write(self.root.join("deck.lock"), content)?;
        Ok(())
    }

    pub fn absolute_path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}
