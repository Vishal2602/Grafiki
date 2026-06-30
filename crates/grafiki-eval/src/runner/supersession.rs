//! Arm D — supersession & knowledge-update.
//!
//! For each item: ingest a fact, then an update (or retraction) through a real
//! mechanism, then probe. Assert the NEW fact is surfaced and the STALE fact is
//! suppressed (mechanism-specific: observations are excluded from search by
//! `valid_to`; superseded decisions stay searchable but drop out of the *active*
//! set), and that retractions cause abstention. Everything is decidable from
//! record state + literal token match — no judge.

use grafiki_core::{
    approve_candidate, ask_memory, delete_observation, init_project, list_decisions, log_decision,
    propose_candidate, search_memory, ApproveCandidateOptions, AskMemoryOptions,
    DecisionListOptions, DeleteObservationOptions, InitOptions, LogDecisionOptions,
    ProposeCandidateOptions, SearchMemoryOptions, SearchMode,
};
use serde_json::json;
use tempfile::TempDir;

use crate::config::{EvalConfig, EvalResult};
use crate::dataset::{SupersessionDataset, SupersessionItem};
use crate::metrics::classify::Counts;
use crate::metrics::stats::{self, Estimate};

const PROJECT: &str = "eval";
const SCOPE: &str = "eval";

pub struct ItemOutcome {
    pub item_id: String,
    pub category: String,
    pub mechanism: String,
    pub new_surfaced: bool,
    pub stale_suppressed: bool,
    pub abstained: bool,
    pub stale_leak: Option<String>,
    pub passed: bool,
}

pub struct SupersessionReport {
    pub dataset_name: String,
    pub item_count: usize,
    pub outcomes: Vec<ItemOutcome>,
    pub pass_rate: Estimate,
    pub stale_leak_list: Vec<(String, String, String)>,
    pub false_supersession_rate: f64,
    pub retraction_abstain_acc: f64,
    pub conflict: Counts,
}

fn is_update(category: &str) -> bool {
    matches!(category, "knowledge_update" | "decision_reversal")
}

fn run_item(item: &SupersessionItem) -> EvalResult<ItemOutcome> {
    let home = TempDir::new()?;
    let home_path = home.path().to_path_buf();
    let start = home_path.clone();
    init_project(InitOptions {
        project_name: Some(PROJECT.to_string()),
        project_dir: start.clone(),
        grafiki_home: Some(home_path.clone()),
    })?;

    let entity = item.entity.clone().unwrap_or_else(|| "Subject".to_string());
    let mut prev: Option<String> = None;

    for (i, ev) in item.events.iter().enumerate() {
        match item.mechanism.as_str() {
            "observation" => {
                if ev.retract {
                    if let Some(id) = &prev {
                        delete_observation(DeleteObservationOptions {
                            project_name: Some(PROJECT.to_string()),
                            start_dir: start.clone(),
                            grafiki_home: Some(home_path.clone()),
                            id: id.clone(),
                        })?;
                    }
                    prev = None;
                    continue;
                }
                // All observation facts go through the candidate gate so the
                // approval path stamps the logical valid_from (captured_at) and
                // source_type — giving both old and new facts a controlled
                // timeline (no post-dating) and a real source tier for arbitration.
                let mut payload = json!({
                    "entity_name": entity,
                    "content": ev.content,
                    "category": "general",
                });
                if let Some(ts) = &ev.captured_at {
                    payload["captured_at"] = json!(ts);
                }
                if ev.supersedes_prev {
                    if let Some(p) = &prev {
                        payload["supersedes"] = json!(p);
                    }
                }
                let source_type = ev
                    .source_type
                    .clone()
                    .unwrap_or_else(|| "agent".to_string());
                let proposed = propose_candidate(ProposeCandidateOptions {
                    project_name: Some(PROJECT.to_string()),
                    start_dir: start.clone(),
                    grafiki_home: Some(home_path.clone()),
                    source_type,
                    source: None,
                    record_type: "observation".to_string(),
                    payload,
                    scope: SCOPE.to_string(),
                    confidence: 0.9,
                    rationale: None,
                    evidence: Vec::new(),
                })?;
                let approved = approve_candidate(ApproveCandidateOptions {
                    project_name: Some(PROJECT.to_string()),
                    start_dir: start.clone(),
                    grafiki_home: Some(home_path.clone()),
                    id: proposed.candidate.id,
                })?;
                // The original fact and each superseding fact advance `prev`; an
                // independent coexisting fact leaves `prev` on the original.
                if i == 0 || ev.supersedes_prev {
                    prev = approved.candidate.trusted_record_id;
                }
            }
            "decision" => {
                if i == 0 || (!ev.supersedes_prev && !ev.retract) {
                    let report = log_decision(LogDecisionOptions {
                        project_name: Some(PROJECT.to_string()),
                        start_dir: start.clone(),
                        grafiki_home: Some(home_path.clone()),
                        title: ev.content.clone(),
                        reasoning: None,
                        alternatives: Vec::new(),
                        tags: Vec::new(),
                        scope: SCOPE.to_string(),
                        supersedes: None,
                    })?;
                    if i == 0 {
                        prev = Some(report.decision_id);
                    }
                } else {
                    let mut payload = json!({ "title": ev.content });
                    if let Some(p) = &prev {
                        payload["supersedes"] = json!(p);
                    }
                    let proposed = propose_candidate(ProposeCandidateOptions {
                        project_name: Some(PROJECT.to_string()),
                        start_dir: start.clone(),
                        grafiki_home: Some(home_path.clone()),
                        source_type: "transcript".to_string(),
                        source: None,
                        record_type: "decision".to_string(),
                        payload,
                        scope: SCOPE.to_string(),
                        confidence: 0.9,
                        rationale: None,
                        evidence: Vec::new(),
                    })?;
                    let approved = approve_candidate(ApproveCandidateOptions {
                        project_name: Some(PROJECT.to_string()),
                        start_dir: start.clone(),
                        grafiki_home: Some(home_path.clone()),
                        id: proposed.candidate.id,
                    })?;
                    prev = approved.candidate.trusted_record_id;
                }
            }
            other => return Err(format!("unknown supersession mechanism '{other}'").into()),
        }
    }

    // Probe. For decisions, suppression is status-based: a superseded decision is
    // still searchable, but drops out of the *active* set — so read active
    // decisions, not raw search results.
    let haystack: Vec<String> = if item.mechanism == "decision" {
        list_decisions(DecisionListOptions {
            project_name: Some(PROJECT.to_string()),
            start_dir: start.clone(),
            grafiki_home: Some(home_path.clone()),
            scope: SCOPE.to_string(),
            status: Some("active".to_string()),
        })?
        .into_iter()
        .map(|d| d.title.to_lowercase())
        .collect()
    } else {
        let report = search_memory(SearchMemoryOptions {
            project_name: Some(PROJECT.to_string()),
            start_dir: start.clone(),
            grafiki_home: Some(home_path.clone()),
            query: item.assertion.query.clone(),
            record_type: "all".to_string(),
            mode: SearchMode::Keyword,
            scope: SCOPE.to_string(),
            limit: 20,
            temporal_weight: 0.0,
        })?;
        // Observations: match against the fact CONTENT (snippet) only, never the
        // entity-slug title, so a token can't pass for the wrong reason.
        report
            .results
            .iter()
            .map(|r| r.snippet.to_lowercase())
            .collect()
    };
    let contains = |tok: &str| {
        let t = tok.to_lowercase();
        haystack.iter().any(|h| h.contains(&t))
    };

    let new_surfaced = item.assertion.new_required.iter().all(|t| contains(t));
    let stale_leak = item
        .assertion
        .stale_forbidden
        .iter()
        .find(|t| contains(t))
        .cloned();
    let stale_suppressed = stale_leak.is_none();

    // Abstention is checked PER FACT (not as a global empty-memory probe): the
    // retracted fact's token must not appear in the generated answer for its query.
    // The fixture keeps a coexisting fact alive so the project is not trivially empty.
    let abstained = if item.assertion.expect_abstain {
        let briefing = ask_memory(AskMemoryOptions {
            project_name: Some(PROJECT.to_string()),
            start_dir: start.clone(),
            grafiki_home: Some(home_path.clone()),
            question: item.assertion.query.clone(),
            scope: SCOPE.to_string(),
            limit: 10,
            agent: Some("eval".to_string()),
        })?;
        let answer = briefing.answer.to_lowercase();
        !item
            .assertion
            .stale_forbidden
            .iter()
            .any(|t| answer.contains(&t.to_lowercase()))
    } else {
        false
    };

    let passed = match item.category.as_str() {
        "retraction" => abstained && stale_suppressed,
        // Negative classes: nothing should be suppressed and the fact(s) that must
        // remain (new_required) stay live.
        "distractor_noise" | "false_supersession_guard" => new_surfaced && stale_suppressed,
        _ => new_surfaced && stale_suppressed,
    };

    Ok(ItemOutcome {
        item_id: item.item_id.clone(),
        category: item.category.clone(),
        mechanism: item.mechanism.clone(),
        new_surfaced,
        stale_suppressed,
        abstained,
        stale_leak,
        passed,
    })
}

pub fn run_supersession(
    dataset: &SupersessionDataset,
    cfg: &EvalConfig,
) -> EvalResult<SupersessionReport> {
    let mut outcomes = Vec::with_capacity(dataset.items.len());
    for item in &dataset.items {
        outcomes.push(run_item(item)?);
    }

    // Headline pass-rate over the genuine update items.
    let pass_vec: Vec<f64> = outcomes
        .iter()
        .filter(|o| is_update(&o.category))
        .map(|o| if o.passed { 1.0 } else { 0.0 })
        .collect();
    let pass_rate = stats::bootstrap_ci(&pass_vec, cfg.bootstrap, cfg.seed);

    // Stale-leak list (the hard gate): any forbidden token that survived.
    let stale_leak_list: Vec<(String, String, String)> = outcomes
        .iter()
        .filter_map(|o| {
            o.stale_leak
                .as_ref()
                .map(|t| (o.item_id.clone(), o.mechanism.clone(), t.clone()))
        })
        .collect();

    // False supersession: a negative-class item whose still-true fact got
    // suppressed. The negative class is both coexisting distractors AND
    // false_supersession_guard items (which DO invoke the supersession mechanism
    // against a higher-trust prior fact, so arbitration must decline — making this
    // a non-vacuous test of the guard).
    let is_negative = |c: &str| c == "distractor_noise" || c == "false_supersession_guard";
    let negatives: Vec<&ItemOutcome> = outcomes
        .iter()
        .filter(|o| is_negative(&o.category))
        .collect();
    let false_supersession_rate = if negatives.is_empty() {
        0.0
    } else {
        negatives.iter().filter(|o| !o.new_surfaced).count() as f64 / negatives.len() as f64
    };

    // Retraction-abstain accuracy.
    let retractions: Vec<&ItemOutcome> = outcomes
        .iter()
        .filter(|o| o.category == "retraction")
        .collect();
    let retraction_abstain_acc = if retractions.is_empty() {
        1.0
    } else {
        retractions
            .iter()
            .filter(|o| o.abstained && o.stale_suppressed)
            .count() as f64
            / retractions.len() as f64
    };

    // Conflict detection P/R/F1: TP = update with stale suppressed; FN = stale
    // leaked; FP = distractor with a true fact suppressed; TN = distractor clean.
    let mut conflict = Counts::default();
    for o in &outcomes {
        if is_update(&o.category) || o.category == "retraction" {
            if o.stale_suppressed {
                conflict.true_pos += 1;
            } else {
                conflict.false_neg += 1;
            }
        } else if is_negative(&o.category) {
            if o.new_surfaced {
                conflict.true_neg += 1;
            } else {
                conflict.false_pos += 1;
            }
        }
    }

    Ok(SupersessionReport {
        dataset_name: dataset.name.clone(),
        item_count: dataset.items.len(),
        outcomes,
        pass_rate,
        stale_leak_list,
        false_supersession_rate,
        retraction_abstain_acc,
        conflict,
    })
}
