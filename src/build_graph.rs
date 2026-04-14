use crate::config::DeckConfig;
use anyhow::{Context, Result};
use petgraph::algo::toposort;
use petgraph::graph::DiGraph;
use std::collections::{BTreeSet, HashMap};

pub struct BuildPlan {
    pub stages: Vec<Vec<String>>,
}

pub struct BuildGraph<'a> {
    config: &'a DeckConfig,
}

impl<'a> BuildGraph<'a> {
    pub fn new(config: &'a DeckConfig) -> Self {
        Self { config }
    }

    pub fn create_plan(&self, targets: &BTreeSet<String>) -> Result<BuildPlan> {
        let mut graph = DiGraph::<String, ()>::new();
        let mut node_map = HashMap::new();

        for name in self.config.images.keys() {
            node_map.insert(name.clone(), graph.add_node(name.clone()));
        }

        for (name, profile) in &self.config.images {
            let to_idx = node_map[name];
            for dep in &profile.depends_on {
                let from_idx = node_map
                    .get(dep)
                    .cloned()
                    .with_context(|| format!("Image '{}' depends on undefined '{}'", name, dep))?;
                graph.add_edge(from_idx, to_idx, ());
            }
        }

        let sorted_indices = toposort(&graph, None)
            .map_err(|cycle| anyhow::anyhow!("Circular dependency: {}", graph[cycle.node_id()]))?;

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

        let build_order: Vec<String> = sorted_indices
            .into_iter()
            .map(|i| graph[i].clone())
            .filter(|name| build_selection.contains(name))
            .collect();

        Ok(BuildPlan {
            stages: self.compute_stages(build_order),
        })
    }

    fn compute_stages(&self, plan: Vec<String>) -> Vec<Vec<String>> {
        let mut stages = Vec::new();
        let mut built = BTreeSet::new();
        let mut remaining = plan;

        while !remaining.is_empty() {
            let (current, next): (Vec<_>, Vec<_>) = remaining.into_iter().partition(|name| {
                let profile = &self.config.images[name];
                profile.depends_on.iter().all(|d| built.contains(d))
            });

            for name in &current {
                built.insert(name.clone());
            }
            stages.push(current);
            remaining = next;
        }
        stages
    }
}
