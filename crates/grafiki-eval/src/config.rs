//! Run configuration and the crate-wide error/result aliases.

use std::path::PathBuf;

/// Crate error type: boxed so we can `?`-propagate `grafiki_core::GrafikiError`,
/// `std::io::Error`, and `serde_json::Error` uniformly without a heavy dep.
pub type EvalError = Box<dyn std::error::Error + Send + Sync>;
pub type EvalResult<T> = std::result::Result<T, EvalError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Md,
}

impl OutputFormat {
    pub fn parse(raw: &str) -> EvalResult<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "md" | "markdown" => Ok(Self::Md),
            other => Err(format!("unknown --format '{other}' (expected json|md)").into()),
        }
    }
}

/// Parameters shared by every arm. Seed + iteration counts make bootstrap CIs
/// and permutation tests reproducible.
#[derive(Debug, Clone)]
pub struct EvalConfig {
    pub seed: u64,
    pub bootstrap: usize,
    pub permutation: usize,
    /// Search depth used by the retrieval arms.
    pub limit: usize,
    pub format: OutputFormat,
    pub out_dir: Option<PathBuf>,
    pub baseline: Option<PathBuf>,
    pub fail_on_regression: bool,
}

impl Default for EvalConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            bootstrap: 2000,
            permutation: 10_000,
            limit: 20,
            format: OutputFormat::Md,
            out_dir: None,
            baseline: None,
            fail_on_regression: false,
        }
    }
}
