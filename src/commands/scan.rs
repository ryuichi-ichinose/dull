use crate::cli::ScanArgs;
use crate::config::{ImageProfile, ProjectContext, save_config};
use anyhow::{Result, bail};
use ignore::WalkBuilder;
use std::collections::BTreeMap;

fn is_valid_image_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    // OCI repository name spec: [a-z0-9]+([._-][a-z0-9]+)*
    let mut chars = name.chars();

    // First char must be alphanumeric lowercase
    if let Some(first) = chars.next()
        && !first.is_ascii_lowercase()
        && !first.is_ascii_digit()
    {
        return false;
    }

    let mut last_was_sep = false;
    for c in chars {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            last_was_sep = false;
        } else if c == '.' || c == '_' || c == '-' {
            if last_was_sep {
                return false; // consecutive separators not allowed
            }
            last_was_sep = true;
        } else {
            return false; // invalid char (includes uppercase)
        }
    }

    !last_was_sep // cannot end with separator
}

pub fn run_scan(mut ctx: ProjectContext, args: ScanArgs) -> Result<()> {
    let mut disk_images = BTreeMap::new();

    let target_dirs = if args.directories.is_empty() {
        vec!["".to_string()]
    } else {
        args.directories.clone()
    };

    println!("Scanning for Containerfiles...");

    let mut builder = WalkBuilder::new(ctx.paths.root.join(&target_dirs[0]));
    for dir in target_dirs.iter().skip(1) {
        builder.add(ctx.paths.root.join(dir));
    }

    for entry in builder.build().filter_map(|e| e.ok()) {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();
        let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if fname == "Dockerfile" || fname == "Containerfile" || fname.ends_with(".Containerfile") {
            let rel_path = path
                .strip_prefix(&ctx.paths.root)?
                .to_string_lossy()
                .to_string();
            let parent = path.parent().unwrap_or(&ctx.paths.root);
            let context_path = parent
                .strip_prefix(&ctx.paths.root)?
                .to_string_lossy()
                .to_string();

            let name = if context_path.is_empty() {
                "root".to_string()
            } else {
                context_path.replace(std::path::MAIN_SEPARATOR, "-")
            }
            .to_lowercase();

            if !is_valid_image_name(&name) {
                bail!(
                    "Invalid image name '{}' derived from path '{}'. \
                     Image names must be OCI compliant (lowercase alphanumeric, dots, underscores, and dashes).",
                    name,
                    rel_path
                );
            }

            let context_str = if let Some(ref c) = args.context {
                c.clone()
            } else if context_path.is_empty() {
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
                    args: BTreeMap::new(),
                },
            );
        }
    }

    ctx.config.images.retain(|name, profile| {
        let full_path = ctx.paths.root.join(&profile.path);
        let exists = full_path.is_file();
        if !exists {
            println!("- Removed missing image (file not found): {}", name);
        }
        exists
    });

    for (name, profile) in disk_images {
        ctx.config
            .images
            .entry(name.clone())
            .and_modify(|existing| {
                existing.context = profile.context.clone();
            })
            .or_insert_with(|| {
                println!("+ Found new image: {}", name);
                profile
            });
    }

    save_config(&ctx)?;
    println!("Updated dull.toml with scanned results.");

    Ok(())
}
