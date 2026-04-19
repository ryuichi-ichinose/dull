use anyhow::{Context, Result, bail};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

use crate::engine::traits::{BuildOptions, BuildResult, ContainerEngine, RegistryAuth};

pub struct BuildahEngine;

impl BuildahEngine {
    pub fn new() -> Result<Self> {
        if Command::new("buildah").arg("--version").output().is_err() {
            bail!("'buildah' command not found. Install it first.");
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
            bail!("Login failed.");
        }
        println!("Login succeeded.");
        Ok(())
    }

    fn build(&self, name: &str, opts: BuildOptions) -> Result<BuildResult> {
        let local_ref = format!("localhost/{}:{}", name, opts.target_tag);

        // Persistent file to capture built image ID
        let iid_file = NamedTempFile::new().context("Failed to create temp iidfile")?;
        let iid_path = iid_file.path().to_str().unwrap();

        let mut child = Command::new("buildah")
            .args(&[
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

        // Initialize log file and ring buffer for errors
        let log_dir = std::path::Path::new(".dull/logs");
        fs::create_dir_all(log_dir).context("Failed to create log directory")?;
        let mut log_file = File::create(log_dir.join(format!("{}.log", name)))
            .with_context(|| format!("Failed to create log file for {}", name))?;
        let mut error_buffer = VecDeque::with_capacity(20);

        // Capture stdout/stderr concurrently via channel to avoid deadlocks
        let stdout = child.stdout.take().context("Failed to take stdout")?;
        let stderr = child.stderr.take().context("Failed to take stderr")?;

        let (tx, rx) = std::sync::mpsc::channel();

        let tx_out = tx.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().filter_map(|l| l.ok()) {
                let _ = tx_out.send(line);
            }
        });

        let tx_err = tx.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().filter_map(|l| l.ok()) {
                let _ = tx_err.send(line);
            }
        });

        drop(tx);

        while let Ok(line) = rx.recv() {
            // Write to log file
            writeln!(log_file, "{}", line)?;

            // Update progress bar message
            if let Some(ref pb) = opts.progress_bar {
                let msg = if line.len() > 50 {
                    format!("{}...", &line[..47])
                } else {
                    line.clone()
                };
                pb.set_message(msg);
            }

            // Keep track of recent lines for error reporting
            if error_buffer.len() == 20 {
                error_buffer.pop_front();
            }
            error_buffer.push_back(line);
        }

        let status = child.wait().context("Failed to wait for buildah")?;

        if !status.success() {
            let error_msg = error_buffer.make_contiguous().join("\n");
            bail!(
                "BUILD FAILED: {}\n\
                 ================================================================================\n\
                 Last 20 lines of log:\n\
                 {}\n\
                 ================================================================================",
                name,
                error_msg
            );
        }

        // Read image ID from file
        let image_id = fs::read_to_string(iid_path)?.trim().to_string();

        // Retrieve full digest for the built image
        let digest_output = Command::new("buildah")
            .arg("images")
            .arg("--no-trunc")
            .arg("--format")
            .arg("{{.Digest}}")
            .arg(&image_id)
            .output()?;

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

    fn push(&self, local_ref: &str, destination: &str) -> anyhow::Result<String> {
        let digest_file = NamedTempFile::new().context("Failed to create temp file for digest")?;
        let digest_path = digest_file.path().to_str().unwrap();

        let status = Command::new("buildah")
            .arg("push")
            .arg("--digestfile")
            .arg(digest_path)
            .arg(local_ref)
            .arg(destination)
            .status()
            .context("Failed to execute buildah push")?;

        if !status.success() {
            anyhow::bail!("Failed to push image to {}", destination);
        }

        let digest = fs::read_to_string(digest_path)?.trim().to_string();
        Ok(digest)
    }
}
