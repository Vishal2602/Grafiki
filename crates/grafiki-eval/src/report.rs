//! Emit machine-readable `results.json` (full provenance) and a human
//! `report.md`, plus the baseline regression gate.

use std::fmt::Write as _;

use serde_json::{json, Value};

use crate::config::EvalConfig;
use crate::metrics::ir::AggregateScores;
use crate::metrics::stats;
use crate::runner::redaction::RedactionReport;
use crate::runner::retrieval::{mode_label, RetrievalReport};

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
// Baseline regression gate
// ---------------------------------------------------------------------------

/// Compare a retrieval (primary-mode) + redaction run against `baseline.json`.
/// Returns the list of regression messages (empty ⇒ pass).
pub fn check_regressions(
    baseline: &Value,
    retrieval: Option<&RetrievalReport>,
    redaction: Option<&RedactionReport>,
) -> Vec<String> {
    let mut failures = Vec::new();

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
    tolerance: f64,
) -> Value {
    let mut obj = serde_json::Map::new();
    if let Some(rep) = retrieval {
        // Pin to the first mode run (keyword in the deterministic CI gate).
        if let Some(m) = rep.modes.first() {
            let mut r = serde_json::Map::new();
            r.insert("primary_mode".into(), json!(mode_label(m.mode)));
            r.insert("tolerance".into(), json!(tolerance));
            for metric in ["ndcg@10", "recall@10"] {
                let v = m.overall.macro_avg.get(metric).copied().unwrap_or(0.0);
                r.insert(metric.into(), json!((v * 10000.0).round() / 10000.0));
            }
            obj.insert("retrieval".into(), Value::Object(r));
        }
    }
    if let Some(rep) = redaction {
        let p = (rep.overall.precision() * 10000.0).round() / 10000.0;
        let rcl = (rep.overall.recall() * 10000.0).round() / 10000.0;
        obj.insert(
            "redaction".into(),
            json!({ "max_leaks": rep.leaks.len(), "min_precision": p, "min_recall": rcl }),
        );
    }
    Value::Object(obj)
}
