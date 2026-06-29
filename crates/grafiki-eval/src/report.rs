//! Emit machine-readable `results.json` (full provenance) and a human
//! `report.md`, plus the baseline regression gate.

use std::fmt::Write as _;

use serde_json::{json, Value};

use crate::config::EvalConfig;
use crate::metrics::ir::AggregateScores;
use crate::metrics::stats;
use crate::runner::redaction::RedactionReport;
use crate::runner::retrieval::{mode_label, RetrievalReport};
use crate::runner::supersession::SupersessionReport;

/// Run provenance recorded in every `results.json` — the harness, not the model,
/// is the dominant source of irreproducibility (lm-eval lesson), so we pin it.
pub fn provenance(cfg: &EvalConfig, embedding_model: &str, embedding_dim: Option<usize>) -> Value {
    let git_hash = std::env::var("GITHUB_SHA")
        .or_else(|_| std::env::var("GIT_COMMIT"))
        .unwrap_or_else(|_| "unknown".to_string());
    json!({
        "eval_version": env!("CARGO_PKG_VERSION"),
        "git_hash": git_hash,
        "embedding_model": embedding_model,
        "embedding_dim": embedding_dim,
        // Mirrors the hand-tuned fusion constants in grafiki-core memory.rs.
        "rrf": { "k": 45.0, "kw_weight": 1.10, "sem_weight": 1.00, "cross_source_bonus": 0.018 },
        "seed": cfg.seed,
        "bootstrap": cfg.bootstrap,
        "permutation": cfg.permutation,
        "convention": "ndcg_linear_gain_trec"
    })
}

/// `{ metric: { mean, ci95:[lo,hi], se } }` for every macro-averaged metric.
fn aggregate_with_ci(scores: &AggregateScores, cfg: &EvalConfig) -> Value {
    let mut obj = serde_json::Map::new();
    for metric in scores.macro_avg.keys() {
        let vec = scores.per_query_vector(metric);
        let est = stats::bootstrap_ci(&vec, cfg.bootstrap, cfg.seed);
        obj.insert(
            metric.clone(),
            json!({ "mean": est.mean, "ci95": [est.ci_low, est.ci_high], "se": est.se }),
        );
    }
    Value::Object(obj)
}

fn macro_only(scores: &AggregateScores) -> Value {
    json!(scores.macro_avg)
}

// ---------------------------------------------------------------------------
// Retrieval
// ---------------------------------------------------------------------------

pub fn retrieval_json(report: &RetrievalReport, cfg: &EvalConfig) -> Value {
    let embedding_model = report
        .embedding
        .as_ref()
        .map(|e| e.model.clone())
        .unwrap_or_else(|| "all-MiniLM-L6-v2 (not built; keyword-only run)".to_string());
    let embedding_dim = report.embedding.as_ref().map(|e| e.dimension);

    let mut per_mode = serde_json::Map::new();
    for m in &report.modes {
        let mut per_type = serde_json::Map::new();
        for (ty, sc) in &m.per_record_type {
            per_type.insert(ty.clone(), macro_only(sc));
        }
        per_mode.insert(
            mode_label(m.mode).to_string(),
            json!({
                "semantic_available": m.semantic_available,
                "fallback_count": m.fallback_count,
                "aggregate": aggregate_with_ci(&m.overall, cfg),
                "per_record_type": Value::Object(per_type),
                "per_query": m.overall.per_query.iter().map(|q| {
                    json!({ "qid": q.qid, "metrics": q.metrics })
                }).collect::<Vec<_>>(),
            }),
        );
    }

    let comparisons: Vec<Value> = report
        .comparisons
        .iter()
        .map(|c| {
            json!({
                "a": "hybrid", "b": mode_label(c.baseline_mode), "metric": c.metric,
                "mean_delta": c.mean_delta, "p_value": c.p_value, "p_holm": c.p_holm,
                "ci95": [c.ci_low, c.ci_high]
            })
        })
        .collect();

    json!({
        "provenance": provenance(cfg, &embedding_model, embedding_dim),
        "arm": "retrieval",
        "dataset": report.dataset_name,
        "dataset_version": report.dataset_version,
        "query_count": report.query_count,
        "corpus_count": report.corpus_count,
        "primary_metric": report.primary_metric,
        "per_mode": Value::Object(per_mode),
        "comparisons": comparisons,
        "cost": { "ingest_ms": report.ingest_ms, "search_ms": report.search_ms }
    })
}

fn fmt_pct(x: f64) -> String {
    format!("{:.4}", x)
}

pub fn retrieval_md(report: &RetrievalReport, cfg: &EvalConfig) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "# Grafiki eval — retrieval (`{}`)\n",
        report.dataset_name
    );
    let _ = writeln!(
        s,
        "Dataset `{}` v{} · {} queries · {} corpus docs · convention: linear-gain TREC nDCG · seed {}\n",
        report.dataset_name, report.dataset_version, report.query_count, report.corpus_count, cfg.seed
    );

    let headline = ["ndcg@10", "recall@10", "mrr", "map"];
    let _ = writeln!(s, "## Headline (macro-average, 95% bootstrap CI)\n");
    let _ = write!(s, "| mode | ");
    for h in headline {
        let _ = write!(s, "{h} | ");
    }
    let _ = writeln!(s, "judged@10 |");
    let _ = write!(s, "|---|");
    for _ in headline {
        let _ = write!(s, "---|");
    }
    let _ = writeln!(s, "---|");
    for m in &report.modes {
        let _ = write!(s, "| {} | ", mode_label(m.mode));
        for h in headline {
            let vec = m.overall.per_query_vector(h);
            let est = stats::bootstrap_ci(&vec, cfg.bootstrap, cfg.seed);
            let _ = write!(
                s,
                "{} [{}, {}] | ",
                fmt_pct(est.mean),
                fmt_pct(est.ci_low),
                fmt_pct(est.ci_high)
            );
        }
        let judged = m.overall.macro_avg.get("judged@10").copied().unwrap_or(0.0);
        let _ = writeln!(s, "{} |", fmt_pct(judged));
    }

    if !report.comparisons.is_empty() {
        let _ = writeln!(
            s,
            "\n## Paired comparison — Hybrid vs baselines on {} (paired permutation, Holm-corrected)\n",
            report.primary_metric
        );
        let _ = writeln!(
            s,
            "| comparison | Δ(hybrid−base) | 95% CI | p | p(Holm) | verdict |"
        );
        let _ = writeln!(s, "|---|---|---|---|---|---|");
        for c in &report.comparisons {
            let verdict = if c.p_holm < 0.05 && c.mean_delta > 0.0 {
                "Hybrid wins"
            } else if c.p_holm < 0.05 && c.mean_delta < 0.0 {
                "Hybrid loses"
            } else {
                "no sig. diff"
            };
            let _ = writeln!(
                s,
                "| hybrid vs {} | {:+.4} | [{:+.4}, {:+.4}] | {:.4} | {:.4} | {} |",
                mode_label(c.baseline_mode),
                c.mean_delta,
                c.ci_low,
                c.ci_high,
                c.p_value,
                c.p_holm,
                verdict
            );
        }
    }

    // Per-record-type for the first mode (usually keyword in CI) to flag a
    // per-type regression hiding under an aggregate win.
    if let Some(m) = report.modes.first() {
        if !m.per_record_type.is_empty() {
            let _ = writeln!(
                s,
                "\n## Per record-type — mode `{}` (nDCG@10)\n",
                mode_label(m.mode)
            );
            let _ = writeln!(s, "| record_type | nDCG@10 | recall@10 |");
            let _ = writeln!(s, "|---|---|---|");
            for (ty, sc) in &m.per_record_type {
                let _ = writeln!(
                    s,
                    "| {} | {} | {} |",
                    ty,
                    fmt_pct(sc.macro_avg.get("ndcg@10").copied().unwrap_or(0.0)),
                    fmt_pct(sc.macro_avg.get("recall@10").copied().unwrap_or(0.0))
                );
            }
        }
    }

    let _ = writeln!(
        s,
        "\n_Cost: ingest {} ms · search {} ms total._",
        report.ingest_ms, report.search_ms
    );
    s
}

// ---------------------------------------------------------------------------
// Redaction
// ---------------------------------------------------------------------------

pub fn redaction_json(report: &RedactionReport, cfg: &EvalConfig) -> Value {
    let o = &report.overall;
    let per_type: Vec<Value> = report
        .per_type
        .iter()
        .map(|t| {
            json!({ "type": t.secret_type, "tp": t.true_pos, "fn": t.false_neg, "recall": t.recall() })
        })
        .collect();
    let leaks: Vec<Value> = report
        .leaks
        .iter()
        .map(|l| json!({ "context": l.context, "type": l.secret_type, "literal": l.literal_preview }))
        .collect();
    json!({
        "provenance": provenance(cfg, "n/a (redaction)", None),
        "arm": "redaction",
        "dataset": report.dataset_name,
        "case_count": report.case_count,
        "positive_secret_count": report.positive_secret_count,
        "benign_count": report.benign_count,
        "overall": {
            "tp": o.true_pos, "fp": o.false_pos, "fn": o.false_neg, "tn": o.true_neg,
            "precision": o.precision(), "recall": o.recall(), "f1": o.f1(), "f2": o.f2()
        },
        "per_type": per_type,
        "leaks": leaks,
        "over_redaction_count": report.over_redactions.len()
    })
}

pub fn redaction_md(report: &RedactionReport) -> String {
    let mut s = String::new();
    let o = &report.overall;
    let _ = writeln!(
        s,
        "# Grafiki eval — redaction (`{}`)\n",
        report.dataset_name
    );
    let _ = writeln!(
        s,
        "{} cases · {} planted secrets · {} benign · **strict scoring (any residual leak = FN)**\n",
        report.case_count, report.positive_secret_count, report.benign_count
    );
    let _ = writeln!(s, "## Overall\n");
    let _ = writeln!(
        s,
        "| precision | recall | F1 | F2 | TP | FP | FN(leaks) | TN |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|---|---|---|");
    let _ = writeln!(
        s,
        "| {:.4} | {:.4} | {:.4} | {:.4} | {} | {} | {} | {} |",
        o.precision(),
        o.recall(),
        o.f1(),
        o.f2(),
        o.true_pos,
        o.false_pos,
        o.false_neg,
        o.true_neg
    );

    let _ = writeln!(s, "\n## Recall by secret type\n");
    let _ = writeln!(s, "| type | TP | FN | recall |");
    let _ = writeln!(s, "|---|---|---|---|");
    for t in &report.per_type {
        let _ = writeln!(
            s,
            "| {} | {} | {} | {:.4} |",
            t.secret_type,
            t.true_pos,
            t.false_neg,
            t.recall()
        );
    }

    let _ = writeln!(s, "\n## Leaks ({}) — the hard gate\n", report.leaks.len());
    if report.leaks.is_empty() {
        let _ = writeln!(s, "_None — no planted secret survived redaction._");
    } else {
        let _ = writeln!(s, "| context | type | leaked literal (masked) |");
        let _ = writeln!(s, "|---|---|---|");
        for l in &report.leaks {
            let _ = writeln!(
                s,
                "| {} | {} | `{}` |",
                l.context, l.secret_type, l.literal_preview
            );
        }
    }

    if !report.over_redactions.is_empty() {
        let _ = writeln!(
            s,
            "\n## Over-redactions ({}) — benign cases that were modified\n",
            report.over_redactions.len()
        );
        let _ = writeln!(s, "| context | before | after |");
        let _ = writeln!(s, "|---|---|---|");
        for r in &report.over_redactions {
            let _ = writeln!(
                s,
                "| {} | `{}` | `{}` |",
                r.context, r.before_preview, r.after_preview
            );
        }
    }
    s
}

// ---------------------------------------------------------------------------
// Supersession (Arm D)
// ---------------------------------------------------------------------------

pub fn supersession_json(report: &SupersessionReport, cfg: &EvalConfig) -> Value {
    let c = &report.conflict;
    let outcomes: Vec<Value> = report
        .outcomes
        .iter()
        .map(|o| {
            json!({
                "item_id": o.item_id, "category": o.category, "mechanism": o.mechanism,
                "new_surfaced": o.new_surfaced, "stale_suppressed": o.stale_suppressed,
                "abstained": o.abstained, "passed": o.passed
            })
        })
        .collect();
    let leaks: Vec<Value> = report
        .stale_leak_list
        .iter()
        .map(|(item, mech, tok)| json!({ "item_id": item, "mechanism": mech, "leaked_token": tok }))
        .collect();
    json!({
        "provenance": provenance(cfg, "n/a (supersession)", None),
        "arm": "supersession",
        "dataset": report.dataset_name,
        "item_count": report.item_count,
        "pass_rate": { "mean": report.pass_rate.mean, "ci95": [report.pass_rate.ci_low, report.pass_rate.ci_high] },
        "stale_leaks": leaks,
        "false_supersession_rate": report.false_supersession_rate,
        "retraction_abstain_acc": report.retraction_abstain_acc,
        "conflict_detection": {
            "precision": c.precision(), "recall": c.recall(), "f1": c.f1(),
            "tp": c.true_pos, "fp": c.false_pos, "fn": c.false_neg, "tn": c.true_neg
        },
        "per_item": outcomes
    })
}

pub fn supersession_md(report: &SupersessionReport) -> String {
    let mut s = String::new();
    let c = &report.conflict;
    let _ = writeln!(
        s,
        "# Grafiki eval — supersession (`{}`)\n",
        report.dataset_name
    );
    let _ = writeln!(
        s,
        "{} items · **strict pass = new surfaced ∧ stale suppressed**\n",
        report.item_count
    );
    let _ = writeln!(s, "## Headline\n");
    let _ = writeln!(s, "| metric | value |");
    let _ = writeln!(s, "|---|---|");
    let _ = writeln!(
        s,
        "| supersession pass-rate | {:.4} [{:.4}, {:.4}] |",
        report.pass_rate.mean, report.pass_rate.ci_low, report.pass_rate.ci_high
    );
    let _ = writeln!(
        s,
        "| stale leaks (hard gate) | {} |",
        report.stale_leak_list.len()
    );
    let _ = writeln!(
        s,
        "| false-supersession rate | {:.4} |",
        report.false_supersession_rate
    );
    let _ = writeln!(
        s,
        "| retraction-abstain accuracy | {:.4} |",
        report.retraction_abstain_acc
    );
    let _ = writeln!(
        s,
        "| conflict detection P / R / F1 | {:.4} / {:.4} / {:.4} |",
        c.precision(),
        c.recall(),
        c.f1()
    );

    if report.stale_leak_list.is_empty() {
        let _ = writeln!(s, "\n_No stale fact survived a supersession._");
    } else {
        let _ = writeln!(s, "\n## Stale leaks — the hard gate\n");
        let _ = writeln!(s, "| item | mechanism | leaked token |");
        let _ = writeln!(s, "|---|---|---|");
        for (item, mech, tok) in &report.stale_leak_list {
            let _ = writeln!(s, "| {item} | {mech} | `{tok}` |");
        }
    }

    let _ = writeln!(s, "\n## Per item\n");
    let _ = writeln!(
        s,
        "| item | category | mech | new | stale-suppressed | pass |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|---|");
    for o in &report.outcomes {
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} | {} | {} |",
            o.item_id,
            o.category,
            o.mechanism,
            if o.new_surfaced { "✓" } else { "·" },
            if o.stale_suppressed { "✓" } else { "LEAK" },
            if o.passed { "✓" } else { "✗" }
        );
    }
    s
}

// ---------------------------------------------------------------------------
// Baseline regression gate
// ---------------------------------------------------------------------------

/// Compare a retrieval (primary-mode) + redaction run against `baseline.json`.
/// Returns the list of regression messages (empty ⇒ pass).
pub fn check_regressions(
    baseline: &Value,
    retrieval: Option<&RetrievalReport>,
    redaction: Option<&RedactionReport>,
    supersession: Option<&SupersessionReport>,
) -> Vec<String> {
    let mut failures = Vec::new();

    // Symmetric guard: if the baseline declares an arm but that arm was not run,
    // fail loudly. Otherwise `--arm retrieval --fail-on-regression` against a full
    // baseline would silently skip the security-critical redaction/leak gate.
    if baseline.get("retrieval").is_some() && retrieval.is_none() {
        failures
            .push("baseline declares a `retrieval` gate but the retrieval arm was not run".into());
    }
    if baseline.get("redaction").is_some() && redaction.is_none() {
        failures
            .push("baseline declares a `redaction` gate but the redaction arm was not run".into());
    }
    if baseline.get("supersession").is_some() && supersession.is_none() {
        failures.push(
            "baseline declares a `supersession` gate but the supersession arm was not run".into(),
        );
    }

    if let (Some(rb), Some(rep)) = (baseline.get("supersession"), supersession) {
        if !rep.stale_leak_list.is_empty() {
            failures.push(format!(
                "supersession stale leaks = {} > 0 (stale facts survived: {})",
                rep.stale_leak_list.len(),
                rep.stale_leak_list
                    .iter()
                    .map(|(i, _, t)| format!("{i}:{t}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if let Some(minp) = rb.get("min_pass_rate").and_then(|v| v.as_f64()) {
            if rep.pass_rate.mean < minp {
                failures.push(format!(
                    "supersession pass-rate = {:.4} < min_pass_rate {minp:.4}",
                    rep.pass_rate.mean
                ));
            }
        }
        if let Some(maxfs) = rb
            .get("max_false_supersession_rate")
            .and_then(|v| v.as_f64())
        {
            if rep.false_supersession_rate > maxfs {
                failures.push(format!(
                    "false-supersession rate = {:.4} > max {maxfs:.4}",
                    rep.false_supersession_rate
                ));
            }
        }
        if let Some(minab) = rb
            .get("min_retraction_abstain_acc")
            .and_then(|v| v.as_f64())
        {
            if rep.retraction_abstain_acc < minab {
                failures.push(format!(
                    "retraction-abstain accuracy = {:.4} < min {minab:.4}",
                    rep.retraction_abstain_acc
                ));
            }
        }
    }

    if let (Some(rb), Some(rep)) = (baseline.get("retrieval"), retrieval) {
        let primary_mode = rb
            .get("primary_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("keyword");
        let tol = rb.get("tolerance").and_then(|v| v.as_f64()).unwrap_or(0.05);
        if let Some(m) = rep
            .modes
            .iter()
            .find(|m| mode_label(m.mode) == primary_mode)
        {
            for metric in ["ndcg@10", "recall@10"] {
                if let Some(floor) = rb.get(metric).and_then(|v| v.as_f64()) {
                    let got = m.overall.macro_avg.get(metric).copied().unwrap_or(0.0);
                    if got < floor - tol {
                        failures.push(format!(
                            "retrieval[{primary_mode}] {metric} = {got:.4} < baseline {floor:.4} − tol {tol:.4}"
                        ));
                    }
                }
            }
        } else {
            failures.push(format!(
                "baseline expects retrieval mode '{primary_mode}' but it was not run"
            ));
        }
    }

    if let (Some(rb), Some(rep)) = (baseline.get("redaction"), redaction) {
        let max_leaks = rb.get("max_leaks").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        if rep.leaks.len() > max_leaks {
            failures.push(format!(
                "redaction leaks = {} > max_leaks {} (secrets survived: {})",
                rep.leaks.len(),
                max_leaks,
                rep.leaks
                    .iter()
                    .map(|l| l.secret_type.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if let Some(minp) = rb.get("min_precision").and_then(|v| v.as_f64()) {
            let got = rep.overall.precision();
            if got < minp {
                failures.push(format!(
                    "redaction precision = {got:.4} < min_precision {minp:.4}"
                ));
            }
        }
        if let Some(minr) = rb.get("min_recall").and_then(|v| v.as_f64()) {
            let got = rep.overall.recall();
            if got < minr {
                failures.push(format!(
                    "redaction recall = {got:.4} < min_recall {minr:.4}"
                ));
            }
        }
    }

    failures
}

/// Build a `baseline.json` from a fresh run (used by `--write-baseline`).
pub fn build_baseline(
    retrieval: Option<&RetrievalReport>,
    redaction: Option<&RedactionReport>,
    supersession: Option<&SupersessionReport>,
    tolerance: f64,
) -> Value {
    // Round floors DOWN (4 dp) so the stored threshold is always ≤ the achievable
    // value — otherwise a byte-identical re-run could fall just below a rounded-up
    // floor and the gate would flap.
    let floor4 = |x: f64| (x * 10000.0).floor() / 10000.0;

    let mut obj = serde_json::Map::new();
    if let Some(rep) = retrieval {
        // Pin to the first mode run (keyword in the deterministic CI gate).
        if let Some(m) = rep.modes.first() {
            let mut r = serde_json::Map::new();
            r.insert("primary_mode".into(), json!(mode_label(m.mode)));
            r.insert("tolerance".into(), json!(tolerance));
            for metric in ["ndcg@10", "recall@10"] {
                let v = m.overall.macro_avg.get(metric).copied().unwrap_or(0.0);
                r.insert(metric.into(), json!(floor4(v)));
            }
            obj.insert("retrieval".into(), Value::Object(r));
        }
    }
    if let Some(rep) = redaction {
        // max_leaks is hard-coded to 0: a secret leak is never an acceptable
        // baseline, so re-snapshotting a regressed redactor must not bake in a
        // tolerance for leaks.
        obj.insert(
            "redaction".into(),
            json!({
                "max_leaks": 0,
                "min_precision": floor4(rep.overall.precision()),
                "min_recall": floor4(rep.overall.recall())
            }),
        );
    }
    if let Some(rep) = supersession {
        // Hard floors: a surviving stale fact and a false supersession are never
        // acceptable, so these are fixed at 0 regardless of the snapshot run.
        obj.insert(
            "supersession".into(),
            json!({
                "min_pass_rate": floor4(rep.pass_rate.mean),
                "max_stale_leaks": 0,
                "max_false_supersession_rate": 0.0,
                "min_retraction_abstain_acc": floor4(rep.retraction_abstain_acc)
            }),
        );
    }
    Value::Object(obj)
}
