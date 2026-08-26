use crate::cli::VenvArgs;
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
        // ENOTSUP/EOPNOTSUPP (95): xattrs unsupported or disabled on this filesystem.
        if e.raw_os_error() == Some(95) {
            return Ok(());
        }
        return Err(e.into());
    }
    Ok(())
}

pub fn run_venv(args: VenvArgs) -> Result<()> {
    let root = env::current_dir()?;
    let paths = ProjectPaths::new(root);

    if paths.dull_dir.exists() {
        println!("Virtual environment (.dull) already exists. Nothing to do.");
        return Ok(());
    }

    if args.global {
        fs::create_dir_all(&paths.containers_dir)?;
        write_atomic(
            &paths.storage_conf,
            "# Intentionally empty: dull uses the host's global container storage.\n",
        )?;
        println!("Initialized .dull/storage.conf (using the host's global container storage)");
    } else {
        fs::create_dir_all(&paths.storage_dir)?;
        apply_selinux_label(&paths.storage_dir)?;

        if is_selinux_enabled() {
            let storage_path = paths.storage_dir.display();
            println!("Warning: SELinux label is temporary. To persist across relabels:");
            println!(
                "  sudo semanage fcontext -a -t container_var_lib_t '{}(/.*)?'",
                storage_path
            );
            println!("  sudo restorecon -R -v {}", storage_path);
            println!();
        }

        let mut overlay_options = format!(
            "use_composefs = \"{composefs}\"\nmountopt = \"{mountopt}\"\n",
            composefs = args.composefs,
            mountopt = args.mountopt,
        );
        if let Some(ref size) = args.size {
            overlay_options.push_str(&format!("size = \"{}\"\n", size));
        }
        if let Some(ref inodes) = args.inodes {
            overlay_options.push_str(&format!("inodes = \"{}\"\n", inodes));
        }

        let conf_content = format!(
            "[storage]\n\
             driver = \"{driver}\"\n\
             graphroot = \"{graphroot}\"\n\
             \n\
             [storage.options]\n\
             pull_options = {{convert_images = \"{convert_images}\", enable_partial_images = \"{enable_partial_images}\", use_hard_links = \"{use_hard_links}\"}}\n\
             \n\
             [storage.options.overlay]\n\
             {overlay_options}",
            driver = args.driver,
            graphroot = paths.storage_dir.display(),
            convert_images = args.convert_images,
            enable_partial_images = args.enable_partial_images,
            use_hard_links = args.use_hard_links,
            overlay_options = overlay_options,
        );

        write_atomic(&paths.storage_conf, &conf_content)?;
        println!("Initialized .dull/storage.conf");
    }

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
