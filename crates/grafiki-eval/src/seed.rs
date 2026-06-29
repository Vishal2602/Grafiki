//! Deterministic corpus seeder for the retrieval arm.
//!
//! Seeds the BEIR-triple corpus into a fresh temp `GRAFIKI_HOME` via the typed
//! public creation APIs (`save_entity` / `log_decision` / `add_context`) so each
//! fixture `doc_id` maps to exactly one trusted record with a known id. (Arm A
//! wants a *frozen, addressable* corpus; the capture→candidate→trusted pipeline
//! is exercised separately by the memory-QA arm, where extraction recall is the
//! thing under test.)
//!
//! Note: an `observation` record is created via `save_entity(observe=…)`, which
//! also creates a backing `entity`. That entity is a legitimate extra corpus doc
//! and is simply left unjudged (grade 0) — the run→doc-id mapping treats any
//! retrieved record without a fixture `doc_id` as an unjudged document.

use std::collections::HashMap;
use std::path::PathBuf;

use grafiki_core::{
    add_context, init_project, log_decision, process_embedding_jobs, save_entity,
    AddContextOptions, InitOptions, LogDecisionOptions, ProcessEmbeddingsOptions,
    SaveEntityOptions,
};
use tempfile::TempDir;

use crate::config::EvalResult;
use crate::dataset::RetrievalDataset;

pub const EVAL_PROJECT: &str = "eval";
pub const EVAL_SCOPE: &str = "eval";

#[derive(Debug, Clone)]
pub struct EmbeddingInfo {
    pub provider: String,
    pub model: String,
    pub dimension: usize,
    pub processed: usize,
}

/// A seeded, frozen store plus the mapping from trusted record ids
/// (`"record_type:id"`) back to fixture `doc_id`s.
pub struct SeededCorpus {
    /// Held to keep the temp dir alive for the lifetime of the corpus.
    _home: TempDir,
    pub start_dir: PathBuf,
    pub home_path: PathBuf,
    pub record_to_doc: HashMap<String, String>,
    pub doc_count: usize,
    pub embedding: Option<EmbeddingInfo>,
}

fn pstr(v: &serde_json::Value, key: &str) -> EvalResult<String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("payload missing required string field '{key}'").into())
}

fn pstr_or(v: &serde_json::Value, key: &str, default: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or(default)
        .to_string()
}

fn popt(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(|s| s.to_string())
}

fn pvec(v: &serde_json::Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Seed `dataset`'s corpus into a fresh temp home. When `build_embeddings` is
/// true, processes embedding jobs so the semantic/hybrid modes are available.
pub fn seed_retrieval(
    dataset: &RetrievalDataset,
    build_embeddings: bool,
) -> EvalResult<SeededCorpus> {
    let home = TempDir::new()?;
    let home_path = home.path().to_path_buf();
    let start_dir = home_path.clone();

    init_project(InitOptions {
        project_name: Some(EVAL_PROJECT.to_string()),
        project_dir: start_dir.clone(),
        grafiki_home: Some(home_path.clone()),
    })?;

    let mut record_to_doc = HashMap::new();
    for doc in &dataset.corpus {
        let scope = doc.scope.clone().unwrap_or_else(|| EVAL_SCOPE.to_string());
        // The retrieval arm searches a single scope (EVAL_SCOPE), whose chain is
        // ["", EVAL_SCOPE]. A doc seeded into any other scope would be written and
        // embedded but never returned, silently capping its query's metrics — so
        // refuse it up front rather than report a misleading low score.
        if !scope.is_empty() && scope != EVAL_SCOPE {
            return Err(format!(
                "corpus doc '{}' uses scope '{}' outside the eval search scope '{}' \
                 (use '{}' or omit `scope`)",
                doc.doc_id, scope, EVAL_SCOPE, EVAL_SCOPE
            )
            .into());
        }
        let p = &doc.payload;
        let key = match doc.record_type.as_str() {
            "entity" => {
                let r = save_entity(SaveEntityOptions {
                    project_name: Some(EVAL_PROJECT.to_string()),
                    start_dir: start_dir.clone(),
                    grafiki_home: Some(home_path.clone()),
                    name: pstr(p, "name")?,
                    entity_type: pstr_or(p, "entity_type", "concept"),
                    observe: popt(p, "observe"),
                    category: pstr_or(p, "category", "general"),
                    scope,
                    relate: None,
                })?;
                format!("entity:{}", r.entity_id)
            }
            "observation" => {
                let r = save_entity(SaveEntityOptions {
                    project_name: Some(EVAL_PROJECT.to_string()),
                    start_dir: start_dir.clone(),
                    grafiki_home: Some(home_path.clone()),
                    name: pstr(p, "name")?,
                    entity_type: pstr_or(p, "entity_type", "concept"),
                    observe: Some(pstr(p, "text")?),
                    category: pstr_or(p, "category", "general"),
                    scope,
                    relate: None,
                })?;
                let obs = r
                    .observation_id
                    .ok_or_else(|| -> crate::config::EvalError {
                        "observation record was not created by save_entity".into()
                    })?;
                format!("observation:{obs}")
            }
            "decision" => {
                let r = log_decision(LogDecisionOptions {
                    project_name: Some(EVAL_PROJECT.to_string()),
                    start_dir: start_dir.clone(),
                    grafiki_home: Some(home_path.clone()),
                    title: pstr(p, "title")?,
                    reasoning: popt(p, "reasoning"),
                    alternatives: pvec(p, "alternatives"),
                    tags: pvec(p, "tags"),
                    scope,
                    supersedes: None,
                })?;
                format!("decision:{}", r.decision_id)
            }
            "context" => {
                let r = add_context(AddContextOptions {
                    project_name: Some(EVAL_PROJECT.to_string()),
                    start_dir: start_dir.clone(),
                    grafiki_home: Some(home_path.clone()),
                    key: popt(p, "key").unwrap_or_else(|| doc.doc_id.clone()),
                    title: pstr(p, "title")?,
                    category: pstr_or(p, "category", "reference"),
                    scope,
                    content: pstr(p, "content")?,
                })?;
                format!("context:{}", r.key)
            }
            other => return Err(format!("unknown corpus record_type '{other}'").into()),
        };
        if let Some(prev) = record_to_doc.insert(key.clone(), doc.doc_id.clone()) {
            return Err(format!(
                "two fixture docs mapped to the same record '{key}': '{prev}' and '{}'",
                doc.doc_id
            )
            .into());
        }
    }

    let embedding = if build_embeddings {
        let rep = process_embedding_jobs(ProcessEmbeddingsOptions {
            project_name: Some(EVAL_PROJECT.to_string()),
            start_dir: start_dir.clone(),
            grafiki_home: Some(home_path.clone()),
            scope: "*".to_string(),
            limit: 1_000_000,
            rebuild: false,
        })?;
        Some(EmbeddingInfo {
            provider: rep.provider,
            model: rep.model,
            dimension: rep.dimension,
            processed: rep.processed,
        })
    } else {
        None
    };

    Ok(SeededCorpus {
        _home: home,
        start_dir,
        home_path,
        record_to_doc,
        doc_count: dataset.corpus.len(),
        embedding,
    })
}
