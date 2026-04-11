mod engine; // モジュールを読み込む

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::engine::{BuildahEngine, ContainerEngine, ImageProfile};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DeckConfig {
    pub project: Option<ProjectConfig>,
    #[serde(default)]
    pub images: BTreeMap<String, ImageProfile>,
}

impl DeckConfig {
    fn get_ordered_targets(&self) -> Vec<(&String, &ImageProfile)> {
        self.images.iter().collect()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProjectConfig {
    pub name: Option<String>,
    #[serde(default)]
    pub registries: BTreeMap<String, String>,
}



#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct DeckLock {
    pub images: BTreeMap<String, String>,
}

#[derive(Parser)]
#[command(name = "deck", version, about = "Container build orchestrator.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Scan,
    Login {
        registry: Option<String>,
        #[arg(short, long)]
        username: Option<String>,
        #[arg(short, long)]
        password: Option<String>,
        #[arg(last = true)]
        extra_args: Vec<String>,
    },
    Registry {
        #[command(subcommand)]
        action: RegistryAction,
    },
    Build {
        #[arg(short,long, conflicts_with = "images")]
        all: bool,
        #[arg(num_args = 1..)]
        images: Vec<String>,
        #[arg(long)]
        no_lock: bool,
        #[arg(long, default_value = "latest")]
        tag: String,
    },
    Push {
        #[arg(short,long, conflicts_with = "images")]
        all: bool,
        #[arg(num_args = 1..)]
        images: Vec<String>,
        #[arg(short, long, default_value = "latest")]
        tag: String,
        #[arg(short = 'r', long)]
        registry: Option<String>,
        #[arg(long)]
        insecure: bool,
    },
}

#[derive(Subcommand)]
enum RegistryAction {
    Add { name: String, url: String },
    Rm { name: String },
    Ls,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let engine = BuildahEngine::new()?;

    match &cli.command {
        Commands::Init => run_init(),
        Commands::Scan => run_scan(),
        Commands::Login { registry, username, password, extra_args } => 
            run_login(&engine, registry.as_deref(), username.as_deref(), password.as_deref(), extra_args),
        Commands::Registry { action } => run_registry(action),
        Commands::Build { all, images, no_lock, tag } => 
            run_build(&engine, *all, images, *no_lock, tag),
        Commands::Push { all, images, tag, registry, insecure} => 
            run_push(&engine, *all, images, tag, registry.as_deref(), *insecure),
    }
}


fn run_login(engine: &impl ContainerEngine, registry_alias: Option<&str>, username: Option<&str>, password: Option<&str>, extra_args: &[String]) -> Result<()> {
    let (_, config) = load_context()?;
    let registries = &config.project.as_ref()
        .context("Project config missing")?
        .registries;

    let target = registry_alias.unwrap_or("default");
    let registry_url = registries.get(target).map(|s| s.as_str()).unwrap_or(target);

    engine.login(registry_url, username, password, extra_args)
}

fn run_registry(action: &RegistryAction) -> Result<()> {
    let (root, mut config) = load_context_or_create()?;
    
    let project = config.project.get_or_insert(ProjectConfig { 
        name: None,
        registries: BTreeMap::new() 
    });

    match action {
        RegistryAction::Add { name, url } => {
            let clean_url = url.trim_end_matches('/');
            project.registries.insert(name.clone(), clean_url.to_string());
            println!("Registry '{}' -> '{}'", name, clean_url);
        }
        RegistryAction::Rm { name } => { 
            if project.registries.remove(name).is_some() {
                println!("Removed '{}'", name);
            } else {
                eprintln!("Registry '{}' not found.", name);
            }
        }
        RegistryAction::Ls => {
            for (name, url) in &project.registries { 
                println!(" - {:<10} : {}", name, url); 
            }
            return Ok(()); 
        }
    }
    save_config(&root.join("deck.toml"), &config)?;
    Ok(())
}

fn run_build(engine: &impl ContainerEngine, all: bool, images: &[String], no_lock: bool, local_tag: &str) -> Result<()> {
    let (root, config) = load_context()?;
    
    let mut lock_data = if !no_lock { 
        load_lock(&root).unwrap_or_default() 
    } else { 
        DeckLock::default() 
    };

    let targets = if all { 
        config.get_ordered_targets() 
    } else {
        if images.is_empty() { bail!("Specify image names or --all"); }
        images.iter()
            .filter_map(|n| config.images.get_key_value(n).map(|(k, v)| (k, v)))
            .collect()
    };

    println!("🚀 Building {} targets...", targets.len());

    for (name, profile) in targets {
        let digest = engine.build(&root, name, profile, local_tag)?;
        lock_data.images.insert(name.clone(), digest);
    }

    if !no_lock {
        lock_data.images.retain(|name, _| config.images.contains_key(name));
        save_lock(&root, &lock_data)?;
        println!("🔒 deck.lock updated.");
    }
    Ok(())
}

fn run_push(engine: &impl ContainerEngine, all: bool, images: &[String], tag: &str, registry_alias: Option<&str>, insecure: bool) -> Result<()> {
    let (root, config) = load_context()?;
    let lock = load_lock(&root).context("❌ deck.lock not found. Run 'build' first.")?;
    
    let registries = &config.project.as_ref()
        .context("Project config missing")?
        .registries;

    let target_alias = registry_alias.unwrap_or("default");
    let registry_url = registries.get(target_alias)
        .or_else(|| if registries.len() == 1 { registries.values().next() } else { None })
        .with_context(|| format!("Registry alias '{}' not found", target_alias))?;

    let targets: Vec<String> = if all { 
        lock.images.keys().cloned().collect() 
    } else {
        if images.is_empty() { bail!("Specify images or --all"); }
        images.iter()
            .filter(|&n| lock.images.contains_key(n))
            .cloned()
            .collect()
    };

    if targets.is_empty() {
        println!("No locked images found to push.");
        return Ok(());
    }

    println!("Pushing {} images to '{}'...", targets.len(), registry_url);

    for name in targets {
        if let Some(digest) = lock.images.get(&name) {
            engine.push(registry_url, &name, digest, tag, insecure)?;
        }
    }
    Ok(())
}

// ============================================================================
//  5. Helpers 
// ============================================================================

fn run_init() -> Result<()> {
    let config_path = env::current_dir()?.join("deck.toml");
    if config_path.exists() { 
        println!("deck.toml already exists.");
        return Ok(()); 
    }
    
    let mut registries = BTreeMap::new();
    registries.insert("default".into(), "localhost:5000/project".into());
    
    let conf = DeckConfig { 
        project: Some(ProjectConfig { 
            name: Some("my-project".into()), 
            registries 
        }), 
        images: BTreeMap::new(), 
    };
    
    save_config(&config_path, &conf)?;
    println!("Initialized deck.toml");
    Ok(())
}

fn run_scan() -> Result<()> {
    let (root, mut config) = load_context_or_create()?;
    let mut new_images = BTreeMap::new();
    
    println!("Scanning for Containerfiles in {:?}", root);
    
    for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let fname = entry.file_name().to_string_lossy();
            if fname == "Dockerfile" || fname == "Containerfile" || fname.ends_with(".Containerfile") {
                let rel_path = entry.path().strip_prefix(&root)?.to_string_lossy().to_string();
                let name = entry.path().parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "root".to_string());
                
                new_images.insert(name, ImageProfile { path: rel_path });
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

fn load_context_or_create() -> Result<(PathBuf, DeckConfig)> {
    let mut curr = env::current_dir()?;
    loop {
        let p = curr.join("deck.toml");
        if p.exists() { 
            let content = fs::read_to_string(&p).context("Failed to read deck.toml")?;
            let config = toml::from_str(&content).context("Failed to parse deck.toml")?;
            return Ok((curr, config)); 
        }
        if !curr.pop() { 
            bail!("No deck.toml found. Run 'deck init' first."); 
        }
    }
}

fn load_context() -> Result<(PathBuf, DeckConfig)> { 
    load_context_or_create() 
}

fn save_config(p: &Path, c: &DeckConfig) -> Result<()> { 
    fs::write(p, toml::to_string_pretty(c)?)?; 
    Ok(()) 
}

fn load_lock(r: &Path) -> Result<DeckLock> {
    let p = r.join("deck.lock");
    if p.exists() { 
        let content = fs::read_to_string(&p)?;
        Ok(serde_json::from_str(&content).context("Failed to parse deck.lock")?) 
    } else { 
        Ok(DeckLock::default()) 
    }
}

fn save_lock(r: &Path, l: &DeckLock) -> Result<()> { 
    fs::write(r.join("deck.lock"), serde_json::to_string_pretty(l)?)?; 
    Ok(()) 
}