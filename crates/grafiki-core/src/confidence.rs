//! M-E3 — the **pure** candidate-confidence & active-learning-ordering core (no I/O).
//!
//! Gives each `extraction_candidate` a *principled* confidence (a source-reliability prior pulled
//! toward certainty by corroboration) and a review priority for **uncertainty sampling** — so a
//! human reviewing the queue sees the most informative candidates first, not just the newest.
//!
//! Borrows mnemosyne's veracity tiers + Bayesian `1 − base^n` corroboration (`veracity_consolidation.py`,
//! MIT), recast so confidence *starts* at the source prior and corroboration drives it toward 1.
//! All functions are pure ⇒ deterministic and unit-tested below.

/// Each additional corroborating mention closes this fraction of the gap to certainty.
const CORROBORATION_DECAY: f64 = 0.7;

/// Source-reliability prior in `[0, 1]` for a candidate's `source_type`. Higher ⇒ more trusted
/// origin. Unknown sources get a neutral mid prior. (Mirrors mnemosyne's veracity tiers.)
pub fn source_prior(source_type: &str) -> f64 {
    match source_type {
        "user" | "stated" | "manual" | "explicit" => 0.9,
        "decision" | "curated" => 0.8,
        "reflection" => 0.7,
        "capture" | "transcript" | "agent" | "session" => 0.6,
        "import" | "imported" => 0.6,
        "tool" => 0.5,
        "inferred" | "heuristic" => 0.5,
        _ => 0.6,
    }
}

/// Calibrated confidence in `[0, 1]`: starts at `source_prior` for a single mention and each extra
/// corroborating mention closes a `CORROBORATION_DECAY` fraction of the remaining gap to 1.0:
/// `c = 1 − (1 − prior)·decay^(mentions − 1)`. Monotonically increasing in both inputs.
pub fn calibrate(source_prior: f64, mentions: u64) -> f64 {
    let prior = source_prior.clamp(0.0, 1.0);
    let extra = mentions.max(1) - 1;
    (1.0 - (1.0 - prior) * CORROBORATION_DECAY.powi(extra as i32)).clamp(0.0, 1.0)
}

/// Convenience: calibrated confidence for a candidate given its `source_type` and corroborating
/// evidence count (the proposal itself counts as the first mention).
pub fn calibrated_confidence(source_type: &str, evidence_count: u64) -> f64 {
    calibrate(source_prior(source_type), 1 + evidence_count)
}

/// Decision uncertainty in `[0, 1]`, maximal at the `0.5` boundary and zero at a confident
/// `0.0`/`1.0`. This is the active-learning signal: the most uncertain candidates are the ones a
/// human reviewer can most usefully adjudicate.
pub fn uncertainty(confidence: f64) -> f64 {
    (1.0 - 2.0 * (confidence.clamp(0.0, 1.0) - 0.5).abs()).clamp(0.0, 1.0)
}

/// Review priority = uncertainty × representativeness, where representativeness grows (log) with
/// corroborating evidence — an uncertain candidate backed by more sources resolves more at once.
pub fn review_priority(uncertainty: f64, evidence_count: u64) -> f64 {
    uncertainty * (1.0 + (evidence_count as f64).ln_1p())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibrate_starts_at_prior_and_rises_with_corroboration() {
        let p = source_prior("tool"); // 0.5
        assert!(
            (calibrate(p, 1) - p).abs() < 1e-9,
            "one mention ⇒ the prior"
        );
        let two = calibrate(p, 2);
        let three = calibrate(p, 3);
        assert!(
            two > p && three > two,
            "corroboration must increase confidence: {p} {two} {three}"
        );
        assert!(three < 1.0 && calibrate(p, 50) <= 1.0, "stays bounded ≤ 1");
    }

    #[test]
    fn trusted_sources_outrank_inferred() {
        assert!(source_prior("user") > source_prior("inferred"));
        assert!(calibrated_confidence("user", 0) > calibrated_confidence("inferred", 0));
        assert!(source_prior("totally-unknown-source") > 0.0); // neutral, not zero
    }

    #[test]
    fn uncertainty_peaks_at_the_boundary() {
        assert!((uncertainty(0.5) - 1.0).abs() < 1e-9);
        assert_eq!(uncertainty(0.0), 0.0);
        assert_eq!(uncertainty(1.0), 0.0);
        assert!(uncertainty(0.5) > uncertainty(0.8) && uncertainty(0.8) > uncertainty(0.95));
    }

    #[test]
    fn review_priority_prefers_uncertain_well_evidenced_candidates() {
        // Same uncertainty, more evidence ⇒ higher priority.
        assert!(review_priority(0.8, 5) > review_priority(0.8, 0));
        // Same evidence, more uncertainty ⇒ higher priority.
        assert!(review_priority(1.0, 3) > review_priority(0.2, 3));
        assert_eq!(review_priority(0.0, 10), 0.0);
    }

    #[test]
    fn deterministic() {
        assert_eq!(
            calibrated_confidence("agent", 3),
            calibrated_confidence("agent", 3)
        );
        assert_eq!(review_priority(0.42, 4), review_priority(0.42, 4));
    }
}
