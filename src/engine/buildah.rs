use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

use super::traits::{ContainerEngine, ImageProfile};

pub struct BuildahEngine;

impl BuildahEngine {
    pub fn new() -> Result<Self> {
        if Command::new("buildah").arg("--version").output().is_err() {
            bail!("❌ 'buildah' command not found. Install it first (e.g., dnf install buildah).");
        }
        Ok(Self)
    }
}

impl ContainerEngine for BuildahEngine {
    fn login(
        &self,
        registry_url: &str,
        username: Option<&str>,
        password: Option<&str>,
        extra_args: &[String],
    ) -> Result<()> {
        println!("Logging into '{}'...", registry_url);

        let mut cmd = Command::new("buildah");
        cmd.arg("login");

        if let Some(u) = username {
            cmd.arg("-u").arg(u);
        }
        if let Some(p) = password {
            cmd.arg("-p").arg(p);
        }

        cmd.args(extra_args);
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

    fn build(&self, root: &Path, name: &str, profile: &ImageProfile, tag: &str) -> Result<(String, String)> {
        let local_ref = format!("localhost/{}:{}", name, tag);

        println!("Building: {} ({})", name, profile.path);

        // 1. Build the image and tag it locally
        let build_status = Command::new("buildah")
            .current_dir(root)
            .arg("bud")
            .arg("--format=oci")
            .arg("--layers=true")
            .arg("-f")
            .arg(&profile.path)
            .arg("-t")
            .arg(&local_ref)
            .arg(".")
            .status()
            .context("Buildah 'bud' execution failed")?;

        if !build_status.success() {
            bail!("Build failed for {}", name);
        }

        // 2. Inspect the local image to get its manifest digest
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

        Ok((digest, local_ref))
    }

    fn push(
        &self,
        registry: &str,
        name: &str,
        digest: &str,
        tag: &str,
        insecure: bool,
    ) -> Result<()> {
        let src = digest;
        let dest = format!("docker://{}/{}:{}", registry, name, tag);
        println!("Pushing Locked Image: {} -> {}", name, dest);
        println!("   (Source Digest: {})", digest);

        let mut cmd = Command::new("buildah");
        cmd.arg("push");
        if insecure {
            cmd.arg("--tls-verify=false");
        }
        cmd.arg(src).arg(&dest);

        let status = cmd.status().context("Buildah push failed")?;

        if !status.success() {
            bail!("Push failed for {}", name);
        }
        Ok(())
    }
}
