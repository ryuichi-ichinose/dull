use crate::cli::ImagesArgs;
use crate::config::ProjectContext;
use crate::engine::BuildahEngine;
use crate::image_reference::parse_oci_reference;
use anyhow::{Context, Result};
use comfy_table::{Cell, Color, Table, presets::UTF8_FULL};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Deserialize, Debug, Clone)]
struct LocalImage {
    id: String,
    digest: String,
    #[serde(default)]
    names: Option<Vec<String>>,
}

#[derive(Serialize)]
struct OutputRow {
    source: String,
    registry: String,
    name: String,
    tag: String,
    digest: String,
    image_id: String,
}

fn shorten_hash(hash: &str) -> &str {
    if hash.is_empty() || hash == "-" {
        return "-";
    }
    let naked = hash.strip_prefix("sha256:").unwrap_or(hash);
    &naked[..12.min(naked.len())]
}

pub async fn run_images(
    engine: &BuildahEngine,
    ctx: ProjectContext,
    args: ImagesArgs,
) -> Result<()> {
    let output = engine
        .command("images")
        .arg("--json")
        .output()
        .await
        .context("Failed to execute 'buildah images'")?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to get local images:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stdout_str = stdout_str.trim();

    let local_images: Vec<LocalImage> = if stdout_str.is_empty() || stdout_str == "null" {
        Vec::new()
    } else {
        serde_json::from_str(stdout_str).context("Failed to parse buildah images JSON")?
    };

    let mut storage_map = HashMap::new();
    for img in &local_images {
        if let Some(names) = &img.names {
            for name in names {
                if name != "<none>" {
                    storage_map.insert(name.clone(), img.clone());
                }
            }
        }
    }

    let mut tracked_fqins = HashSet::new();
    let mut output_rows = Vec::new();

    if !args.state {
        for (_key, reference) in &ctx.lock.images {
            let fqin = reference.to_tag_string();
            tracked_fqins.insert(fqin.clone());

            let registry = reference.registry.as_deref().unwrap_or("-");
            let name = &reference.name;
            let tag = reference.tag.as_deref().unwrap_or("-");
            let expected_digest = reference.digest.as_deref().unwrap_or("-");

            if let Some(local_img) = storage_map.get(&fqin) {
                output_rows.push(OutputRow {
                    source: "Lock".to_string(),
                    registry: registry.to_string(),
                    name: name.to_string(),
                    tag: tag.to_string(),
                    digest: shorten_hash(&local_img.digest).to_string(),
                    image_id: shorten_hash(&local_img.id).to_string(),
                });
            } else {
                output_rows.push(OutputRow {
                    source: "Lock (Missing)".to_string(),
                    registry: registry.to_string(),
                    name: name.to_string(),
                    tag: tag.to_string(),
                    digest: shorten_hash(expected_digest).to_string(),
                    image_id: "<MISSING>".to_string(),
                });
            }
        }
    }

    if !args.lock {
        for (_key, reference) in &ctx.state.images {
            let fqin = reference.to_tag_string();
            tracked_fqins.insert(fqin.clone());

            let registry = reference.registry.as_deref().unwrap_or("-");
            let name = &reference.name;
            let tag = reference.tag.as_deref().unwrap_or("-");
            let expected_digest = reference.digest.as_deref().unwrap_or("-");

            if let Some(local_img) = storage_map.get(&fqin) {
                output_rows.push(OutputRow {
                    source: "State".to_string(),
                    registry: registry.to_string(),
                    name: name.to_string(),
                    tag: tag.to_string(),
                    digest: shorten_hash(&local_img.digest).to_string(),
                    image_id: shorten_hash(&local_img.id).to_string(),
                });
            } else {
                output_rows.push(OutputRow {
                    source: "State (Missing)".to_string(),
                    registry: registry.to_string(),
                    name: name.to_string(),
                    tag: tag.to_string(),
                    digest: shorten_hash(expected_digest).to_string(),
                    image_id: "<MISSING>".to_string(),
                });
            }
        }
    }

    if args.all {
        for local_img in &local_images {
            if let Some(names) = &local_img.names {
                for name_str in names {
                    if name_str != "<none>" && !tracked_fqins.contains(name_str) {
                        let parsed = parse_oci_reference(name_str);

                        let registry = parsed
                            .as_ref()
                            .and_then(|r| r.registry.as_deref())
                            .unwrap_or("-");
                        let name = parsed.as_ref().map(|r| r.name.as_str()).unwrap_or(name_str);
                        let tag = parsed
                            .as_ref()
                            .and_then(|r| r.tag.as_deref())
                            .unwrap_or("-");

                        output_rows.push(OutputRow {
                            source: "Untracked".to_string(),
                            registry: registry.to_string(),
                            name: name.to_string(),
                            tag: tag.to_string(),
                            digest: shorten_hash(&local_img.digest).to_string(),
                            image_id: shorten_hash(&local_img.id).to_string(),
                        });
                    }
                }
            }
        }
    }

    if args.json {
        let json_output = serde_json::to_string_pretty(&output_rows)?;
        println!("{}", json_output);
        return Ok(());
    }

    if output_rows.is_empty() {
        println!("No images found.");
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        "SOURCE", "REGISTRY", "NAME", "TAG", "DIGEST", "IMAGE ID",
    ]);

    for row in output_rows {
        let (c_src, c_def, c_name, c_id) = match row.source.as_str() {
            "Lock" => (Color::Cyan, Color::Reset, Color::White, Color::DarkGrey),
            "State" => (Color::Green, Color::Reset, Color::White, Color::DarkGrey),
            "Untracked" => (
                Color::DarkYellow,
                Color::DarkGrey,
                Color::DarkGrey,
                Color::DarkGrey,
            ),
            _ if row.source.contains("Missing") => (Color::Red, Color::Red, Color::Red, Color::Red),
            _ => (Color::Reset, Color::Reset, Color::Reset, Color::Reset),
        };

        table.add_row(vec![
            Cell::new(&row.source).fg(c_src),
            Cell::new(&row.registry).fg(c_def),
            Cell::new(&row.name).fg(c_name),
            Cell::new(&row.tag).fg(c_def),
            Cell::new(&row.digest).fg(c_def),
            Cell::new(&row.image_id).fg(c_id),
        ]);
    }

    println!("{table}");
    Ok(())
}
