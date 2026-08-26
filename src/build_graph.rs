use crate::config::{DullConfig, ImageProfile};
use anyhow::{Context, Result};
use petgraph::Direction;
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

pub fn compute_plan(config: &DullConfig, targets: &BTreeSet<String>) -> Result<BuildPlan> {
    let mut graph = DiGraph::<String, ()>::new();
    let mut node_map = HashMap::new();

    for name in config.images.keys() {
        node_map.insert(name.clone(), graph.add_node(name.clone()));
    }

    for group in config.build_groups.values() {
        let Some(base) = &group.base else { continue };
        let from_idx = *node_map
            .get(base)
            .with_context(|| format!("Build group base '{}' is not a defined image", base))?;
        for image_name in &group.images {
            let to_idx = *node_map.get(image_name).with_context(|| {
                format!("Build group references undefined image '{}'", image_name)
            })?;
            graph.add_edge(from_idx, to_idx, ());
        }
    }

    // DFS on the reversed graph walks each target's dependencies.
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

    let sorted_indices = toposort(&graph, None).map_err(|cycle| {
        anyhow::anyhow!(
            "Circular dependency detected involving: {}",
            graph[cycle.node_id()]
        )
    })?;

    let tasks = sorted_indices
        .into_iter()
        .filter(|&i| build_selection.contains(&graph[i]))
        .map(|i| {
            let name = graph[i].clone();
            let profile = config.images[&name].clone();
            // Predecessors = dependencies; keep only ones also in this plan.
            let dependencies = graph
                .neighbors_directed(i, Direction::Incoming)
                .map(|dep_idx| graph[dep_idx].clone())
                .filter(|d| build_selection.contains(d))
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
