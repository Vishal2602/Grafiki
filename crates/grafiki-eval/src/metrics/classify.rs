//! Binary classification metrics for the redaction and abstention arms.
//!
//! Precision/recall/F1/F-beta with the standard `0` convention when a
//! denominator is empty. F2 (β=2, recall-weighted) is reported alongside F1
//! because for redaction a *leak* (false negative) is worse than an
//! over-redaction (false positive) — the Presidio/PII convention — while
//! precision stays first-class because over-redaction corrupts memory and
//! poisons the FTS/embedding indices.

/// A 2×2 confusion count.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub true_pos: u64,
    pub false_pos: u64,
    pub false_neg: u64,
    pub true_neg: u64,
}

impl Counts {
    pub fn add(&mut self, other: Counts) {
        self.true_pos += other.true_pos;
        self.false_pos += other.false_pos;
        self.false_neg += other.false_neg;
        self.true_neg += other.true_neg;
    }

    /// Precision = TP / (TP + FP). `0.0` when no positives are predicted.
    pub fn precision(&self) -> f64 {
        let denom = self.true_pos + self.false_pos;
        if denom == 0 {
            0.0
        } else {
            self.true_pos as f64 / denom as f64
        }
    }

    /// Recall = TP / (TP + FN). `0.0` when there are no actual positives.
    pub fn recall(&self) -> f64 {
        let denom = self.true_pos + self.false_neg;
        if denom == 0 {
            0.0
        } else {
            self.true_pos as f64 / denom as f64
        }
    }

    /// F-beta = (1+β²)·P·R / (β²·P + R). `0.0` when P+R (weighted) is 0.
    pub fn f_beta(&self, beta: f64) -> f64 {
        let p = self.precision();
        let r = self.recall();
        let b2 = beta * beta;
        let denom = b2 * p + r;
        if denom == 0.0 {
            0.0
        } else {
            (1.0 + b2) * p * r / denom
        }
    }

    /// F1 = harmonic mean of precision and recall.
    pub fn f1(&self) -> f64 {
        self.f_beta(1.0)
    }

    /// F2 = recall-weighted (β=2); leaks penalized harder than over-redaction.
    pub fn f2(&self) -> f64 {
        self.f_beta(2.0)
    }

    pub fn total(&self) -> u64 {
        self.true_pos + self.false_pos + self.false_neg + self.true_neg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_classifier() {
        let c = Counts {
            true_pos: 10,
            false_pos: 0,
            false_neg: 0,
            true_neg: 5,
        };
        assert_eq!(c.precision(), 1.0);
        assert_eq!(c.recall(), 1.0);
        assert_eq!(c.f1(), 1.0);
        assert_eq!(c.f2(), 1.0);
    }

    #[test]
    fn fbeta_known_value() {
        // TP=8, FP=2, FN=4 → P=0.8, R=0.6667 → F1=0.7273, F2≈0.6897
        let c = Counts {
            true_pos: 8,
            false_pos: 2,
            false_neg: 4,
            true_neg: 0,
        };
        assert!((c.precision() - 0.8).abs() < 1e-9);
        assert!((c.recall() - 2.0 / 3.0).abs() < 1e-9);
        assert!((c.f1() - 0.7272727272727273).abs() < 1e-9);
        assert!((c.f2() - 0.6896551724137931).abs() < 1e-9);
    }

    #[test]
    fn empty_is_zero_not_nan() {
        let c = Counts::default();
        assert_eq!(c.precision(), 0.0);
        assert_eq!(c.recall(), 0.0);
        assert_eq!(c.f1(), 0.0);
    }
}
