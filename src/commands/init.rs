use crate::config::{DeckConfig, ProjectConfig, save_config_file};
use anyhow::Result;
use std::collections::BTreeMap;
use std::env;

pub fn run_init() -> Result<()> {
    let config_path = env::current_dir()?.join("deck.toml");
    if config_path.exists() {
        println!("deck.toml already exists.");
        return Ok(());
    }

    let mut registries = BTreeMap::new();
    registries.insert("default".into(), "localhost:5000/project".into());

    let conf = DeckConfig {
        project: Some(ProjectConfig {
            name: Some("my-project".into()),
            registries,
        }),
        images: BTreeMap::new(),
    };

    save_config_file(&config_path, &conf)?;
    println!("Initialized deck.toml");
    Ok(())
}
