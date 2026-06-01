use crate::config::{DullConfig, ProjectConfig, ProjectPaths, write_atomic};
use anyhow::Result;
use std::collections::BTreeMap;
use std::env;

pub fn run_init() -> Result<()> {
    let root = env::current_dir()?;
    let paths = ProjectPaths::new(root.clone());

    if paths.config_file.exists() {
        println!("dull.toml already exists.");
        return Ok(());
    }

    let dir_name = root
        .file_name()
        .and_then(|os_str| os_str.to_str())
        .unwrap_or("default-project")
        .to_string();

    let conf = DullConfig {
        project: ProjectConfig { name: dir_name },
        registries: BTreeMap::new(),
        dependencies: Vec::new(),
        images: BTreeMap::new(),
    };

    let toml_content = toml::to_string_pretty(&conf)?;
    write_atomic(&paths.config_file, &toml_content)?;

    println!("Initialized dull.toml");
    Ok(())
}
