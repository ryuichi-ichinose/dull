use anyhow::{Context, Result, bail};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::Write;
use std::process::Stdio;
use tempfile::NamedTempFile;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::engine::traits::{BuildOptions, BuildResult, ContainerEngine, RegistryAuth};

pub struct BuildahEngine;

impl BuildahEngine {
    pub fn new() -> Result<Self> {
        if std::process::Command::new("buildah")
            .arg("--version")
            .output()
            .is_err()
        {
            bail!("'buildah' command not found. Install it first.");
        }
        Ok(Self)
    }

    async fn get_storage_tmp(&self) -> Result<std::path::PathBuf> {
        let output = Command::new("buildah")
            .args(["info", "--format", "{{.store.GraphRoot}}"])
            .output()
            .await?;

        if !output.status.success() {
            bail!("Failed to get buildah info");
        }

        let root_path = String::from_utf8(output.stdout)?.trim().to_string();
        if root_path.is_empty() || root_path == "<no value>" {
            bail!("Could not determine GraphRoot from buildah info");
        }

        let tmp_path = std::path::PathBuf::from(root_path).join("dull-tmp");

        if !tmp_path.exists() {
            fs::create_dir_all(&tmp_path).context("Failed to create dull-tmp in storage root")?;
        }

        Ok(tmp_path)
    }
}

impl ContainerEngine for BuildahEngine {
    async fn login(&self, registry_url: &str, auth: &RegistryAuth<'_>) -> Result<()> {
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
            .kill_on_drop(true)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .await?;

        if !status.success() {
            bail!("Login failed.");
        }
        println!("Login succeeded.");
        Ok(())
    }

    async fn build(&self, name: &str, opts: BuildOptions<'_>) -> Result<BuildResult> {
        let local_ref = format!("localhost/{}:{}", name, opts.target_tag);

        // Get storage-local tmp directory for efficient rename(2)
        let storage_tmp = self.get_storage_tmp().await?;
        let iid_file = NamedTempFile::new_in(&storage_tmp)
            .context("Failed to create temp iidfile in storage tmp")?;
        let iid_path = iid_file.path().to_str().unwrap();

        let mut child = Command::new("buildah")
            .kill_on_drop(true)
            .env("TMPDIR", &storage_tmp)
            .args([
                "bud",
                "--format=oci",
                "--layers=true",
                "--iidfile",
                iid_path,
            ])
            .arg("-f")
            .arg(opts.dockerfile_path)
            .arg("-t")
            .arg(&local_ref)
            .arg(opts.context_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn buildah")?;

        let log_dir = std::path::Path::new(".dull/logs");
        fs::create_dir_all(log_dir).context("Failed to create log directory")?;
        let mut log_file = File::create(log_dir.join(format!("{}.log", name)))
            .with_context(|| format!("Failed to create log file for {}", name))?;
        let mut error_buffer = VecDeque::with_capacity(20);

        let stdout = child.stdout.take().context("Failed to take stdout")?;
        let stderr = child.stderr.take().context("Failed to take stderr")?;

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        let mut stdout_done = false;
        let mut stderr_done = false;

        while !stdout_done || !stderr_done {
            tokio::select! {
                line = stdout_reader.next_line(), if !stdout_done => {
                    match line {
                        Ok(Some(line)) => {
                            writeln!(log_file, "{}", line)?;
                            if let Some(ref pb) = opts.progress_bar {
                                let msg = if line.len() > 50 { format!("{}...", &line[..47]) } else { line.clone() };
                                pb.set_message(msg);
                            }
                            if error_buffer.len() == 20 { error_buffer.pop_front(); }
                            error_buffer.push_back(line);
                        }
                        Ok(None) => stdout_done = true,
                        Err(_) => stdout_done = true,
                    }
                }
                line = stderr_reader.next_line(), if !stderr_done => {
                    match line {
                        Ok(Some(line)) => {
                            writeln!(log_file, "{}", line)?;
                            if let Some(ref pb) = opts.progress_bar {
                                let msg = if line.len() > 50 { format!("{}...", &line[..47]) } else { line.clone() };
                                pb.set_message(msg);
                            }
                            if error_buffer.len() == 20 { error_buffer.pop_front(); }
                            error_buffer.push_back(line);
                        }
                        Ok(None) => stderr_done = true,
                        Err(_) => stderr_done = true,
                    }
                }
            }
        }

        let status = child.wait().await.context("Failed to wait for buildah")?;

        if !status.success() {
            let error_msg = error_buffer.make_contiguous().join("\n");
            bail!(
                "BUILD FAILED: {}\n================================================================================\nLast 20 lines of log:\n{}\n================================================================================",
                name,
                error_msg
            );
        }

        let image_id = fs::read_to_string(iid_path)?.trim().to_string();
        let digest_output = Command::new("buildah")
            .arg("images")
            .arg("--no-trunc")
            .arg("--format")
            .arg("{{.Digest}}")
            .arg(&image_id)
            .output()
            .await?;

        let digest = String::from_utf8(digest_output.stdout)?.trim().to_string();
        let final_digest = if digest.is_empty() || digest == "<none>" {
            image_id
        } else {
            digest
        };

        if opts.progress_bar.is_none() {
            println!("Built {}: {}", name, final_digest);
        }
        Ok(BuildResult {
            digest: final_digest,
            local_ref,
        })
    }

    async fn push(&self, local_ref: &str, destination: &str) -> anyhow::Result<String> {
        let digest_file = NamedTempFile::new().context("Failed to create temp file for digest")?;
        let digest_path = digest_file.path().to_str().unwrap();

        let status = Command::new("buildah")
            .kill_on_drop(true)
            .arg("push")
            .arg("--digestfile")
            .arg(digest_path)
            .arg(local_ref)
            .arg(destination)
            .status()
            .await
            .context("Failed to execute buildah push")?;

        if !status.success() {
            anyhow::bail!("Failed to push image to {}", destination);
        }

        let digest = fs::read_to_string(digest_path)?.trim().to_string();
        Ok(digest)
    }
}
