use anyhow::{Context, Result, bail};
use std::process::Command;

pub const MIN_BUILDAH_VERSION: (u32, u32, u32) = (1, 43, 1);
pub const MIN_SKOPEO_VERSION: (u32, u32, u32) = (1, 22, 2);

pub fn require_tool_version(command: &str, min_version: (u32, u32, u32)) -> Result<()> {
    let output = Command::new(command)
        .arg("--version")
        .output()
        .with_context(|| format!("'{}' command not found. Please install it.", command))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    let current_version = parse_version_from_output(&stdout).with_context(|| {
        format!(
            "Failed to parse version from {} output: {}",
            command, stdout
        )
    })?;

    if current_version < min_version {
        let (req_maj, req_min, req_pat) = min_version;
        let (cur_maj, cur_min, cur_pat) = current_version;
        bail!(
            "{} version is too old. Found {}.{}.{}, but requires at least {}.{}.{}",
            command,
            cur_maj,
            cur_min,
            cur_pat,
            req_maj,
            req_min,
            req_pat
        );
    }

    Ok(())
}

fn parse_version_from_output(output: &str) -> Option<(u32, u32, u32)> {
    for word in output.split_whitespace() {
        let cleaned = word.strip_prefix('v').unwrap_or(word);

        if cleaned.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            let parts: Vec<&str> = cleaned.split('.').collect();

            if parts.len() >= 2 {
                let major = parts[0].parse::<u32>().ok()?;
                let minor = parts[1].parse::<u32>().ok()?;

                let patch_str = parts.get(2).unwrap_or(&"0");
                let patch_clean: String = patch_str
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                let patch = patch_clean.parse::<u32>().unwrap_or(0);

                return Some((major, minor, patch));
            }
        }
    }
    None
}
