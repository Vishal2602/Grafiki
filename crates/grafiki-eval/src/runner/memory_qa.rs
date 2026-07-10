//! Arm B — deterministic memory-QA replay.
//!
//! Replays dated, multi-session turns through the real capture ledger, review
//! queue, trusted-memory promotion, and scoped retrieval. The judge-free primary
//! score is retrieval of the gold turn evidence; abstention and normalized gold
//! answer containment are reported separately.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::time::Instant;

use grafiki_core::{
    approve_candidate, ingest_capture_event, init_project, process_embedding_jobs,
    propose_candidate, reject_candidate, search_memory, start_capture_session,
    stop_capture_session, ApproveCandidateOptions, EvidenceInput, IngestCaptureEventOptions,
    InitOptions, ProcessEmbeddingsOptions, ProposeCandidateOptions, RejectCandidateOptions,
    SearchMemoryOptions, SearchMode, StartCaptureOptions, StopCaptureOptions,
};
use tempfile::TempDir;

use crate::config::{EvalConfig, EvalResult};
use crate::dataset::MemoryQaDataset;
use crate::metrics::ir::{self, AggregateScores, MetricConfig, Qrel, Qrels, RunList, Runs};
use crate::seed::{EVAL_PROJECT, EVAL_SCOPE};

use super::retrieval::mode_label;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApproverPolicy {
    AutoAll,
    Oracle,
    RejectAll,
}

impl ApproverPolicy {
    pub fn parse(raw: &str) -> EvalResult<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "auto-all" => Ok(Self::AutoAll),
            "oracle" => Ok(Self::Oracle),
            "reject-all" => Ok(Self::RejectAll),
            other => Err(format!(
                "unknown --approver '{other}' (expected auto-all|oracle|reject-all)"
            )
            .into()),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::AutoAll => "auto-all",
            Self::Oracle => "oracle",
            Self::RejectAll => "reject-all",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryQaOutcome {
    pub question_id: String,
    pub question_type: String,
    pub abstain_expected: bool,
    pub abstained: bool,
    pub answer_contains_gold: bool,
    pub gold_evidence: Vec<String>,
    pub retrieved_evidence: Vec<String>,
}

pub struct MemoryQaReport {
    pub dataset_name: String,
    pub mode: SearchMode,
    pub approver: ApproverPolicy,
    pub session_count: usize,
    pub question_count: usize,
    pub candidate_count: usize,
    pub approved_count: usize,
    pub rejected_count: usize,
    pub semantic_available: bool,
    pub fallback_count: usize,
    pub answerable: AggregateScores,
    pub per_question_type: BTreeMap<String, AggregateScores>,
    pub abstention_accuracy: f64,
    pub answer_contains_gold_rate: f64,
    pub outcomes: Vec<MemoryQaOutcome>,
    pub ingest_ms: u128,
    pub search_ms: u128,
}

fn candidate_matches_oracle(
    candidate: &grafiki_core::ExtractionCandidate,
    gold_evidence: &HashSet<String>,
) -> bool {
    candidate.evidence.iter().any(|evidence| {
        evidence
            .source
            .as_ref()
            .is_some_and(|source| gold_evidence.contains(source))
    })
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn run_memory_qa(
    dataset: &MemoryQaDataset,
    mode: SearchMode,
    approver: ApproverPolicy,
    cfg: &EvalConfig,
) -> EvalResult<MemoryQaReport> {
    let home = TempDir::new()?;
    let home_path = home.path().to_path_buf();
    let start_dir = home_path.clone();
    init_project(InitOptions {
        project_name: Some(EVAL_PROJECT.to_owned()),
        project_dir: start_dir.clone(),
        grafiki_home: Some(home_path.clone()),
    })?;

    let gold_evidence: HashSet<String> = dataset
        .questions
        .iter()
        .flat_map(|question| question.evidence_ids.iter().cloned())
        .collect();
    let mut candidate_count = 0usize;
    let mut approved_count = 0usize;
    let mut rejected_count = 0usize;
    let ingest_started = Instant::now();

    for session in &dataset.sessions {
        let capture = start_capture_session(StartCaptureOptions {
            project_name: Some(EVAL_PROJECT.to_owned()),
            start_dir: start_dir.clone(),
            grafiki_home: Some(home_path.clone()),
            scope: EVAL_SCOPE.to_owned(),
            source_app: Some("grafiki-eval".to_owned()),
            consent_profile: Some("eval-explicit".to_owned()),
            redaction_profile: Some("default".to_owned()),
        })?;
        let capture_id = capture.capture.id;
        let mut captured_events = Vec::new();
        for (index, turn) in session.turns.iter().enumerate() {
            let evidence_id = turn
                .turn_id
                .clone()
                .unwrap_or_else(|| format!("{}:{}", session.session_id, index + 1));
            let captured = ingest_capture_event(IngestCaptureEventOptions {
                project_name: Some(EVAL_PROJECT.to_owned()),
                start_dir: start_dir.clone(),
                grafiki_home: Some(home_path.clone()),
                capture_id: Some(capture_id.clone()),
                scope: EVAL_SCOPE.to_owned(),
                source_type: "transcript".to_owned(),
                source: Some(evidence_id),
                title: Some(format!("{} — {}", session.session_id, turn.role)),
                text: Some(turn.content.clone()),
                payload: None,
                metadata: Some(serde_json::json!({
                    "session_id": session.session_id,
                    "role": turn.role,
                })),
                privacy_level: Some("internal".to_owned()),
                redacted: false,
                captured_at: Some(session.date.clone()),
            })?;
            captured_events.push(captured.event);
        }

        // Use one reviewable context candidate per source session. This keeps
        // evidence provenance granular (the generic auto-summary intentionally
        // aggregates recent events, which is useful in-product but would blur
        // the gold session boundary in an evaluation).
        let session_content = session
            .turns
            .iter()
            .map(|turn| format!("{}: {}", turn.role, turn.content))
            .collect::<Vec<_>>()
            .join("\n");
        let evidence = captured_events
            .iter()
            .enumerate()
            .map(|(index, event)| EvidenceInput {
                source_event_id: Some(event.id.clone()),
                source_type: event.source_type.clone(),
                source: event.source.clone(),
                title: event.title.clone(),
                excerpt: event.text.clone().unwrap_or_default(),
                uri: Some(format!("grafiki://capture/{}", event.id)),
                byte_start: None,
                byte_end: None,
                // Preserve transcript order explicitly. Evidence ULIDs created
                // in the same millisecond have random suffixes and must never be
                // used as a relevance/ranking tie-break.
                line_start: Some(index as i64 + 1),
                line_end: Some(index as i64 + 1),
                captured_at: Some(event.captured_at.clone()),
            })
            .collect();
        let mutation = propose_candidate(ProposeCandidateOptions {
            project_name: Some(EVAL_PROJECT.to_owned()),
            start_dir: start_dir.clone(),
            grafiki_home: Some(home_path.clone()),
            source_type: "capture:auto".to_owned(),
            source: Some(session.session_id.clone()),
            record_type: "context".to_owned(),
            payload: serde_json::json!({
                "key": format!("eval-session-{}", session.session_id),
                "title": format!("Session memory: {}", session.session_id),
                "category": "audit",
                "content": session_content,
            }),
            scope: EVAL_SCOPE.to_owned(),
            confidence: 0.8,
            rationale: Some("Memory-QA replay candidate; review policy is explicit.".to_owned()),
            evidence,
        })?;
        candidate_count += 1;
        let should_approve = match approver {
            ApproverPolicy::AutoAll => true,
            ApproverPolicy::Oracle => candidate_matches_oracle(&mutation.candidate, &gold_evidence),
            ApproverPolicy::RejectAll => false,
        };
        if should_approve {
            approve_candidate(ApproveCandidateOptions {
                project_name: Some(EVAL_PROJECT.to_owned()),
                start_dir: start_dir.clone(),
                grafiki_home: Some(home_path.clone()),
                id: mutation.candidate.id,
            })?;
            approved_count += 1;
        } else {
            reject_candidate(RejectCandidateOptions {
                project_name: Some(EVAL_PROJECT.to_owned()),
                start_dir: start_dir.clone(),
                grafiki_home: Some(home_path.clone()),
                id: mutation.candidate.id,
                rationale: Some(format!("{} evaluation policy", approver.label())),
            })?;
            rejected_count += 1;
        }
        stop_capture_session(StopCaptureOptions {
            project_name: Some(EVAL_PROJECT.to_owned()),
            start_dir: start_dir.clone(),
            grafiki_home: Some(home_path.clone()),
            capture_id,
        })?;
    }

    if matches!(
        mode,
        SearchMode::Semantic | SearchMode::Hybrid | SearchMode::Rerank
    ) {
        process_embedding_jobs(ProcessEmbeddingsOptions {
            project_name: Some(EVAL_PROJECT.to_owned()),
            start_dir: start_dir.clone(),
            grafiki_home: Some(home_path.clone()),
            scope: "*".to_owned(),
            limit: 1_000_000,
            rebuild: false,
        })?;
    }
    let ingest_ms = ingest_started.elapsed().as_millis();

    let mut qrels: Qrels = BTreeMap::new();
    let mut runs: Runs = BTreeMap::new();
    let mut outcomes = Vec::new();
    let mut fallback_count = 0usize;
    let mut semantic_available = false;
    let mut search_ms = 0u128;

    for question in &dataset.questions {
        let started = Instant::now();
        let search = search_memory(SearchMemoryOptions {
            project_name: Some(EVAL_PROJECT.to_owned()),
            start_dir: start_dir.clone(),
            grafiki_home: Some(home_path.clone()),
            query: question.question.clone(),
            record_type: "all".to_owned(),
            mode,
            scope: EVAL_SCOPE.to_owned(),
            limit: cfg.limit,
            temporal_weight: 0.0,
        })?;
        search_ms += started.elapsed().as_millis();
        semantic_available |= search.semantic_available;
        if search.fallback.is_some() {
            fallback_count += 1;
        }

        let mut seen = HashSet::new();
        let mut retrieved_evidence: RunList = Vec::new();
        for result in &search.results {
            for evidence in &result.evidence {
                let id = evidence
                    .source
                    .clone()
                    .or_else(|| evidence.source_event_id.clone());
                if let Some(id) = id {
                    if seen.insert(id.clone()) {
                        retrieved_evidence.push(id);
                    }
                }
            }
        }

        let combined = normalize(
            &search
                .results
                .iter()
                .flat_map(|result| [result.title.as_str(), result.snippet.as_str()])
                .collect::<Vec<_>>()
                .join(" "),
        );
        let gold = normalize(&question.answer);
        let answer_contains_gold = !gold.is_empty() && combined.contains(&gold);

        // Grafiki's deterministic briefing may still include unrelated active
        // state. The safety property is whether retrieval found evidence for the
        // question; no relevant-memory result is the product's abstention signal.
        let abstained = search.results.is_empty();

        if !question.abstain {
            let qrel: Qrel = question
                .evidence_ids
                .iter()
                .map(|id| (id.clone(), 1))
                .collect();
            qrels.insert(question.question_id.clone(), qrel);
            runs.insert(question.question_id.clone(), retrieved_evidence.clone());
        }
        outcomes.push(MemoryQaOutcome {
            question_id: question.question_id.clone(),
            question_type: question.question_type.clone(),
            abstain_expected: question.abstain,
            abstained,
            answer_contains_gold,
            gold_evidence: question.evidence_ids.clone(),
            retrieved_evidence,
        });
    }

    let metric_cfg = MetricConfig::default();
    let answerable = ir::evaluate(&qrels, &runs, &metric_cfg);
    let question_types: BTreeSet<&str> = dataset
        .questions
        .iter()
        .filter(|question| !question.abstain)
        .map(|question| question.question_type.as_str())
        .collect();
    let mut per_question_type = BTreeMap::new();
    for question_type in question_types {
        let subset: Qrels = dataset
            .questions
            .iter()
            .filter(|question| !question.abstain && question.question_type == question_type)
            .filter_map(|question| {
                qrels
                    .get(&question.question_id)
                    .map(|qrel| (question.question_id.clone(), qrel.clone()))
            })
            .collect();
        per_question_type.insert(
            question_type.to_owned(),
            ir::evaluate(&subset, &runs, &metric_cfg),
        );
    }

    let abstentions: Vec<&MemoryQaOutcome> = outcomes
        .iter()
        .filter(|outcome| outcome.abstain_expected)
        .collect();
    let abstention_accuracy = if abstentions.is_empty() {
        0.0
    } else {
        abstentions
            .iter()
            .filter(|outcome| outcome.abstained)
            .count() as f64
            / abstentions.len() as f64
    };
    let answerable_outcomes: Vec<&MemoryQaOutcome> = outcomes
        .iter()
        .filter(|outcome| !outcome.abstain_expected)
        .collect();
    let answer_contains_gold_rate = if answerable_outcomes.is_empty() {
        0.0
    } else {
        answerable_outcomes
            .iter()
            .filter(|outcome| outcome.answer_contains_gold)
            .count() as f64
            / answerable_outcomes.len() as f64
    };

    // Keep the temporary project alive until all reads and report construction
    // are complete.
    drop(home);
    Ok(MemoryQaReport {
        dataset_name: dataset.name.clone(),
        mode,
        approver,
        session_count: dataset.sessions.len(),
        question_count: dataset.questions.len(),
        candidate_count,
        approved_count,
        rejected_count,
        semantic_available,
        fallback_count,
        answerable,
        per_question_type,
        abstention_accuracy,
        answer_contains_gold_rate,
        outcomes,
        ingest_ms,
        search_ms,
    })
}

pub fn report_mode(report: &MemoryQaReport) -> &'static str {
    mode_label(report.mode)
}
