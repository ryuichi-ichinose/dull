use crate::config::{ProjectPaths, write_atomic};
use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;

fn apply_selinux_label(path: &Path) -> Result<()> {
    let label = b"system_u:object_r:container_file_t:s0";
    if let Err(e) = xattr::set(path, "security.selinux", label) {
        // Ignore ENOTSUP or other SELinux-related issues if not supported
        if e.raw_os_error() == Some(95) || e.raw_os_error() == Some(61) {
            return Ok(());
        }
        return Err(e.into());
    }
    Ok(())
}

pub fn run_venv() -> Result<()> {
    let root = env::current_dir()?;
    let paths = ProjectPaths::new(root);

    if paths.dull_dir.exists() {
        println!("Virtual environment (.dull) already exists. Nothing to do.");
        return Ok(());
    }

    let global_cache = "/var/lib/shared-container-cache";

    fs::create_dir_all(&paths.storage_dir)?;
    apply_selinux_label(&paths.storage_dir)?;

    let mut additional_stores_line = String::new();
    if Path::new(global_cache).exists() {
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
        paths.storage_dir.display(),
        additional_stores_line
    );

    write_atomic(&paths.storage_conf, &conf_content)?;
    println!("Initialized .dull/storage.conf");

    // Self-contained ignore rule
    let gitignore_path = paths.dull_dir.join(".gitignore");
    fs::write(&gitignore_path, "*\n")?;

    let dockerignore_path = paths.dull_dir.join(".dockerignore");
    fs::write(&dockerignore_path, "*\n")?;

    Ok(())
}
