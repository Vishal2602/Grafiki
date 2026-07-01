//! Auto-extraction of durable memory from a raw agent-session transcript — the
//! "Granola for agents" capture. A local model reads the transcript and returns
//! structured memory items, which are then PROPOSED for the user's review (never
//! silently trusted — see `test-data-pollutes-real-memory`). See
//! `docs/CHAT_DESIGN.md` and the capture pipeline in `memory.rs`.
//!
//! This module holds the **pure** half — the extraction prompt and a tolerant
//! parser — so it is deterministic and CI-testable without a model. The
//! orchestration (load capture events → run the model → propose candidates) lives
//! in [`crate::memory::extract_capture_memory`].

use crate::chat::ChatMessage;
use serde::{Deserialize, Serialize};

/// A durable memory item the model extracted from a transcript. Restricted to
/// self-contained kinds (`decision`, `context`) so a proposed candidate never
/// depends on an entity that may not exist yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedMemory {
    /// `"decision"` or `"context"`.
    pub kind: String,
    pub title: String,
    pub content: String,
}

/// Build the extraction prompt for a model provider: read the transcript, return
/// ONLY durable engineering memory as a JSON array. Kept deliberately strict —
/// decisions/conventions/gotchas/constraints in, chit-chat and transient steps
/// out — because these become review candidates a human then approves.
pub fn build_extraction_messages(transcript: &str) -> Vec<ChatMessage> {
    let system = "You read a coding-session transcript and extract ONLY durable engineering \
         memory that a future session would need. Output a JSON array; each item is \
         {\"kind\": \"decision\" or \"context\", \"title\": a short label, \"content\": one or two \
         sentences}. Use \"decision\" for a choice plus its reasoning; use \"context\" for a durable \
         note such as a convention, gotcha, constraint, or architecture fact. IGNORE chit-chat, \
         transient steps, raw tool output, and anything not durable. If nothing durable is present, \
         output []. Output ONLY the JSON array — no prose, no markdown, no code fences.";
    vec![
        ChatMessage {
            role: "system".to_owned(),
            content: system.to_owned(),
        },
        ChatMessage {
            role: "user".to_owned(),
            content: format!("Transcript:\n{}", transcript.trim()),
        },
    ]
}

/// Parse the model's response into extracted items. Tolerant of code fences and
/// surrounding prose (small models add them): it slices out the first JSON array,
/// then keeps only well-formed items with a recognized kind and non-empty
/// title/content. Anything malformed is dropped rather than trusted.
pub fn parse_extracted_memories(response: &str) -> Vec<ExtractedMemory> {
    let Some(json) = slice_first_json_array(response) else {
        return Vec::new();
    };
    let items: Vec<serde_json::Value> = match serde_json::from_str(json) {
        Ok(items) => items,
        Err(_) => return Vec::new(),
    };
    items
        .into_iter()
        .filter_map(|item| {
            let kind = item
                .get("kind")
                .and_then(|v| v.as_str())?
                .trim()
                .to_lowercase();
            if kind != "decision" && kind != "context" {
                return None;
            }
            let title = item
                .get("title")
                .and_then(|v| v.as_str())?
                .trim()
                .to_owned();
            let content = item
                .get("content")
                .and_then(|v| v.as_str())?
                .trim()
                .to_owned();
            if title.is_empty() || content.is_empty() {
                return None;
            }
            Some(ExtractedMemory {
                kind,
                title,
                content,
            })
        })
        .collect()
}

/// Slice out the first `[ … ]` span, so a code-fenced or prose-wrapped array from
/// a small model still parses.
fn slice_first_json_array(text: &str) -> Option<&str> {
    let start = text.find('[')?;
    let end = text.rfind(']')?;
    (end > start).then(|| &text[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extraction_prompt_asks_for_durable_json_only() {
        let messages = build_extraction_messages("A: let's use SQLite for V1.");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("JSON array"));
        assert!(messages[0].content.contains("decision"));
        assert!(messages[0].content.contains("IGNORE"));
        assert!(messages[1].content.contains("let's use SQLite"));
    }

    #[test]
    fn parses_clean_array_and_drops_malformed_items() {
        let response = r#"[
            {"kind":"decision","title":"Use SQLite","content":"Chosen for V1 because it is embedded."},
            {"kind":"context","title":"Rename convention","content":"Fields are camelCase."},
            {"kind":"nonsense","title":"x","content":"y"},
            {"kind":"decision","title":"","content":"missing title"},
            {"kind":"context","title":"no content"}
        ]"#;
        let items = parse_extracted_memories(response);
        assert_eq!(
            items.len(),
            2,
            "bad kind + empty title + missing content dropped"
        );
        assert_eq!(items[0].kind, "decision");
        assert_eq!(items[0].title, "Use SQLite");
        assert_eq!(items[1].kind, "context");
    }

    #[test]
    fn tolerates_code_fences_and_prose() {
        let response = "Here is the memory:\n```json\n[{\"kind\":\"decision\",\"title\":\"T\",\"content\":\"C\"}]\n```";
        let items = parse_extracted_memories(response);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "T");
    }

    #[test]
    fn empty_or_unparseable_yields_nothing() {
        assert!(parse_extracted_memories("[]").is_empty());
        assert!(parse_extracted_memories("I found nothing durable.").is_empty());
        assert!(parse_extracted_memories("[not valid json").is_empty());
    }
}
