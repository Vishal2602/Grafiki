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

    /// Node ids in lexicographic order (backed by the `BTreeSet`).
    pub fn nodes(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter().map(String::as_str)
    }

    /// `(neighbor, weight)` pairs for `node`, in insertion order; empty if absent.
    /// The same neighbor can appear more than once when several relations connect
    /// the pair — callers that need a per-neighbor weight sum must fold (see
    /// [`detect_communities`], which canonicalizes adjacency before use).
    pub fn neighbors(&self, node: &str) -> &[(String, f64)] {
        self.out.get(node).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Σ of incident edge weights (weighted degree), folding multi-edges.
    pub fn weighted_degree(&self, node: &str) -> f64 {
        self.out
            .get(node)
            .map(|edges| edges.iter().map(|(_, w)| *w).sum())
            .unwrap_or(0.0)
    }

    /// `2·m` — total weighted degree over all nodes (each undirected edge counted
    /// twice, matching the symmetric storage). Modularity's normalizer.
    pub fn total_degree(&self) -> f64 {
        self.nodes.iter().map(|n| self.weighted_degree(n)).sum()
    }
}

/// The modularity contribution `Q_c` of one community (its `members`) within `graph`:
/// `Q_c = Σ_in/2m − (Σ_tot/2m)²`, where `Σ_in` is the within-community weighted-degree
/// sum (each internal edge counted from both endpoints, matching the symmetric
/// storage), `Σ_tot` is the summed weighted degree of the members, and `2m =
/// total_degree()`. Higher ⇒ a tighter, more separable community (a community equal to
/// the whole graph scores 0). Pure and deterministic. Empty graph/community ⇒ 0.0.
pub fn community_modularity(graph: &Graph, members: &[String]) -> f64 {
    let m2 = graph.total_degree();
    if m2 <= 0.0 || members.is_empty() {
        return 0.0;
    }
    let member_set: BTreeSet<&str> = members.iter().map(String::as_str).collect();
    let mut sigma_in = 0.0;
    let mut sigma_tot = 0.0;
    for member in members {
        sigma_tot += graph.weighted_degree(member);
        for (neighbor, weight) in graph.neighbors(member) {
            if member_set.contains(neighbor.as_str()) {
                sigma_in += *weight; // counted from both endpoints ⇒ internal edge ×2
            }
        }
    }
    sigma_in / m2 - (sigma_tot / m2).powi(2)
}

/// A detected community: member node (entity) ids in lexicographic order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Community {
    /// Dense, deterministic id in `0..k` (renumbered by lowest member id).
    pub id: usize,
    /// Member entity ids, lexicographically sorted.
    pub members: Vec<String>,
}

/// Move guard: a relabel is taken only if it improves modularity past this float
/// floor, so last-bit noise can never trigger a spurious (determinism-breaking) move.
const MOVE_EPSILON: f64 = 1e-12;
/// Hard cap on local-moving sweeps; knowledge graphs converge in well under 20, so
/// this only bounds pathological inputs while keeping the iteration count a pure
/// function of the graph (identical input ⇒ identical sweep count).
const MAX_SWEEPS: usize = 100;

/// Deterministic single-level Louvain (greedy modularity maximization) over the
/// undirected weighted graph. Returns ALL communities (including singletons),
/// `id`-ascending, each `members` lexicographically sorted. Empty graph ⇒ empty Vec.
///
/// Pure: no I/O, no randomness, no clock. Determinism is guaranteed by canonical
/// ordering at every choice point:
/// - nodes are visited in lexicographic (`BTreeSet`) order;
/// - adjacency is folded into a `BTreeMap<&str, f64>` per node (multi-edges summed),
///   so the ΔQ float sum is over neighbors in a fixed lexicographic order and does
///   **not** depend on edge-insertion order (the SQL load is also `ORDER BY`-ed);
/// - on a ΔQ tie the lowest community id wins, and a move requires ΔQ > `MOVE_EPSILON`;
/// - surviving communities are renumbered `0..k` by their lowest member id.
///
/// The modularity gain of moving isolated node `i` into community `C` is the standard
/// `ΔQ(i→C) = k_{i,C}/m − a_C·k_i/(2·m²)`, where `m = total_degree()/2`, `k_i` is `i`'s
/// weighted degree, `k_{i,C}` is the edge weight from `i` into `C`, and `a_C` is the
/// summed weighted degree of `C`'s members.
pub fn detect_communities(graph: &Graph) -> Vec<Community> {
    let nodes: Vec<&str> = graph.nodes().collect(); // lexicographic
    if nodes.is_empty() {
        return Vec::new();
    }

    // Canonical adjacency: per node, neighbor -> summed weight, lexicographic order.
    // This folds multi-edges and makes every ΔQ sum insertion-order-independent.
    let adjacency: BTreeMap<&str, BTreeMap<&str, f64>> = nodes
        .iter()
        .map(|&node| {
            let mut folded: BTreeMap<&str, f64> = BTreeMap::new();
            for (neighbor, weight) in graph.neighbors(node) {
                *folded.entry(neighbor.as_str()).or_insert(0.0) += *weight;
            }
            (node, folded)
        })
        .collect();

    let degree: BTreeMap<&str, f64> = nodes
        .iter()
        .map(|&n| (n, adjacency[n].values().sum::<f64>()))
        .collect();
    let m2: f64 = degree.values().sum();

    // No edges: every node is its own community (renumbered below).
    let mut community: BTreeMap<&str, usize> =
        nodes.iter().enumerate().map(|(i, &n)| (n, i)).collect();
    if m2 > 0.0 {
        // a_c = Σ weighted degree of nodes currently in community c.
        let mut a_tot: BTreeMap<usize, f64> = nodes
            .iter()
            .enumerate()
            .map(|(i, &n)| (i, degree[n]))
            .collect();

        for _ in 0..MAX_SWEEPS {
            let mut moved = false;
            for &node in &nodes {
                let current = community[node];
                let k_i = degree[node];

                // Edge weight from `node` into each adjacent community (canonical order).
                let mut k_into: BTreeMap<usize, f64> = BTreeMap::new();
                for (neighbor, weight) in &adjacency[node] {
                    *k_into.entry(community[*neighbor]).or_insert(0.0) += *weight;
                }

                // Remove `node` from its community before scoring candidates.
                *a_tot.get_mut(&current).unwrap() -= k_i;

                // Gain of (re)joining community `c`: k_{i,c}/m − a_c·k_i/(2m²).
                // m = m2/2, so k_{i,c}/m = 2·k_{i,c}/m2 and a_c·k_i/(2m²) = a_c·k_i/(m2²/2).
                let gain = |c: usize| -> f64 {
                    let k_ic = k_into.get(&c).copied().unwrap_or(0.0);
                    let a_c = a_tot.get(&c).copied().unwrap_or(0.0);
                    2.0 * k_ic / m2 - 2.0 * a_c * k_i / (m2 * m2)
                };

                // Candidate communities: current ∪ neighbor communities. Pick the
                // best gain; tie → lowest community id (k_into is a BTreeMap, so the
                // scan is in ascending-id order and the first max wins the tie).
                let stay_gain = gain(current);
                let mut best_community = current;
                let mut best_gain = stay_gain;
                for &c in k_into.keys() {
                    let g = gain(c);
                    if g > best_gain + MOVE_EPSILON {
                        best_gain = g;
                        best_community = c;
                    }
                }

                // Commit (best_community == current is a no-op re-add).
                *a_tot.get_mut(&best_community).unwrap() += k_i;
                if best_community != current {
                    community.insert(node, best_community);
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
    }

    // Group members by community id, then renumber densely by lowest member id.
    let mut members_by_community: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for &node in &nodes {
        members_by_community
            .entry(community[node])
            .or_default()
            .push(node.to_string());
    }
    // Order communities by their lexicographically-lowest member (members are already
    // pushed in lexicographic node order, so `[0]` is the lowest).
    let mut groups: Vec<Vec<String>> = members_by_community.into_values().collect();
    groups.sort_by(|a, b| a[0].cmp(&b[0]));
    groups
        .into_iter()
        .enumerate()
        .map(|(id, members)| Community { id, members })
        .collect()
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

    // --- detect_communities ------------------------------------------------

    /// Which community a node landed in, for assertions independent of dense ids.
    fn community_of<'a>(communities: &'a [Community], node: &str) -> Option<&'a Community> {
        communities
            .iter()
            .find(|c| c.members.iter().any(|m| m == node))
    }

    fn two_cliques_bridge() -> Vec<(&'static str, &'static str, f64)> {
        // Two triangles {a1,a2,a3} and {b1,b2,b3}, joined by ONE weak bridge a1-b1.
        vec![
            ("a1", "a2", 1.0),
            ("a1", "a3", 1.0),
            ("a2", "a3", 1.0),
            ("b1", "b2", 1.0),
            ("b1", "b3", 1.0),
            ("b2", "b3", 1.0),
            ("a1", "b1", 0.2), // weak bridge — modularity should cut it
        ]
    }

    #[test]
    fn empty_graph_has_no_communities() {
        assert!(detect_communities(&Graph::new()).is_empty());
    }

    #[test]
    fn isolated_nodes_are_singletons() {
        let mut g = Graph::new();
        g.add_edge("x", "x", 1.0); // self-edge just registers the node
        g.add_edge("y", "y", 1.0);
        let comms = detect_communities(&g);
        assert_eq!(comms.len(), 2, "two unconnected nodes ⇒ two singletons");
        assert!(comms.iter().all(|c| c.members.len() == 1));
    }

    #[test]
    fn detect_communities_splits_two_cliques() {
        let mut g = Graph::new();
        for (u, v, w) in two_cliques_bridge() {
            g.add_edge(u, v, w);
        }
        let comms = detect_communities(&g);
        assert_eq!(
            comms.len(),
            2,
            "weak bridge must not merge the two cliques: {comms:?}"
        );
        // Each clique's members co-located.
        for clique in [["a1", "a2", "a3"], ["b1", "b2", "b3"]] {
            let id = community_of(&comms, clique[0]).unwrap().id;
            for member in clique {
                assert_eq!(
                    community_of(&comms, member).unwrap().id,
                    id,
                    "{member} should be with {}",
                    clique[0]
                );
            }
        }
        // Ids are dense and members sorted.
        assert_eq!(comms[0].id, 0);
        assert_eq!(comms[1].id, 1);
        assert!(comms
            .iter()
            .all(|c| c.members.windows(2).all(|w| w[0] <= w[1])));
    }

    #[test]
    fn detect_communities_is_deterministic_under_shuffled_edges() {
        // Build the SAME graph from two different edge-insertion orders (which is
        // what the un-ORDER-BY-ed SQL row order could produce) and assert the
        // partition is byte-identical — the C2 determinism guarantee.
        let edges = two_cliques_bridge();
        let mut forward = Graph::new();
        for (u, v, w) in &edges {
            forward.add_edge(u, v, *w);
        }
        let mut shuffled = Graph::new();
        // A deterministic non-trivial permutation (reverse + endpoint swap).
        for (u, v, w) in edges.iter().rev() {
            shuffled.add_edge(v, u, *w);
        }
        assert_eq!(
            detect_communities(&forward),
            detect_communities(&shuffled),
            "community detection must be insertion-order independent"
        );
    }

    #[test]
    fn hub_does_not_absorb_two_cliques() {
        // Two cliques each attached to a shared hub by a single edge. The hub must
        // join ONE clique, not collapse both into a single giant community (C6).
        let mut g = Graph::new();
        for (u, v, w) in [
            ("a1", "a2", 1.0),
            ("a1", "a3", 1.0),
            ("a2", "a3", 1.0),
            ("b1", "b2", 1.0),
            ("b1", "b3", 1.0),
            ("b2", "b3", 1.0),
            ("hub", "a1", 1.0),
            ("hub", "b1", 1.0),
        ] {
            g.add_edge(u, v, w);
        }
        let comms = detect_communities(&g);
        assert!(
            comms.len() >= 2,
            "hub must not absorb both cliques into one community: {comms:?}"
        );
        // The two clique cores stay distinct.
        assert_ne!(
            community_of(&comms, "a2").unwrap().id,
            community_of(&comms, "b2").unwrap().id,
            "the two clique interiors must be different communities"
        );
    }

    #[test]
    fn community_modularity_rewards_tight_communities() {
        // Two disjoint triangles. A single triangle is a real cluster (positive Q_c);
        // the whole graph taken as one community scores ~0.
        let mut g = Graph::new();
        for (u, v) in [
            ("a1", "a2"),
            ("a1", "a3"),
            ("a2", "a3"),
            ("b1", "b2"),
            ("b1", "b3"),
            ("b2", "b3"),
        ] {
            g.add_edge(u, v, 1.0);
        }
        let triangle = vec!["a1".to_string(), "a2".to_string(), "a3".to_string()];
        let q_triangle = community_modularity(&g, &triangle);
        assert!(
            q_triangle > 0.0,
            "a real cluster has positive modularity: {q_triangle}"
        );

        let everything: Vec<String> = g.nodes().map(str::to_string).collect();
        let q_all = community_modularity(&g, &everything);
        assert!(
            q_all.abs() < 1e-9,
            "the whole graph as one community has Q≈0: {q_all}"
        );

        // Deterministic + empty cases.
        assert_eq!(q_triangle, community_modularity(&g, &triangle));
        assert_eq!(community_modularity(&Graph::new(), &triangle), 0.0);
        assert_eq!(community_modularity(&g, &[]), 0.0);
    }
}
