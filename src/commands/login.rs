use crate::cli::LoginArgs;
use crate::config::ProjectContext;
use crate::engine::{RegistryAuth, SkopeoEngine};
use anyhow::Result;

pub async fn run_login(engine: &SkopeoEngine, ctx: ProjectContext, args: LoginArgs) -> Result<()> {
    let registries = &ctx.config.registries;
    let target = args.registry.as_deref().unwrap_or("default");
    let registry_url = registries
        .get(target)
        .map(|r| r.url.as_str())
        .unwrap_or(target);

    let auth = RegistryAuth {
        username: args.username.as_deref(),
        password: args.password.as_deref(),
    };

    engine.login(registry_url, &auth, args.insecure).await
}
