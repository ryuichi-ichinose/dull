use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
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
        println!("Building: {} ({})", name, profile.path);

        let local_ref = format!("localhost/{}:{}", name, tag);
        let iid_path = env::temp_dir().join(format!("deck_iid_{}", uuid::Uuid::new_v4()));

        let status = Command::new("buildah")
            .current_dir(root)
            .arg("bud")
            .arg("--format=oci")
            .arg("--layers=true")
            .arg("-f")
            .arg(&profile.path)
            .arg("-t")
            .arg(&local_ref)
            .arg("--iidfile")
            .arg(&iid_path)
            .arg(".")
            .status()
            .context("Buildah execution failed")?;

        if !status.success() {
            let _ = fs::remove_file(&iid_path);
            bail!("Build failed for {}", name);
        }

        let digest = fs::read_to_string(&iid_path)
            .context("Failed to read image ID")?
            .trim()
            .to_string();

        let _ = fs::remove_file(&iid_path);
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
