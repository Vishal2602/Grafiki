use std::path::PathBuf;

use grafiki_core::SearchMode;
use grafiki_eval::config::EvalConfig;
use grafiki_eval::dataset::MemoryQaDataset;
use grafiki_eval::runner::memory_qa::{run_memory_qa, ApproverPolicy};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/memory_qa/grafiki_sessions_v1")
}

#[test]
fn capture_review_memory_replay_retrieves_evidence_and_abstains() {
    let dataset = MemoryQaDataset::load(&fixture()).expect("load memory-QA fixture");
    let report = run_memory_qa(
        &dataset,
        SearchMode::Keyword,
        ApproverPolicy::AutoAll,
        &EvalConfig::default(),
    )
    .expect("run memory-QA replay");

    assert_eq!(report.session_count, 5);
    assert_eq!(report.question_count, 6);
    assert!(
        report.approved_count > 0,
        "review must promote trusted memory"
    );
    assert!(
        report.answerable.macro_avg["recall@10"] >= 0.8,
        "gold evidence recall unexpectedly low: {:?}",
        report.answerable.macro_avg
    );
    assert_eq!(
        report.answerable.macro_avg["ndcg@10"], 1.0,
        "gold source turns must rank before later summaries"
    );
    for outcome in report
        .outcomes
        .iter()
        .filter(|outcome| !outcome.abstain_expected)
    {
        assert_eq!(
            outcome.retrieved_evidence.first(),
            outcome.gold_evidence.first(),
            "{} ranked non-gold evidence first: {:?}",
            outcome.question_id,
            outcome.retrieved_evidence
        );
    }
    assert_eq!(
        report.abstention_accuracy, 1.0,
        "unknown questions must produce the fixed refusal"
    );
}

#[test]
fn reject_all_isolates_the_review_gate() {
    let dataset = MemoryQaDataset::load(&fixture()).expect("load memory-QA fixture");
    let report = run_memory_qa(
        &dataset,
        SearchMode::Keyword,
        ApproverPolicy::RejectAll,
        &EvalConfig::default(),
    )
    .expect("run reject-all replay");

    assert_eq!(report.approved_count, 0);
    assert_eq!(report.rejected_count, report.candidate_count);
    assert_eq!(report.answerable.macro_avg["recall@10"], 0.0);
}
