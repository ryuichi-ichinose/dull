use crate::cli::{LoginArgs, RegistryAction};
use crate::config::{ProjectContext, RegistryProfile, save_config};
use crate::engine::{BuildahEngine, RegistryAuth};
use anyhow::Result;

pub async fn run_login(engine: &BuildahEngine, ctx: ProjectContext, args: LoginArgs) -> Result<()> {
    let registries = &ctx.config.registries;
    let target = args.registry.as_deref().unwrap_or("default");
    let registry_url = registries
        .get(target)
        .map(|r| r.url.as_str())
        .unwrap_or(target);

    let auth = RegistryAuth {
        username: args.username.as_deref(),
        password: args.password.as_deref(),
        extra_args: &args.extra_args,
    };

    engine.login(registry_url, &auth).await
}

pub fn run_registry(mut ctx: ProjectContext, action: RegistryAction) -> Result<()> {
    match action {
        RegistryAction::Add { name, url } => {
            let clean_url = url.trim_end_matches('/');
            ctx.config.registries.insert(
                name.clone(),
                RegistryProfile {
                    url: clean_url.to_string(),
                },
            );
            println!("Registry '{}' -> '{}'", name, clean_url);
        }
        RegistryAction::Rm { name } => {
            if ctx.config.registries.remove(&name).is_some() {
                println!("Removed '{}'", name);
            } else {
                eprintln!("Registry '{}' not found.", name);
            }
        }
        RegistryAction::Ls => {
            for (name, profile) in &ctx.config.registries {
                println!(" - {:<10} : {}", name, profile.url);
            }
            return Ok(());
        }
    }

    save_config(&ctx)?;
    Ok(())
}
