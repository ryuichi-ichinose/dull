use anyhow::{Context, Result, bail};
use std::fs;
use std::process::Command;
use tempfile::NamedTempFile;

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

        // イメージIDを確実に受け取るためのファイル
        let iid_file = NamedTempFile::new().context("Failed to create temp iidfile")?;
        let iid_path = iid_file.path().to_str().unwrap();

        let mut cmd = Command::new("buildah");
        cmd.arg("bud")
            .arg("--format=oci")
            .arg("--layers=true")
            .arg("--iidfile")
            .arg(iid_path) // --digestfile の代わりにこれを使え
            .arg("-f")
            .arg(opts.dockerfile_path)
            .arg("-t")
            .arg(&local_ref)
            .arg(opts.context_dir);

        // TODO: ここで出力をキャプチャし、ログファイルへ流す実装を入れろ
        let status = cmd.status().context("Buildah execution failed")?;

        if !status.success() {
            bail!("Build failed for {}", name);
        }

        // 1. まずはイメージIDをファイルから読み取る
        let image_id = fs::read_to_string(iid_path)?.trim().to_string();

        // 2. そのIDを元に「ローカルでの」ダイジェストを取得する
        // buildah images --no-trunc --format {{.Digest}} <ID>
        let digest_output = Command::new("buildah")
            .arg("images")
            .arg("--no-trunc")
            .arg("--format")
            .arg("{{.Digest}}")
            .arg(&image_id)
            .output()?;

        let digest = String::from_utf8(digest_output.stdout)?.trim().to_string();

        // もしローカルビルド直後でダイジェストが空なら、イメージIDを暫定ハッシュとして使う
        let final_digest = if digest.is_empty() || digest == "<none>" {
            image_id
        } else {
            digest
        };

        println!("✅ Built {}: {}", name, final_digest);
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
            .arg(digest_path) // 結果をこのファイルに書き込ませる
            .arg(local_ref)
            .arg(destination)
            .status()
            .context("Failed to execute buildah push")?;

        if !status.success() {
            anyhow::bail!("Failed to push image to {}", destination);
        }

        // 一時ファイルから純粋なダイジェスト文字列だけを読み取る
        let digest = fs::read_to_string(digest_path)?.trim().to_string();

        Ok(digest)
    }
}
