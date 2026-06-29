//! Parity oracle: asserts the Rust IR metrics equal `pytrec_eval`'s output on a
//! frozen `(qrels, run)` fixture. Until this passes, the numbers are not
//! "BEIR-comparable".
//!
//! Ground truth was generated once, offline, with the reference implementation:
//! ```text
//! pip install pytrec_eval numpy
//! qrel = {'q1':{'d1':2,'d2':0,'d3':1,'d4':2,'d5':1,'d6':0},
//!         'q2':{'a':1,'b':1,'c':0,'d':2}}
//! run  = {'q1':{'d1':6,'d2':5,'d3':4,'d4':3,'d5':2,'d6':1},
//!         'q2':{'b':4,'d':3,'a':2,'c':1}}
//! RelevanceEvaluator(qrel, {'ndcg_cut.1','ndcg_cut.3','ndcg_cut.5','ndcg_cut.10',
//!   'recall.5','recall.10','P.5','recip_rank','map','success.1','success.5'}).evaluate(run)
//! ```
//! (Run scores are strictly descending so the ranking is unambiguous.)

use std::collections::BTreeMap;

use grafiki_eval::metrics::ir::{self, MetricConfig, Qrel, Qrels, RunList, Runs};

fn qrel(pairs: &[(&str, i64)]) -> Qrel {
    pairs.iter().map(|(d, g)| (d.to_string(), *g)).collect()
}
fn run(docs: &[&str]) -> RunList {
    docs.iter().map(|d| d.to_string()).collect()
}

/// `(qid, metric) -> value` straight from pytrec_eval.
fn expected() -> BTreeMap<(&'static str, &'static str), f64> {
    let mut m = BTreeMap::new();
    // q1
    m.insert(("q1", "ndcg@1"), 1.0);
    m.insert(("q1", "ndcg@3"), 0.6645649565734895);
    m.insert(("q1", "ndcg@5"), 0.8940187669412318);
    m.insert(("q1", "ndcg@10"), 0.8940187669412318);
    m.insert(("q1", "recall@5"), 1.0);
    m.insert(("q1", "recall@10"), 1.0);
    m.insert(("q1", "P@5"), 0.8);
    m.insert(("q1", "mrr"), 1.0);
    m.insert(("q1", "map"), 0.8041666666666667);
    m.insert(("q1", "success@1"), 1.0);
    m.insert(("q1", "success@5"), 1.0);
    // q2
    m.insert(("q2", "ndcg@1"), 0.5);
    m.insert(("q2", "ndcg@3"), 0.8821211986607034);
    m.insert(("q2", "ndcg@5"), 0.8821211986607034);
    m.insert(("q2", "ndcg@10"), 0.8821211986607034);
    m.insert(("q2", "recall@5"), 1.0);
    m.insert(("q2", "recall@10"), 1.0);
    m.insert(("q2", "P@5"), 0.6);
    m.insert(("q2", "mrr"), 1.0);
    m.insert(("q2", "map"), 1.0);
    m.insert(("q2", "success@1"), 1.0);
    m.insert(("q2", "success@5"), 1.0);
    // q3 — relevant x1,x2,x3; run omits x3 (recall < 1) and interleaves u1
    // (unjudged) at rank 2, pinning the unjudged-doc and missed-relevant conventions.
    m.insert(("q3", "ndcg@1"), 1.0);
    m.insert(("q3", "ndcg@3"), 0.7984848580994974);
    m.insert(("q3", "ndcg@5"), 0.7984848580994974);
    m.insert(("q3", "ndcg@10"), 0.7984848580994974);
    m.insert(("q3", "recall@5"), 0.6666666666666666);
    m.insert(("q3", "recall@10"), 0.6666666666666666);
    m.insert(("q3", "P@5"), 0.4);
    m.insert(("q3", "mrr"), 1.0);
    m.insert(("q3", "map"), 0.5555555555555555);
    m.insert(("q3", "success@1"), 1.0);
    m.insert(("q3", "success@5"), 1.0);
    m
}

#[test]
fn pytrec_eval_parity() {
    let mut qrels: Qrels = BTreeMap::new();
    qrels.insert(
        "q1".to_string(),
        qrel(&[
            ("d1", 2),
            ("d2", 0),
            ("d3", 1),
            ("d4", 2),
            ("d5", 1),
            ("d6", 0),
        ]),
    );
    qrels.insert(
        "q2".to_string(),
        qrel(&[("a", 1), ("b", 1), ("c", 0), ("d", 2)]),
    );
    qrels.insert(
        "q3".to_string(),
        qrel(&[("x1", 2), ("x2", 1), ("x3", 1), ("x4", 0)]),
    );

    let mut runs: Runs = BTreeMap::new();
    runs.insert("q1".to_string(), run(&["d1", "d2", "d3", "d4", "d5", "d6"]));
    runs.insert("q2".to_string(), run(&["b", "d", "a", "c"]));
    // u1 is unjudged (absent from qrels); x3 is relevant but not retrieved.
    runs.insert("q3".to_string(), run(&["x1", "u1", "x2", "x4"]));

    let scores = ir::evaluate(&qrels, &runs, &MetricConfig::default());

    let exp = expected();
    for q in &scores.per_query {
        for (metric, value) in &q.metrics {
            if let Some(want) = exp.get(&(q.qid.as_str(), metric.as_str())) {
                assert!(
                    (value - want).abs() < 1e-9,
                    "metric {metric} for {}: got {value}, pytrec_eval {want}",
                    q.qid
                );
            }
        }
    }

    // Spot-check the macro-average matches pytrec_eval's mean over the 3 queries.
    assert!((scores.macro_avg["ndcg@10"] - 0.8582082745671441_f64).abs() < 1e-9);
    assert!((scores.macro_avg["map"] - 0.786574074074074_f64).abs() < 1e-9);
    assert!((scores.macro_avg["P@5"] - 0.6).abs() < 1e-9);
    assert!((scores.macro_avg["recall@10"] - 0.8888888888888888_f64).abs() < 1e-9);
}
