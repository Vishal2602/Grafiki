//! Shared deterministic character budgets for briefings and model prompts.
//!
//! Exact tokenizer counts vary by provider, so Grafiki uses a conservative
//! four-characters-per-token envelope. Keeping this policy in one module avoids
//! unbounded session briefings and RAG prompts drifting apart.

/// Roughly 2,000 tokens for ordinary English/code context.
pub const DEFAULT_CONTEXT_BUDGET_CHARS: usize = 8_000;
/// Prevent one pathological record from consuming the whole context window.
pub const DEFAULT_ITEM_BUDGET_CHARS: usize = 900;

/// Unicode-safe deterministic truncation that preserves a visible omission
/// marker inside the requested character count.
pub fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    if max_chars == 0 {
        return String::new();
    }
    if max_chars == 1 {
        return "…".to_owned();
    }
    let mut out: String = text.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}

/// Take already-prioritized strings in order while respecting a total budget
/// and a per-item cap. Earlier items win; IDs at the start of each string remain
/// intact when long bodies are truncated.
pub fn budget_ranked_strings(
    items: impl IntoIterator<Item = String>,
    total_chars: usize,
    item_chars: usize,
) -> Vec<String> {
    let mut remaining = total_chars;
    let mut kept = Vec::new();
    for item in items {
        if remaining == 0 {
            break;
        }
        let cap = item_chars.min(remaining);
        let item = truncate_chars(item.trim(), cap);
        let used = item.chars().count();
        if used == 0 {
            continue;
        }
        kept.push(item);
        remaining = remaining.saturating_sub(used + 1);
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::{budget_ranked_strings, truncate_chars};

    #[test]
    fn truncation_is_unicode_safe_and_bounded() {
        assert_eq!(truncate_chars("abcdef", 4), "abc…");
        assert_eq!(truncate_chars("用户资料", 3), "用户…");
        assert!(truncate_chars("anything", 0).is_empty());
    }

    #[test]
    fn ranked_budget_keeps_early_ids_and_caps_large_items() {
        let kept = budget_ranked_strings(
            vec![
                format!("[observation:01A] {}", "x".repeat(200)),
                "[event:01B] ok".into(),
            ],
            80,
            60,
        );
        assert_eq!(kept.len(), 2);
        assert!(kept[0].starts_with("[observation:01A]"));
        assert!(kept.iter().map(|s| s.chars().count()).sum::<usize>() <= 80);
    }
}
