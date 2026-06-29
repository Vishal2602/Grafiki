//! H2 — deterministic conflict / contradiction detection and arbitration.
//!
//! This module is the **decision** half of conflict resolution: given a candidate
//! fact and a related trusted fact (already narrowed by the embedding gate, which
//! lives in the search layer), decide whether they conflict and, if so, who wins
//! — using only structure and metadata, never an LLM.
//!
//! Design (see `docs/CONFLICT_DESIGN.md`):
//! - **Embedding similarity is a candidate-generation gate, never a contradiction
//!   signal** — so it lives in the caller; this module only sees pairs already
//!   judged "about the same thing".
//! - **Cardinality registry is the false-positive guard.** A different value for a
//!   *single-valued* attribute (`employer`, `status`) is a conflict; a different
//!   value for a *multi-valued* one (`tag`, `language`) is coexistence, never a
//!   conflict; unknown cardinality routes to human review, not auto-apply.
//! - **Arbitrate on metadata** — source-priority → recency → confidence — not an
//!   LLM's judgment of which fact is "fresher". A higher-trust trusted fact is
//!   never silently overwritten by a lower-trust auto-extraction.

/// How many distinct values an attribute may simultaneously hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    /// Exactly one value at a time — a different value supersedes (e.g. `employer`).
    Single,
    /// Many values coexist — a different value is *not* a conflict (e.g. `tag`).
    Multi,
    /// Not in the registry — cannot auto-decide; route to review.
    Unknown,
}

/// The verdict for a candidate-vs-trusted pair already known to share a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictVerdict {
    /// Same slot, incompatible value, structurally certain → safe to auto-supersede.
    AutoSupersede,
    /// Identical value, different slot, or a multi-valued attribute → no conflict.
    NoConflict,
    /// Same slot but cardinality/normalization is uncertain → human review.
    Review,
}

/// Attributes that hold exactly one value at a time. Entries must already be in
/// `normalize_predicate` canonical form (a unit test enforces this). Ambiguous
/// attributes that *can* legitimately hold several values (email, role, title)
/// are deliberately absent → they route to review rather than auto-supersede.
const SINGLE_VALUED: &[&str] = &[
    "employer",
    "timezone",
    "marital_status",
    "status",
    "owner",
    "location",
    "city",
    "country",
    "version",
    "state",
    "priority",
    "stage",
    "manager",
];

/// Attributes that legitimately hold many values at once.
const MULTI_VALUED: &[&str] = &[
    "tag",
    "tags",
    "language",
    "speaks_language",
    "skill",
    "skills",
    "member",
    "members",
    "visited",
    "alias",
    "aliases",
    "label",
    "labels",
    "dependency",
    "depends_on",
    "uses",
];

/// Cardinality of a (normalized) attribute name.
pub fn attribute_cardinality(predicate: &str) -> Cardinality {
    let p = normalize_predicate(predicate);
    if SINGLE_VALUED.contains(&p.as_str()) {
        Cardinality::Single
    } else if MULTI_VALUED.contains(&p.as_str()) {
        Cardinality::Multi
    } else {
        Cardinality::Unknown
    }
}

/// Normalize a value for comparison: trim, collapse internal whitespace, lowercase.
pub fn normalize_value(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Normalize an attribute/predicate name and fold a few common synonyms so the
/// same slot under two spellings (`works_at` / `employer`) compares equal.
pub fn normalize_predicate(predicate: &str) -> String {
    let p = predicate.trim().to_lowercase().replace([' ', '-'], "_");
    match p.as_str() {
        "works_at" | "works_for" | "company" | "current_employer" => "employer".to_string(),
        "tz" => "timezone".to_string(),
        "located_in" | "based_in" => "location".to_string(),
        other => other.to_string(),
    }
}

/// Normalize a record key (state/context) for exact supersession matching.
pub fn normalize_key(key: &str) -> String {
    key.trim().to_lowercase()
}

/// A normalized `(subject, predicate, value)` triple for slot comparison.
#[derive(Debug, Clone)]
pub struct Slot {
    pub subject: String,
    pub predicate: String,
    pub value: String,
}

/// Key supersession (1.1): a non-empty new key equal (normalized) to an existing
/// key supersedes by definition.
pub fn key_conflict(existing_key: &str, new_key: &str) -> bool {
    !new_key.trim().is_empty() && normalize_key(existing_key) == normalize_key(new_key)
}

/// Structural slot conflict (1.2): same subject + predicate, incompatible value,
/// gated by the cardinality registry.
pub fn slot_conflict(existing: &Slot, incoming: &Slot) -> ConflictVerdict {
    // Different slot → not a conflict at all.
    if normalize_value(&existing.subject) != normalize_value(&incoming.subject)
        || normalize_predicate(&existing.predicate) != normalize_predicate(&incoming.predicate)
    {
        return ConflictVerdict::NoConflict;
    }
    // Same value → idempotent restatement, not a conflict.
    if normalize_value(&existing.value) == normalize_value(&incoming.value) {
        return ConflictVerdict::NoConflict;
    }
    match attribute_cardinality(&incoming.predicate) {
        Cardinality::Single => ConflictVerdict::AutoSupersede,
        Cardinality::Multi => ConflictVerdict::NoConflict,
        Cardinality::Unknown => ConflictVerdict::Review,
    }
}

/// Temporal relation between two facts by valid-from timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporalRelation {
    /// Incoming is strictly later — a forward succession (safe to close the old window).
    Succession,
    /// Same timestamp — concurrent; decide by slot/arbitration, not time.
    Concurrent,
    /// Incoming is older, or a timestamp is missing — do not treat as supersession.
    Unknown,
}

/// Parse a timestamp into a comparable `[year, month, day, hour, min, sec]`.
///
/// Tolerant of common RFC3339 variants (the `T` or space separator, fractional
/// seconds, a trailing `Z` or offset) so a non-canonical caller-supplied
/// `captured_at` cannot silently invert recency the way a raw byte comparison
/// would. **Assumes UTC** — a numeric offset is ignored, not applied (Grafiki
/// stamps `…Z`). Returns `None` if fewer than three numeric fields are present
/// (no date), so unparseable input is treated as "unknown time", never reordered.
fn parse_timestamp(value: &str) -> Option<[i64; 6]> {
    let nums: Vec<i64> = value
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.parse::<i64>().ok())
        .collect();
    if nums.len() < 3 {
        return None;
    }
    let mut out = [0i64; 6];
    for (slot, n) in out.iter_mut().zip(nums.iter()) {
        *slot = *n;
    }
    Some(out)
}

/// Compare two facts' `valid_from` timestamps by parsed instant (NOT lexically —
/// see [`parse_timestamp`]). Unparseable input on either side ⇒ `Unknown`.
pub fn temporal_relation(existing_valid_from: &str, incoming_valid_from: &str) -> TemporalRelation {
    match (
        parse_timestamp(existing_valid_from),
        parse_timestamp(incoming_valid_from),
    ) {
        (Some(existing), Some(incoming)) => match incoming.cmp(&existing) {
            std::cmp::Ordering::Greater => TemporalRelation::Succession,
            std::cmp::Ordering::Equal => TemporalRelation::Concurrent,
            std::cmp::Ordering::Less => TemporalRelation::Unknown,
        },
        _ => TemporalRelation::Unknown,
    }
}

/// Why a winner was chosen (recorded on every supersession for explainability).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArbitrationBasis {
    SourcePriority,
    Recency,
    Confidence,
    Tie,
}

/// Which fact wins arbitration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Winner {
    /// The new fact supersedes the trusted one.
    Incoming,
    /// The trusted fact stands; the new one does not supersede.
    Trusted,
    /// Indeterminate → route to human review.
    Review,
}

/// Source-trust tier (higher = more trusted). Human/manual entry outranks a
/// transcript, which outranks an automatic extraction (the default for anything
/// unrecognized, including `session:`/`connector:` provenance).
///
/// Matches on whole, `:`/`-`/`_`-delimited tokens by **exact equality** — never a
/// substring — so e.g. `humanitarian` does not read as `human`, nor
/// `transcription_service` as `transcript`.
pub fn source_priority(source_type: &str) -> u8 {
    const TIER3: &[&str] = &["manual", "human"];
    const TIER2: &[&str] = &["transcript", "decision"];
    let lower = source_type.to_lowercase();
    let mut tier = 1u8;
    for token in lower.split(|c: char| !c.is_ascii_alphanumeric()) {
        if token.is_empty() {
            continue;
        }
        if TIER3.contains(&token) {
            return 3;
        }
        if TIER2.contains(&token) {
            tier = tier.max(2);
        }
    }
    tier
}

/// Metadata needed to arbitrate a supersession.
#[derive(Debug, Clone)]
pub struct FactMeta {
    pub valid_from: String,
    pub source_type: String,
    pub confidence: f64,
}

/// Decide who wins, lexicographically: source-priority, then recency, then
/// confidence.
///
/// Source-priority is **symmetric and dominant**: a strictly-higher-trust trusted
/// fact is never auto-superseded by a lower-trust incoming one (routed to
/// review), and a strictly-higher-trust *incoming* fact (e.g. a human correction)
/// wins outright regardless of recency. Only same-tier facts fall through to
/// recency, then to confidence.
pub fn arbitrate(trusted: &FactMeta, incoming: &FactMeta) -> (Winner, ArbitrationBasis) {
    let trusted_tier = source_priority(&trusted.source_type);
    let incoming_tier = source_priority(&incoming.source_type);
    if trusted_tier > incoming_tier {
        return (Winner::Review, ArbitrationBasis::SourcePriority);
    }
    if incoming_tier > trusted_tier {
        return (Winner::Incoming, ArbitrationBasis::SourcePriority);
    }

    // Same tier → recency by parsed instant (missing/unparseable times skip to
    // confidence rather than being mistaken for an ordering).
    match (
        parse_timestamp(&trusted.valid_from),
        parse_timestamp(&incoming.valid_from),
    ) {
        (Some(t), Some(i)) if i > t => return (Winner::Incoming, ArbitrationBasis::Recency),
        (Some(t), Some(i)) if i < t => return (Winner::Trusted, ArbitrationBasis::Recency),
        _ => {}
    }

    let eps = 1e-9;
    if incoming.confidence > trusted.confidence + eps {
        (Winner::Incoming, ArbitrationBasis::Confidence)
    } else if trusted.confidence > incoming.confidence + eps {
        (Winner::Trusted, ArbitrationBasis::Confidence)
    } else {
        (Winner::Review, ArbitrationBasis::Tie)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(subject: &str, predicate: &str, value: &str) -> Slot {
        Slot {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn key_conflict_matches_normalized_keys() {
        assert!(key_conflict("Deploy Target", "deploy target"));
        assert!(key_conflict("api-base", "  api-base  "));
        assert!(!key_conflict("deploy-target", "rollback-plan"));
        assert!(!key_conflict("anything", "")); // empty new key is not a conflict
    }

    #[test]
    fn single_valued_different_value_auto_supersedes() {
        let v = slot_conflict(
            &slot("Alice", "employer", "Acme Corp"),
            &slot("alice", "employer", "Globex"),
        );
        assert_eq!(v, ConflictVerdict::AutoSupersede);
        // A real antonym/status flip is a genuine conflict under a single-valued slot.
        assert_eq!(
            slot_conflict(
                &slot("svc", "status", "alive"),
                &slot("svc", "status", "dead")
            ),
            ConflictVerdict::AutoSupersede
        );
    }

    #[test]
    fn multi_valued_different_value_coexists() {
        let v = slot_conflict(
            &slot("Alice", "language", "English"),
            &slot("Alice", "language", "French"),
        );
        assert_eq!(v, ConflictVerdict::NoConflict);
        assert_eq!(
            slot_conflict(&slot("p", "tag", "urgent"), &slot("p", "tag", "backend")),
            ConflictVerdict::NoConflict
        );
    }

    #[test]
    fn unknown_cardinality_routes_to_review() {
        assert_eq!(
            slot_conflict(
                &slot("x", "favorite_color", "blue"),
                &slot("x", "favorite_color", "red")
            ),
            ConflictVerdict::Review
        );
    }

    #[test]
    fn idempotent_and_cross_slot_are_no_conflict() {
        // Same value, even if single-valued → not a conflict.
        assert_eq!(
            slot_conflict(
                &slot("a", "employer", "Acme"),
                &slot("a", "employer", "ACME")
            ),
            ConflictVerdict::NoConflict
        );
        // Different subject.
        assert_eq!(
            slot_conflict(
                &slot("a", "employer", "Acme"),
                &slot("b", "employer", "Globex")
            ),
            ConflictVerdict::NoConflict
        );
        // Different predicate.
        assert_eq!(
            slot_conflict(
                &slot("a", "employer", "Acme"),
                &slot("a", "timezone", "PST")
            ),
            ConflictVerdict::NoConflict
        );
    }

    #[test]
    fn predicate_synonyms_fold_to_same_slot() {
        // "works_at" and "employer" are the same slot → a real conflict.
        assert_eq!(
            slot_conflict(
                &slot("a", "works_at", "Acme"),
                &slot("a", "employer", "Globex")
            ),
            ConflictVerdict::AutoSupersede
        );
    }

    #[test]
    fn temporal_relation_orders_by_timestamp() {
        assert_eq!(
            temporal_relation("2026-01-01T00:00:00Z", "2026-06-01T00:00:00Z"),
            TemporalRelation::Succession
        );
        assert_eq!(
            temporal_relation("2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"),
            TemporalRelation::Concurrent
        );
        assert_eq!(
            temporal_relation("2026-06-01T00:00:00Z", "2026-01-01T00:00:00Z"),
            TemporalRelation::Unknown
        );
        assert_eq!(
            temporal_relation("", "2026-01-01T00:00:00Z"),
            TemporalRelation::Unknown
        );
    }

    #[test]
    fn temporal_relation_is_robust_to_format_drift() {
        // A space separator + 5 seconds newer must NOT lexically invert (' ' < 'T').
        assert_eq!(
            temporal_relation("2026-06-29T10:00:00Z", "2026-06-29 10:00:05"),
            TemporalRelation::Succession
        );
        // Fractional seconds compare as concurrent (sub-second precision dropped),
        // not as older ('.' < 'Z' would have inverted a raw byte comparison).
        assert_eq!(
            temporal_relation("2026-06-29T10:00:00Z", "2026-06-29T10:00:00.500Z"),
            TemporalRelation::Concurrent
        );
        // Unparseable input is "unknown time", never reordered.
        assert_eq!(
            temporal_relation("not-a-date", "2026-06-29T10:00:00Z"),
            TemporalRelation::Unknown
        );
    }

    #[test]
    fn source_priority_matches_whole_tokens_only() {
        assert_eq!(source_priority("manual"), 3);
        assert_eq!(source_priority("human"), 3);
        assert_eq!(source_priority("transcript"), 2);
        assert_eq!(source_priority("decision"), 2);
        assert_eq!(source_priority("connector:test"), 1);
        assert_eq!(source_priority("session:01ABC"), 1);
        assert_eq!(source_priority("agent"), 1);
        // No substring collisions.
        assert_eq!(source_priority("humanitarian"), 1);
        assert_eq!(source_priority("transcription_service"), 1);
    }

    #[test]
    fn higher_trust_incoming_wins_over_recency() {
        // A newer low-trust trusted fact must yield to an older human correction.
        let trusted = FactMeta {
            valid_from: "2026-06-01T00:00:00Z".into(),
            source_type: "transcript".into(),
            confidence: 0.5,
        };
        let incoming = FactMeta {
            valid_from: "2026-01-01T00:00:00Z".into(), // older, but human
            source_type: "manual".into(),
            confidence: 0.9,
        };
        assert_eq!(
            arbitrate(&trusted, &incoming),
            (Winner::Incoming, ArbitrationBasis::SourcePriority)
        );
    }

    #[test]
    fn registry_entries_are_canonical() {
        for entry in SINGLE_VALUED.iter().chain(MULTI_VALUED.iter()) {
            assert_eq!(
                &normalize_predicate(entry),
                entry,
                "registry entry '{entry}' is not in normalize_predicate canonical form"
            );
        }
    }

    #[test]
    fn arbitration_protects_higher_trust_facts() {
        // Lower-trust auto-extraction must not silently overwrite a human fact.
        let trusted = FactMeta {
            valid_from: "2026-01-01T00:00:00Z".into(),
            source_type: "manual".into(),
            confidence: 0.9,
        };
        let incoming = FactMeta {
            valid_from: "2026-06-01T00:00:00Z".into(), // newer, but lower trust
            source_type: "auto-extract".into(),
            confidence: 0.6,
        };
        assert_eq!(
            arbitrate(&trusted, &incoming),
            (Winner::Review, ArbitrationBasis::SourcePriority)
        );
    }

    #[test]
    fn arbitration_recency_wins_within_tier() {
        let trusted = FactMeta {
            valid_from: "2026-01-01T00:00:00Z".into(),
            source_type: "transcript".into(),
            confidence: 0.8,
        };
        let incoming = FactMeta {
            valid_from: "2026-06-01T00:00:00Z".into(),
            source_type: "transcript".into(),
            confidence: 0.5,
        };
        assert_eq!(
            arbitrate(&trusted, &incoming),
            (Winner::Incoming, ArbitrationBasis::Recency)
        );
        // Older incoming → trusted stands.
        assert_eq!(
            arbitrate(&incoming, &trusted),
            (Winner::Trusted, ArbitrationBasis::Recency)
        );
    }

    #[test]
    fn arbitration_confidence_breaks_recency_tie() {
        let trusted = FactMeta {
            valid_from: "2026-01-01T00:00:00Z".into(),
            source_type: "transcript".into(),
            confidence: 0.5,
        };
        let incoming = FactMeta {
            valid_from: "2026-01-01T00:00:00Z".into(),
            source_type: "transcript".into(),
            confidence: 0.9,
        };
        assert_eq!(
            arbitrate(&trusted, &incoming),
            (Winner::Incoming, ArbitrationBasis::Confidence)
        );
        // Full tie → review.
        let tie = FactMeta {
            valid_from: "2026-01-01T00:00:00Z".into(),
            source_type: "transcript".into(),
            confidence: 0.5,
        };
        assert_eq!(
            arbitrate(&trusted, &tie),
            (Winner::Review, ArbitrationBasis::Tie)
        );
    }
}
