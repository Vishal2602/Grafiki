//! Arm E — reflection / consolidation.
//!
//! Proves that a thematic query — answerable by NO single raw observation — is lifted
//! once `run_reflection` consolidates a community into a `context` summary. The arm
//! runs the **real pipeline** end-to-end (seed grade-1 facts + relations →
//! `run_reflection` → `approve_candidate` → re-search), so the gate cannot pass unless
//! the summarizer actually produces the right consolidated doc and it wins retrieval
//! (REFLECTION_DESIGN §0/C3/C4). Keyword mode only ⇒ model-free: the candidate is a
//! `context` record, which keeps it off the `#[cfg(feature="fastembed")]`
//! observation-conflict branch in `propose_candidate` (design §0/C10).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use grafiki_core::{
    approve_candidate, get_context, list_context, run_reflection, search_memory,
    ApproveCandidateOptions, ContextListOptions, GetContextOptions, RunReflectionOptions,
    SearchMemoryOptions, SearchMode, SearchResult,
};

use crate::config::{EvalConfig, EvalResult};
use crate::dataset::{Query, RetrievalDataset};
use crate::metrics::ir::{self, AggregateScores, MetricConfig, RunList, Runs};
use crate::seed::{seed_retrieval, EVAL_PROJECT, EVAL_SCOPE};

pub const PRIMARY_METRIC: &str = "ndcg@10";

pub struct ProducedSummary {
    pub key: String,
    pub content: String,
}

pub struct ReflectionArmReport {
    pub dataset_name: String,
    pub query_count: usize,
    /// Raw facts only — the grade-3 summary doesn't exist yet.
    pub baseline: AggregateScores,
    /// After `run_reflection` + approval seeds the consolidated `context` docs.
    pub with_reflections: AggregateScores,
    pub delta_ndcg_at_10: f64,
    /// Every `context` summary the real pipeline produced (key + verbatim content).
    pub produced: Vec<ProducedSummary>,
    /// Grade-3 `context` target docs in the fixture.
    pub expected_summaries: usize,
    /// Produced summaries whose content byte-equals a committed fixture target.
    pub summaries_matched: usize,
    /// Structural lift: every grade-3 summary is retrieved WITH reflections and is
    /// ABSENT in the baseline — i.e. the gain is specifically the consolidated doc, not
    /// a raw observation accidentally satisfying the query. (Keyword search sections
    /// results by record type — entities, observations, decisions, context — so a
    /// `context` summary cannot outrank an observation; the meaningful structural claim
    /// is therefore *retrievability*, scored by nDCG/recall below.)
    pub target_retrieved_with_reflections: bool,
    pub target_absent_in_baseline: bool,
    /// Recall@10 over all relevant docs (grade ≥ 1), baseline vs with-reflections.
    pub baseline_recall_at_10: f64,
    pub with_recall_at_10: f64,
    /// Re-running on the unchanged store proposes zero new candidates.
    pub idempotent_rerun: bool,
}

/// Map a retrieved record back to its fixture `doc_id`. Context summaries are produced
/// (their keys are store-local ULID-derived), so they're matched by verbatim content;
/// everything else uses the seed-time `record_to_doc`. An unmapped record keeps its
/// `type:id` key, which won't appear in qrels (grade 0).
fn map_result_doc(
    result: &SearchResult,
    record_to_doc: &HashMap<String, String>,
    content_to_doc: &HashMap<String, String>,
) -> String {
    if result.record_type == "context" {
        if let Some(doc) = content_to_doc.get(&result.snippet) {
            return doc.clone();
        }
    }
    let key = format!("{}:{}", result.record_type, result.id);
    record_to_doc.get(&key).cloned().unwrap_or(key)
}

#[allow(clippy::too_many_arguments)]
fn search_runs(
    start_dir: &Path,
    home: &Path,
    queries: &[Query],
    record_to_doc: &HashMap<String, String>,
    content_to_doc: &HashMap<String, String>,
    limit: usize,
) -> EvalResult<Runs> {
    let mut runs: Runs = BTreeMap::new();
    for q in queries {
        let report = search_memory(SearchMemoryOptions {
            project_name: Some(EVAL_PROJECT.to_string()),
            start_dir: start_dir.to_path_buf(),
            grafiki_home: Some(home.to_path_buf()),
            query: q.text.clone(),
            record_type: "all".to_string(),
            mode: SearchMode::Keyword,
            scope: EVAL_SCOPE.to_string(),
            limit,
            temporal_weight: 0.0,
        })?;
        let docs: RunList = report
            .results
            .iter()
            .map(|r| map_result_doc(r, record_to_doc, content_to_doc))
            .collect();
        runs.insert(q.id.clone(), docs);
    }
    Ok(runs)
}

pub fn run_reflection_arm(
    dataset: &RetrievalDataset,
    cfg: &EvalConfig,
) -> EvalResult<ReflectionArmReport> {
    // The `context` corpus docs are the EXPECTED community summaries (grade 3); they
    // are NOT seeded — `run_reflection` must produce them. Index them by content so a
    // produced summary maps back to its fixture doc_id.
    let target_docs: Vec<&crate::dataset::CorpusDoc> = dataset
        .corpus
        .iter()
        .filter(|d| d.record_type == "context")
        .collect();
    let content_to_doc: HashMap<String, String> = target_docs
        .iter()
        .filter_map(|d| {
            d.payload
                .get("content")
                .and_then(|v| v.as_str())
                .map(|c| (c.to_string(), d.doc_id.clone()))
        })
        .collect();
    let expected_summaries = target_docs.len();

    // Seed everything EXCEPT the context targets (entities + observations + relations).
    let seed_corpus: Vec<crate::dataset::CorpusDoc> = dataset
        .corpus
        .iter()
        .filter(|d| d.record_type != "context")
        .cloned()
        .collect();
    let seed_ds = RetrievalDataset {
        name: dataset.name.clone(),
        version: dataset.version.clone(),
        description: dataset.description.clone(),
        corpus: seed_corpus,
        queries: dataset.queries.clone(),
        qrels: dataset.qrels.clone(),
        relations: dataset.relations.clone(),
    };
    let corpus = seed_retrieval(&seed_ds, false)?;
    let mcfg = MetricConfig::default();

    // --- Baseline: raw facts only; the grade-3 summary is unreachable. ---
    let baseline_runs = search_runs(
        &corpus.start_dir,
        &corpus.home_path,
        &dataset.queries,
        &corpus.record_to_doc,
        &content_to_doc,
        cfg.limit,
    )?;
    let baseline = ir::evaluate(&dataset.qrels, &baseline_runs, &mcfg);

    // --- Real pipeline: detect communities → summarize → propose → approve. ---
    let mut opts = RunReflectionOptions::new(EVAL_SCOPE, corpus.start_dir.clone());
    opts.project_name = Some(EVAL_PROJECT.to_string());
    opts.grafiki_home = Some(corpus.home_path.clone());
    let report = run_reflection(opts)?;
    for detail in &report.details {
        if let Some(id) = &detail.candidate_id {
            approve_candidate(ApproveCandidateOptions {
                project_name: Some(EVAL_PROJECT.to_string()),
                start_dir: corpus.start_dir.clone(),
                grafiki_home: Some(corpus.home_path.clone()),
                id: id.clone(),
            })?;
        }
    }

    // Enumerate produced summaries (none were seeded, so every context row is one).
    let summaries = list_context(ContextListOptions {
        project_name: Some(EVAL_PROJECT.to_string()),
        start_dir: corpus.start_dir.clone(),
        grafiki_home: Some(corpus.home_path.clone()),
        category: None,
        scope: EVAL_SCOPE.to_string(),
    })?;
    let mut produced = Vec::new();
    for summary in &summaries {
        let doc = get_context(GetContextOptions {
            project_name: Some(EVAL_PROJECT.to_string()),
            start_dir: corpus.start_dir.clone(),
            grafiki_home: Some(corpus.home_path.clone()),
            key: summary.key.clone(),
        })?;
        produced.push(ProducedSummary {
            key: doc.key,
            content: doc.content,
        });
    }
    let summaries_matched = produced
        .iter()
        .filter(|p| content_to_doc.contains_key(&p.content))
        .count();

    // --- With reflections: re-search; the consolidated docs are now retrievable. ---
    let with_runs = search_runs(
        &corpus.start_dir,
        &corpus.home_path,
        &dataset.queries,
        &corpus.record_to_doc,
        &content_to_doc,
        cfg.limit,
    )?;
    let with_reflections = ir::evaluate(&dataset.qrels, &with_runs, &mcfg);

    // Structural lift: the grade-3 target must be retrieved WITH reflections and ABSENT
    // in the baseline (the gain is specifically the consolidated doc).
    let targets_for = |qrel: &crate::metrics::ir::Qrel| -> Vec<String> {
        qrel.iter()
            .filter(|(_, &g)| g >= 2)
            .map(|(d, _)| d.clone())
            .collect()
    };
    let target_retrieved_with_reflections = dataset.qrels.iter().all(|(qid, qrel)| {
        let ranked = match with_runs.get(qid) {
            Some(r) => r,
            None => return false,
        };
        targets_for(qrel)
            .iter()
            .all(|t| ranked.iter().any(|d| d == t))
    });
    let target_absent_in_baseline = dataset.qrels.iter().all(|(qid, qrel)| {
        let ranked = match baseline_runs.get(qid) {
            Some(r) => r,
            None => return true,
        };
        targets_for(qrel)
            .iter()
            .all(|t| !ranked.iter().any(|d| d == t))
    });
    let baseline_recall_at_10 = baseline.macro_avg.get("recall@10").copied().unwrap_or(0.0);
    let with_recall_at_10 = with_reflections
        .macro_avg
        .get("recall@10")
        .copied()
        .unwrap_or(0.0);

    // Idempotency: a second run on the unchanged store proposes nothing new.
    let mut opts2 = RunReflectionOptions::new(EVAL_SCOPE, corpus.start_dir.clone());
    opts2.project_name = Some(EVAL_PROJECT.to_string());
    opts2.grafiki_home = Some(corpus.home_path.clone());
    let rerun = run_reflection(opts2)?;
    let idempotent_rerun = rerun.candidates_created == 0;

    let delta_ndcg_at_10 = with_reflections
        .macro_avg
        .get(PRIMARY_METRIC)
        .copied()
        .unwrap_or(0.0)
        - baseline
            .macro_avg
            .get(PRIMARY_METRIC)
            .copied()
            .unwrap_or(0.0);

    Ok(ReflectionArmReport {
        dataset_name: dataset.name.clone(),
        query_count: dataset.queries.len(),
        baseline,
        with_reflections,
        delta_ndcg_at_10,
        produced,
        expected_summaries,
        summaries_matched,
        target_retrieved_with_reflections,
        target_absent_in_baseline,
        baseline_recall_at_10,
        with_recall_at_10,
        idempotent_rerun,
    })
}
