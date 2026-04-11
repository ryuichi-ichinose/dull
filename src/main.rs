mod engine; // モジュールを読み込む

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use petgraph::algo::toposort;
use petgraph::graph::DiGraph;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use walkdir::WalkDir;

use crate::engine::{BuildahEngine, ContainerEngine, ImageProfile};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DeckConfig {
    pub project: Option<ProjectConfig>,
    #[serde(default)]
    pub images: BTreeMap<String, ImageProfile>,
}

impl DeckConfig {
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProjectConfig {
    pub name: Option<String>,
    #[serde(default)]
    pub registries: BTreeMap<String, String>,
}



#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LockImageProfile {
    pub digest: String,
    pub local_ref: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct DeckLock {
    pub images: BTreeMap<String, LockImageProfile>,
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
        #[arg(short, long, conflicts_with = "images")]
        all: bool,
        #[arg(num_args = 1..)]
        images: Vec<String>,
        #[arg(long)]
        no_lock: bool,
        #[arg(long)]
        tag: Option<String>,
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

fn run_build(
    engine: &(impl ContainerEngine + Sync),
    all: bool,
    images: &[String],
    no_lock: bool,
    tag: &Option<String>,
) -> Result<()> {
    let (root, config) = load_context()?;

    // 1. Determine Target Nodes
    let targets_to_build: BTreeSet<String> = if all {
        config.images.keys().cloned().collect()
    } else {
        if images.is_empty() {
            bail!("Specify image names or use --all");
        }
        images.iter().cloned().collect()
    };

    // 2. Build Full Dependency Graph
    let mut graph = DiGraph::<String, ()>::new();
    let mut node_map = HashMap::new();
    for name in config.images.keys() {
        let index = graph.add_node(name.clone());
        node_map.insert(name.clone(), index);
    }

    for (name, profile) in &config.images {
        let to_index = node_map[name];
        for dep in &profile.depends_on {
            if let Some(&from_index) = node_map.get(dep) {
                graph.add_edge(from_index, to_index, ());
            } else {
                bail!("Image '{}' depends on '{}', which is not defined.", name, dep);
            }
        }
    }

    // 3. Resolve all dependencies for the selected targets
    let mut build_selection = BTreeSet::new();
    let mut reversed_graph = graph.clone();
    reversed_graph.reverse();

    for target_name in &targets_to_build {
        if !node_map.contains_key(target_name) {
            bail!("Image '{}' not found in deck.toml", target_name);
        }
        let start_node = node_map[target_name];

        // Traverse upwards (dependencies) in the original graph
        // by traversing downwards in the reversed graph.
        let mut dfs = petgraph::visit::Dfs::new(&reversed_graph, start_node);
        while let Some(nx) = dfs.next(&reversed_graph) {
            build_selection.insert(graph[nx].clone());
        }
    }


    // 4. Topologically Sort the Graph of all dependencies
    let sorted_nodes = match toposort(&graph, None) {
        Ok(nodes) => nodes,
        Err(cycle) => bail!(
            "Circular dependency detected! Cycle includes: {}",
            graph[cycle.node_id()].clone()
        ),
    };

    // Filter the sorted list to only include the nodes we need to build
    let build_plan: Vec<String> = sorted_nodes
        .into_iter()
        .map(|node_index| graph[node_index].clone())
        .filter(|name| build_selection.contains(name))
        .collect();

    if build_plan.is_empty() {
        println!("No images to build.");
        return Ok(());
    }

    // 5. Execute Build Plan
    println!("🚀 Build plan ({} targets):", build_plan.len());
    for (i, name) in build_plan.iter().enumerate() {
        println!("   {}. {}", i + 1, name);
    }
    println!();


    let new_locks = Mutex::new(BTreeMap::new());
    let build_errors = Mutex::new(Vec::new());

    let mut staged_plan: Vec<Vec<String>> = Vec::new();
    let mut built_nodes = BTreeSet::new();

    let mut plan_clone = build_plan.clone();
    while !plan_clone.is_empty() {
        let mut current_stage = Vec::new();
        let mut next_plan = Vec::new();

        for name in plan_clone {
            let profile = &config.images[&name];
            let all_deps_built = profile.depends_on.iter().all(|dep| built_nodes.contains(dep));

            if all_deps_built {
                current_stage.push(name.clone());
            } else {
                next_plan.push(name.clone());
            }
        }

        if current_stage.is_empty() && !next_plan.is_empty() {
            // This case should not be reached if toposort is correct
            return Err(anyhow!("Failed to create build stages. Check for logical error in dependency resolution."));
        }
        
        staged_plan.push(current_stage);
        for name in staged_plan.last().unwrap() {
            built_nodes.insert(name.clone());
        }
        plan_clone = next_plan;
    }


    for (i, stage) in staged_plan.iter().enumerate() {
        println!("\n-- Stage {} --", i + 1);
        
        let stage_results: Vec<Result<(String, (String, String))>> = stage
            .par_iter()
            .map(|name| {
                let profile = &config.images[name];
                let tag_to_use = tag.as_deref().unwrap_or(&profile.local_tag);
                
                println!("Building: {} ({}) with tag '{}'", name, profile.path, tag_to_use);
                engine.build(&root, name, profile, tag_to_use)
                    .map(|(digest, local_ref)| (name.clone(), (digest, local_ref)))
                    .map_err(|e| {
                        eprintln!("❌ Build failed for {}: {}", name, e);
                        e
                    })
            })
            .collect();
            
        for result in stage_results {
            match result {
                Ok((name, (digest, local_ref))) => {
                    let lock_profile = LockImageProfile { digest, local_ref };
                    new_locks.lock().unwrap().insert(name, lock_profile);
                }
                Err(e) => {
                    build_errors.lock().unwrap().push(e.to_string());
                }
            }
        }

        let errors = build_errors.lock().unwrap();
        if !errors.is_empty() {
            bail!("Build failed in Stage {}.\nErrors:\n - {}", i + 1, errors.join("\n - "));
        }
    }


    // 6. Update Lock File
    if !no_lock {
        let mut lock_data = load_lock(&root).unwrap_or_default();
        // Update existing entries and add new ones
        lock_data.images.extend(new_locks.into_inner().unwrap());
        // Prune images that are no longer in the main config
        lock_data
            .images
            .retain(|name, _| config.images.contains_key(name));
        save_lock(&root, &lock_data)?;
        println!("\n🔒 deck.lock updated.");
    } else {
        println!("\n--no-lock specified, skipping lock file update.");
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
        if let Some(lock_profile) = lock.images.get(&name) {
            engine.push(registry_url, &name, &lock_profile.digest, tag, insecure)?;
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
                let parent_dir = entry.path().parent().unwrap_or(&root);
                let context_path = parent_dir.strip_prefix(&root)?.to_string_lossy().to_string();
                let name = parent_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "root".to_string());
                
                new_images.insert(
                    name,
                    ImageProfile {
                        path: rel_path,
                        depends_on: Vec::new(),
                        local_tag: "latest".to_string(),
                        context: if context_path.is_empty() { ".".to_string() } else { context_path },
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