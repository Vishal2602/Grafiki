//! H5 — Reflection / consolidation: the **pure** extractive community-summary core.
//!
//! This module is I/O-free, model-free, and deterministic. Given a community's
//! members and their (already-redacted) observations, it produces a verbatim,
//! template-arranged summary — nothing is generated. The DB orchestration
//! (`run_reflection`) lives in `memory.rs`, mirroring how `conflict.rs`/`graph.rs`
//! are pure cores that `memory.rs` drives.
//!
//! Determinism (the trust invariant: same store ⇒ byte-identical output) is held by:
//! - ranking observations by a **rounded** `confidence × within-community salience`
//!   with a fully lexical tie-break (so float jitter cannot reorder near-ties);
//! - a `dedup_key` derived from membership + the *set* of source-observation ids,
//!   **not** from the rendered text or any PageRank order (so re-ranking never mints
//!   a new candidate — design §0/C5);
//! - keyword extraction that mirrors `fts5_terms_query`'s exact split predicate with
//!   an explicit lowercase, char-length filter, and a sorted built-in stop-list.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Default minimum community size to summarize (a lone entity is not a "theme").
pub const DEFAULT_MIN_COMMUNITY_SIZE: usize = 2;
/// Default maximum community size to summarize; a larger community is skipped
/// (reported `skipped_too_large`) rather than yielding one meaningless mega-summary
/// (design §0/C6).
pub const DEFAULT_MAX_COMMUNITY_SIZE: usize = 25;
/// Default cap on representative observations kept per summary (scannable output).
pub const DEFAULT_MAX_OBS_PER_SUMMARY: usize = 8;
/// Number of extracted keywords per summary.
pub const KEYWORD_COUNT: usize = 12;
/// Max characters in the comma-joined theme label.
pub const MAX_LABEL_LEN: usize = 80;
/// Decimals the ranking score is rounded to before sorting (kills float jitter).
const SCORE_DECIMALS: i32 = 6;

#[derive(Debug, Clone)]
pub struct RunReflectionOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    /// The single scope to reflect over (v1 is single-scope — design §0/C7).
    pub scope: String,
    pub min_community_size: usize,
    pub max_community_size: usize,
    pub max_obs_per_summary: usize,
    /// Bypass the dedup check (re-propose even if an equivalent candidate exists).
    pub force: bool,
}

impl RunReflectionOptions {
    /// Construct with the documented defaults for the size/cap knobs.
    pub fn new(scope: impl Into<String>, start_dir: PathBuf) -> Self {
        Self {
            project_name: None,
            start_dir,
            grafiki_home: None,
            scope: scope.into(),
            min_community_size: DEFAULT_MIN_COMMUNITY_SIZE,
            max_community_size: DEFAULT_MAX_COMMUNITY_SIZE,
            max_obs_per_summary: DEFAULT_MAX_OBS_PER_SUMMARY,
            force: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectionReport {
    pub project: String,
    pub scope: String,
    /// All communities found, including singletons.
    pub communities_detected: usize,
    /// Communities that met the size + has-observations bar and were summarized.
    pub communities_summarized: usize,
    pub candidates_created: usize,
    /// Communities skipped because an equivalent candidate already exists.
    pub skipped_existing: usize,
    /// Communities skipped because they exceeded `max_community_size`.
    pub skipped_too_large: usize,
    pub details: Vec<CommunityDetail>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommunityDetail {
    pub community_id: usize,
    pub member_entity_ids: Vec<String>,
    pub member_entity_names: Vec<String>,
    /// Kept (post-cap) representative observation count.
    pub observation_count: usize,
    /// The community's modularity contribution `Q_c` (cohesion); higher ⇒ tighter.
    pub modularity_contribution: f64,
    pub dedup_key: String,
    /// `Some` when a candidate was created; `None` when skipped.
    pub candidate_id: Option<String>,
    /// `created` | `skipped_existing` | `skipped_too_small` | `skipped_too_large` |
    /// `skipped_no_observations`.
    pub status: String,
}

/// One source observation feeding the summarizer. `content` MUST already be redacted
/// by the caller (the orchestrator runs `redact_text` at load — design §0/C1).
#[derive(Debug, Clone, PartialEq)]
pub struct SourceObservation {
    pub observation_id: String,
    pub entity_id: String,
    pub entity_name: String,
    pub content: String,
    pub category: String,
    pub confidence: f64,
    /// Within-community weighted degree of the owning entity (stable, local — not PPR).
    pub salience: f64,
}

/// The deterministic output of [`build_summary`].
#[derive(Debug, Clone, PartialEq)]
pub struct SummaryDraft {
    pub title: String,
    pub content: String,
    pub keywords: Vec<String>,
    /// Representative observations kept (post-cap), in display order — one evidence
    /// link each.
    pub kept: Vec<SourceObservation>,
    pub dedup_key: String,
}

/// Fixed precedence favoring decisional/structural facts; every `observations`
/// category CHECK value is covered, unknown ⇒ 99.
fn category_rank(category: &str) -> u8 {
    match category {
        "decision" => 0,
        "architecture" => 1,
        "risk" => 2,
        "pattern" => 3,
        "convention" => 4,
        "gotcha" => 5,
        "learned" => 6,
        "dependency" => 7,
        "blocker" => 8,
        "preference" => 9,
        "progress" => 10,
        "general" => 11,
        _ => 99,
    }
}

/// Common English + code stop-words. ASCII, sorted + unique so `is_stopword` can
/// `binary_search`; auditable and stable (design §0/C8).
const STOPWORDS: &[&str] = &[
    "a", "about", "above", "after", "again", "all", "also", "am", "an", "and", "any", "are", "as",
    "at", "be", "because", "been", "before", "being", "below", "between", "both", "but", "by",
    "can", "could", "did", "do", "does", "doing", "done", "down", "during", "each", "few", "for",
    "from", "further", "had", "has", "have", "having", "he", "her", "here", "hers", "him", "his",
    "how", "if", "in", "into", "is", "it", "its", "just", "may", "me", "might", "more", "most",
    "must", "my", "no", "nor", "not", "now", "of", "off", "on", "once", "only", "or", "other",
    "our", "out", "over", "own", "per", "same", "she", "should", "so", "some", "such", "than",
    "that", "the", "their", "them", "then", "there", "these", "they", "this", "those", "through",
    "to", "too", "under", "until", "up", "use", "used", "uses", "very", "was", "we", "were",
    "what", "when", "where", "which", "while", "who", "whom", "why", "will", "with", "would",
    "you", "your",
];

fn is_stopword(token: &str) -> bool {
    STOPWORDS.binary_search(&token).is_ok()
}

/// Split exactly like `fts5_terms_query` (non-alphanumeric except `_`/`-`), then
/// lowercase and keep tokens of **char length > 1** (design §0/C8).
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .map(str::trim)
        .filter(|t| t.chars().count() > 1)
        .map(|t| t.to_lowercase())
        .collect()
}

/// Top-`n` TF terms across `texts`, dropping stop-words; sorted by
/// `(count desc, token asc)`. Raw TF (not TF-IDF) so output is local to the community.
fn extract_keywords(texts: &[String], n: usize) -> Vec<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for text in texts {
        for token in tokenize(text) {
            if is_stopword(&token) {
                continue;
            }
            *counts.entry(token).or_insert(0) += 1;
        }
    }
    let mut ranked: Vec<(String, usize)> = counts.into_iter().collect();
    // BTreeMap gave us token-ascending order; a stable sort by count desc keeps that
    // as the tie-break.
    ranked.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    ranked.into_iter().take(n).map(|(token, _)| token).collect()
}

/// Comma-join `names` (lexicographic), truncated to `MAX_LABEL_LEN` chars on a name
/// boundary (never mid-name); appends `…` when truncated.
fn theme_label(names: &[String]) -> String {
    let mut sorted: Vec<&String> = names.iter().collect();
    sorted.sort();
    let mut label = String::new();
    let mut truncated = false;
    for name in sorted {
        let candidate = if label.is_empty() {
            name.clone()
        } else {
            format!("{label}, {name}")
        };
        if candidate.chars().count() > MAX_LABEL_LEN && !label.is_empty() {
            truncated = true;
            break;
        }
        label = candidate;
    }
    if truncated {
        label.push('…');
    }
    label
}

/// `sha256(scope ∥ sorted member ids ∥ sorted source-observation id SET)`, hex, 16
/// chars. Depends ONLY on membership + the set of source facts — never on the rendered
/// text or any PageRank order (design §0/C5), so re-runs are idempotent under re-ranking.
pub fn community_dedup_key(
    scope: &str,
    member_ids: &[String],
    observation_ids: &[String],
) -> String {
    let mut members = member_ids.to_vec();
    members.sort();
    let mut obs = observation_ids.to_vec();
    obs.sort();
    obs.dedup();
    let mut hasher = Sha256::new();
    hasher.update(scope.as_bytes());
    hasher.update([0x01]);
    hasher.update(members.join("\u{1}").as_bytes());
    hasher.update([0x01]);
    hasher.update(obs.join("\u{1}").as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    digest[..16].to_string()
}

/// Round to `SCORE_DECIMALS` so last-bit float differences never reorder near-ties.
fn rounded_score(confidence: f64, salience: f64) -> f64 {
    let factor = 10f64.powi(SCORE_DECIMALS);
    (confidence * salience * factor).round() / factor
}

/// Build the deterministic extractive summary for one community.
///
/// `member_ids`/`member_names` are aligned and cover the whole community.
/// `observations` is every currently-valid, already-redacted observation across the
/// members. The `dedup_key` is computed over the FULL observation set; the rendered
/// `content` uses only the top `max_obs` ranked representatives.
pub fn build_summary(
    scope: &str,
    member_ids: &[String],
    member_names: &[String],
    observations: &[SourceObservation],
    max_obs: usize,
) -> SummaryDraft {
    let all_obs_ids: Vec<String> = observations
        .iter()
        .map(|o| o.observation_id.clone())
        .collect();
    let dedup_key = community_dedup_key(scope, member_ids, &all_obs_ids);

    // Rank: score desc, then a fully lexical tie-break (category, entity, obs id).
    let mut ranked: Vec<&SourceObservation> = observations.iter().collect();
    ranked.sort_by(|a, b| {
        let sa = rounded_score(a.confidence, a.salience);
        let sb = rounded_score(b.confidence, b.salience);
        sb.partial_cmp(&sa)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| category_rank(&a.category).cmp(&category_rank(&b.category)))
            .then_with(|| a.entity_id.cmp(&b.entity_id))
            .then_with(|| a.observation_id.cmp(&b.observation_id))
    });
    let kept: Vec<SourceObservation> = ranked.into_iter().take(max_obs).cloned().collect();

    let label = theme_label(member_names);
    let title = format!("Theme: {label}");

    let kept_texts: Vec<String> = kept.iter().map(|o| o.content.clone()).collect();
    let keywords = extract_keywords(&kept_texts, KEYWORD_COUNT);

    let mut content = format!(
        "Community theme across {} entities: {}.\n\nKey facts:",
        member_names.len(),
        label
    );
    for obs in &kept {
        content.push_str(&format!("\n- [{}] {}", obs.entity_name, obs.content));
    }
    if !keywords.is_empty() {
        content.push_str(&format!("\n\nKeywords: {}.", keywords.join(", ")));
    }

    SummaryDraft {
        title,
        content,
        keywords,
        kept,
        dedup_key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(
        id: &str,
        ent: &str,
        name: &str,
        content: &str,
        cat: &str,
        conf: f64,
        sal: f64,
    ) -> SourceObservation {
        SourceObservation {
            observation_id: id.to_string(),
            entity_id: ent.to_string(),
            entity_name: name.to_string(),
            content: content.to_string(),
            category: cat.to_string(),
            confidence: conf,
            salience: sal,
        }
    }

    fn sample() -> (Vec<String>, Vec<String>, Vec<SourceObservation>) {
        let ids = vec!["auth-service".to_string(), "jwt-library".to_string()];
        let names = vec!["Auth Service".to_string(), "JWT Library".to_string()];
        let observations = vec![
            obs(
                "o3",
                "jwt-library",
                "JWT Library",
                "JWT refresh tokens rotate every 15 minutes.",
                "architecture",
                0.9,
                2.0,
            ),
            obs(
                "o1",
                "auth-service",
                "Auth Service",
                "Auth endpoints rate-limited to 10 rps per IP.",
                "decision",
                0.8,
                2.0,
            ),
            obs(
                "o2",
                "auth-service",
                "Auth Service",
                "Session cookies are HttpOnly and Secure.",
                "convention",
                0.7,
                2.0,
            ),
        ];
        (ids, names, observations)
    }

    #[test]
    fn build_summary_is_deterministic() {
        let (ids, names, observations) = sample();
        let a = build_summary("eval", &ids, &names, &observations, 8);
        // Re-run with a different INPUT ORDER — output must be byte-identical.
        let mut shuffled = observations.clone();
        shuffled.reverse();
        let b = build_summary("eval", &ids, &names, &shuffled, 8);
        assert_eq!(a, b);
    }

    #[test]
    fn dedup_key_ignores_ranking_but_tracks_facts() {
        let (ids, names, observations) = sample();
        let base = build_summary("eval", &ids, &names, &observations, 8);

        // Same facts, salience perturbed (simulating global PPR jitter): SAME key.
        let mut jittered = observations.clone();
        for o in &mut jittered {
            o.salience += 0.0001;
        }
        let same = build_summary("eval", &ids, &names, &jittered, 8);
        assert_eq!(
            base.dedup_key, same.dedup_key,
            "re-ranking must not change the dedup key"
        );

        // Add a new source fact: key MUST change.
        let mut more = observations.clone();
        more.push(obs(
            "o9",
            "jwt-library",
            "JWT Library",
            "Tokens are blacklisted on logout.",
            "pattern",
            0.9,
            2.0,
        ));
        let changed = build_summary("eval", &ids, &names, &more, 8);
        assert_ne!(
            base.dedup_key, changed.dedup_key,
            "a new source fact must change the dedup key"
        );

        // Different scope: different key.
        let other_scope = build_summary("other", &ids, &names, &observations, 8);
        assert_ne!(base.dedup_key, other_scope.dedup_key);
    }

    #[test]
    fn ranking_prefers_high_score_then_category() {
        let (ids, names, observations) = sample();
        let draft = build_summary("eval", &ids, &names, &observations, 8);
        // o1 (decision, 0.8) and o3 (architecture, 0.9) — o3 has higher score (0.9*2).
        assert_eq!(draft.kept[0].observation_id, "o3");
        // Cap respected.
        let capped = build_summary("eval", &ids, &names, &observations, 2);
        assert_eq!(capped.kept.len(), 2);
    }

    #[test]
    fn keywords_are_deterministic_and_drop_stopwords() {
        let texts = vec![
            "Token rotation and token expiry are handled by the token service.".to_string(),
            "The service rotates tokens.".to_string(),
        ];
        let kw = extract_keywords(&texts, 5);
        assert!(kw.contains(&"token".to_string()));
        assert!(!kw.contains(&"the".to_string()), "stop-words dropped");
        assert!(!kw.contains(&"and".to_string()));
        assert_eq!(
            kw,
            extract_keywords(&texts, 5),
            "byte-identical across runs"
        );
    }

    #[test]
    fn stopwords_list_is_sorted_for_binary_search() {
        assert!(
            STOPWORDS.windows(2).all(|w| w[0] < w[1]),
            "STOPWORDS must be sorted + unique"
        );
    }

    #[test]
    fn theme_label_truncates_on_name_boundary() {
        let names: Vec<String> = (0..20)
            .map(|i| format!("EntityWithALongName{i:02}"))
            .collect();
        let label = theme_label(&names);
        assert!(
            label.chars().count() <= MAX_LABEL_LEN + 1,
            "label respects MAX_LABEL_LEN (+ ellipsis)"
        );
        assert!(label.ends_with('…'));
        // Never cuts mid-name: the truncated label's last full name is intact.
        assert!(!label
            .trim_end_matches('…')
            .ends_with("EntityWithALongName0"));
    }
}
