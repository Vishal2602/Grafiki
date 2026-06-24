# Semantic Search Plan

## Goal

Add optional semantic search without weakening Grafiki's local-first core. Keyword FTS must remain the baseline path, and semantic search should layer on top only when embeddings and vector storage are available.

## Current Baseline

Grafiki already has:

- SQLite storage.
- FTS5 keyword search for observations, decisions, and context.
- Scoped search using global and ancestor scope chains.
- JSON/plain/Markdown outputs through CLI, HTTP, and MCP.

## Proposed Architecture

```text
records -> embedding queue -> embedding worker -> vector index
   |              |                    |              |
   +------ FTS keyword search ---------+--------------+--> hybrid ranked results
```

Semantic search should be split into three layers:

1. Queue and metadata layer in `grafiki-core`.
2. Embedding provider abstraction behind a feature flag.
3. Vector index adapter behind a feature flag.

This keeps the current CLI useful even on machines where embedding dependencies are unavailable.

## Record Coverage

Phase 3 should embed:

- observations: `content`
- decisions: `title + reasoning`
- context: `title + content`
- entities: `name + entity_type`

Sessions and events should stay out of the first semantic index because they are high-volume operational records.

## Schema Additions

Add normal tables first:

```sql
CREATE TABLE embedding_jobs (
    id TEXT PRIMARY KEY,
    record_type TEXT NOT NULL,
    record_id TEXT NOT NULL,
    scope TEXT NOT NULL DEFAULT '',
    content_hash TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE(record_type, record_id, content_hash)
);

CREATE TABLE embedding_metadata (
    record_type TEXT NOT NULL,
    record_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    dimension INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    embedded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY(record_type, record_id, provider, model)
);
```

Then add a vector table only when the vector backend is enabled. With sqlite-vec, the first index should use a `vec0` virtual table with a 384-dimensional float vector for `all-MiniLM-L6-v2`.

## Embedding Model

Recommended first local model:

- `sentence-transformers/all-MiniLM-L6-v2`
- 384 dimensions
- good fit for sentence and short paragraph semantic search

Implementation options:

- First implementation: provider trait plus deterministic test provider.
- Local implementation: fastembed-backed all-MiniLM-L6-v2 provider.
- Later option: Ollama or remote API provider for users who already have embedding infrastructure.

Important model detail: the model card says this model maps sentences and paragraphs to a 384-dimensional dense vector space and is intended for semantic search, clustering, and sentence similarity.

## Vector Backend

Recommended first vector backend:

- sqlite-vec
- use `vec0` virtual tables
- query with `MATCH` and `ORDER BY distance`

Important dependency detail: sqlite-vec is pre-v1, so the adapter should be isolated behind a small module and tests. Its own README calls out expected breaking changes.

## Hybrid Ranking

Search should return:

- keyword-only results when semantic search is disabled or unavailable
- semantic-only results for `--mode semantic`
- reciprocal-rank-fused results for default hybrid mode

Proposed CLI:

```bash
grafiki search "token refresh design" --scope grafiki/core --mode hybrid
grafiki search "token refresh design" --scope grafiki/core --mode semantic
grafiki embeddings status
grafiki embeddings rebuild --scope grafiki/core
```

Proposed HTTP:

```text
GET /api/search?q=token+refresh&scope=grafiki/core&mode=hybrid
GET /api/embeddings/status?scope=grafiki/core
POST /api/embeddings/rebuild
```

Proposed MCP tools:

```text
grafiki_search mode: "keyword" | "semantic" | "hybrid"
grafiki_embeddings_status
```

## Queue Behavior

Every write to an embeddable record should enqueue work:

- new observation
- changed context document
- new decision
- new or updated entity

Queue processing should:

- skip unchanged `content_hash`
- retry failed jobs with capped attempts
- never block the write path
- preserve keyword search as a fallback

## Acceptance Criteria

- Existing `cargo test` passes without embedding/vector features.
- Existing keyword search behavior does not regress.
- A deterministic test embedding provider can prove semantic ranking without downloading a model.
- If sqlite-vec or the embedding model is unavailable, `grafiki search` still works with keyword results and explains the fallback in JSON metadata.
- `grafiki embeddings status` reports pending, embedded, failed, provider, model, dimension, vector backend, indexed records, fresh records, and missing/stale records for the configured provider.
- `scripts/smoke.sh` continues to pass without model downloads.
- `scripts/smoke.sh` verifies keyword fallback behavior when the configured embedding provider is invalid or unavailable.
- `scripts/smoke_fastembed.sh` provides a passing end-to-end local model/vector-index check.
- `grafiki embeddings rebuild` processes queued jobs and enables semantic/hybrid search over stored vectors.
- Semantic and hybrid results include rounded relevance scores in JSON output.
- A retrieval-quality fixture verifies that hybrid search separates auth, storage, billing, and search-index topics.

## Implementation Order

1. Add search mode enum and result metadata while keeping current keyword behavior. Done.
2. Add embedding job schema and enqueue jobs on writes. Done.
3. Add deterministic test embedding provider. Done.
4. Add in-memory semantic ranking tests without sqlite-vec. Done.
5. Add vector backend trait. Done.
6. Add sqlite-vec adapter behind a feature flag. Done.
7. Add persisted deterministic vectors and semantic/hybrid search over stored vectors. Done.
8. Add CLI, HTTP, and MCP status/process/rebuild commands. Done.
9. Add daemon/server worker loop for queued embedding jobs. Done.
10. Add local MiniLM provider behind a feature flag. Done with fastembed.
11. Integrate sqlite-vec into the worker/search path. Done.
12. Polish provider configuration and hybrid ranking. Done: `embeddings status` reports provider/backend/index/freshness configuration, keyword mode bypasses embedding provider setup, semantic/hybrid provider errors fall back to keyword results, semantic/hybrid results expose rounded relevance scores, hybrid mode uses weighted reciprocal rank fusion with cross-source and text-match boosts, default smoke covers provider mismatch fallbacks, and fastembed/sqlite-vec smoke coverage passes.
13. Add a larger retrieval-quality fixture. Done for auth, storage, billing, and search-index topics.
14. Continue relevance tuning with larger real-world corpora after the desktop flow is defined.

## References

- sqlite-vec README: https://github.com/asg017/sqlite-vec
- all-MiniLM-L6-v2 model card: https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2
- fastembed crate docs: https://docs.rs/fastembed/latest/fastembed/
