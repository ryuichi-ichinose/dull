use crate::config::ProjectContext;
use crate::image_reference::ImageReference;
use anyhow::{Result, bail};
use std::fs;
use tempfile::NamedTempFile;
use tokio::process::Command as TokioCommand;

pub struct SkopeoEngine {
    pub ctx: ProjectContext,
}

impl SkopeoEngine {
    pub fn new(ctx: ProjectContext) -> Result<Self> {
        if std::process::Command::new("skopeo")
            .arg("--version")
            .output()
            .is_err()
        {
            bail!("'skopeo' command not found. Install it first.");
        }
        if !ctx.paths.storage_conf.exists() {
            bail!("Storage not configured. Run 'dull init' and 'dull venv' first.");
        }
        Ok(Self { ctx })
    }

    fn command(&self) -> TokioCommand {
        let mut cmd = TokioCommand::new("skopeo");
        cmd.env("CONTAINERS_STORAGE_CONF", &self.ctx.paths.storage_conf)
            .env("TMPDIR", &self.ctx.paths.tmp_dir);
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
            bail!("Failed to push image to {}:\n{}", dest_str, err_msg.trim());
        }

        Ok(fs::read_to_string(digest_path)?.trim().to_string())
    }
}
