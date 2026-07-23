use crate::config::ProjectContext;
use crate::engine::ProcessGroupGuard;
use crate::engine::version::{MIN_SKOPEO_VERSION, require_tool_version};
use crate::image_reference::ImageReference;
use anyhow::{Context, Result, bail};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;
use tempfile::NamedTempFile;
use tokio::process::Command as TokioCommand;

#[derive(Debug)]
pub struct RegistryAuth<'a> {
    pub username: Option<&'a str>,
    pub password: Option<&'a str>,
}

pub struct SkopeoEngine {
    pub ctx: ProjectContext,
}

impl SkopeoEngine {
    pub fn new(ctx: ProjectContext) -> Result<Self> {
        require_tool_version("skopeo", MIN_SKOPEO_VERSION)?;

        if !ctx.paths.storage_conf.exists() {
            bail!("Storage not configured. Run 'dull init' and 'dull venv' first.");
        }
        Ok(Self { ctx })
    }

    fn command(&self) -> TokioCommand {
        let mut cmd = TokioCommand::new("skopeo");
        cmd.env("CONTAINERS_STORAGE_CONF", &self.ctx.paths.storage_conf)
            .env("TMPDIR", &self.ctx.paths.tmp_dir)
            .env("REGISTRY_AUTH_FILE", &self.ctx.paths.auth_file);
        unsafe {
            cmd.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        cmd
    }

    pub async fn login(
        &self,
        registry_url: &str,
        auth: &RegistryAuth<'_>,
        insecure: bool,
    ) -> Result<()> {
        let mut cmd = self.command();
        cmd.arg("login");

        if let Some(u) = auth.username {
            cmd.arg("-u").arg(u);
        }
        if let Some(p) = auth.password {
            cmd.arg("-p").arg(p);
        }
        if insecure {
            cmd.arg("--tls-verify=false");
        }
        cmd.arg(registry_url);

        let mut child = cmd
            .kill_on_drop(true)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;

        let _guard = ProcessGroupGuard::new(&child);
        let status = child.wait().await?;

        if !status.success() {
            bail!("Login failed.");
        }
        Ok(())
    }

    pub async fn push(
        &self,
        source: &ImageReference,
        destination: &ImageReference,
        insecure: bool,
        sign_by: Option<&str>,
        sign_by_sigstore_private_key: Option<&Path>,
        sign_passphrase_file: Option<&Path>,
    ) -> Result<String> {
        let digest_file = NamedTempFile::new_in(&self.ctx.paths.tmp_dir)?;
        let digest_path = digest_file.path().to_str().unwrap();

        let src_str = source.storage_full_url();
        let dest_str = destination.docker_tag_url();

        let mut cmd = self.command();
        cmd.kill_on_drop(true).arg("copy");

        if insecure {
            cmd.arg("--dest-tls-verify=false");
        }
        if let Some(key_id) = sign_by {
            cmd.arg("--sign-by").arg(key_id);
        }
        if let Some(key_path) = sign_by_sigstore_private_key {
            cmd.arg("--sign-by-sigstore-private-key").arg(key_path);
        }
        if let Some(pass_file) = sign_passphrase_file {
            cmd.arg("--sign-passphrase-file").arg(pass_file);
        }

        cmd.args(["--digestfile", digest_path, &src_str, &dest_str]);

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = cmd.spawn()?;

        let _guard = ProcessGroupGuard::new(&child);
        let output = child.wait_with_output().await?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            bail!(
                "Failed to push image from {} to {}:\n{}",
                src_str,
                dest_str,
                err_msg.trim()
            );
        }

        Ok(fs::read_to_string(digest_path)?.trim().to_string())
    }

    pub async fn fetch(&self, reference: &ImageReference, insecure: bool) -> Result<String> {
        let src_str = reference.docker_canonical_url();
        let dest_str = reference.storage_tag_url();

        let digest_file = NamedTempFile::new_in(&self.ctx.paths.tmp_dir)?;
        let digest_path = digest_file.path().to_str().unwrap();

        let mut cmd = self.command();
        cmd.kill_on_drop(true).arg("copy");

        if insecure {
            cmd.arg("--src-tls-verify=false");
        }

        cmd.args(["--digestfile", digest_path, &src_str, &dest_str]);

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = cmd.spawn()?;

        let _guard = ProcessGroupGuard::new(&child);
        let output = child.wait_with_output().await?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            bail!(
                "Failed to fetch image from {} to {}:\n{}",
                src_str,
                dest_str,
                err_msg.trim()
            );
        }

        Ok(fs::read_to_string(digest_path)?.trim().to_string())
    }

    pub async fn export_to_oci(
        &self,
        local_reference: &ImageReference,
        dest_dir: &Path,
    ) -> Result<()> {
        let src = local_reference.storage_tag_url();
        let dest = format!("oci:{}", dest_dir.display());

        let mut cmd = self.command();
        cmd.kill_on_drop(true).arg("copy").arg(&src).arg(&dest);

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = cmd.spawn()?;

        let _guard = ProcessGroupGuard::new(&child);
        let output = child
            .wait_with_output()
            .await
            .context("Failed to execute skopeo copy for export")?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to export to OCI format:\n{}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }

        Ok(())
    }

    pub async fn inspect_remote(
        &self,
        reference: &ImageReference,
        insecure: bool,
    ) -> Result<Option<ImageReference>> {
        let dest_str = reference.docker_tag_url();
        let mut cmd = self.command();
        cmd.arg("inspect").arg("--raw");

        if insecure {
            cmd.arg("--tls-verify=false");
        }

        cmd.arg(&dest_str);

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = cmd.spawn()?;

        let _guard = ProcessGroupGuard::new(&child);
        let output = child.wait_with_output().await?;

        if output.status.success() {
            let mut temp_file = NamedTempFile::new_in(&self.ctx.paths.tmp_dir)?;
            temp_file.write_all(&output.stdout)?;
            let temp_path = temp_file.path().to_str().unwrap();

            let mut digest_cmd = self.command();
            digest_cmd.arg("manifest-digest").arg(temp_path);

            digest_cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
            let digest_child = digest_cmd.spawn()?;

            let _digest_guard = ProcessGroupGuard::new(&digest_child);
            let digest_output = digest_child.wait_with_output().await?;

            if !digest_output.status.success() {
                let err_msg = String::from_utf8_lossy(&digest_output.stderr);
                bail!(
                    "Failed to calculate manifest digest using skopeo:\n{}",
                    err_msg.trim()
                );
            }

            let digest_str = String::from_utf8_lossy(&digest_output.stdout)
                .trim()
                .to_string();

            let mut remote_ref = reference.clone();
            remote_ref.digest = Some(digest_str);

            Ok(Some(remote_ref))
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            bail!("Failed to inspect remote registry:\n{}", err.trim());
        }
    }
}
