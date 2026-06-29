//! Information-retrieval metrics, computed to match `trec_eval` / BEIR / MTEB
//! exactly so Grafiki's numbers are comparable to the published literature.
//!
//! Conventions (see `docs/EVAL_DESIGN.md` §4 and verified against `pytrec_eval`):
//! - **Linear gain** nDCG by default (`gain = grade`, `discount = 1/log2(rank+1)`),
//!   the convention `pytrec_eval`/BEIR/MTEB report. The exponential Burges variant
//!   (`2^grade - 1`) is available via [`GainKind::Exponential`] but is never the
//!   default — switching it would make our numbers non-comparable.
//! - **Per-query then macro-average** (mean over queries), never micro-average.
//! - `Precision@k` divides by `k` (not by `min(k, retrieved)`), matching `trec_eval`'s `P.k`.
//! - `IDCG@k` is the ideal ranking's DCG@k (sort judged grades descending, take k).
//! - Empty / no-relevant edge cases resolve to `0.0` (documented per metric).
//!
//! A run is represented as an **ordered list of doc-ids** (rank 1 = first). The
//! caller is responsible for applying a deterministic tie-break when turning
//! scores into that order (Grafiki's documented order, or — for the oracle — a
//! score-descending sort over distinct scores).

use std::collections::BTreeMap;

/// Gain convention for DCG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GainKind {
    /// `gain = grade` — TREC / BEIR / MTEB / `pytrec_eval` default. **Use this.**
    Linear,
    /// `gain = 2^grade - 1` — Burges et al. Provided for cross-library comparison
    /// only; never the CI default.
    Exponential,
}

/// Graded relevance judgments for one query: `doc-id -> grade` (grade ≥ 0).
/// A grade of `0` means "judged, non-relevant"; an absent doc means "unjudged".
pub type Qrel = BTreeMap<String, i64>;
/// All judgments: `query-id -> Qrel`.
pub type Qrels = BTreeMap<String, Qrel>;
/// A ranked run for one query as ordered doc-ids (index 0 = rank 1).
pub type RunList = Vec<String>;
/// All runs: `query-id -> RunList`.
pub type Runs = BTreeMap<String, RunList>;

/// DCG discount at a 1-indexed rank: `1 / log2(rank + 1)` (rank 1 ⇒ 1.0).
#[inline]
fn log2_discount(rank_1indexed: usize) -> f64 {
    1.0 / ((rank_1indexed as f64) + 1.0).log2()
}

/// Gain of a single grade under the chosen convention. Negative grades clamp to 0.
#[inline]
fn gain(grade: i64, kind: GainKind) -> f64 {
    let g = grade.max(0);
    match kind {
        GainKind::Linear => g as f64,
        GainKind::Exponential => 2f64.powi(g as i32) - 1.0,
    }
}

#[inline]
fn grade_of(qrel: &Qrel, doc: &str) -> i64 {
    qrel.get(doc).copied().unwrap_or(0)
}

/// Number of docs judged relevant at threshold `t` (`grade ≥ t`).
pub fn relevant_count(qrel: &Qrel, t: i64) -> usize {
    qrel.values().filter(|&&g| g >= t).count()
}

/// DCG@k over the ranked `run` using `qrel` grades.
pub fn dcg_at_k(run: &[String], qrel: &Qrel, k: usize, kind: GainKind) -> f64 {
    run.iter()
        .take(k)
        .enumerate()
        .map(|(i, doc)| gain(grade_of(qrel, doc), kind) * log2_discount(i + 1))
        .sum()
}

/// IDCG@k: DCG@k of the ideal ranking (all judged docs sorted by grade descending).
pub fn idcg_at_k(qrel: &Qrel, k: usize, kind: GainKind) -> f64 {
    let mut grades: Vec<i64> = qrel.values().copied().filter(|&g| g > 0).collect();
    grades.sort_unstable_by(|a, b| b.cmp(a));
    grades
        .iter()
        .take(k)
        .enumerate()
        .map(|(i, &g)| gain(g, kind) * log2_discount(i + 1))
        .sum()
}

/// nDCG@k = DCG@k / IDCG@k, in `[0, 1]`. Returns `0.0` when there are no
/// relevant docs (IDCG = 0), matching the trec_eval/BEIR edge convention.
pub fn ndcg_at_k(run: &[String], qrel: &Qrel, k: usize, kind: GainKind) -> f64 {
    let idcg = idcg_at_k(qrel, k, kind);
    if idcg == 0.0 {
        0.0
    } else {
        dcg_at_k(run, qrel, k, kind) / idcg
    }
}

/// Recall@k = (relevant in top k) / (total relevant). `0.0` if no relevant docs.
pub fn recall_at_k(run: &[String], qrel: &Qrel, k: usize, t: i64) -> f64 {
    let total = relevant_count(qrel, t);
    if total == 0 {
        return 0.0;
    }
    let hits = run
        .iter()
        .take(k)
        .filter(|d| grade_of(qrel, d) >= t)
        .count();
    hits as f64 / total as f64
}

/// Precision@k = (relevant in top k) / k (denominator is `k`, the trec_eval `P.k`
/// convention — not `min(k, retrieved)`).
pub fn precision_at_k(run: &[String], qrel: &Qrel, k: usize, t: i64) -> f64 {
    if k == 0 {
        return 0.0;
    }
    let hits = run
        .iter()
        .take(k)
        .filter(|d| grade_of(qrel, d) >= t)
        .count();
    hits as f64 / k as f64
}

/// Reciprocal rank of the first relevant doc within `cutoff` (`None` = unbounded,
/// matching trec_eval `recip_rank`). `0.0` if no relevant doc is found.
pub fn reciprocal_rank(run: &[String], qrel: &Qrel, t: i64, cutoff: Option<usize>) -> f64 {
    let limit = cutoff.unwrap_or(run.len());
    for (i, d) in run.iter().take(limit).enumerate() {
        if grade_of(qrel, d) >= t {
            return 1.0 / (i as f64 + 1.0);
        }
    }
    0.0
}

/// Average precision over the full run (trec_eval `map` is uncut). `0.0` if no
/// relevant docs.
pub fn average_precision(run: &[String], qrel: &Qrel, t: i64) -> f64 {
    let total = relevant_count(qrel, t);
    if total == 0 {
        return 0.0;
    }
    let mut hits = 0usize;
    let mut sum = 0.0;
    for (i, d) in run.iter().enumerate() {
        if grade_of(qrel, d) >= t {
            hits += 1;
            sum += hits as f64 / (i as f64 + 1.0);
        }
    }
    sum / total as f64
}

/// Success@k / Hit@k: `1.0` if ≥1 relevant doc appears in the top k, else `0.0`.
pub fn success_at_k(run: &[String], qrel: &Qrel, k: usize, t: i64) -> f64 {
    let hit = run.iter().take(k).any(|d| grade_of(qrel, d) >= t);
    if hit {
        1.0
    } else {
        0.0
    }
}

/// Judged@k (diagnostic, not a quality metric): fraction of the top-k that carry
/// a judgment (relevant or not). A low value means the gold pool is too sparse,
/// so nDCG/Recall are unreliable. Denominator is `min(k, retrieved)`.
pub fn judged_at_k(run: &[String], qrel: &Qrel, k: usize) -> f64 {
    let n = run.len().min(k);
    if n == 0 {
        return 0.0;
    }
    let judged = run.iter().take(k).filter(|d| qrel.contains_key(*d)).count();
    judged as f64 / n as f64
}

/// Which metrics to compute and at which cutoffs.
#[derive(Debug, Clone)]
pub struct MetricConfig {
    pub ndcg_cutoffs: Vec<usize>,
    pub recall_cutoffs: Vec<usize>,
    pub precision_cutoffs: Vec<usize>,
    pub success_cutoffs: Vec<usize>,
    pub judged_cutoff: usize,
    /// MRR cutoff; `None` = unbounded (trec_eval `recip_rank`).
    pub mrr_cutoff: Option<usize>,
    pub rel_threshold: i64,
    pub gain: GainKind,
}

impl Default for MetricConfig {
    fn default() -> Self {
        Self {
            ndcg_cutoffs: vec![1, 3, 5, 10],
            recall_cutoffs: vec![5, 10, 20],
            precision_cutoffs: vec![5, 10],
            success_cutoffs: vec![1, 5],
            judged_cutoff: 10,
            // Unbounded, matching trec_eval / pytrec_eval `recip_rank` (the oracle
            // maps "mrr" → recip_rank). A cut variant would diverge once the first
            // relevant doc falls past the cutoff.
            mrr_cutoff: None,
            rel_threshold: 1,
            gain: GainKind::Linear,
        }
    }
}

/// Per-query metric values, keyed by canonical metric name (e.g. `"ndcg@10"`).
#[derive(Debug, Clone)]
pub struct QueryScores {
    pub qid: String,
    pub metrics: BTreeMap<String, f64>,
}

/// Aggregate over a query set: every per-query vector plus the macro-average.
#[derive(Debug, Clone)]
pub struct AggregateScores {
    pub per_query: Vec<QueryScores>,
    pub macro_avg: BTreeMap<String, f64>,
}

impl AggregateScores {
    /// Per-query values for one metric, in `per_query` order (for paired tests).
    pub fn per_query_vector(&self, metric: &str) -> Vec<f64> {
        self.per_query
            .iter()
            .map(|q| q.metrics.get(metric).copied().unwrap_or(0.0))
            .collect()
    }
}

/// Compute the full metric set for one query.
fn score_query(qid: &str, run: &[String], qrel: &Qrel, cfg: &MetricConfig) -> QueryScores {
    let mut m = BTreeMap::new();
    let t = cfg.rel_threshold;
    for &k in &cfg.ndcg_cutoffs {
        m.insert(format!("ndcg@{k}"), ndcg_at_k(run, qrel, k, cfg.gain));
    }
    for &k in &cfg.recall_cutoffs {
        m.insert(format!("recall@{k}"), recall_at_k(run, qrel, k, t));
    }
    for &k in &cfg.precision_cutoffs {
        m.insert(format!("P@{k}"), precision_at_k(run, qrel, k, t));
    }
    for &k in &cfg.success_cutoffs {
        m.insert(format!("success@{k}"), success_at_k(run, qrel, k, t));
    }
    m.insert(
        "mrr".to_string(),
        reciprocal_rank(run, qrel, t, cfg.mrr_cutoff),
    );
    m.insert("map".to_string(), average_precision(run, qrel, t));
    m.insert(
        format!("judged@{}", cfg.judged_cutoff),
        judged_at_k(run, qrel, cfg.judged_cutoff),
    );
    QueryScores {
        qid: qid.to_string(),
        metrics: m,
    }
}

/// Evaluate `runs` against `qrels`. The query set is the keys of `qrels`
/// (trec_eval convention); a query with no run entry scores as an empty list
/// (all zeros). Macro-averages over the query set.
pub fn evaluate(qrels: &Qrels, runs: &Runs, cfg: &MetricConfig) -> AggregateScores {
    let empty: RunList = Vec::new();
    let mut per_query = Vec::with_capacity(qrels.len());
    for (qid, qrel) in qrels {
        let run = runs.get(qid).unwrap_or(&empty);
        per_query.push(score_query(qid, run, qrel, cfg));
    }
    let mut macro_avg: BTreeMap<String, f64> = BTreeMap::new();
    if !per_query.is_empty() {
        // Union of metric keys (all queries share the same set, but be safe).
        let mut keys: Vec<String> = per_query[0].metrics.keys().cloned().collect();
        keys.sort();
        for key in keys {
            let sum: f64 = per_query
                .iter()
                .map(|q| q.metrics.get(&key).copied().unwrap_or(0.0))
                .sum();
            macro_avg.insert(key, sum / per_query.len() as f64);
        }
    }
    AggregateScores {
        per_query,
        macro_avg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qrel(pairs: &[(&str, i64)]) -> Qrel {
        pairs.iter().map(|(d, g)| (d.to_string(), *g)).collect()
    }
    fn run(docs: &[&str]) -> RunList {
        docs.iter().map(|d| d.to_string()).collect()
    }

    // Hand-computed sanity example (independent of pytrec_eval).
    // Ranking d1(2), d2(0), d3(1); DCG@3 = 2/1 + 0 + 1/log2(4)=2.5.
    // Ideal grades [2,1]; IDCG@3 = 2/1 + 1/log2(3) = 2 + 0.63093 = 2.63093.
    // nDCG@3 = 2.5 / 2.63093 = 0.95025…
    #[test]
    fn hand_computed_ndcg3() {
        let q = qrel(&[("d1", 2), ("d2", 0), ("d3", 1)]);
        let r = run(&["d1", "d2", "d3"]);
        let got = ndcg_at_k(&r, &q, 3, GainKind::Linear);
        assert!((got - 0.9502344167898356).abs() < 1e-12, "got {got}");
    }

    #[test]
    fn no_relevant_is_zero() {
        let q = qrel(&[("d1", 0), ("d2", 0)]);
        let r = run(&["d1", "d2"]);
        assert_eq!(ndcg_at_k(&r, &q, 10, GainKind::Linear), 0.0);
        assert_eq!(recall_at_k(&r, &q, 10, 1), 0.0);
        assert_eq!(reciprocal_rank(&r, &q, 1, None), 0.0);
        assert_eq!(average_precision(&r, &q, 1), 0.0);
    }

    #[test]
    fn precision_denominator_is_k() {
        // 4 docs, 3 relevant, all retrieved; P@5 = 3/5 = 0.6 (k denominator).
        let q = qrel(&[("a", 1), ("b", 1), ("c", 0), ("d", 2)]);
        let r = run(&["b", "d", "a", "c"]);
        assert!((precision_at_k(&r, &q, 5, 1) - 0.6).abs() < 1e-12);
    }
}
