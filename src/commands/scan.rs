use crate::config::{ImageProfile, ProjectContext, save_config};
use anyhow::Result;
use std::collections::BTreeMap;
use walkdir::WalkDir;

pub fn run_scan(mut ctx: ProjectContext) -> Result<()> {
    let mut new_found = BTreeMap::new();
    println!("Scanning for Containerfiles in {:?}", ctx.root);

    for entry in WalkDir::new(&ctx.root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }

        let fname = entry.file_name().to_string_lossy();
        if fname == "Dockerfile" || fname == "Containerfile" || fname.ends_with(".Containerfile") {
            let rel_path = entry
                .path()
                .strip_prefix(&ctx.root)?
                .to_string_lossy()
                .to_string();
            let parent = entry.path().parent().unwrap_or(&ctx.root);
            let context_path = parent
                .strip_prefix(&ctx.root)?
                .to_string_lossy()
                .to_string();

            let name = if context_path.is_empty() {
                "root".to_string()
            } else {
                context_path.replace(std::path::MAIN_SEPARATOR, "-")
            };

            let context_str = if context_path.is_empty() {
                ".".to_string()
            } else {
                context_path
            };

            new_found.insert(
                name,
                ImageProfile {
                    path: rel_path,
                    depends_on: Vec::new(),
                    local_tag: "latest".to_string(),
                    context: context_str,
                },
            );
        }
    }

    if new_found.is_empty() {
        println!("No Containerfiles found.");
    } else {
        for (k, v) in new_found {
            ctx.config.images.entry(k).or_insert(v);
        }
        save_config(&ctx)?;
        println!("Updated dull.toml with found images.");
    }
    Ok(())
}
