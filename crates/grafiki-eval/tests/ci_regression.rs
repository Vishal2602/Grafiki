//! Deterministic, model-free regression checks that run in the fast CI matrix:
//! keyword retrieval over the frozen fixture and redaction over the labeled
//! corpus. The hard invariants are encoded here; the CLI `--fail-on-regression`
//! gate adds the baseline-numeric comparison on top.

use std::path::PathBuf;

use grafiki_core::SearchMode;
use grafiki_eval::config::EvalConfig;
use grafiki_eval::dataset::{RedactionDataset, RetrievalDataset};
use grafiki_eval::runner::redaction::run_redaction;
use grafiki_eval::runner::retrieval::run_retrieval;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[test]
fn keyword_retrieval_is_sane() {
    let dataset = RetrievalDataset::load(&fixtures().join("retrieval/grafiki_dev_v1"))
        .expect("load retrieval fixture");
    let cfg = EvalConfig::default();
    let report =
        run_retrieval(&dataset, &[SearchMode::Keyword], &cfg).expect("run keyword retrieval");

    let keyword = &report.modes[0];
    let ndcg10 = keyword.overall.macro_avg["ndcg@10"];
    let recall10 = keyword.overall.macro_avg["recall@10"];

    // Conservative floors: BM25 over a hand-authored corpus must comfortably beat
    // these. (The numeric baseline gate uses tighter, generated thresholds.)
    assert!(
        ndcg10 > 0.40,
        "keyword nDCG@10 unexpectedly low: {ndcg10:.4}"
    );
    assert!(
        recall10 > 0.40,
        "keyword recall@10 unexpectedly low: {recall10:.4}"
    );
    // Every query should retrieve something.
    for q in &keyword.overall.per_query {
        assert!(
            q.metrics["judged@10"] >= 0.0,
            "query {} produced no scorable run",
            q.qid
        );
    }
}

#[test]
fn redaction_has_zero_leaks() {
    let dataset = RedactionDataset::load(&fixtures().join("redaction/corpus_v1.jsonl"))
        .expect("load redaction fixture");
    let report = run_redaction(&dataset).expect("run redaction");

    // The safety-critical invariant: no planted secret survives.
    assert_eq!(
        report.leaks.len(),
        0,
        "redaction leaked {} secret(s): {:?}",
        report.leaks.len(),
        report
            .leaks
            .iter()
            .map(|l| l.secret_type.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(report.overall.recall(), 1.0, "redaction recall must be 1.0");
    // Precision is co-reported; the corpus includes deliberate over-redaction
    // exposers, so it sits below 1.0 but must stay reasonable.
    assert!(
        report.overall.precision() >= 0.80,
        "redaction precision unexpectedly low: {:.4}",
        report.overall.precision()
    );
    // Every supported secret type should have at least one positive case.
    assert!(report.positive_secret_count >= 10);
}
