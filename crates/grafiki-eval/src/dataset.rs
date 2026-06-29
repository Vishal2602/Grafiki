//! Typed loaders for the eval fixtures.
//!
//! Formats are deliberately BEIR-/LongMemEval-shaped so external datasets can be
//! adapted later without touching the metrics or runners:
//! - **Retrieval** (BEIR triple): `corpus_seed.jsonl`, `queries.jsonl`,
//!   `qrels.tsv`, optional `dataset.json` (name/version/description).
//! - **Redaction**: a single `*.jsonl` of labeled cases.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::config::EvalResult;
use crate::metrics::ir::Qrels;

/// Read a UTF-8 file and deserialize each non-empty, non-`#` line as `T`.
fn read_jsonl<T: for<'de> Deserialize<'de>>(path: &Path) -> EvalResult<Vec<T>> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let value: T = serde_json::from_str(trimmed)
            .map_err(|e| format!("{}:{}: {e}", path.display(), i + 1))?;
        out.push(value);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Retrieval (Arm A)
// ---------------------------------------------------------------------------

/// One corpus record to seed. `payload` fields depend on `record_type`:
/// - `entity`: `{ name, entity_type, category }`
/// - `observation`: `{ name, entity_type, category, text }`
/// - `decision`: `{ title, reasoning?, tags? }`
/// - `context`: `{ title, category, content }` (key defaults to `doc_id`)
#[derive(Debug, Clone, Deserialize)]
pub struct CorpusDoc {
    pub doc_id: String,
    pub record_type: String,
    pub payload: serde_json::Value,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Query {
    #[serde(rename = "_id", alias = "id")]
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DatasetMeta {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// A relation edge between two corpus entities (referenced by `doc_id`), used to
/// seed the graph for the H3 graph-retrieval arm.
#[derive(Debug, Clone, Deserialize)]
pub struct RelationSpec {
    pub from: String,
    pub to: String,
    pub relation: String,
}

#[derive(Debug, Clone)]
pub struct RetrievalDataset {
    pub name: String,
    pub version: String,
    pub description: String,
    pub corpus: Vec<CorpusDoc>,
    pub queries: Vec<Query>,
    pub qrels: Qrels,
    /// Optional graph edges (from `relations.jsonl`) for the graph arm.
    pub relations: Vec<RelationSpec>,
}

/// Parse a TREC `qrels.tsv`: `query-id <TAB> corpus-id <TAB> grade`. An optional
/// header row whose third column isn't an integer is skipped.
fn load_qrels(path: &Path) -> EvalResult<Qrels> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let mut qrels: Qrels = BTreeMap::new();
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = trimmed.split('\t').map(str::trim).collect();
        if cols.len() < 3 {
            return Err(format!(
                "{}:{}: expected 3 tab-separated columns, got {}",
                path.display(),
                i + 1,
                cols.len()
            )
            .into());
        }
        let grade: i64 = match cols[2].parse() {
            Ok(g) => g,
            // Header row (e.g. "query-id\tcorpus-id\tscore") — skip once.
            Err(_) if i == 0 => continue,
            Err(e) => {
                return Err(
                    format!("{}:{}: bad grade '{}': {e}", path.display(), i + 1, cols[2]).into(),
                )
            }
        };
        qrels
            .entry(cols[0].to_string())
            .or_default()
            .insert(cols[1].to_string(), grade);
    }
    Ok(qrels)
}

impl RetrievalDataset {
    /// Load the BEIR triple from a directory.
    pub fn load(dir: &Path) -> EvalResult<Self> {
        let corpus: Vec<CorpusDoc> = read_jsonl(&dir.join("corpus_seed.jsonl"))?;
        let queries: Vec<Query> = read_jsonl(&dir.join("queries.jsonl"))?;
        let qrels = load_qrels(&dir.join("qrels.tsv"))?;
        let relations_path = dir.join("relations.jsonl");
        let relations: Vec<RelationSpec> = if relations_path.exists() {
            read_jsonl(&relations_path)?
        } else {
            Vec::new()
        };

        let meta_path = dir.join("dataset.json");
        let meta: DatasetMeta = if meta_path.exists() {
            serde_json::from_str(&std::fs::read_to_string(&meta_path)?)?
        } else {
            DatasetMeta::default()
        };
        let fallback_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("dataset")
            .to_string();

        let dataset = Self {
            name: meta.name.unwrap_or(fallback_name),
            version: meta.version.unwrap_or_else(|| "0".to_string()),
            description: meta.description.unwrap_or_default(),
            corpus,
            queries,
            qrels,
            relations,
        };
        dataset.validate()?;
        Ok(dataset)
    }

    /// Sanity checks: unique doc-ids/query-ids and qrels that reference real
    /// queries **and** real corpus docs (a qrels corpus-id with no corpus doc can
    /// never be retrieved and would silently cap a query's metrics).
    fn validate(&self) -> EvalResult<()> {
        let mut doc_ids = std::collections::HashSet::new();
        for d in &self.corpus {
            if !doc_ids.insert(d.doc_id.as_str()) {
                return Err(format!("duplicate corpus doc_id '{}'", d.doc_id).into());
            }
        }
        let mut query_ids = std::collections::HashSet::new();
        for q in &self.queries {
            if !query_ids.insert(q.id.as_str()) {
                return Err(format!("duplicate query _id '{}'", q.id).into());
            }
        }
        for (qid, qrel) in &self.qrels {
            if !query_ids.contains(qid.as_str()) {
                return Err(format!("qrels reference unknown query '{qid}'").into());
            }
            for doc in qrel.keys() {
                if !doc_ids.contains(doc.as_str()) {
                    return Err(format!(
                        "qrels for query '{qid}' reference unknown corpus doc '{doc}'"
                    )
                    .into());
                }
            }
        }
        for rel in &self.relations {
            for (role, doc) in [("from", &rel.from), ("to", &rel.to)] {
                if !doc_ids.contains(doc.as_str()) {
                    return Err(format!(
                        "relation '{}' references unknown corpus doc '{doc}'",
                        role
                    )
                    .into());
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Redaction (Arm C)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct GoldSecret {
    pub literal: String,
    #[serde(rename = "type")]
    pub secret_type: String,
}

/// A resolved redaction case. Either `text` or `json_payload` carries the input;
/// `gold_secrets` lists the planted secrets (empty for benign cases).
#[derive(Debug, Clone)]
pub struct RedactionCase {
    pub text: Option<String>,
    pub json_payload: Option<serde_json::Value>,
    pub gold_secrets: Vec<GoldSecret>,
    pub benign: bool,
    pub context: String,
}

/// On-disk form. To keep secret-shaped strings out of the committed file (and
/// past GitHub push-protection / secret scanning), a case may carry `assemble`:
/// the fragments are concatenated **at load time** into a secret-like value `S`
/// — so the prefix and body never appear contiguously in the repo, but the
/// in-memory text is a real-format secret the redactor must catch. `template`
/// substitutes `S` for `{S}`; `secret_type` (when set) adds `S` as a gold
/// secret, otherwise the assembled string is a benign decoy.
#[derive(Debug, Clone, Deserialize)]
struct RawRedactionCase {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    json_payload: Option<serde_json::Value>,
    #[serde(default)]
    gold_secrets: Vec<GoldSecret>,
    #[serde(default)]
    benign: bool,
    #[serde(default)]
    context: String,
    /// Fragments concatenated into the secret-like value `S`.
    #[serde(default)]
    assemble: Vec<String>,
    /// Text template; every `{S}` is replaced by the assembled value.
    #[serde(default)]
    template: Option<String>,
    /// When set, the assembled value is a planted secret of this type.
    #[serde(default)]
    secret_type: Option<String>,
}

impl RawRedactionCase {
    fn resolve(self) -> RedactionCase {
        let mut text = self.text;
        let mut gold = self.gold_secrets;
        if !self.assemble.is_empty() {
            let s = self.assemble.concat();
            text = Some(match &self.template {
                Some(t) => t.replace("{S}", &s),
                None => s.clone(),
            });
            if let Some(ty) = self.secret_type {
                gold.push(GoldSecret {
                    literal: s,
                    secret_type: ty,
                });
            }
        }
        RedactionCase {
            text,
            json_payload: self.json_payload,
            gold_secrets: gold,
            benign: self.benign,
            context: self.context,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RedactionDataset {
    pub name: String,
    pub cases: Vec<RedactionCase>,
}

// ---------------------------------------------------------------------------
// Supersession / knowledge-update (Arm D)
// ---------------------------------------------------------------------------

/// One event in a supersession item's timeline.
#[derive(Debug, Clone, Deserialize)]
pub struct SupersessionEvent {
    /// The fact text (observation content / decision title). Empty for a retraction.
    #[serde(default)]
    pub content: String,
    /// Logical time of this fact; used for arbitration recency + the bitemporal
    /// cut. Observation supersessions need this strictly later than the prior fact.
    #[serde(default)]
    pub captured_at: Option<String>,
    /// This event supersedes the immediately-prior one.
    #[serde(default)]
    pub supersedes_prev: bool,
    /// This event retracts the prior fact with no replacement (expect abstention).
    #[serde(default)]
    pub retract: bool,
    /// Source-trust of this fact (drives arbitration). Defaults to a low tier.
    #[serde(default)]
    pub source_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SupersessionAssert {
    pub query: String,
    #[serde(default)]
    pub new_required: Vec<String>,
    #[serde(default)]
    pub stale_forbidden: Vec<String>,
    #[serde(default)]
    pub expect_abstain: bool,
}

/// A supersession test item: a fact, then an update/retraction, then a probe.
#[derive(Debug, Clone, Deserialize)]
pub struct SupersessionItem {
    pub item_id: String,
    /// `knowledge_update` | `decision_reversal` | `retraction` | `distractor_noise`.
    pub category: String,
    /// `observation` | `decision`.
    pub mechanism: String,
    #[serde(default)]
    pub entity: Option<String>,
    pub events: Vec<SupersessionEvent>,
    #[serde(rename = "assert")]
    pub assertion: SupersessionAssert,
}

#[derive(Debug, Clone)]
pub struct SupersessionDataset {
    pub name: String,
    pub items: Vec<SupersessionItem>,
}

impl SupersessionDataset {
    pub fn load(dir: &Path) -> EvalResult<Self> {
        let items: Vec<SupersessionItem> = read_jsonl(&dir.join("updates.jsonl"))?;
        // Fail-loud validation: tokens disjoint and present; retractions have no
        // replacement event; distractors do not supersede.
        for it in &items {
            let new: std::collections::HashSet<&str> = it
                .assertion
                .new_required
                .iter()
                .map(|s| s.as_str())
                .collect();
            for s in &it.assertion.stale_forbidden {
                if new.contains(s.as_str()) {
                    return Err(format!(
                        "item '{}': token '{s}' is in both new_required and stale_forbidden",
                        it.item_id
                    )
                    .into());
                }
            }
            if it.category == "retraction" && !it.assertion.expect_abstain {
                return Err(format!("retraction item '{}' must expect_abstain", it.item_id).into());
            }
            if it.category == "distractor_noise"
                && it.events.iter().any(|e| e.supersedes_prev || e.retract)
            {
                return Err(format!(
                    "distractor_noise item '{}' must not supersede or retract",
                    it.item_id
                )
                .into());
            }
            // A token must not be able to "pass" by matching the entity slug
            // instead of the fact content.
            if let Some(entity) = &it.entity {
                let e = entity.to_lowercase();
                for tok in it
                    .assertion
                    .new_required
                    .iter()
                    .chain(it.assertion.stale_forbidden.iter())
                {
                    if e.contains(&tok.to_lowercase()) {
                        return Err(format!(
                            "item '{}': token '{tok}' is a substring of its entity name '{entity}' \
                             — it could match for the wrong reason",
                            it.item_id
                        )
                        .into());
                    }
                }
            }
        }
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("supersession")
            .to_string();
        Ok(Self { name, items })
    }
}

impl RedactionDataset {
    pub fn load(path: &Path) -> EvalResult<Self> {
        let raw: Vec<RawRedactionCase> = read_jsonl(path)?;
        let cases: Vec<RedactionCase> = raw.into_iter().map(RawRedactionCase::resolve).collect();
        // The scorer keys benign-vs-positive off gold_secrets emptiness; enforce
        // that the declared `benign` flag agrees, so an authoring mistake fails
        // loudly instead of silently mis-scoring.
        for (i, c) in cases.iter().enumerate() {
            if c.benign != c.gold_secrets.is_empty() {
                return Err(format!(
                    "redaction case {} (context '{}'): benign={} but gold_secrets is {}",
                    i + 1,
                    c.context,
                    c.benign,
                    if c.gold_secrets.is_empty() {
                        "empty"
                    } else {
                        "non-empty"
                    }
                )
                .into());
            }
        }
        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("redaction")
            .to_string();
        Ok(Self { name, cases })
    }
}
