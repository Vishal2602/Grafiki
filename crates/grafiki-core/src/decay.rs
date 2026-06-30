//! M-E1 / M-E2 — the **pure** decay & salience core (no I/O, no clock).
//!
//! Two deterministic scalars feed an opt-in temporal boost in the hybrid fusion:
//! - **recency** — a Weibull survival curve over a record's age, with per-category shape;
//! - **salience** — reinforcement from how often/recently the agent actually reused a record
//!   (derived by the orchestrator from the `agent_queries` audit log).
//!
//! Borrows mnemosyne's per-type Weibull parameters (`weibull.py`, MIT) adapted to Grafiki's
//! observation categories. Everything here is a pure function of its inputs, so the same store
//! plus the same reference time yields byte-identical scores — unit-tested below.

/// Neutral decay for types/categories without a meaningful age signal (entities, unknown).
/// `k = 1` (exponential) with a long scale so the curve is gentle.
pub const DEFAULT_PARAMS: (f64, f64) = (1.0, 720.0); // ~30-day scale

/// Per observation-category Weibull `(k = shape, eta = scale in hours)`. Lower `k` ⇒ slower
/// (long-lived) decay; higher `k` ⇒ faster. Scales mirror how durable each fact kind is.
/// Every value in the `observations.category` CHECK set is covered.
pub fn decay_params(category: &str) -> (f64, f64) {
    match category {
        // long-lived: stable preferences/conventions decay slowly
        "preference" => (0.4, 4380.0), // ~6 months
        "convention" => (0.5, 4380.0),
        "architecture" => (0.6, 2160.0), // ~3 months
        "pattern" => (0.6, 1680.0),
        "learned" => (0.7, 1440.0), // ~2 months
        "dependency" => (0.75, 2160.0),
        // medium
        "decision" => (1.0, 720.0), // ~1 month, exponential
        "risk" => (1.0, 720.0),
        "general" => (1.0, 720.0),
        // shorter-lived / time-sensitive working state
        "gotcha" => (0.9, 480.0),
        "blocker" => (1.1, 336.0),  // ~2 weeks, slightly accelerating
        "progress" => (1.2, 168.0), // ~1 week, fast
        _ => DEFAULT_PARAMS,
    }
}

/// Weibull survival freshness in `[0, 1]`: `exp(-(age/eta)^k)`. A brand-new record scores ~1.0
/// and the score decays toward 0 with age. Non-finite/negative age (clock skew, future stamps)
/// clamps to the freshest value `1.0`; non-positive params fall back to the default scale.
pub fn weibull_freshness(age_hours: f64, k: f64, eta: f64) -> f64 {
    if !age_hours.is_finite() || age_hours <= 0.0 {
        return 1.0;
    }
    let (k, eta) = if k > 0.0 && eta > 0.0 {
        (k, eta)
    } else {
        DEFAULT_PARAMS
    };
    let value = (-(age_hours / eta).powf(k)).exp();
    value.clamp(0.0, 1.0)
}

/// Convenience: freshness for a record of `category` aged `age_hours`.
pub fn category_freshness(category: &str, age_hours: f64) -> f64 {
    let (k, eta) = decay_params(category);
    weibull_freshness(age_hours, k, eta)
}

/// Access volume at which reuse salience saturates (≈ this many retrievals ⇒ full volume term).
const SALIENCE_SATURATION: f64 = 8.0;
/// Scale (hours) for how fast the "recently reused" half of salience decays (~1 week).
const SALIENCE_RECENCY_ETA: f64 = 168.0;

/// Reuse salience in `[0, 1]` from the audit log: blends access **volume** (how many times the
/// record was returned to the agent, log-compressed + saturating) with access **recency** (how
/// fresh the most recent reuse is, exponential). Zero accesses ⇒ 0.0. Pure.
pub fn reuse_salience(access_count: u64, last_access_age_hours: f64) -> f64 {
    if access_count == 0 {
        return 0.0;
    }
    let volume = (access_count as f64).ln_1p() / SALIENCE_SATURATION.ln_1p();
    let volume = volume.clamp(0.0, 1.0);
    let recency = weibull_freshness(last_access_age_hours, 1.0, SALIENCE_RECENCY_ETA);
    // Volume-dominant, lifted by recency: a frequently AND recently reused record scores highest.
    (0.6 * volume + 0.4 * volume * recency).clamp(0.0, 1.0)
}

/// Sub-weight of recency vs salience inside the combined temporal term (recency-dominant).
pub const RECENCY_SUBWEIGHT: f64 = 0.6;
pub const SALIENCE_SUBWEIGHT: f64 = 0.4;

/// The combined temporal relevance term in `[0, 1]` for a candidate. The orchestrator multiplies
/// this by `temporal_weight · RRF_UNIT` to get the additive fusion boost.
pub fn temporal_term(recency: f64, salience: f64) -> f64 {
    (RECENCY_SUBWEIGHT * recency + SALIENCE_SUBWEIGHT * salience).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freshness_is_monotonic_decreasing_in_age() {
        let (k, eta) = decay_params("general");
        let fresh = weibull_freshness(0.0, k, eta);
        let week = weibull_freshness(168.0, k, eta);
        let month = weibull_freshness(720.0, k, eta);
        let year = weibull_freshness(8760.0, k, eta);
        assert!((fresh - 1.0).abs() < 1e-9, "age 0 ⇒ 1.0, got {fresh}");
        assert!(
            fresh > week && week > month && month > year,
            "must decay: {fresh} {week} {month} {year}"
        );
        assert!((0.0..=1.0).contains(&year));
    }

    #[test]
    fn preferences_decay_slower_than_progress() {
        // At one month, a slow-decay preference should stay much fresher than fast progress notes.
        let pref = category_freshness("preference", 720.0);
        let progress = category_freshness("progress", 720.0);
        assert!(
            pref > progress,
            "preference {pref} should outlast progress {progress}"
        );
    }

    #[test]
    fn future_and_zero_age_clamp_to_one() {
        assert_eq!(weibull_freshness(-50.0, 1.0, 720.0), 1.0);
        assert_eq!(weibull_freshness(f64::NAN, 1.0, 720.0), 1.0);
        assert_eq!(weibull_freshness(0.0, 1.0, 720.0), 1.0);
    }

    #[test]
    fn bad_params_fall_back_to_default() {
        // Non-positive shape/scale must not produce NaN/inf.
        let v = weibull_freshness(100.0, 0.0, -5.0);
        assert!(v.is_finite() && (0.0..=1.0).contains(&v));
    }

    #[test]
    fn salience_zero_without_access_and_rises_with_reuse() {
        assert_eq!(reuse_salience(0, 0.0), 0.0);
        let once = reuse_salience(1, 1.0);
        let many = reuse_salience(20, 1.0);
        assert!(
            many > once && once > 0.0,
            "more reuse ⇒ more salience: {once} {many}"
        );
        // Recent reuse beats stale reuse at equal volume.
        let recent = reuse_salience(5, 1.0);
        let stale = reuse_salience(5, 10_000.0);
        assert!(
            recent > stale,
            "recent reuse {recent} should beat stale {stale}"
        );
        assert!((0.0..=1.0).contains(&many));
    }

    #[test]
    fn temporal_term_is_bounded_and_recency_dominant() {
        assert_eq!(temporal_term(0.0, 0.0), 0.0);
        assert!((temporal_term(1.0, 1.0) - 1.0).abs() < 1e-9);
        // Equal-magnitude: recency contributes more than salience.
        assert!(temporal_term(1.0, 0.0) > temporal_term(0.0, 1.0));
    }

    #[test]
    fn deterministic() {
        assert_eq!(
            category_freshness("architecture", 333.0),
            category_freshness("architecture", 333.0)
        );
        assert_eq!(reuse_salience(7, 42.0), reuse_salience(7, 42.0));
    }
}
