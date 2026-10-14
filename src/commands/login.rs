use crate::cli::LoginArgs;
use crate::config::ProjectContext;
use crate::engine::{RegistryAuth, SkopeoEngine};
use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};

pub async fn run_login(engine: &SkopeoEngine, ctx: ProjectContext, args: LoginArgs) -> Result<()> {
    let registries = &ctx.config.registries;
    let registry_url = registries
        .get(&args.registry)
        .map(|r| r.url.as_str())
        .unwrap_or(&args.registry);

    let password_from_stdin = if args.password_stdin {
        let mut line = String::new();
        BufReader::new(tokio::io::stdin())
            .read_line(&mut line)
            .await
            .context("Failed to read password from stdin")?;
        Some(line.trim_end_matches(['\r', '\n']).to_string())
    } else {
        None
    };

    let auth = RegistryAuth {
        username: args.username.as_deref(),
        password: password_from_stdin.as_deref().or(args.password.as_deref()),
    };

    engine.login(registry_url, &auth, args.insecure).await
}
