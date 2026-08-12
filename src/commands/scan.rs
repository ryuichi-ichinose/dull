use crate::cli::ScanArgs;
use crate::config::{ImageProfile, ProjectContext, save_config};
use anyhow::{Result, bail};
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use std::collections::BTreeMap;

fn is_valid_image_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    // OCI repository name spec: [a-z0-9]+([._-/][a-z0-9]+)*
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
        } else if c == '.' || c == '_' || c == '-' || c == '/' {
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

    let mut override_builder = OverrideBuilder::new(&ctx.paths.root);
    override_builder.add("**/Dockerfile")?;
    override_builder.add("**/Containerfile")?;
    override_builder.add("**/*.Dockerfile")?;
    override_builder.add("**/*.Containerfile")?;
    override_builder.add("**/Dockerfile.*")?;
    override_builder.add("**/Containerfile.*")?;
    override_builder.add("Dockerfile")?;
    override_builder.add("Containerfile")?;
    override_builder.add("*.Dockerfile")?;
    override_builder.add("*.Containerfile")?;
    override_builder.add("Dockerfile.*")?;
    override_builder.add("Containerfile.*")?;
    let overrides = override_builder.build()?;

    let mut builder = WalkBuilder::new(ctx.paths.root.join(&target_dirs[0]));

    builder.overrides(overrides);

    for dir in target_dirs.iter().skip(1) {
        builder.add(ctx.paths.root.join(dir));
    }

    for entry in builder.build().filter_map(|e| e.ok()) {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();
        let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Extract prefix or suffix from the filename
        let file_ext = if fname == "Dockerfile" || fname == "Containerfile" {
            ""
        } else if let Some(prefix) = fname.strip_suffix(".Dockerfile") {
            prefix
        } else if let Some(prefix) = fname.strip_suffix(".Containerfile") {
            prefix
        } else if let Some(suffix) = fname.strip_prefix("Dockerfile.") {
            suffix
        } else if let Some(suffix) = fname.strip_prefix("Containerfile.") {
            suffix
        } else {
            continue;
        };

        let rel_path = path
            .strip_prefix(&ctx.paths.root)?
            .to_string_lossy()
            .to_string();
        let parent = path.parent().unwrap_or(&ctx.paths.root);
        let context_path = parent
            .strip_prefix(&ctx.paths.root)?
            .to_string_lossy()
            .to_string();

        let mut base_name = if context_path.is_empty() {
            "root".to_string()
        } else {
            // Replace path separators with '/' instead of '-'
            context_path.replace(std::path::MAIN_SEPARATOR, "/")
        };

        if !file_ext.is_empty() {
            // Always append the extracted string to the end
            base_name = format!("{}-{}", base_name, file_ext);
        }

        let name = base_name.to_lowercase();

        if !is_valid_image_name(&name) {
            bail!(
                "Invalid image name '{}' derived from path '{}'. \
                 Image names must be OCI compliant (lowercase alphanumeric, dots, underscores, dashes, and slashes).",
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

        let profile = ImageProfile {
            path: rel_path,
            local_tag: "latest".to_string(),
            context: context_str,
            args: BTreeMap::new(),
        };

        if let Some(existing) = disk_images.insert(name.clone(), profile.clone()) {
            bail!(
                "Image name collision detected: The lowercase OCI name '{}' is generated for both '{}' and '{}'.\n\
                 OCI image names must be unique and lowercase. Please resolve this ambiguity by renaming the directories or files.",
                name,
                existing.path,
                profile.path
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
