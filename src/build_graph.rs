use crate::config::{DullConfig, ImageProfile};
use anyhow::{Context, Result};
use petgraph::algo::toposort;
use petgraph::graph::DiGraph;
use std::collections::{BTreeSet, HashMap};

pub struct BuildTask {
    pub name: String,
    pub profile: ImageProfile,
    pub dependencies: Vec<String>,
}

pub struct BuildPlan {
    pub tasks: Vec<BuildTask>,
}

// Compute build plan with all necessary metadata for execution
pub fn compute_plan(config: &DullConfig, targets: &BTreeSet<String>) -> Result<BuildPlan> {
    let mut graph = DiGraph::<String, ()>::new();
    let mut node_map = HashMap::new();

    for name in config.images.keys() {
        node_map.insert(name.clone(), graph.add_node(name.clone()));
    }

    for (name, profile) in &config.images {
        let to_idx = node_map[name];
        for dep in &profile.depends_on {
            let from_idx = node_map
                .get(dep)
                .cloned()
                .with_context(|| format!("Image '{}' depends on undefined '{}'", name, dep))?;
            graph.add_edge(from_idx, to_idx, ());
        }
    }

    // Identify all required nodes for the given targets
    let mut build_selection = BTreeSet::new();
    let mut reversed = graph.clone();
    reversed.reverse();

    for target in targets {
        let start = *node_map
            .get(target)
            .with_context(|| format!("Target '{}' not found", target))?;
        let mut dfs = petgraph::visit::Dfs::new(&reversed, start);
        while let Some(nx) = dfs.next(&reversed) {
            build_selection.insert(graph[nx].clone());
        }
    }

    // Get a topologically sorted order of ALL images first
    let sorted_indices = toposort(&graph, None).map_err(|cycle| {
        anyhow::anyhow!(
            "Circular dependency detected involving: {}",
            graph[cycle.node_id()]
        )
    })?;

    // Create tasks in topological order
    let tasks = sorted_indices
        .into_iter()
        .map(|i| graph[i].clone())
        .filter(|name| build_selection.contains(name))
        .map(|name| {
            let profile = config.images[&name].clone();
            // Filter dependencies to only include those that are also in the plan
            let dependencies = profile
                .depends_on
                .iter()
                .filter(|d| build_selection.contains(*d))
                .cloned()
                .collect();

            BuildTask {
                name,
                profile,
                dependencies,
            }
        })
        .collect();

    Ok(BuildPlan { tasks })
}
