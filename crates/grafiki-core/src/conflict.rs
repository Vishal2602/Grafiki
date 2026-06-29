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

/// Attributes that hold exactly one value at a time.
const SINGLE_VALUED: &[&str] = &[
    "employer",
    "current_employer",
    "timezone",
    "marital_status",
    "status",
    "owner",
    "location",
    "city",
    "country",
    "version",
    "state",
    "role",
    "title",
    "priority",
    "stage",
    "manager",
    "email",
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

/// Compare two facts' `valid_from` timestamps. Grafiki stamps RFC3339-style
/// `%Y-%m-%dT%H:%M:%SZ`, which is lexically sortable.
pub fn temporal_relation(existing_valid_from: &str, incoming_valid_from: &str) -> TemporalRelation {
    if existing_valid_from.trim().is_empty() || incoming_valid_from.trim().is_empty() {
        return TemporalRelation::Unknown;
    }
    match incoming_valid_from.cmp(existing_valid_from) {
        std::cmp::Ordering::Greater => TemporalRelation::Succession,
        std::cmp::Ordering::Equal => TemporalRelation::Concurrent,
        std::cmp::Ordering::Less => TemporalRelation::Unknown,
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
/// transcript, which outranks an automatic extraction.
pub fn source_priority(source_type: &str) -> u8 {
    let s = source_type.to_lowercase();
    if s.contains("manual") || s.contains("human") {
        3
    } else if s.contains("transcript") || s.contains("decision") {
        2
    } else {
        1
    }
}

/// Metadata needed to arbitrate a supersession.
#[derive(Debug, Clone)]
pub struct FactMeta {
    pub valid_from: String,
    pub source_type: String,
    pub confidence: f64,
}

/// Decide who wins, lexicographically: source-priority, then recency, then
/// confidence. A strictly-higher-trust trusted fact is never auto-superseded by a
/// lower-trust incoming one (routed to review instead).
pub fn arbitrate(trusted: &FactMeta, incoming: &FactMeta) -> (Winner, ArbitrationBasis) {
    let trusted_tier = source_priority(&trusted.source_type);
    let incoming_tier = source_priority(&incoming.source_type);
    if trusted_tier > incoming_tier {
        return (Winner::Review, ArbitrationBasis::SourcePriority);
    }

    match temporal_relation(&trusted.valid_from, &incoming.valid_from) {
        TemporalRelation::Succession => return (Winner::Incoming, ArbitrationBasis::Recency),
        TemporalRelation::Unknown
            if !trusted.valid_from.is_empty() && !incoming.valid_from.is_empty() =>
        {
            // Incoming is strictly older than trusted → trusted stands.
            return (Winner::Trusted, ArbitrationBasis::Recency);
        }
        _ => {} // concurrent or unknown timestamps → fall through to confidence
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
