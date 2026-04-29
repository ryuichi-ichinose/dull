use crate::config::{DullConfig, ProjectConfig, save_config_file, write_atomic};
use anyhow::Result;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

fn apply_selinux_label(path: &Path) -> Result<()> {
    let label = b"system_u:object_r:container_file_t:s0";
    if let Err(e) = xattr::set(path, "security.selinux", label) {
        // Ignore ENOTSUP (Operation not supported) or other SELinux-related issues if not supported by the system
        if e.raw_os_error() == Some(95) || e.raw_os_error() == Some(61) {
            return Ok(());
        }
        return Err(e.into());
    }
    Ok(())
}

pub fn run_init() -> Result<()> {
    let root = env::current_dir()?;
    let config_path = root.join("dull.toml");
    if config_path.exists() {
        println!("dull.toml already exists.");
        return Ok(());
    }

    let dir_name = root
        .file_name()
        .and_then(|os_str| os_str.to_str())
        .unwrap_or("default-project")
        .to_string();

    let conf = DullConfig {
        project: ProjectConfig { name: dir_name },
        registries: BTreeMap::new(),
        images: BTreeMap::new(),
    };

    save_config_file(&config_path, &conf)?;
    println!("Initialized dull.toml");

    // Initialize .dull environment
    let dull_dir = root.join(".dull");
    let storage_dir = dull_dir.join("storage");
    let storage_conf = dull_dir.join("storage.conf");
    let global_cache = "/var/lib/shared-container-cache";

    fs::create_dir_all(&storage_dir)?;
    apply_selinux_label(&storage_dir)?;

    let mut additional_stores_line = String::new();
    if std::path::Path::new(global_cache).exists() {
        additional_stores_line = format!("additionalimagestores = [\"{}\"]", global_cache);
    }

    let conf_content = format!(
        "[storage]\n\
         driver = \"overlay\"\n\
         graphroot = \"{}\"\n\
         {}\n\
         \n\
         [storage.options]\n\
         pull_options = {{convert_images = \"true\", enable_partial_images = \"true\", use_hard_links = \"true\"}}\n\
         \n\
         [storage.options.overlay]\n\
         use_composefs = \"false\"\n\
         mountopt = \"nodev,metacopy=on\"\n",
        storage_dir.display(),
        additional_stores_line
    );

    write_atomic(&storage_conf, &conf_content)?;
    println!("Initialized .dull/storage.conf");

    // Update .gitignore
    let gitignore_path = root.join(".gitignore");
    let mut gitignore_content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    if !gitignore_content.contains(".dull") {
        if !gitignore_content.is_empty() && !gitignore_content.ends_with('\n') {
            gitignore_content.push('\n');
        }
        gitignore_content.push_str(".dull\n");
        fs::write(&gitignore_path, gitignore_content)?;
        println!("Added .dull to .gitignore");
    }

    Ok(())
}
