use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

// --- Structs moved from main.rs and engine/traits.rs ---

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
pub struct DeckConfig {
    pub project: Option<ProjectConfig>,
    #[serde(default)]
    pub images: BTreeMap<String, ImageProfile>,
}

impl DeckConfig {}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProjectConfig {
    pub name: Option<String>,
    #[serde(default)]
    pub registries: BTreeMap<String, String>,
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

// --- Functions moved from main.rs ---

pub fn load_context_or_create() -> Result<(PathBuf, DeckConfig)> {
    let mut curr = env::current_dir()?;
    loop {
        let p = curr.join("deck.toml");
        if p.exists() {
            let content = fs::read_to_string(&p).context("Failed to read deck.toml")?;
            let config = toml::from_str(&content).context("Failed to parse deck.toml")?;
            return Ok((curr, config));
        }
        if !curr.pop() {
            bail!("No deck.toml found. Run 'deck init' first.");
        }
    }
}

pub fn load_context() -> Result<(PathBuf, DeckConfig)> {
    load_context_or_create()
}

pub fn save_config(p: &Path, c: &DeckConfig) -> Result<()> {
    fs::write(p, toml::to_string_pretty(c)?)?;
    Ok(())
}

pub fn load_lock(r: &Path) -> Result<DeckLock> {
    let p = r.join("deck.lock");
    if p.exists() {
        let content = fs::read_to_string(&p)?;
        Ok(serde_json::from_str(&content).context("Failed to parse deck.lock")?)
    } else {
        Ok(DeckLock::default())
    }
}

pub fn save_lock(r: &Path, l: &DeckLock) -> Result<()> {
    fs::write(r.join("deck.lock"), serde_json::to_string_pretty(l)?)?;
    Ok(())
}
