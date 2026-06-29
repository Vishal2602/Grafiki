//! Arm A — retrieval quality (keyword vs semantic vs hybrid) over a frozen store.
//!
//! Seeds the corpus once, runs every requested [`SearchMode`] against the
//! byte-identical store, scores each with the IR metrics, breaks results down
//! per record-type, and (when Hybrid is present) runs a paired permutation test
//! of Hybrid vs each baseline mode on the primary metric (nDCG@10) with Holm
//! correction.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use grafiki_core::{search_memory, SearchMemoryOptions, SearchMode};

use crate::config::{EvalConfig, EvalResult};
use crate::dataset::RetrievalDataset;
use crate::metrics::ir::{self, AggregateScores, MetricConfig, Qrel, Qrels, RunList, Runs};
use crate::metrics::stats;
use crate::seed::{seed_retrieval, EmbeddingInfo, EVAL_PROJECT, EVAL_SCOPE};

pub const PRIMARY_METRIC: &str = "ndcg@10";

pub fn mode_label(mode: SearchMode) -> &'static str {
    match mode {
        SearchMode::Keyword => "keyword",
        SearchMode::Semantic => "semantic",
        SearchMode::Hybrid => "hybrid",
        SearchMode::Graph => "graph",
        SearchMode::Rerank => "rerank",
    }
}

pub struct ModeResult {
    pub mode: SearchMode,
    pub semantic_available: bool,
    pub fallback_count: usize,
    pub overall: AggregateScores,
    pub per_record_type: BTreeMap<String, AggregateScores>,
}

pub struct Comparison {
    /// The mode being compared against Hybrid (the baseline arm `b`).
    pub baseline_mode: SearchMode,
    pub metric: String,
    /// mean(hybrid - baseline) on the primary metric (positive ⇒ Hybrid better).
    pub mean_delta: f64,
    pub p_value: f64,
    pub p_holm: f64,
    pub ci_low: f64,
    pub ci_high: f64,
}

pub struct RetrievalReport {
    pub dataset_name: String,
    pub dataset_version: String,
    pub query_count: usize,
    pub corpus_count: usize,
    pub primary_metric: String,
    pub modes: Vec<ModeResult>,
    pub comparisons: Vec<Comparison>,
    pub embedding: Option<EmbeddingInfo>,
    pub ingest_ms: u128,
    pub search_ms: u128,
}

/// Run the retrieval arm for the requested `modes`.
pub fn run_retrieval(
    dataset: &RetrievalDataset,
    modes: &[SearchMode],
    cfg: &EvalConfig,
) -> EvalResult<RetrievalReport> {
    // Semantic/Hybrid/Rerank need the model (Rerank's candidate pool includes the
    // semantic arm, and the cross-encoder itself is a model). Keyword and Graph are
    // model-free (Graph seeds from the keyword arm + PPR over relations).
    let needs_embeddings = modes.iter().any(|m| {
        matches!(
            m,
            SearchMode::Semantic | SearchMode::Hybrid | SearchMode::Rerank
        )
    });

    let t_seed = Instant::now();
    let corpus = seed_retrieval(dataset, needs_embeddings)?;
    let ingest_ms = t_seed.elapsed().as_millis();

    // doc_id -> record_type, for the per-record-type breakdown.
    let doc_type: BTreeMap<&str, &str> = dataset
        .corpus
        .iter()
        .map(|d| (d.doc_id.as_str(), d.record_type.as_str()))
        .collect();

    // Seed-time signal: were embeddings actually built for the corpus? This (not
    // a per-query empty result) is what gates the semantic/hybrid modes.
    let embeddings_built = corpus.embedding.as_ref().is_some_and(|e| e.processed > 0);

    let mcfg = MetricConfig::default();
    let mut mode_results: Vec<ModeResult> = Vec::new();
    let mut search_ms: u128 = 0;

    for &mode in modes {
        // Determinism guard: never report a silent keyword-fallback as a
        // semantic/hybrid result (EVAL_DESIGN §2.3). Gate on the seed-time signal,
        // not a single zero-match query — a semantic query can legitimately return
        // no positive-cosine hit without meaning the index is missing.
        if matches!(
            mode,
            SearchMode::Semantic | SearchMode::Hybrid | SearchMode::Rerank
        ) && !embeddings_built
        {
            return Err(format!(
                "mode '{}' requested but no embeddings were built for the corpus \
                 (process_embedding_jobs produced 0 vectors) — this would be a keyword \
                 fallback masquerading as a semantic result",
                mode_label(mode)
            )
            .into());
        }

        let mut runs: Runs = BTreeMap::new();
        let mut fallback_count = 0usize;

        for q in &dataset.queries {
            let t = Instant::now();
            let report = search_memory(SearchMemoryOptions {
                project_name: Some(EVAL_PROJECT.to_string()),
                start_dir: corpus.start_dir.clone(),
                grafiki_home: Some(corpus.home_path.clone()),
                query: q.text.clone(),
                record_type: "all".to_string(),
                mode,
                scope: EVAL_SCOPE.to_string(),
                limit: cfg.limit,
            })?;
            search_ms += t.elapsed().as_millis();

            if report.fallback.is_some() {
                fallback_count += 1;
            }
            // Map each retrieved record back to its fixture doc_id; a record with
            // no fixture id (e.g. an observation's backing entity) is unjudged and
            // keeps its "type:id" key, which won't appear in qrels (grade 0).
            let run: RunList = report
                .results
                .iter()
                .map(|r| {
                    let key = format!("{}:{}", r.record_type, r.id);
                    corpus.record_to_doc.get(&key).cloned().unwrap_or(key)
                })
                .collect();
            runs.insert(q.id.clone(), run);
        }

        let overall = ir::evaluate(&dataset.qrels, &runs, &mcfg);

        let mut per_record_type = BTreeMap::new();
        let types: BTreeSet<&str> = doc_type.values().copied().collect();
        for ty in types {
            let filtered: Qrels = dataset
                .qrels
                .iter()
                .filter_map(|(qid, qrel)| {
                    let sub: Qrel = qrel
                        .iter()
                        .filter(|(doc, _)| doc_type.get(doc.as_str()).copied() == Some(ty))
                        .map(|(d, g)| (d.clone(), *g))
                        .collect();
                    // Keep only queries that have ≥1 relevant doc of this type.
                    if sub.values().any(|&g| g > 0) {
                        Some((qid.clone(), sub))
                    } else {
                        None
                    }
                })
                .collect();
            if !filtered.is_empty() {
                per_record_type.insert(ty.to_string(), ir::evaluate(&filtered, &runs, &mcfg));
            }
        }

        mode_results.push(ModeResult {
            mode,
            semantic_available: embeddings_built,
            fallback_count,
            overall,
            per_record_type,
        });
    }

    // Paired comparisons: Hybrid vs each other mode on the primary metric.
    let mut comparisons = Vec::new();
    if let Some(hybrid) = mode_results
        .iter()
        .find(|m| matches!(m.mode, SearchMode::Hybrid))
    {
        let hv = hybrid.overall.per_query_vector(PRIMARY_METRIC);
        let mut pending: Vec<(SearchMode, f64, f64, f64, f64)> = Vec::new();
        for other in &mode_results {
            if matches!(other.mode, SearchMode::Hybrid) {
                continue;
            }
            let ov = other.overall.per_query_vector(PRIMARY_METRIC);
            let (delta, p) = stats::paired_permutation(&hv, &ov, cfg.permutation, cfg.seed);
            let est = stats::paired_bootstrap_delta(&hv, &ov, cfg.bootstrap, cfg.seed);
            pending.push((other.mode, delta, p, est.ci_low, est.ci_high));
        }
        let pvals: Vec<f64> = pending.iter().map(|x| x.2).collect();
        let holm = stats::holm(&pvals);
        for (i, (bmode, delta, p, lo, hi)) in pending.into_iter().enumerate() {
            comparisons.push(Comparison {
                baseline_mode: bmode,
                metric: PRIMARY_METRIC.to_string(),
                mean_delta: delta,
                p_value: p,
                p_holm: holm[i],
                ci_low: lo,
                ci_high: hi,
            });
        }
    }

    Ok(RetrievalReport {
        dataset_name: dataset.name.clone(),
        dataset_version: dataset.version.clone(),
        query_count: dataset.queries.len(),
        corpus_count: corpus.doc_count,
        primary_metric: PRIMARY_METRIC.to_string(),
        modes: mode_results,
        comparisons,
        embedding: corpus.embedding,
        ingest_ms,
        search_ms,
    })
}
