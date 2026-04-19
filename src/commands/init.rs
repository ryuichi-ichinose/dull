use crate::config::{DullConfig, ProjectConfig, save_config_file};
use anyhow::Result;
use std::collections::BTreeMap;
use std::env;

pub fn run_init() -> Result<()> {
    let config_path = env::current_dir()?.join("dull.toml");
    if config_path.exists() {
        println!("dull.toml already exists.");
        return Ok(());
    }

    let mut registries = BTreeMap::new();
    registries.insert("default".into(), "localhost:5000/project".into());

    let conf = DullConfig {
        project: Some(ProjectConfig {
            name: Some("my-project".into()),
            registries,
        }),
        images: BTreeMap::new(),
    };

    save_config_file(&config_path, &conf)?;
    println!("Initialized dull.toml");
    Ok(())
}
