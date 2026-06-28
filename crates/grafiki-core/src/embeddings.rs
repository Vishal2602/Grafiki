use std::env;
#[cfg(feature = "fastembed")]
use std::sync::Mutex;

use sha2::{Digest, Sha256};

use crate::{GrafikiError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingProviderSummary {
    pub requested_provider: String,
    pub provider: String,
    pub model: String,
    pub dimension: Option<usize>,
    pub note: Option<String>,
}

pub trait EmbeddingProvider {
    fn provider_name(&self) -> &'static str;
    fn model_name(&self) -> &'static str;
    fn dimension(&self) -> usize;
    fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

#[derive(Debug, Clone)]
pub struct DeterministicEmbeddingProvider {
    dimension: usize,
}

impl Default for DeterministicEmbeddingProvider {
    fn default() -> Self {
        Self { dimension: 64 }
    }
}

impl DeterministicEmbeddingProvider {
    pub fn new(dimension: usize) -> Result<Self> {
        if dimension == 0 {
            return Err(GrafikiError::Embedding(
                "embedding dimension must be greater than zero".to_owned(),
            ));
        }
        Ok(Self { dimension })
    }
}

impl EmbeddingProvider for DeterministicEmbeddingProvider {
    fn provider_name(&self) -> &'static str {
        "deterministic"
    }

    fn model_name(&self) -> &'static str {
        "hashed-token-v1"
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut vector = vec![0.0; self.dimension];
        for token in tokenize(text) {
            let digest = Sha256::digest(token.as_bytes());
            let mut bucket_bytes = [0u8; 8];
            bucket_bytes.copy_from_slice(&digest[..8]);
            let index = u64::from_be_bytes(bucket_bytes) as usize % self.dimension;
            let sign = if digest[8] % 2 == 0 { 1.0 } else { -1.0 };
            vector[index] += sign;
        }
        normalize(&mut vector);
        Ok(vector)
    }
}

#[cfg(feature = "fastembed")]
pub struct FastEmbedProvider {
    model: Mutex<fastembed::TextEmbedding>,
}

#[cfg(feature = "fastembed")]
impl FastEmbedProvider {
    pub fn try_new() -> Result<Self> {
        // Pin the model cache to a stable per-user directory under GRAFIKI_HOME
        // instead of fastembed's CWD-relative default, so first run is
        // deterministic and a pre-warmed cache can be shipped/located reliably.
        let cache_dir = fastembed_cache_dir()?;
        let options = fastembed::InitOptions::new(fastembed::EmbeddingModel::AllMiniLML6V2)
            .with_cache_dir(cache_dir.clone())
            .with_show_download_progress(false);
        let model = fastembed::TextEmbedding::try_new(options).map_err(|error| {
            GrafikiError::Embedding(format!(
                "could not load the fastembed MiniLM model from {}: {error}. \
                 If you are offline, run `grafiki embeddings prefetch` on a networked machine to \
                 pre-download it, or set GRAFIKI_EMBEDDING_PROVIDER=deterministic for offline use.",
                cache_dir.display()
            ))
        })?;
        Ok(Self {
            model: Mutex::new(model),
        })
    }
}

/// Stable, per-user cache directory for the fastembed model (under GRAFIKI_HOME).
/// Note: if the `HF_HOME` env var is set, fastembed/hf-hub honor it over this
/// cache_dir, so the model may resolve elsewhere; unset HF_HOME (or point it at
/// this directory) for a fully pinned location.
#[cfg(feature = "fastembed")]
fn fastembed_cache_dir() -> Result<std::path::PathBuf> {
    let dir = crate::project::grafiki_home()?
        .join("models")
        .join("fastembed");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(feature = "fastembed")]
impl EmbeddingProvider for FastEmbedProvider {
    fn provider_name(&self) -> &'static str {
        "fastembed"
    }

    fn model_name(&self) -> &'static str {
        "sentence-transformers/all-MiniLM-L6-v2"
    }

    fn dimension(&self) -> usize {
        384
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut model = self
            .model
            .lock()
            .map_err(|_| GrafikiError::Embedding("fastembed model lock poisoned".to_owned()))?;
        let mut embeddings = model
            .embed(vec![text], None)
            .map_err(|error| GrafikiError::Embedding(error.to_string()))?;
        let embedding = embeddings
            .pop()
            .ok_or_else(|| GrafikiError::Embedding("fastembed returned no embedding".to_owned()))?;
        if embedding.len() != self.dimension() {
            return Err(GrafikiError::Embedding(format!(
                "fastembed dimension mismatch: expected {}, got {}",
                self.dimension(),
                embedding.len()
            )));
        }
        Ok(embedding)
    }
}

pub enum RuntimeEmbeddingProvider {
    Deterministic(DeterministicEmbeddingProvider),
    #[cfg(feature = "fastembed")]
    FastEmbed(FastEmbedProvider),
}

impl EmbeddingProvider for RuntimeEmbeddingProvider {
    fn provider_name(&self) -> &'static str {
        match self {
            Self::Deterministic(provider) => provider.provider_name(),
            #[cfg(feature = "fastembed")]
            Self::FastEmbed(provider) => provider.provider_name(),
        }
    }

    fn model_name(&self) -> &'static str {
        match self {
            Self::Deterministic(provider) => provider.model_name(),
            #[cfg(feature = "fastembed")]
            Self::FastEmbed(provider) => provider.model_name(),
        }
    }

    fn dimension(&self) -> usize {
        match self {
            Self::Deterministic(provider) => provider.dimension(),
            #[cfg(feature = "fastembed")]
            Self::FastEmbed(provider) => provider.dimension(),
        }
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        match self {
            Self::Deterministic(provider) => provider.embed(text),
            #[cfg(feature = "fastembed")]
            Self::FastEmbed(provider) => provider.embed(text),
        }
    }
}

pub fn configured_embedding_provider() -> Result<RuntimeEmbeddingProvider> {
    match env::var("GRAFIKI_EMBEDDING_PROVIDER")
        .unwrap_or_else(|_| "deterministic".to_owned())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "deterministic" | "test" => Ok(RuntimeEmbeddingProvider::Deterministic(
            DeterministicEmbeddingProvider::default(),
        )),
        "auto" => auto_embedding_provider(),
        "fastembed" | "local" | "minilm" | "all-minilm-l6-v2" => fastembed_provider(),
        requested => Err(GrafikiError::Embedding(format!(
            "unknown embedding provider: {requested}"
        ))),
    }
}

/// Force the fastembed model into the pinned cache so later runs work offline.
/// Returns the resolved cache directory on success.
pub fn prefetch_embedding_model() -> Result<String> {
    #[cfg(feature = "fastembed")]
    {
        let cache_dir = fastembed_cache_dir()?;
        // try_new performs the model download into the cache; the embed call then
        // verifies the model can actually run inference.
        let provider = FastEmbedProvider::try_new()?;
        provider.embed("warm up the embedding model")?;
        Ok(cache_dir.display().to_string())
    }
    #[cfg(not(feature = "fastembed"))]
    {
        Err(GrafikiError::Embedding(
            "embeddings prefetch requires building with the `fastembed` feature".to_owned(),
        ))
    }
}

pub fn configured_embedding_provider_summary() -> EmbeddingProviderSummary {
    let requested =
        env::var("GRAFIKI_EMBEDDING_PROVIDER").unwrap_or_else(|_| "deterministic".to_owned());
    let requested = requested.trim().to_ascii_lowercase();
    match requested.as_str() {
        "" | "deterministic" | "test" => embedding_provider_summary(
            "deterministic",
            "deterministic",
            "hashed-token-v1",
            Some(DeterministicEmbeddingProvider::default().dimension()),
            None,
        ),
        "auto" => auto_embedding_provider_summary(),
        "fastembed" | "local" | "minilm" | "all-minilm-l6-v2" => {
            fastembed_provider_summary(&requested)
        }
        requested => embedding_provider_summary(
            requested,
            "unknown",
            "unknown",
            None,
            Some(format!("unknown embedding provider: {requested}")),
        ),
    }
}

fn embedding_provider_summary(
    requested_provider: &str,
    provider: &str,
    model: &str,
    dimension: Option<usize>,
    note: Option<String>,
) -> EmbeddingProviderSummary {
    EmbeddingProviderSummary {
        requested_provider: requested_provider.to_owned(),
        provider: provider.to_owned(),
        model: model.to_owned(),
        dimension,
        note,
    }
}

fn auto_embedding_provider() -> Result<RuntimeEmbeddingProvider> {
    #[cfg(feature = "fastembed")]
    {
        if let Ok(provider) = FastEmbedProvider::try_new() {
            return Ok(RuntimeEmbeddingProvider::FastEmbed(provider));
        }
    }
    Ok(RuntimeEmbeddingProvider::Deterministic(
        DeterministicEmbeddingProvider::default(),
    ))
}

fn auto_embedding_provider_summary() -> EmbeddingProviderSummary {
    #[cfg(feature = "fastembed")]
    {
        return embedding_provider_summary(
            "auto",
            "fastembed",
            "sentence-transformers/all-MiniLM-L6-v2",
            Some(384),
            Some(
                "auto uses fastembed when its model is available and falls back to deterministic \
                 otherwise. Run `grafiki embeddings prefetch` to pre-download the model for \
                 offline use."
                    .to_owned(),
            ),
        );
    }
    #[cfg(not(feature = "fastembed"))]
    {
        embedding_provider_summary(
            "auto",
            "deterministic",
            "hashed-token-v1",
            Some(DeterministicEmbeddingProvider::default().dimension()),
            Some("fastembed feature is not enabled; auto uses deterministic embeddings".to_owned()),
        )
    }
}

fn fastembed_provider() -> Result<RuntimeEmbeddingProvider> {
    #[cfg(feature = "fastembed")]
    {
        return Ok(RuntimeEmbeddingProvider::FastEmbed(
            FastEmbedProvider::try_new()?,
        ));
    }
    #[cfg(not(feature = "fastembed"))]
    {
        Err(GrafikiError::Embedding(
            "fastembed provider requires building with the `fastembed` feature".to_owned(),
        ))
    }
}

fn fastembed_provider_summary(requested_provider: &str) -> EmbeddingProviderSummary {
    #[cfg(feature = "fastembed")]
    {
        return embedding_provider_summary(
            requested_provider,
            "fastembed",
            "sentence-transformers/all-MiniLM-L6-v2",
            Some(384),
            None,
        );
    }
    #[cfg(not(feature = "fastembed"))]
    {
        embedding_provider_summary(
            requested_provider,
            "fastembed",
            "sentence-transformers/all-MiniLM-L6-v2",
            Some(384),
            Some("fastembed provider requires building with the `fastembed` feature".to_owned()),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticMatch<T> {
    pub item: T,
    pub score: f32,
}

pub fn rank_by_embedding<T: Clone>(
    query: &str,
    records: &[(T, String)],
    provider: &impl EmbeddingProvider,
) -> Result<Vec<SemanticMatch<T>>> {
    let query_embedding = provider.embed(query)?;
    if query_embedding.len() != provider.dimension() {
        return Err(GrafikiError::Embedding(format!(
            "query embedding dimension mismatch: expected {}, got {}",
            provider.dimension(),
            query_embedding.len()
        )));
    }

    let mut matches = Vec::with_capacity(records.len());
    for (item, text) in records {
        let record_embedding = provider.embed(text)?;
        if record_embedding.len() != query_embedding.len() {
            return Err(GrafikiError::Embedding(format!(
                "record embedding dimension mismatch: expected {}, got {}",
                query_embedding.len(),
                record_embedding.len()
            )));
        }
        matches.push(SemanticMatch {
            item: item.clone(),
            score: cosine_similarity(&query_embedding, &record_embedding),
        });
    }

    matches.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(matches)
}

pub trait VectorBackend {
    fn dimension(&self) -> usize;
    fn upsert(&mut self, record: VectorRecord) -> Result<()>;
    fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<VectorSearchResult>>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorRecord {
    pub record_type: String,
    pub record_id: String,
    pub scope: String,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorSearchResult {
    pub record_type: String,
    pub record_id: String,
    pub scope: String,
    pub score: f32,
}

#[derive(Debug, Clone)]
pub struct InMemoryVectorBackend {
    dimension: usize,
    records: Vec<VectorRecord>,
}

impl InMemoryVectorBackend {
    pub fn new(dimension: usize) -> Result<Self> {
        if dimension == 0 {
            return Err(GrafikiError::Embedding(
                "vector backend dimension must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            dimension,
            records: Vec::new(),
        })
    }
}

impl VectorBackend for InMemoryVectorBackend {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn upsert(&mut self, record: VectorRecord) -> Result<()> {
        if record.embedding.len() != self.dimension {
            return Err(GrafikiError::Embedding(format!(
                "vector dimension mismatch: expected {}, got {}",
                self.dimension,
                record.embedding.len()
            )));
        }

        match self.records.iter_mut().find(|existing| {
            existing.record_type == record.record_type && existing.record_id == record.record_id
        }) {
            Some(existing) => *existing = record,
            None => self.records.push(record),
        }
        Ok(())
    }

    fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<VectorSearchResult>> {
        if query_embedding.len() != self.dimension {
            return Err(GrafikiError::Embedding(format!(
                "query vector dimension mismatch: expected {}, got {}",
                self.dimension,
                query_embedding.len()
            )));
        }

        let mut results: Vec<_> = self
            .records
            .iter()
            .map(|record| VectorSearchResult {
                record_type: record.record_type.clone(),
                record_id: record.record_id.clone(),
                scope: record.scope.clone(),
                score: cosine_similarity(query_embedding, &record.embedding),
            })
            .collect();
        results.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit.max(1));
        Ok(results)
    }
}

#[cfg(feature = "sqlite-vec")]
pub struct SqliteVecBackend<'connection> {
    connection: &'connection rusqlite::Connection,
    table_name: String,
    dimension: usize,
}

#[cfg(feature = "sqlite-vec")]
impl<'connection> SqliteVecBackend<'connection> {
    pub fn new(
        connection: &'connection rusqlite::Connection,
        table_name: &str,
        dimension: usize,
    ) -> Result<Self> {
        if dimension == 0 {
            return Err(GrafikiError::Embedding(
                "sqlite-vec dimension must be greater than zero".to_owned(),
            ));
        }
        let table_name = validate_sql_identifier(table_name)?;
        register_sqlite_vec()?;
        connection.execute(
            &format!(
                "
                CREATE VIRTUAL TABLE IF NOT EXISTS {table_name}
                USING vec0(
                    record_type TEXT,
                    record_id TEXT,
                    scope TEXT,
                    embedding float[{dimension}]
                )
                "
            ),
            [],
        )?;
        Ok(Self {
            connection,
            table_name,
            dimension,
        })
    }
}

#[cfg(feature = "sqlite-vec")]
impl VectorBackend for SqliteVecBackend<'_> {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn upsert(&mut self, record: VectorRecord) -> Result<()> {
        if record.embedding.len() != self.dimension {
            return Err(GrafikiError::Embedding(format!(
                "vector dimension mismatch: expected {}, got {}",
                self.dimension,
                record.embedding.len()
            )));
        }

        self.connection.execute(
            &format!(
                "DELETE FROM {} WHERE record_type = ?1 AND record_id = ?2",
                self.table_name
            ),
            rusqlite::params![record.record_type, record.record_id],
        )?;
        self.connection.execute(
            &format!(
                "
                INSERT INTO {} (record_type, record_id, scope, embedding)
                VALUES (?1, ?2, ?3, ?4)
                ",
                self.table_name
            ),
            rusqlite::params![
                record.record_type,
                record.record_id,
                record.scope,
                vector_json(&record.embedding)?
            ],
        )?;
        Ok(())
    }

    fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<VectorSearchResult>> {
        if query_embedding.len() != self.dimension {
            return Err(GrafikiError::Embedding(format!(
                "query vector dimension mismatch: expected {}, got {}",
                self.dimension,
                query_embedding.len()
            )));
        }

        let sql = format!(
            "
            SELECT record_type, record_id, scope, distance
            FROM {}
            WHERE embedding MATCH ?1
            ORDER BY distance
            LIMIT ?2
            ",
            self.table_name
        );
        let query = vector_json(query_embedding)?;
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(rusqlite::params![query, limit.max(1) as i64], |row| {
            let distance: f32 = row.get(3)?;
            Ok(VectorSearchResult {
                record_type: row.get(0)?,
                record_id: row.get(1)?,
                scope: row.get(2)?,
                score: 1.0 / (1.0 + distance),
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }
}

#[cfg(feature = "sqlite-vec")]
pub fn register_sqlite_vec() -> Result<()> {
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }
    Ok(())
}

#[cfg(feature = "sqlite-vec")]
fn validate_sql_identifier(raw: &str) -> Result<String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(GrafikiError::Embedding(
            "sqlite-vec table name cannot be empty".to_owned(),
        ));
    }
    let mut chars = value.chars();
    let first = chars.next().unwrap();
    if !(first == '_' || first.is_ascii_alphabetic())
        || !chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return Err(GrafikiError::Embedding(format!(
            "invalid sqlite-vec table name: {raw}"
        )));
    }
    Ok(value.to_owned())
}

#[cfg(feature = "sqlite-vec")]
fn vector_json(vector: &[f32]) -> Result<String> {
    Ok(serde_json::to_string(vector)?)
}

pub fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.len() != right.len() || left.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0;
    let mut left_norm = 0.0;
    let mut right_norm = 0.0;
    for (left, right) in left.iter().zip(right.iter()) {
        dot += left * right;
        left_norm += left * left;
        right_norm += right * right;
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}

fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm == 0.0 {
        return;
    }
    for value in vector {
        *value /= norm;
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        cosine_similarity, rank_by_embedding, DeterministicEmbeddingProvider, EmbeddingProvider,
        InMemoryVectorBackend, VectorBackend, VectorRecord,
    };

    #[test]
    fn deterministic_provider_returns_stable_normalized_vectors() {
        let provider = DeterministicEmbeddingProvider::new(16).unwrap();

        let first = provider.embed("JWT refresh uses rotating tokens").unwrap();
        let second = provider.embed("jwt refresh uses rotating tokens").unwrap();

        assert_eq!(first, second);
        assert_eq!(first.len(), 16);
        assert!((cosine_similarity(&first, &first) - 1.0).abs() < 0.0001);
    }

    #[test]
    fn in_memory_ranking_prefers_related_records() {
        let provider = DeterministicEmbeddingProvider::new(32).unwrap();
        let records = vec![
            (
                "auth",
                "JWT refresh uses rotating tokens for session renewal".to_owned(),
            ),
            (
                "storage",
                "SQLite WAL keeps local writes responsive".to_owned(),
            ),
            ("ui", "Dashboard tabs show queue status".to_owned()),
        ];

        let ranked = rank_by_embedding("rotating refresh token", &records, &provider).unwrap();

        assert_eq!(ranked[0].item, "auth");
        assert!(ranked[0].score > ranked[1].score);
    }

    #[test]
    fn in_memory_vector_backend_searches_by_similarity() {
        let provider = DeterministicEmbeddingProvider::new(32).unwrap();
        let mut backend = InMemoryVectorBackend::new(provider.dimension()).unwrap();
        backend
            .upsert(VectorRecord {
                record_type: "observation".to_owned(),
                record_id: "auth".to_owned(),
                scope: "project/core".to_owned(),
                embedding: provider
                    .embed("JWT refresh uses rotating tokens for session renewal")
                    .unwrap(),
            })
            .unwrap();
        backend
            .upsert(VectorRecord {
                record_type: "decision".to_owned(),
                record_id: "storage".to_owned(),
                scope: "project/core".to_owned(),
                embedding: provider
                    .embed("Use SQLite WAL for local database writes")
                    .unwrap(),
            })
            .unwrap();

        let query = provider.embed("refresh rotating token").unwrap();
        let results = backend.search(&query, 2).unwrap();

        assert_eq!(results[0].record_id, "auth");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn in_memory_vector_backend_upsert_replaces_existing_record() {
        let provider = DeterministicEmbeddingProvider::new(16).unwrap();
        let mut backend = InMemoryVectorBackend::new(provider.dimension()).unwrap();
        backend
            .upsert(VectorRecord {
                record_type: "context".to_owned(),
                record_id: "guide".to_owned(),
                scope: "project".to_owned(),
                embedding: provider.embed("old onboarding notes").unwrap(),
            })
            .unwrap();
        backend
            .upsert(VectorRecord {
                record_type: "context".to_owned(),
                record_id: "guide".to_owned(),
                scope: "project".to_owned(),
                embedding: provider.embed("new token refresh guide").unwrap(),
            })
            .unwrap();

        let query = provider.embed("token refresh").unwrap();
        let results = backend.search(&query, 10).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].record_id, "guide");
        assert!(results[0].score > 0.0);
    }

    #[cfg(feature = "sqlite-vec")]
    #[test]
    fn sqlite_vec_backend_searches_by_distance() {
        use super::{register_sqlite_vec, SqliteVecBackend};

        let provider = DeterministicEmbeddingProvider::new(16).unwrap();
        register_sqlite_vec().unwrap();
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        let mut backend =
            SqliteVecBackend::new(&connection, "grafiki_test_vectors", provider.dimension())
                .unwrap();
        backend
            .upsert(VectorRecord {
                record_type: "observation".to_owned(),
                record_id: "auth".to_owned(),
                scope: "project/core".to_owned(),
                embedding: provider.embed("JWT refresh uses rotating tokens").unwrap(),
            })
            .unwrap();
        backend
            .upsert(VectorRecord {
                record_type: "decision".to_owned(),
                record_id: "storage".to_owned(),
                scope: "project/core".to_owned(),
                embedding: provider
                    .embed("SQLite WAL keeps writes responsive")
                    .unwrap(),
            })
            .unwrap();

        let query = provider.embed("rotating token refresh").unwrap();
        let results = backend.search(&query, 2).unwrap();

        assert_eq!(results[0].record_id, "auth");
        assert!(results[0].score > results[1].score);
    }
}
