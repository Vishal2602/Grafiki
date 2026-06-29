//! Deterministic, model-free CI gate for Arm D (supersession): drive the bundled
//! knowledge-update fixture through the candidate gate and assert the hard
//! invariants — no stale fact survives, no false supersession, retractions
//! abstain, and the pass-rate clears the floor.

use std::path::PathBuf;

use grafiki_eval::config::EvalConfig;
use grafiki_eval::dataset::SupersessionDataset;
use grafiki_eval::runner::supersession::run_supersession;

#[test]
fn supersession_invariants_hold() {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/supersession/grafiki_updates_v1");
    let dataset = SupersessionDataset::load(&dir).expect("load supersession fixture");
    let report = run_supersession(&dataset, &EvalConfig::default()).expect("run supersession");

    assert!(
        report.stale_leak_list.is_empty(),
        "a stale fact survived supersession: {:?}",
        report.stale_leak_list
    );
    assert_eq!(
        report.false_supersession_rate, 0.0,
        "a still-true fact was wrongly suppressed"
    );
    assert_eq!(
        report.retraction_abstain_acc, 1.0,
        "a retraction failed to abstain"
    );
    assert!(
        report.pass_rate.mean >= 0.9,
        "supersession pass-rate unexpectedly low: {:.4}",
        report.pass_rate.mean
    );
}
