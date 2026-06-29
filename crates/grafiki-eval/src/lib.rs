//! `grafiki-eval` — the Grafiki evaluation harness (research item H1).
//!
//! Measures Grafiki's memory layer with TREC/BEIR-grade rigor, in-process
//! against `grafiki-core`'s public APIs, over deterministic offline fixtures:
//!
//! - **Arm A — retrieval quality**: keyword/semantic/hybrid over a frozen store,
//!   scored with linear-gain TREC nDCG/Recall/MRR/MAP (+ paired permutation test).
//! - **Arm C — redaction safety**: precision/recall/F1/F2 + a hard leak gate.
//! - **Arm B — memory-QA replay**: capture→candidate→trusted→ask (v1.5, needs the
//!   embedding model; scaffolding lives in the runner module).
//!
//! The default build is self-contained, offline, and deterministic so the
//! keyword-retrieval + redaction gate runs in the fast CI matrix. The `fastembed`
//! feature unlocks the semantic/hybrid arms (model downloaded on first use).
//!
//! See `docs/EVAL_DESIGN.md` for the full design and references.

pub mod config;
pub mod dataset;
pub mod metrics;
pub mod report;
pub mod runner;
pub mod seed;
