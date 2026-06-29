//! Eval arms. Each runner is independent so a new arm (e.g. memory-QA, SWE-bench)
//! slots in without touching the metrics or the others.

pub mod redaction;
pub mod reflection;
pub mod retrieval;
pub mod supersession;
