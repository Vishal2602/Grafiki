//! Metric primitives for the eval harness.
//!
//! - [`ir`] — retrieval metrics (nDCG/Recall/Precision/MRR/MAP/Success/Judged),
//!   matching `trec_eval`/BEIR/MTEB to ~1e-6 (proven by `tests/metrics_oracle.rs`).
//! - [`classify`] — precision/recall/F1/F-beta for redaction & abstention.
//! - [`stats`] — bootstrap CIs, paired bootstrap/permutation, Holm correction.

pub mod classify;
pub mod ir;
pub mod stats;
