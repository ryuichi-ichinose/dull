use crate::cli::RegistryAction;
use crate::config::{load_context, load_context_or_create, save_config, ProjectConfig};
use crate::engine::ContainerEngine;
use anyhow::{Context, Result};
use std::collections::BTreeMap;

pub fn run_login(
    engine: &impl ContainerEngine,
    registry_alias: Option<&str>,
    username: Option<&str>,
    password: Option<&str>,
    extra_args: &[String],
) -> Result<()> {
    let (_, config) = load_context()?;
    let registries = &config
        .project
        .as_ref()
        .context("Project config missing")?
        .registries;

    let target = registry_alias.unwrap_or("default");
    let registry_url = registries.get(target).map(|s| s.as_str()).unwrap_or(target);

    engine.login(registry_url, username, password, extra_args)
}

pub fn run_registry(action: &RegistryAction) -> Result<()> {
    let (root, mut config) = load_context_or_create()?;

    let project = config.project.get_or_insert(ProjectConfig {
        name: None,
        registries: BTreeMap::new(),
    });

    match action {
        RegistryAction::Add { name, url } => {
            let clean_url = url.trim_end_matches('/');
            project
                .registries
                .insert(name.clone(), clean_url.to_string());
            println!("Registry '{}' -> '{}'", name, clean_url);
        }
        RegistryAction::Rm { name } => {
            if project.registries.remove(name).is_some() {
                println!("Removed '{}'", name);
            } else {
                eprintln!("Registry '{}' not found.", name);
            }
        }
        RegistryAction::Ls => {
            for (name, url) in &project.registries {
                println!(" - {:<10} : {}", name, url);
            }
            return Ok(());
        }
    }
    save_config(&root.join("deck.toml"), &config)?;
    Ok(())
}
