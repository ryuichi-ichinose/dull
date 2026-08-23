use crate::cli::ImagesArgs;
use crate::config::ProjectContext;
use crate::engine::{BuildahEngine, LocalImage};
use crate::image_reference::{ImageReference, parse_oci_reference, shorten_digest};
use anyhow::Result;
use comfy_table::{Cell, Color, Table, presets::UTF8_FULL};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Serialize)]
struct OutputRow {
    source: String,
    registry: String,
    name: String,
    tag: String,
    digest: String,
    image_id: String,
}

fn build_tracked_row(
    source_label: &str,
    reference: &ImageReference,
    storage_map: &HashMap<String, LocalImage>,
) -> (String, OutputRow) {
    let fqin = reference.to_tag_string();
    let registry = reference.registry.as_deref().unwrap_or("-").to_string();
    let name = reference.name.clone();
    let tag = reference.tag.as_deref().unwrap_or("-").to_string();

    let row = match storage_map.get(&fqin) {
        Some(local_img) => OutputRow {
            source: source_label.to_string(),
            registry,
            name,
            tag,
            digest: shorten_digest(&local_img.digest).to_string(),
            image_id: shorten_digest(&local_img.id).to_string(),
        },
        None => OutputRow {
            source: format!("{} (Missing)", source_label),
            registry,
            name,
            tag,
            digest: shorten_digest(reference.digest.as_deref().unwrap_or("-")).to_string(),
            image_id: "<MISSING>".to_string(),
        },
    };

    (fqin, row)
}

pub async fn run_images(
    engine: &BuildahEngine,
    ctx: ProjectContext,
    args: ImagesArgs,
) -> Result<()> {
    let local_images = engine.list_images().await?;

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
        for reference in ctx.lock.images.values() {
            let (fqin, row) = build_tracked_row("Lock", reference, &storage_map);
            tracked_fqins.insert(fqin);
            output_rows.push(row);
        }
    }

    if !args.lock {
        for reference in ctx.state.images.values() {
            let (fqin, row) = build_tracked_row("State", reference, &storage_map);
            tracked_fqins.insert(fqin);
            output_rows.push(row);
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
                            digest: shorten_digest(&local_img.digest).to_string(),
                            image_id: shorten_digest(&local_img.id).to_string(),
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
