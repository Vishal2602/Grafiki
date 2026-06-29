//! Arm C — redaction precision/recall/F1/F2 over a labeled corpus.
//!
//! Grafiki's redactor *substitutes* secrets with `[REDACTED_…]` markers rather
//! than emitting spans, so scoring is an input→output diff, not span-vs-span:
//! - **TP** — a planted secret literal is gone from the output.
//! - **FN (leak)** — the literal survives verbatim (the worst case: a credential
//!   persisted to SQLite). Every leak is recorded for the hard CI gate.
//! - **FP (over-redaction)** — a benign case was modified (corrupts memory and
//!   poisons the FTS/embedding indices), so precision is first-class too.

use std::collections::BTreeMap;

use grafiki_core::{redact_json, redact_text};

use crate::config::EvalResult;
use crate::dataset::{RedactionCase, RedactionDataset};
use crate::metrics::classify::Counts;

pub struct Leak {
    pub context: String,
    pub secret_type: String,
    /// Masked preview of the leaked literal (first chars only — never the whole
    /// secret, even though the corpus is synthetic).
    pub literal_preview: String,
}

pub struct OverRedaction {
    pub context: String,
    pub before_preview: String,
    pub after_preview: String,
}

pub struct TypeStat {
    pub secret_type: String,
    pub true_pos: u64,
    pub false_neg: u64,
}

impl TypeStat {
    pub fn recall(&self) -> f64 {
        let d = self.true_pos + self.false_neg;
        if d == 0 {
            0.0
        } else {
            self.true_pos as f64 / d as f64
        }
    }
}

pub struct RedactionReport {
    pub dataset_name: String,
    pub case_count: usize,
    pub positive_secret_count: usize,
    pub benign_count: usize,
    pub overall: Counts,
    pub per_type: Vec<TypeStat>,
    pub leaks: Vec<Leak>,
    pub over_redactions: Vec<OverRedaction>,
}

fn preview(s: &str) -> String {
    const MAX: usize = 72;
    let one_line = s.replace(['\n', '\r'], "⏎");
    if one_line.chars().count() > MAX {
        let head: String = one_line.chars().take(MAX).collect();
        format!("{head}…")
    } else {
        one_line
    }
}

/// Mask a (synthetic) secret literal so committed reports never carry a full
/// key-shaped string: keep a short prefix, then the length.
fn mask(literal: &str) -> String {
    let head: String = literal.chars().take(6).collect();
    format!("{head}… ({} chars)", literal.chars().count())
}

/// Produce the (input, redacted-output) string pair for a case, routing each
/// input type through the redactor production actually applies to it: text via
/// `redact_text` (the `ingest_capture_event` path) and `json_payload` via the
/// structured `redact_json` (the `propose_candidate` path). The two differ — the
/// JSON path is value-level and key-aware — so testing only the text path would
/// be blind to leaks on the dominant candidate-payload sink.
fn redact_case(case: &RedactionCase) -> EvalResult<(String, String)> {
    match (&case.text, &case.json_payload) {
        (Some(t), _) => {
            let (out, _) = redact_text(t);
            Ok((t.clone(), out))
        }
        (None, Some(j)) => {
            let (redacted, _) = redact_json(j);
            Ok((serde_json::to_string(j)?, serde_json::to_string(&redacted)?))
        }
        (None, None) => Err("redaction case has neither `text` nor `json_payload`".into()),
    }
}

/// A planted secret leaks if its literal — or a long contiguous run of it —
/// survives redaction. The substring check catches *partial* redaction: the
/// redactor trims surrounding punctuation and replaces only the trimmed core, so
/// a near-miss could leave most of the secret behind, which a bare
/// `contains(literal)` would wrongly score as a clean true positive.
fn leaked(literal: &str, output: &str) -> bool {
    if output.contains(literal) {
        return true;
    }
    let chars: Vec<char> = literal.chars().collect();
    let n = chars.len();
    if n < 16 {
        // Short literals: exact match only, to avoid coincidental substring hits.
        return false;
    }
    let threshold = (n * 3 / 4).max(16);
    (0..=n - threshold).any(|start| {
        let sub: String = chars[start..start + threshold].iter().collect();
        output.contains(&sub)
    })
}

pub fn run_redaction(dataset: &RedactionDataset) -> EvalResult<RedactionReport> {
    let mut overall = Counts::default();
    let mut tp_by_type: BTreeMap<String, u64> = BTreeMap::new();
    let mut fn_by_type: BTreeMap<String, u64> = BTreeMap::new();
    let mut leaks = Vec::new();
    let mut over_redactions = Vec::new();
    let mut positive_secret_count = 0usize;
    let mut benign_count = 0usize;

    for case in &dataset.cases {
        let (input, output) = redact_case(case)?;

        // Benign cases (no planted secret) drive precision: any modification is
        // an over-redaction (FP), else a true negative.
        if case.gold_secrets.is_empty() {
            benign_count += 1;
            if output != input {
                overall.false_pos += 1;
                over_redactions.push(OverRedaction {
                    context: case.context.clone(),
                    before_preview: preview(&input),
                    after_preview: preview(&output),
                });
            } else {
                overall.true_neg += 1;
            }
            continue;
        }

        // Positive cases: each planted secret is a detection unit.
        for gs in &case.gold_secrets {
            positive_secret_count += 1;
            if leaked(&gs.literal, &output) {
                overall.false_neg += 1;
                *fn_by_type.entry(gs.secret_type.clone()).or_default() += 1;
                leaks.push(Leak {
                    context: case.context.clone(),
                    secret_type: gs.secret_type.clone(),
                    literal_preview: mask(&gs.literal),
                });
            } else {
                overall.true_pos += 1;
                *tp_by_type.entry(gs.secret_type.clone()).or_default() += 1;
            }
        }
    }

    let mut types: Vec<String> = tp_by_type
        .keys()
        .chain(fn_by_type.keys())
        .cloned()
        .collect();
    types.sort();
    types.dedup();
    let per_type = types
        .into_iter()
        .map(|t| TypeStat {
            true_pos: tp_by_type.get(&t).copied().unwrap_or(0),
            false_neg: fn_by_type.get(&t).copied().unwrap_or(0),
            secret_type: t,
        })
        .collect();

    Ok(RedactionReport {
        dataset_name: dataset.name.clone(),
        case_count: dataset.cases.len(),
        positive_secret_count,
        benign_count,
        overall,
        per_type,
        leaks,
        over_redactions,
    })
}
