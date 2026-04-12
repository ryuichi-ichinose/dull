use crate::config::{load_context_or_create, save_config, ImageProfile};
use anyhow::Result;
use std::collections::BTreeMap;
use walkdir::WalkDir;

pub fn run_scan() -> Result<()> {
    let (root, mut config) = load_context_or_create()?;
    let mut new_images = BTreeMap::new();

    println!("Scanning for Containerfiles in {:?}", root);

    for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let fname = entry.file_name().to_string_lossy();
            if fname == "Dockerfile" || fname == "Containerfile" || fname.ends_with(".Containerfile") {
                let rel_path = entry.path().strip_prefix(&root)?.to_string_lossy().to_string();
                let parent_dir = entry.path().parent().unwrap_or(&root);
                let context_path = parent_dir.strip_prefix(&root)?.to_string_lossy();
                
                let name = if context_path.is_empty() {
                    "root".to_string()
                } else {
                    context_path.replace(std::path::MAIN_SEPARATOR, "-")
                };

                let context_str = if context_path.is_empty() {
                    ".".to_string()
                } else {
                    context_path.to_string()
                };

                new_images.insert(
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
    }

    if new_images.is_empty() {
        println!("No Dockerfiles found.");
    } else {
        for (k, v) in new_images {
            config.images.entry(k).or_insert(v);
        }
        save_config(&root.join("deck.toml"), &config)?;
        println!("Updated deck.toml with found images.");
    }
    Ok(())
}
