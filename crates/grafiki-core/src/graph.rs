//! H3 — Personalized PageRank over the in-memory relations graph.
//!
//! Graph-aware retrieval seeds from the entities surfaced by lexical/dense search,
//! spreads importance over the `relations` graph with Personalized PageRank, and
//! fuses the resulting node ranking into the existing RRF. This module is the pure,
//! deterministic algorithm core (no I/O); the search layer builds the graph from
//! SQLite and maps ranked entities back to retrievable records.
//!
//! PPR (HippoRAG, arXiv:2405.14831): `score(v) = (1−d)·p(v) + d·Σ_{u→v} score(u)·w(u,v)/W_out(u)`,
//! where `p` is the seed personalization vector and `d` is the damping (edge-follow)
//! probability. Computed by power iteration to a fixed L1 tolerance — deterministic
//! given the inputs. Edges are added in both directions so importance spreads
//! across the knowledge graph regardless of the stored edge orientation.

use std::collections::{BTreeMap, BTreeSet};

/// Default damping (edge-follow probability); `1 − d` restarts to the seeds.
/// ~0.5 matches HippoRAG's retrieval setting (more local than web-PageRank's 0.85).
pub const DEFAULT_DAMPING: f64 = 0.5;
pub const DEFAULT_MAX_ITERS: usize = 50;
pub const DEFAULT_TOLERANCE: f64 = 1e-6;

/// A sparse weighted graph keyed by node id (entity id). Edges are stored
/// symmetrically (added both ways on insert) for retrieval-style traversal.
#[derive(Debug, Default)]
pub struct Graph {
    out: BTreeMap<String, Vec<(String, f64)>>,
    nodes: BTreeSet<String>,
}

impl Graph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an undirected weighted edge (recorded in both directions). Non-positive
    /// or non-finite weights are clamped to a small positive value so every stored
    /// relation still conducts importance.
    pub fn add_edge(&mut self, from: &str, to: &str, weight: f64) {
        if from == to {
            self.nodes.insert(from.to_string());
            return;
        }
        let w = if weight.is_finite() && weight > 0.0 {
            weight
        } else {
            1.0
        };
        self.out
            .entry(from.to_string())
            .or_default()
            .push((to.to_string(), w));
        self.out
            .entry(to.to_string())
            .or_default()
            .push((from.to_string(), w));
        self.nodes.insert(from.to_string());
        self.nodes.insert(to.to_string());
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn contains(&self, node: &str) -> bool {
        self.nodes.contains(node)
    }
}

/// Personalized PageRank by power iteration.
///
/// `seeds` maps node id → non-negative weight (it is L1-normalized internally to
/// form the personalization/teleport vector; seeds absent from the graph are
/// dropped). Returns the stationary score per node (summing to ~1). Empty graph or
/// no in-graph seeds ⇒ empty map.
pub fn personalized_pagerank(
    graph: &Graph,
    seeds: &BTreeMap<String, f64>,
    damping: f64,
    max_iters: usize,
    tolerance: f64,
) -> BTreeMap<String, f64> {
    if graph.is_empty() {
        return BTreeMap::new();
    }

    // Personalization vector: normalized seed weights restricted to graph nodes.
    let mut personalization: BTreeMap<&str, f64> = BTreeMap::new();
    let mut seed_total = 0.0;
    for (node, &weight) in seeds {
        if weight > 0.0 && graph.nodes.contains(node) {
            *personalization.entry(node.as_str()).or_insert(0.0) += weight;
            seed_total += weight;
        }
    }
    if seed_total == 0.0 {
        return BTreeMap::new();
    }
    for value in personalization.values_mut() {
        *value /= seed_total;
    }

    let p = |node: &str| personalization.get(node).copied().unwrap_or(0.0);

    // Initialize scores at the personalization vector.
    let mut score: BTreeMap<&str, f64> = graph.nodes.iter().map(|n| (n.as_str(), p(n))).collect();

    for _ in 0..max_iters {
        let mut next: BTreeMap<&str, f64> = graph
            .nodes
            .iter()
            .map(|n| (n.as_str(), (1.0 - damping) * p(n)))
            .collect();

        let mut dangling_mass = 0.0;
        for node in &graph.nodes {
            let s = score[node.as_str()];
            if s == 0.0 {
                continue;
            }
            match graph.out.get(node) {
                Some(edges) if !edges.is_empty() => {
                    let total_w: f64 = edges.iter().map(|(_, w)| *w).sum();
                    for (neighbor, w) in edges {
                        *next.get_mut(neighbor.as_str()).unwrap() += damping * s * (w / total_w);
                    }
                }
                // Dangling node (no out-edges): its mass restarts to the seeds.
                _ => dangling_mass += s,
            }
        }
        if dangling_mass > 0.0 {
            for (node, value) in next.iter_mut() {
                *value += damping * dangling_mass * p(node);
            }
        }

        // L1 convergence check.
        let delta: f64 = graph
            .nodes
            .iter()
            .map(|n| (next[n.as_str()] - score[n.as_str()]).abs())
            .sum();
        score = next;
        if delta < tolerance {
            break;
        }
    }

    score.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeds(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
        pairs.iter().map(|(n, w)| (n.to_string(), *w)).collect()
    }

    #[test]
    fn empty_graph_or_no_seeds_is_empty() {
        let g = Graph::new();
        assert!(personalized_pagerank(&g, &seeds(&[("a", 1.0)]), 0.5, 50, 1e-6).is_empty());
        let mut g2 = Graph::new();
        g2.add_edge("a", "b", 1.0);
        // seed not in graph
        assert!(personalized_pagerank(&g2, &seeds(&[("z", 1.0)]), 0.5, 50, 1e-6).is_empty());
    }

    #[test]
    fn scores_sum_to_one_and_seed_neighbor_ranks_high() {
        // a - b - c chain; seed at a. PPR mass should be highest at a, then b, then c.
        let mut g = Graph::new();
        g.add_edge("a", "b", 1.0);
        g.add_edge("b", "c", 1.0);
        let pr = personalized_pagerank(&g, &seeds(&[("a", 1.0)]), 0.5, 100, 1e-9);
        let sum: f64 = pr.values().sum();
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "PPR mass must sum to 1, got {sum}"
        );
        assert!(pr["a"] > pr["b"], "seed should outrank its neighbor");
        assert!(pr["b"] > pr["c"], "1-hop should outrank 2-hop");
    }

    #[test]
    fn unseeded_neighbor_beats_unconnected_node() {
        // seed at `auth`; `db` is a neighbor, `unrelated` is disconnected.
        let mut g = Graph::new();
        g.add_edge("auth", "db", 1.0);
        g.add_edge("ui", "unrelated", 1.0); // separate component
        let pr = personalized_pagerank(&g, &seeds(&[("auth", 1.0)]), 0.5, 100, 1e-9);
        assert!(
            pr["db"] > pr.get("unrelated").copied().unwrap_or(0.0),
            "a connected neighbor must outrank a disconnected node"
        );
        assert_eq!(
            pr.get("unrelated").copied().unwrap_or(0.0),
            0.0,
            "a node in a seed-less component gets no mass"
        );
    }

    #[test]
    fn is_deterministic() {
        let mut g = Graph::new();
        g.add_edge("a", "b", 2.0);
        g.add_edge("b", "c", 1.0);
        g.add_edge("a", "c", 0.5);
        let s = seeds(&[("a", 1.0), ("b", 0.5)]);
        let first = personalized_pagerank(&g, &s, 0.5, 100, 1e-9);
        let second = personalized_pagerank(&g, &s, 0.5, 100, 1e-9);
        assert_eq!(first, second);
    }
}
