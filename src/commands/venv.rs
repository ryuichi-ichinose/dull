use crate::config::{ProjectPaths, write_atomic};
use anyhow::Result;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

fn is_selinux_enabled() -> bool {
    Path::new("/sys/fs/selinux").exists()
}

fn apply_selinux_label(path: &Path) -> Result<()> {
    if !is_selinux_enabled() {
        return Ok(());
    }

    let label = b"system_u:object_r:container_var_lib_t:s0";
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

    fs::create_dir_all(&paths.storage_dir)?;
    apply_selinux_label(&paths.storage_dir)?;

    if is_selinux_enabled() {
        let storage_path = paths.storage_dir.display();
        println!("WARNING: The SELinux label applied to the storage directory is temporary.");
        println!(
            "To ensure it survives a filesystem relabel (restorecon), register it permanently:"
        );
        println!(
            "  sudo semanage fcontext -a -t container_var_lib_t '{}(/.*)?'",
            storage_path
        );
        println!("  sudo restorecon -R -v {}", storage_path);
        println!();
    }

    let conf_content = format!(
        "[storage]\n\
         driver = \"overlay\"\n\
         graphroot = \"{}\"\n\
         \n\
         [storage.options]\n\
         pull_options = {{convert_images = \"true\", enable_partial_images = \"true\", use_hard_links = \"true\"}}\n\
         \n\
         [storage.options.overlay]\n\
         use_composefs = \"false\"\n\
         mountopt = \"nodev,metacopy=on\"\n",
        paths.storage_dir.display(),
    );

    write_atomic(&paths.storage_conf, &conf_content)?;
    println!("Initialized .dull/storage.conf");

    // Self-contained ignore rule
    let gitignore_path = paths.dull_dir.join(".gitignore");
    fs::write(&gitignore_path, "*\n")?;

    let dockerignore_path = paths.root.join(".dockerignore");
    let mut needs_dull_ignore = true;
    let mut needs_leading_newline = false;

    if dockerignore_path.exists() {
        let content = fs::read_to_string(&dockerignore_path)?;
        if content.lines().any(|line| line.trim() == ".dull") {
            needs_dull_ignore = false;
        } else if !content.is_empty() && !content.ends_with('\n') {
            needs_leading_newline = true;
        }
    }

    if needs_dull_ignore {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&dockerignore_path)?;

        if needs_leading_newline {
            writeln!(file)?;
        }

        write!(file, ".dull\n")?;
    }
    Ok(())
}
