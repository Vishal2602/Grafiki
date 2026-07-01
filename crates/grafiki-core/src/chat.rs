//! "Chat with your memory" — grounded, cited RAG over Grafiki's retrieval.
//!
//! See `docs/CHAT_DESIGN.md`. This module holds the **pure** half: the reply
//! shape, the provider seam, and a deterministic model-free provider. The DB
//! orchestration (retrieve → ground → generate → cite) lives in
//! [`crate::memory::chat`], which reuses the existing hybrid retrieval.
//!
//! Two hard product constraints (both flow from "long sessions hallucinate"):
//! 1. **Grounded, never invented** — the answer is built ONLY from retrieved
//!    memory; when nothing is relevant the honest answer is [`NO_MEMORY_ANSWER`].
//! 2. **Cited** — every answer names the memories it used, so it is auditable.

use serde::{Deserialize, Serialize};

/// The fixed, honest answer when memory has nothing relevant. Grafiki abstains
/// rather than fabricate — the whole reason it exists is to stop hallucination.
pub const NO_MEMORY_ANSWER: &str = "I don't have anything in your memory about that yet.";

/// A grounded memory snippet handed to a [`ChatProvider`] as context.
#[derive(Debug, Clone)]
pub struct GroundedMemory {
    /// Citation index as referenced in the answer (`[1]`, `[2]`, …).
    pub index: usize,
    pub record_type: String,
    pub id: String,
    pub title: String,
    pub snippet: String,
    /// M-E5: the retrieved content tripped the prompt-injection guard, so a model
    /// provider must treat it as strictly untrusted data.
    pub suspicious: bool,
}

/// A source Grafiki used to answer — surfaced so every answer is auditable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub index: usize,
    pub record_type: String,
    pub id: String,
    pub title: String,
    pub snippet: String,
}

/// The answer plus its sources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatReply {
    pub question: String,
    pub scope: String,
    pub answer: String,
    pub citations: Vec<Citation>,
    /// `false` ⇒ Grafiki abstained (no relevant memory); it did NOT invent an answer.
    pub used_memory: bool,
    /// Any retrieved snippet tripped the injection guard (surfaced as a warning).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flagged_injection: bool,
}

/// Turns retrieved memory into an answer. This is the seam where a local model
/// (see `docs/CHAT_DESIGN.md` Phase 2) plugs in; the default is deterministic and
/// model-free so chat works — and is CI-testable — on the base build.
pub trait ChatProvider {
    fn generate(&self, question: &str, memories: &[GroundedMemory]) -> String;
}

/// Model-free default: a grounded EXTRACTIVE answer that quotes the most relevant
/// memories and references them by citation index. It cannot hallucinate — every
/// word comes from stored memory — so it is the honest floor before a local model
/// is available. (Empty input is handled upstream via [`NO_MEMORY_ANSWER`].)
pub struct ExtractiveProvider;

impl ChatProvider for ExtractiveProvider {
    fn generate(&self, _question: &str, memories: &[GroundedMemory]) -> String {
        let mut out = String::from("Based on your memory:");
        for memory in memories {
            let title = memory.title.trim();
            let snippet = memory.snippet.trim();
            out.push_str(&format!("\n[{}] ", memory.index));
            if !title.is_empty() && title != snippet {
                out.push_str(title);
                if !snippet.is_empty() {
                    out.push_str(" — ");
                }
            }
            out.push_str(snippet);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem(index: usize, title: &str, snippet: &str) -> GroundedMemory {
        GroundedMemory {
            index,
            record_type: "observation".to_owned(),
            id: format!("id{index}"),
            title: title.to_owned(),
            snippet: snippet.to_owned(),
            suspicious: false,
        }
    }

    #[test]
    fn extractive_answer_quotes_and_cites_every_memory() {
        let out = ExtractiveProvider.generate(
            "where do we deploy?",
            &[
                mem(1, "Deploy Target", "We deploy to GCP europe-west1"),
                mem(2, "Region", "us-east-1"),
            ],
        );
        // Grounded: every word comes from the snippets; nothing invented.
        assert!(out.contains("GCP europe-west1"));
        assert!(out.contains("us-east-1"));
        // Cited: each memory is referenced by its index.
        assert!(out.contains("[1]"));
        assert!(out.contains("[2]"));
        // A title distinct from its snippet is shown; a title equal to the
        // snippet (record 2's "Region"/"us-east-1") is not duplicated awkwardly.
        assert!(out.contains("Deploy Target — We deploy to GCP europe-west1"));
    }
}
