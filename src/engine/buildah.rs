use crate::config::ProjectContext;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::{HashSet, VecDeque};
use std::fs::{self};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Stdio;
use tempfile::NamedTempFile;
use tokio::fs::{self as tokio_fs, File as TokioFile};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::Command as TokioCommand;

#[derive(Debug)]
pub struct BuildOptions<'a> {
    pub dockerfile_path: &'a Path,
    pub context_dir: &'a Path,
    pub target_tag: &'a str,
    pub build_args: &'a std::collections::BTreeMap<String, String>,
    pub progress_bar: Option<indicatif::ProgressBar>,
}

#[derive(Debug, Clone)]
pub struct BuildResult {
    pub digest: String,
    pub local_ref: String,
}

#[derive(Debug)]
pub struct RegistryAuth<'a> {
    pub username: Option<&'a str>,
    pub password: Option<&'a str>,
    pub extra_args: &'a [String],
}

#[derive(Deserialize, Debug)]
struct BuildahImage {
    id: String,
    digest: String,
    #[serde(default)]
    names: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct GcReport {
    pub untagged: HashSet<String>,
    pub removed_ids: HashSet<String>,
}

pub struct BuildahEngine {
    pub ctx: ProjectContext,
}

impl BuildahEngine {
    pub fn new(ctx: ProjectContext) -> Result<Self> {
        if std::process::Command::new("buildah")
            .arg("--version")
            .output()
            .is_err()
        {
            bail!("'buildah' command not found. Install it first.");
        }
        if !ctx.paths.storage_conf.exists() {
            bail!("Storage not configured. Run 'dull init' first.");
        }
        fs::create_dir_all(&ctx.paths.tmp_dir)?;
        Ok(Self { ctx })
    }

    /// Core helper to ensure isolation: Injects necessary environment variables for Tokio commands
    pub fn command(&self, subcmd: &str) -> TokioCommand {
        let mut cmd = TokioCommand::new("buildah");
        cmd.env("CONTAINERS_STORAGE_CONF", &self.ctx.paths.storage_conf)
            .env("TMPDIR", &self.ctx.paths.tmp_dir);
        if !subcmd.is_empty() {
            cmd.arg(subcmd);
        }
        cmd
    }

    /// Core helper for synchronous std::process::Command (used for exec/shell)
    pub fn std_command(&self, program: &str) -> std::process::Command {
        let mut cmd = std::process::Command::new(program);
        cmd.env("CONTAINERS_STORAGE_CONF", &self.ctx.paths.storage_conf)
            .env("TMPDIR", &self.ctx.paths.tmp_dir);
        cmd
    }

    pub async fn login(&self, registry_url: &str, auth: &RegistryAuth<'_>) -> Result<()> {
        let mut cmd = self.command("login");
        if let Some(u) = auth.username {
            cmd.arg("-u").arg(u);
        }
        if let Some(p) = auth.password {
            cmd.arg("-p").arg(p);
        }

        cmd.args(auth.extra_args).arg(registry_url);

        let status = cmd
            .kill_on_drop(true)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await?;

        if !status.success() {
            bail!("Login failed.");
        }
        Ok(())
    }

    pub async fn build(&self, name: &str, opts: BuildOptions<'_>) -> Result<BuildResult> {
        let local_ref = format!("localhost/{}:{}", name, opts.target_tag);
        let iid_file = NamedTempFile::new_in(&self.ctx.paths.tmp_dir)?;
        let iid_path = iid_file.path().to_str().unwrap();

        let mut cmd = self.command("bud");
        cmd.kill_on_drop(true)
            .args(["--format=oci", "--layers=true", "--iidfile", iid_path]);

        // Inject build arguments resolved from config
        for (k, v) in opts.build_args {
            cmd.arg("--build-arg").arg(format!("{}={}", k, v));
        }

        let mut child = cmd
            .arg("-f")
            .arg(opts.dockerfile_path)
            .arg("-t")
            .arg(&local_ref)
            .arg(opts.context_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let log_dir = self.ctx.paths.dull_dir.join("logs");
        tokio_fs::create_dir_all(&log_dir).await?;

        let log_file = TokioFile::create(log_dir.join(format!("{}.log", name))).await?;
        let mut log_writer = BufWriter::new(log_file);
        let mut error_buffer = VecDeque::with_capacity(20);

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();
        let mut stdout_done = false;
        let mut stderr_done = false;

        while !stdout_done || !stderr_done {
            tokio::select! {
                line = stdout_reader.next_line(), if !stdout_done => {
                    if let Ok(Some(line)) = line {

                        log_writer.write_all(line.as_bytes()).await?;
                        log_writer.write_all(b"\n").await?;

                        if let Some(ref pb) = opts.progress_bar { pb.set_message(line.clone()); }
                        if error_buffer.len() == 20 { error_buffer.pop_front(); }
                        error_buffer.push_back(line);
                    } else { stdout_done = true; }
                }
                line = stderr_reader.next_line(), if !stderr_done => {
                    if let Ok(Some(line)) = line {
                        log_writer.write_all(line.as_bytes()).await?;
                        log_writer.write_all(b"\n").await?;

                        if let Some(ref pb) = opts.progress_bar { pb.set_message(line.clone()); }
                        if error_buffer.len() == 20 { error_buffer.pop_front(); }
                        error_buffer.push_back(line);
                    } else { stderr_done = true; }
                }
            }
        }

        log_writer.flush().await?;

        let status = child.wait().await?;
        if !status.success() {
            bail!(
                "BUILD FAILED: {}\nLog:\n{}",
                name,
                error_buffer.make_contiguous().join("\n")
            );
        }

        let image_id = fs::read_to_string(iid_path)?.trim().to_string();
        let digest_output = self
            .command("images")
            .args(["--no-trunc", "--format", "{{.Digest}}", &image_id])
            .output()
            .await?;

        let digest_str = String::from_utf8(digest_output.stdout)?;
        let mut final_digest = None;
        for line in digest_str.lines() {
            let line = line.trim();
            if !line.is_empty() && line != "<none>" {
                final_digest = Some(line.to_string());
                break;
            }
        }

        let digest = final_digest.context("Failed to retrieve digest")?;
        Ok(BuildResult { digest, local_ref })
    }

    pub async fn push(&self, local_ref: &str, destination: &str) -> Result<String> {
        let digest_file = NamedTempFile::new_in(&self.ctx.paths.tmp_dir)?;
        let digest_path = digest_file.path().to_str().unwrap();

        let status = self
            .command("push")
            .kill_on_drop(true)
            .args(["--digestfile", digest_path, local_ref, destination])
            .status()
            .await?;

        if !status.success() {
            bail!("Failed to push image");
        }
        Ok(fs::read_to_string(digest_path)?.trim().to_string())
    }

    /// Analyzes local storage against active references and destroys garbage
    pub async fn gc(
        &self,
        active_hashes: &HashSet<String>,
        active_refs: &HashSet<String>,
    ) -> Result<GcReport> {
        let output = self.command("images").arg("--json").output().await?;

        if !output.status.success() {
            bail!(
                "Failed to execute buildah images:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let stdout_str = stdout_str.trim();

        if stdout_str.is_empty() || stdout_str == "null" {
            return Ok(GcReport {
                untagged: HashSet::new(),
                removed_ids: HashSet::new(),
            });
        }

        let buildah_images: Vec<BuildahImage> = serde_json::from_str(stdout_str)?;
        let mut garbage_ids = HashSet::new();
        let mut garbage_tags = HashSet::new();

        for img in buildah_images {
            let id_full = if img.id.starts_with("sha256:") {
                img.id.clone()
            } else {
                format!("sha256:{}", img.id)
            };

            let is_known = active_hashes.contains(&img.id)
                || active_hashes.contains(&id_full)
                || active_hashes.contains(&img.digest);

            if !is_known {
                garbage_ids.insert(img.id);
            } else {
                for name in img.names.unwrap_or_default() {
                    if !name.is_empty() && !name.contains("<none>") && !active_refs.contains(&name)
                    {
                        garbage_tags.insert(name);
                    }
                }
            }
        }

        // Phase 1: Untag
        if !garbage_tags.is_empty() {
            let mut args = vec!["rmi"];
            args.extend(garbage_tags.iter().map(|s| s.as_str()));
            self.command("").args(&args).status().await?;
        }

        // Phase 2: Remove IDs
        if !garbage_ids.is_empty() {
            let mut args = vec!["rmi", "-f"];
            args.extend(garbage_ids.iter().map(|s| s.as_str()));
            self.command("").args(&args).status().await?;
        }

        Ok(GcReport {
            untagged: garbage_tags,
            removed_ids: garbage_ids,
        })
    }

    /// Physically obliterates all images in the isolated container storage
    pub async fn clean(&self) -> Result<()> {
        let status = self.command("rmi").arg("-a").arg("-f").status().await?;

        if !status.success() {
            bail!("Clean failed while removing imges.");
        }
        Ok(())
    }

    /// Overwrites the current process with an interactive shell configured for the isolated environment
    pub fn shell(&self, project_name: &str) -> Result<()> {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let mut cmd = self.std_command(&shell);
        cmd.env("DULL_PROJECT_NAME", project_name);

        let err = cmd.exec(); // Never returns if successful
        bail!("Failed to exec shell: {}", err);
    }
}
