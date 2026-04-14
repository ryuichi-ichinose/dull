use crate::cli::PushArgs;
use crate::config::{load_context, load_lock};
use crate::engine::ContainerEngine;
use anyhow::{Context, Result, bail};

pub fn run_push(engine: &impl ContainerEngine, args: PushArgs) -> Result<()> {
    let (root, config) = load_context()?;
    let lock = load_lock(&root).context("❌ deck.lock not found. Run 'build' first.")?;

    let registries = &config
        .project
        .as_ref()
        .context("Project config missing")?
        .registries;

    let target_alias = args.registry.as_deref().unwrap_or("default");
    let registry_url = registries
        .get(target_alias)
        .or_else(|| {
            if registries.len() == 1 {
                registries.values().next()
            } else {
                None
            }
        })
        .with_context(|| format!("Registry alias '{}' not found", target_alias))?;

    let targets: Vec<String> = if args.all {
        lock.images.keys().cloned().collect()
    } else {
        if args.images.is_empty() {
            bail!("Specify images or --all");
        }
        args.images
            .iter()
            .filter(|&n| lock.images.contains_key(n))
            .cloned()
            .collect()
    };

    if targets.is_empty() {
        println!("No locked images found to push.");
        return Ok(());
    }

    println!("Pushing {} images to '{}'...", targets.len(), registry_url);

    for name in targets {
        if let Some(lock_profile) = lock.images.get(&name) {
            engine.push(
                registry_url,
                &name,
                &lock_profile.digest,
                &args.tag,
                args.insecure,
            )?;
        }
    }
    Ok(())
}
