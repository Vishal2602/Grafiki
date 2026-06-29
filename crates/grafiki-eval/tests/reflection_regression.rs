//! Deterministic, model-free CI gate for Arm E (reflection / consolidation).
//!
//! Runs the REAL H5 pipeline end-to-end on `grafiki_themes_v1`: seed grade-1
//! observations + relations → `run_reflection` → approve the produced `context`
//! candidates → re-search. Asserts the summarizer produced every expected summary
//! byte-for-byte, that each consolidated doc is retrieved only AFTER reflection (absent
//! in the baseline), that re-running is idempotent, and that the lift is real (capped
//! baseline nDCG@10 + a positive delta + recall@10 → 1.0). Keyword mode only ⇒ no model.

use std::path::PathBuf;

use grafiki_eval::config::EvalConfig;
use grafiki_eval::dataset::RetrievalDataset;
use grafiki_eval::runner::reflection::{run_reflection_arm, PRIMARY_METRIC};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[test]
fn reflection_arm_e_communities_lift_thematic_retrieval() {
    let dataset = RetrievalDataset::load(&fixtures().join("retrieval/grafiki_themes_v1"))
        .expect("load themes fixture");
    let report = run_reflection_arm(&dataset, &EvalConfig::default()).expect("run reflection arm");

    // Correctness + determinism: the real pipeline produced every expected community
    // summary, byte-identical to the committed fixture target (design §0/C3/C4).
    assert_eq!(
        report.summaries_matched,
        report.expected_summaries,
        "every produced summary must byte-equal its committed fixture doc.\n\
         produced contents:\n{}",
        report
            .produced
            .iter()
            .map(|p| format!("--- {}\n{}", p.key, p.content))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let baseline = report.baseline.macro_avg[PRIMARY_METRIC];
    let with = report.with_reflections.macro_avg[PRIMARY_METRIC];
    eprintln!(
        "Arm E: nDCG@10 {baseline:.4} → {with:.4} (Δ{:.4}); recall@10 {:.4} → {:.4}; \
         summaries {}/{}",
        report.delta_ndcg_at_10,
        report.baseline_recall_at_10,
        report.with_recall_at_10,
        report.summaries_matched,
        report.expected_summaries,
    );

    // Structural lift: the consolidated doc is retrieved WITH reflections and ABSENT in
    // the baseline — the gain is specifically the summary, not a raw observation that
    // happened to match (design §0/C3, adapted to keyword search's type-sectioned rank).
    assert!(
        report.target_absent_in_baseline,
        "no community summary should be retrievable before reflection runs"
    );
    assert!(
        report.target_retrieved_with_reflections,
        "every community summary must be retrieved once reflection has run"
    );

    // Idempotent: a second run on the unchanged store proposes nothing new (§0/C5/C9).
    assert!(
        report.idempotent_rerun,
        "reflection must be idempotent on an unchanged store"
    );

    // Headline metrics. The fixture grades one grade-3 summary + three grade-1 partial
    // observations per query. The baseline can never retrieve the grade-3 doc (it does
    // not exist yet), so it is structurally capped; with-reflections lifts both nDCG@10
    // and recall@10. (Keyword search sections context after observations, so the
    // absolute with-reflections nDCG is bounded below 1 even though every relevant doc
    // is now retrieved — hence the lift is asserted as a delta + recall, not a high
    // absolute nDCG.)
    assert!(
        baseline < 0.55,
        "baseline nDCG@10 should be capped without consolidation: {baseline}"
    );
    assert!(
        report.delta_ndcg_at_10 > 0.15,
        "consolidation must lift thematic nDCG@10: Δ{}",
        report.delta_ndcg_at_10
    );
    assert!(
        report.with_recall_at_10 > report.baseline_recall_at_10,
        "consolidation must lift recall@10: {} → {}",
        report.baseline_recall_at_10,
        report.with_recall_at_10
    );
    assert!(
        report.with_recall_at_10 >= 0.99,
        "with reflections every relevant doc (incl. the summary) should be retrieved: {}",
        report.with_recall_at_10
    );
}
