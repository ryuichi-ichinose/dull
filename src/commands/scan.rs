use crate::config::{ImageProfile, ProjectContext, save_config};
use anyhow::Result;
use ignore::WalkBuilder;
use std::collections::BTreeMap;

pub fn run_scan(mut ctx: ProjectContext) -> Result<()> {
    let mut disk_images = BTreeMap::new();
    println!("Scanning for Containerfiles in {:?}", ctx.root);

    for entry in WalkBuilder::new(&ctx.root).build().filter_map(|e| e.ok()) {
        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();
        let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if fname == "Dockerfile" || fname == "Containerfile" || fname.ends_with(".Containerfile") {
            let rel_path = path.strip_prefix(&ctx.root)?.to_string_lossy().to_string();
            let parent = path.parent().unwrap_or(&ctx.root);
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

            disk_images.insert(
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

    ctx.config.images.retain(|name, _| {
        let exists = disk_images.contains_key(name);
        if !exists {
            println!("- Removed missing image: {}", name);
        }
        exists
    });

    for (name, profile) in disk_images {
        ctx.config.images.entry(name.clone()).or_insert_with(|| {
            println!("+ Found new image: {}", name);
            profile
        });
    }

    save_config(&ctx)?;
    println!("Updated dull.toml with scanned results.");

    Ok(())
}
