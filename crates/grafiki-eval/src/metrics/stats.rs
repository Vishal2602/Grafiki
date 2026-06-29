//! Uncertainty and significance: bootstrap CIs, paired bootstrap, and a paired
//! permutation test — the `lm-evaluation-harness` defaults for reporting a
//! metric with honest error bars and comparing two systems on the same items.
//!
//! All randomness comes from a seeded SplitMix64 PRNG so every run is bit-for-bit
//! reproducible given `(seed, iterations)`. No external `rand` dependency.

/// Deterministic SplitMix64 PRNG (Steele, Lea & Flood 2014). Tiny, fast, and
/// good enough for resampling; seeded so results are reproducible.
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform index in `[0, n)`. `n` must be > 0.
    #[inline]
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// Uniform `bool` (for permutation sign-flips).
    #[inline]
    pub fn flip(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }
}

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        0.0
    } else {
        xs.iter().sum::<f64>() / xs.len() as f64
    }
}

/// Linear-interpolation percentile (`q` in `[0,1]`) over an already-sorted slice.
fn percentile_sorted(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let pos = q * (sorted.len() as f64 - 1.0);
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let frac = pos - lo as f64;
        sorted[lo] * (1.0 - frac) + sorted[hi] * frac
    }
}

/// A point estimate with a bootstrap 95% confidence interval and standard error.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimate {
    pub mean: f64,
    pub ci_low: f64,
    pub ci_high: f64,
    pub se: f64,
}

/// Bootstrap CI of the mean of per-item scores. Resamples `n` items with
/// replacement `iters` times, recomputing the mean; the 95% CI is the
/// 2.5/97.5 percentiles of the bootstrap means and SE is their std-dev.
pub fn bootstrap_ci(samples: &[f64], iters: usize, seed: u64) -> Estimate {
    let point = mean(samples);
    if samples.is_empty() || iters == 0 {
        return Estimate {
            mean: point,
            ci_low: point,
            ci_high: point,
            se: 0.0,
        };
    }
    let n = samples.len();
    let mut rng = Rng::new(seed);
    let mut means = Vec::with_capacity(iters);
    for _ in 0..iters {
        let mut acc = 0.0;
        for _ in 0..n {
            acc += samples[rng.below(n)];
        }
        means.push(acc / n as f64);
    }
    means.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let m = mean(&means);
    let var = means.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / means.len() as f64;
    Estimate {
        mean: point,
        ci_low: percentile_sorted(&means, 0.025),
        ci_high: percentile_sorted(&means, 0.975),
        se: var.sqrt(),
    }
}

/// Paired bootstrap CI on the mean per-item delta `a[i] - b[i]` (same items,
/// two systems). Resamples item indices once per iteration and applies the same
/// indices to both arms, preserving pairing.
pub fn paired_bootstrap_delta(a: &[f64], b: &[f64], iters: usize, seed: u64) -> Estimate {
    assert_eq!(
        a.len(),
        b.len(),
        "paired bootstrap requires equal-length arms"
    );
    let deltas: Vec<f64> = a.iter().zip(b).map(|(x, y)| x - y).collect();
    bootstrap_ci(&deltas, iters, seed)
}

/// Two-sided paired permutation test on per-item deltas. Under the null, the
/// sign of each delta is exchangeable; we randomly flip signs `iters` times and
/// count how often `|mean|` is at least the observed `|mean|`. Assumption-free
/// (preferred over a paired t-test for small, non-normal metric samples).
///
/// Returns `(observed_mean_delta, p_value)`. The p-value uses the standard
/// `(hits + 1) / (iters + 1)` correction so it is never exactly 0.
pub fn paired_permutation(a: &[f64], b: &[f64], iters: usize, seed: u64) -> (f64, f64) {
    assert_eq!(
        a.len(),
        b.len(),
        "paired permutation requires equal-length arms"
    );
    let deltas: Vec<f64> = a.iter().zip(b).map(|(x, y)| x - y).collect();
    let observed = mean(&deltas).abs();
    if deltas.is_empty() || iters == 0 {
        return (mean(&deltas), 1.0);
    }
    let mut rng = Rng::new(seed);
    let mut hits = 0usize;
    for _ in 0..iters {
        let mut acc = 0.0;
        for &d in &deltas {
            if rng.flip() {
                acc -= d;
            } else {
                acc += d;
            }
        }
        if (acc / deltas.len() as f64).abs() >= observed - 1e-12 {
            hits += 1;
        }
    }
    let p = (hits as f64 + 1.0) / (iters as f64 + 1.0);
    (mean(&deltas), p)
}

/// Holm–Bonferroni step-down correction over a family of p-values. Returns the
/// adjusted p-values in the input order (each clamped to ≤ 1).
pub fn holm(pvalues: &[f64]) -> Vec<f64> {
    let m = pvalues.len();
    if m == 0 {
        return Vec::new();
    }
    // Sort indices ascending by p-value.
    let mut order: Vec<usize> = (0..m).collect();
    order.sort_by(|&i, &j| pvalues[i].partial_cmp(&pvalues[j]).unwrap());
    let mut adjusted = vec![0.0f64; m];
    let mut running_max = 0.0f64;
    for (rank, &idx) in order.iter().enumerate() {
        let factor = (m - rank) as f64;
        let adj = (factor * pvalues[idx]).min(1.0);
        running_max = running_max.max(adj); // enforce monotonic non-decreasing
        adjusted[idx] = running_max;
    }
    adjusted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.below(1000), b.below(1000));
        }
    }

    #[test]
    fn bootstrap_constant_sample_has_zero_width() {
        let est = bootstrap_ci(&[0.5, 0.5, 0.5, 0.5], 500, 1);
        assert!((est.mean - 0.5).abs() < 1e-12);
        assert!((est.ci_low - 0.5).abs() < 1e-12);
        assert!((est.ci_high - 0.5).abs() < 1e-12);
        assert!(est.se.abs() < 1e-12);
    }

    #[test]
    fn permutation_identical_arms_is_nonsignificant() {
        let a = vec![0.3, 0.7, 0.5, 0.9];
        let (delta, p) = paired_permutation(&a, &a, 2000, 7);
        assert_eq!(delta, 0.0);
        assert!(p > 0.99); // every permutation ties the observed |mean|=0
    }

    #[test]
    fn permutation_large_consistent_gap_is_significant() {
        // 8 paired items, all with a large positive delta. The smallest
        // achievable two-sided permutation p-value with n items is 2/2^n, so
        // n=8 ⇒ 0.0078 < 0.05 (with n=5 it would floor at 0.0625 — a real
        // property of the test, not a bug).
        let a = vec![0.9, 0.95, 0.92, 0.88, 0.91, 0.93, 0.89, 0.94];
        let b = vec![0.1, 0.15, 0.12, 0.08, 0.11, 0.13, 0.09, 0.14];
        let (delta, p) = paired_permutation(&a, &b, 20_000, 3);
        assert!(delta > 0.7);
        assert!(p < 0.02, "p={p}");
    }

    #[test]
    fn holm_orders_and_clamps() {
        let adj = holm(&[0.01, 0.04, 0.03]);
        // sorted: 0.01(*3)=0.03, 0.03(*2)=0.06, 0.04(*1)=0.04 -> monotone: 0.06
        assert!((adj[0] - 0.03).abs() < 1e-12);
        assert!((adj[2] - 0.06).abs() < 1e-12);
        assert!((adj[1] - 0.06).abs() < 1e-12);
        assert!(adj.iter().all(|&p| p <= 1.0));
    }
}
