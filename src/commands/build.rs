use crate::config::{load_context, load_lock, save_lock, LockImageProfile};
use crate::engine::ContainerEngine;
use anyhow::{anyhow, bail, Result};
use petgraph::{algo::toposort, graph::DiGraph};
use rayon::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Mutex,
};

pub fn run_build(
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
            return Err(anyhow!("Failed to create build stages. Check for logical error in dependency resolution."));
        }
        
        staged_plan.push(current_stage);
        for name in staged_plan.last().unwrap() {
            built_nodes.insert(name.clone());
        }
        plan_clone = next_plan;
    }

    for (i, stage) in staged_plan.iter().enumerate() {
        println!("
-- Stage {} --", i + 1);
        
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
            bail!("Build failed in Stage {}.
Errors:
 - {}", i + 1, errors.join("
 - "));
        }
    }

    // 6. Update Lock File
    if !no_lock {
        let mut lock_data = load_lock(&root).unwrap_or_default();
        lock_data.images.extend(new_locks.into_inner().unwrap());
        lock_data
            .images
            .retain(|name, _| config.images.contains_key(name));
        save_lock(&root, &lock_data)?;
        println!("
🔒 deck.lock updated.");
    } else {
        println!("
--no-lock specified, skipping lock file update.");
    }

    Ok(())
}
