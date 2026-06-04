use crate::config::ProjectContext;
use crate::engine::version::{MIN_SKOPEO_VERSION, require_tool_version};
use crate::image_reference::ImageReference;
use anyhow::{Context, Result, bail};
use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;
use tokio::process::Command as TokioCommand;

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
            .env("TMPDIR", &self.ctx.paths.tmp_dir);
        unsafe {
            cmd.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
        cmd
    }

    pub async fn push(
        &self,
        source: &ImageReference,
        destination: &ImageReference,
        insecure: bool,
    ) -> Result<String> {
        let digest_file = NamedTempFile::new_in(&self.ctx.paths.tmp_dir)?;
        let digest_path = digest_file.path().to_str().unwrap();

        let src_str = format!("containers-storage:{}", source.to_full_string());
        let dest_str = format!("docker://{}", destination.to_tag_string());

        let mut cmd = self.command();
        cmd.kill_on_drop(true).arg("copy");

        if insecure {
            cmd.arg("--dest-tls-verify=false");
        }

        let output = cmd
            .args(["--digestfile", digest_path, &src_str, &dest_str])
            .output()
            .await?;

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
        let src_str = format!("docker://{}", reference.to_digest_string());
        let dest_str = format!("containers-storage:{}", reference.to_tag_string());

        let digest_file = NamedTempFile::new_in(&self.ctx.paths.tmp_dir)?;
        let digest_path = digest_file.path().to_str().unwrap();

        let mut cmd = self.command();
        cmd.kill_on_drop(true).arg("copy");

        if insecure {
            cmd.arg("--src-tls-verify=false");
        }

        let output = cmd
            .args(["--digestfile", digest_path, &src_str, &dest_str])
            .output()
            .await?;

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

    pub async fn inspect_remote(
        &self,
        reference: &ImageReference,
        insecure: bool,
    ) -> Result<Option<ImageReference>> {
        let dest_str = format!("docker://{}", reference.to_tag_string());
        let mut cmd = self.command();
        cmd.arg("inspect").arg("--raw");

        if insecure {
            cmd.arg("--tls-verify=false");
        }

        let output = cmd.arg(&dest_str).output().await?;

        if output.status.success() {
            let mut temp_file = NamedTempFile::new_in(&self.ctx.paths.tmp_dir)?;
            temp_file.write_all(&output.stdout)?;
            let temp_path = temp_file.path().to_str().unwrap();

            let mut digest_cmd = self.command();
            let digest_output = digest_cmd
                .arg("manifest-digest")
                .arg(temp_path)
                .output()
                .await?;

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
            let err = String::from_utf8_lossy(&output.stderr).to_lowercase();
            if err.contains("manifest unknown")
                || err.contains("not found")
                || err.contains("404")
                || err.contains("no such manifest")
            {
                Ok(None)
            } else {
                bail!("Failed to inspect remote registry:\n{}", err.trim());
            }
        }
    }
}
