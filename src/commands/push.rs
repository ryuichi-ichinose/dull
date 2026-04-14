use crate::cli::PushArgs;
use crate::config::ProjectContext;
use crate::engine::ContainerEngine;
use anyhow::{Context, Result, bail};

pub fn run_push(engine: &impl ContainerEngine, ctx: ProjectContext, args: PushArgs) -> Result<()> {
    let project = ctx
        .config
        .project
        .as_ref()
        .context("Project config missing")?;

    let target_alias = args.registry.as_deref().unwrap_or("default");
    let registry_url = project
        .registries
        .get(target_alias)
        .or_else(|| {
            (project.registries.len() == 1)
                .then(|| project.registries.values().next())
                .flatten()
        })
        .with_context(|| format!("Registry '{}' not found", target_alias))?;

    let targets: Vec<String> = if args.all {
        ctx.lock.images.keys().cloned().collect()
    } else {
        if args.images.is_empty() {
            bail!("Specify images or --all");
        }
        args.images
            .iter()
            .filter(|n| ctx.lock.images.contains_key(*n))
            .cloned()
            .collect()
    };

    for name in targets {
        let lock_profile = &ctx.lock.images[&name];
        engine.push(
            registry_url,
            &name,
            &lock_profile.digest,
            &args.tag,
            args.insecure,
        )?;
    }
    Ok(())
}
