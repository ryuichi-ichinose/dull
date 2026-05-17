use crate::cli::RegistryAction;
use crate::config::{ProjectContext, RegistryProfile, save_config};
use anyhow::Result;

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
