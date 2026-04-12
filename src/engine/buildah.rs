use anyhow::{bail, Context, Result};
use std::process::Command;

use crate::engine::traits::{BuildOptions, BuildResult, ContainerEngine, RegistryAuth};

pub struct BuildahEngine;

impl BuildahEngine {
    pub fn new() -> Result<Self> {
        if Command::new("buildah").arg("--version").output().is_err() {
            bail!("❌ 'buildah' command not found. Install it first.");
        }
        Ok(Self)
    }
}

impl ContainerEngine for BuildahEngine {
    fn login(&self, registry_url: &str, auth: &RegistryAuth) -> Result<()> {
        println!("Logging into '{}'...", registry_url);

        let mut cmd = Command::new("buildah");
        cmd.arg("login");

        if let Some(u) = auth.username {
            cmd.arg("-u").arg(u);
        }
        if let Some(p) = auth.password {
            cmd.arg("-p").arg(p);
        }

        cmd.args(auth.extra_args);
        cmd.arg(registry_url);

        let status = cmd
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()?;

        if !status.success() {
            bail!("❌ Login failed.");
        }
        println!("Login succeeded.");
        Ok(())
    }

    fn build(&self, name: &str, opts: BuildOptions) -> Result<BuildResult> {
        let local_ref = format!("localhost/{}:{}", name, opts.target_tag);

        println!("Building: {} ({:?})", name, opts.dockerfile_path);

        let build_status = Command::new("buildah")
            .arg("bud")
            .arg("--format=oci")
            .arg("--layers=true")
            .arg("-f").arg(opts.dockerfile_path)
            .arg("-t").arg(&local_ref)
            .arg(opts.context_dir)
            .status()
            .context("Buildah 'bud' execution failed")?;

        if !build_status.success() {
            bail!("Build failed for {}", name);
        }

        println!("Inspecting image digest for {}", local_ref);
        let inspect_output = Command::new("buildah")
            .arg("inspect")
            .arg("--format")
            .arg("{{.FromImageDigest}}")
            .arg(&local_ref)
            .output()
            .context("Buildah 'inspect' execution failed")?;

        if !inspect_output.status.success() {
            bail!(
                "Failed to inspect image digest for {}: {}",
                local_ref,
                String::from_utf8_lossy(&inspect_output.stderr)
            );
        }

        let digest = String::from_utf8(inspect_output.stdout)?
            .trim()
            .to_string();

        if digest.is_empty() {
            bail!("Got empty digest for {}", local_ref);
        }

        println!("Built Digest: {}", digest);

        Ok(BuildResult { digest, local_ref })
    }

    fn push(&self, registry: &str, name: &str, digest: &str, tag: &str, insecure: bool) -> Result<()> {
        let dest = format!("docker://{}/{}:{}", registry, name, tag);
        println!("Pushing Locked Image: {} -> {}", name, dest);
        println!("   (Source Digest: {})", digest);

        let mut cmd = Command::new("buildah");
        cmd.arg("push");
        if insecure {
            cmd.arg("--tls-verify=false");
        }
        cmd.arg(digest).arg(&dest);

        let status = cmd.status().context("Buildah push failed")?;

        if !status.success() {
            bail!("Push failed for {}", name);
        }
        Ok(())
    }
}