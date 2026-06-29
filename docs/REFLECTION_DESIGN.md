# H5 — Reflection / Consolidation + Community Summaries

**Status:** design, build-ready
**Depends on:** H3 (`graph.rs` `Graph` + `personalized_pagerank`), the candidate gate
(`propose_candidate` / `approve_candidate`), `evidence_links` provenance, the scope chain,
and bitemporal `valid_to`.
**Authors' constraint set (non-negotiable):** local-first / **no LLM** in the default path,
**deterministic** (same store ⇒ byte-identical output), **human-in-the-loop** (reflections enter
as pending `extraction_candidate`s with provenance), and **reuse existing infra** (no parallel
mechanisms, minimal new schema).

This doc merges three independent designs (algorithm-first, retrieval-value-first,
integration-first). Conflicts are resolved in favor of the hard constraints; the resolution is
stated inline at each decision point.

---

## 0. Adversarial-critique resolutions (BINDING — override the body below)

A panel review (verdict *sound-with-fixes*) found 10 issues; the resolutions below are **binding**
and override anything in §§1–6 they touch. They are what the implementation actually does.

| # | Sev | Area | Resolution (overrides) |
|---|---|---|---|
| C1 | HIGH | provenance/redaction | **§4.5 was wrong.** Source observations are **not** redacted at ingest (`save_entity(observe=…)` inserts raw text), and `insert_candidate_evidence` only *compacts* the excerpt — it does **not** redact. So reflection **redacts its own inputs**: each observation's `content` is run through the public `redact_text(…)` seam in the orchestrator *before* it is used for either the summary `content` **or** the evidence `excerpt`. No unredacted source text reaches `evidence_links`. |
| C2 | HIGH | determinism | **§2.2 only covered node order.** Adjacency order is also canonicalized: (a) the shared subgraph SQL gets `ORDER BY r.from_entity, r.to_entity, r.relation`, and (b) `detect_communities` folds each node's neighbors into a `BTreeMap<&str,f64>` (summing multi-edges) so ΔQ is summed in a fixed lexicographic neighbor order — making the float result insertion-order-independent. A regression builds the graph from a **shuffled** edge list and asserts the identical partition. |
| C3/C4 | HIGH | eval-validity | Arm E runs the **real pipeline** as the hard gate: seed grade-1 observations + relations → `run_reflection` → `approve_candidate` → re-search → score. It asserts (a) every produced summary's `content` **byte-equals** the committed fixture target (correctness+determinism); (b) baseline nDCG@10 is capped; (c) the lift delta clears a floor; (d) a **structural** per-query check that the consolidated `context` doc is retrieved **only after** reflection (present with-reflections, absent in baseline) — note keyword search sections results by record type, so a `context` doc cannot *outrank* an observation; the meaningful structural claim is therefore retrievability, plus recall@10 → 1.0. The token-coverage property (no single grade-1 doc covers all of a thematic query's terms) is a **manual fixture-authoring constraint** of `grafiki_themes_v1` (verified by inspection), not a runtime `validate()` check. |
| C5 | MED | idempotency | **`dedup_key` does NOT depend on PPR/content order.** It is `sha256(scope ∥ sorted member_entity_ids ∥ sorted SET of currently-valid source-observation ids)`. Re-running after PPR re-ranking yields the **same** key; it changes only when membership or the source-fact set changes. Member salience for ranking uses **within-community weighted degree** (a stable, local integer-ish sum), **not** global PPR (which is globally volatile). |
| C6 | MED | monster-community | A `MAX_COMMUNITY_SIZE` guard (default 25): a community larger than this is **skipped and reported** (`skipped_too_large`) rather than producing one meaningless mega-summary. A hub-and-spoke unit test asserts a hub bridging two cliques does not collapse them into one community. |
| C7 | MED | scope | v1 detects over a **single scope** (the run scope), not the whole chain — the subgraph + observation loads filter `scope = run_scope`. No mixed-scope community, so the candidate's single scope is always correct. |
| C8 | LOW | determinism | Keyword tokenization inlines `fts5_terms_query`'s exact split predicate (`!c.is_alphanumeric() && c!='_' && c!='-'`), lowercases explicitly, and filters by **char count > 1**. The stop-list is a sorted `const &[&str]`. Unit-tested byte-identical across runs. |
| C9 | LOW | idempotency | Two-part dedup. The `extraction_candidates` check (`json_extract(payload,'$.dedup_key') = ?` scoped to the run scope, status incl. `rejected` so a human rejection is durable) is what `--force` bypasses. The `context.key` existence check is **unconditional** — it skips even under `--force`, because `context.key` is UNIQUE and `add_context` is a plain INSERT, so re-approving a colliding key would error at approval. This makes the unique constraint a backstop, never a crash path. |
| C10 | LOW | model-free | The model-free guarantee depends on `record_type == "context"` keeping the candidate off the `#[cfg(feature="fastembed")]` observation-conflict branch in `propose_candidate`; the CI gate asserts it runs on default features. |

**Module layout (refines §4.8):** the *pure* algorithm (`Community`, `detect_communities`) lives in
`graph.rs`; the *pure* summary core (types, `build_summary`, keyword/dedup helpers) lives in new
`reflection.rs`; the `run_reflection` **orchestrator** (DB I/O, redaction, dedup, `propose_candidate`)
lives in `memory.rs` — mirroring how `conflict.rs`/`graph.rs` are pure cores that `memory.rs` drives.

---

## 1. Goal & scope

### 1.1 Why H5 exists

Grafiki's retrieval through H4 is **entity-local**: every result is a single observation, decision,
context doc, or entity. Some questions are **thematic / global** — they are not answered by any one
record but by a *consolidation* of a tightly-related region of the graph:

- "What are the themes in the auth subsystem?"
- "What did we decide about caching, across sessions?"
- "What are the cross-cutting security concerns?"

H5 detects **communities** in the in-scope relations graph, builds a **deterministic, extractive
summary** of each, and enters it as a **pending candidate** that — once a human approves it —
becomes a retrievable record answering the thematic query. (Refs: Generative-Agents *reflection*,
GraphRAG *community summaries*, LightRAG, HippoRAG. Grafiki keeps the structure but drops the
LLM-summarize step, which those systems all assume.)

### 1.2 v1 ships

1. A deterministic, model-free **community detector** in `grafiki-core` (new `reflection.rs`),
   reusing `Graph` from `graph.rs`.
2. A deterministic **extractive summarizer** that selects and arranges existing observation text
   (no generation), ranked by PPR + confidence.
3. A `run_reflection` orchestrator in `memory.rs` that loads the in-scope subgraph (the
   `graph_search_results` pattern), detects communities, builds summaries, and **proposes each as a
   `context` candidate** (record_type `context`, category `architecture`) with `evidence_links` to
   every source observation, an idempotent dedup key, and redaction.
4. A `grafiki reflect` CLI command (off by default; never auto-runs).
5. **Eval Arm E** (`grafiki_themes_v1` fixture + `reflection_regression.rs`): proves that
   thematic queries lift from grade-0/1 raw observations to grade-2 community summaries, and that
   detection is byte-identical across runs. Model-free, runs in the CI eval-gate.

### 1.3 Deferred (explicitly not v1)

- Multi-level / hierarchical communities (recurse Louvain on the community graph).
- Leiden connectivity-refinement (drop-in over the v1 partition, no API change).
- Temporal community evolution and cross-scope stitching.
- Any LLM-augmented summary — strictly `#[cfg(feature = "llm-summaries")]`, never on the default
  path, never required by an eval.
- MCP exposure of `reflect` (CLI + library only in v1).
- A dedicated `community_summaries` table (see §4.6 for why we reject it in v1).

---

## 2. Community detection

### 2.1 Decision: deterministic single-level **Louvain** (greedy modularity)

| Algorithm | Determinism | Structure-aware | Notes |
|---|---|---|---|
| Connected components | yes | no | one weak edge merges two real themes into one blob; useless as a theme detector |
| Label propagation (sync) | only with a forced canonical order | no | optimizes nothing; oscillates; needs the same tie-break work as Louvain for no benefit |
| **Louvain, single-level (chosen)** | **yes, with explicit tie-breaks** | **yes (maximizes modularity)** | resists the giant-blob failure; proven in GraphRAG/LightRAG; Leiden is a later drop-in |
| Leiden | yes | yes | strictly better partitions but more code; deferred to v2 |

**Conflict resolution.** The three input designs split between "degree-ordered greedy clustering"
(retrieval-value angle) and "Louvain" (algorithm + integration angles). We pick **Louvain**: the
degree-ordered scheme has a tunable `weight ≥ 0.5` / `shared-neighbors ≥ 2` threshold that is a free
parameter with no principled value and no modularity objective, whereas Louvain optimizes a single
well-defined quantity and degrades gracefully. The cost (a ∆-modularity computation) is small at
Grafiki's scale (typically < 100 entities per scope).

### 2.2 Determinism — the exact rules

`Graph` already stores nodes in a `BTreeSet<String>` and adjacency in a `BTreeMap`, so iteration
order is lexicographic and stable. On top of that, every choice point is pinned:

1. **Initialization.** Each node is its own community. Community ids are assigned by **lexicographic
   node order** (the first node in `BTreeSet` order is community 0, etc.). No randomized seeding.
2. **Visit order.** Nodes are visited in `BTreeSet` (lexicographic) order each sweep.
3. **Move target.** For a node, candidate communities = {its current community} ∪ {community of each
   neighbor}. Compute ∆Q for each. Pick the **maximum ∆Q**; on a tie, pick the **lowest community
   id**. A move is taken only if ∆Q **> `MOVE_EPSILON`** (`1e-12`) — strictly positive past a float
   guard, so float noise never triggers a spurious move and breaks determinism. A node never leaves
   its community for a non-improving target.
4. **Convergence.** Sweep until a full sweep makes no move, or `MAX_SWEEPS = 100` (knowledge graphs
   converge in well under 20). Fixed cap ⇒ identical iteration count for identical input.
5. **Renumbering.** After convergence, relabel surviving communities to a dense `0..k` range, again
   in lexicographic order of their **lowest-id member**, so ids are a pure function of membership.

Because there is no randomness and every tie is broken lexicographically, the partition is a pure
function of the `(nodes, edges, weights)` triple — i.e. of the store.

### 2.3 Singleton / min-size handling

- The detector returns **all** communities (including singletons) so callers can reason about
  coverage.
- The orchestrator (`run_reflection`) **only summarizes communities with `size ≥ MIN_COMMUNITY_SIZE`
  (default 2)**. A single entity is not a "theme"; its lone observation is already retrievable
  atomically. Singleton entities are left untouched — no summary, no candidate.
- A community must also have **≥ 1 currently-valid observation across its members** to be
  summarizable; an all-entities-no-observations community is skipped (nothing to extract).

### 2.4 Signatures — fit `graph.rs` exactly

`graph.rs` keeps `out`/`nodes` private. We add the read-only accessors Louvain needs, plus the new
detector. These are the **only** additions to `graph.rs`; the PPR core is untouched.

```rust
// crates/grafiki-core/src/graph.rs  (additions)

impl Graph {
    /// Lexically-ordered node ids (already backed by a BTreeSet).
    pub fn nodes(&self) -> impl Iterator<Item = &str> { self.nodes.iter().map(String::as_str) }

    /// (neighbor, weight) pairs for `node`, in insertion order; empty if absent.
    /// Note: the same neighbor can appear more than once if multiple relations
    /// connect the pair — callers that need a per-neighbor weight sum must fold.
    pub fn neighbors(&self, node: &str) -> &[(String, f64)] {
        self.out.get(node).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Σ of incident edge weights (weighted degree), folding multi-edges.
    pub fn weighted_degree(&self, node: &str) -> f64 {
        self.out.get(node).map(|e| e.iter().map(|(_, w)| *w).sum()).unwrap_or(0.0)
    }

    /// 2·m — total weighted degree over all nodes (each undirected edge counted twice,
    /// matching the symmetric storage). Modularity's normalizer.
    pub fn total_degree(&self) -> f64 {
        self.nodes.iter().map(|n| self.weighted_degree(n)).sum()
    }
}

/// A detected community: member node (entity) ids in lexicographic order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Community {
    /// Dense, deterministic id in `0..k` (see §2.2 renumbering).
    pub id: usize,
    /// Member entity ids, lexicographically sorted.
    pub members: Vec<String>,
}

/// Deterministic single-level Louvain. Returns ALL communities (incl. singletons),
/// `id`-ascending, each `members` lexicographically sorted. Empty graph ⇒ empty Vec.
/// Pure: no I/O, no randomness, no clock.
pub fn detect_communities(graph: &Graph) -> Vec<Community>;
```

`detect_communities` keeps the partition as a `BTreeMap<&str, usize>` over `graph.nodes()` and the
per-community degree sum (`a_c`) in a `BTreeMap<usize, f64>` updated incrementally on each move. The
∆Q of moving node *i* (weighted degree `k_i`, edge weight `k_{i,C}` into community `C`) is the
standard single-level Louvain gain, with `m2 = graph.total_degree()`:

```
ΔQ(i → C) = (k_{i,C} - k_{i,Cur}) / (m2/2)
          - k_i * ( a_C - (a_Cur - k_i) ) / ( (m2/2)^2 )   // /2 because m = m2/2
```

(Implementation folds multi-edges with `weighted_degree`; `k_{i,C}` sums `neighbors(i)` whose
partition is `C`, including the self-loop term as zero since `add_edge` drops self-edges.)

### 2.5 Complexity

Let `V` = entities in scope, `E` = relations, `d̄` = mean degree. One sweep is
`O(V·d̄) = O(E)` (each node inspects its neighbors once; ∆Q is `O(1)` given the maintained `a_c`
table). With the fixed sweep cap that is `O(MAX_SWEEPS · E)`, i.e. **`O(E)`** in practice. Graph
load is the existing single scoped SQL scan, `O(E)`. Space is `O(V + E)`. At Grafiki's per-scope
scale this is sub-millisecond.

### 2.6 Failure modes the design already handles

- **Giant blob.** Louvain's modularity objective resists merging weakly-linked clusters; a single
  low-weight bridge between two dense groups is cut because keeping it lowers Q. (LPA/components do
  not have this property — another reason Louvain wins.)
- **Disconnected components** become separate communities automatically (no edges to merge across).
- **Star / hub graph** keeps the hub with whichever leaf-group maximizes Q rather than absorbing all
  leaves; a `min-size` filter then drops any 1-node residue.

---

## 3. Extractive summary (deterministic, no LLM)

### 3.1 Decision: extract-and-arrange, ranked by PPR × confidence

A summary is a **template-filled arrangement of verbatim observation text**. Nothing is generated.
The text is a pure function of the community's members and their currently-valid observations, so it
is byte-identical across runs.

### 3.2 Representative-observation selection (the ranking)

For a community `C`:

1. **Member salience.** Run the existing `personalized_pagerank` on the **same in-scope subgraph**,
   seeded uniformly on `C`'s members (`DEFAULT_DAMPING`, `DEFAULT_MAX_ITERS`, `DEFAULT_TOLERANCE`).
   This reuses H3's deterministic core verbatim and gives each member a salience score `ppr[m]`.
   (Conflict resolution: the algorithm angle proposed plain degree ranking; PPR is already present,
   deterministic, and strictly more informative, so we reuse it.)
2. **Candidate observations.** For each member, load its currently-valid observations
   (`valid_to IS NULL`) with `(content, category, confidence)`.
3. **Score.** `score(obs) = confidence × ppr[owning_member]`.
4. **Deterministic order.** Sort observations by `score` **descending**, breaking ties by
   `(category_rank, entity_id, observation_id)` ascending — all stable, store-derived keys.
   `category_rank` is a fixed table favoring decisional/structural facts:
   `decision(0) < architecture(1) < risk(2) < pattern(3) < convention(4) < gotcha(5) <
   learned(6) < dependency(7) < blocker(8) < preference(9) < progress(10) < general(11)`
   (every value in the observations `category` CHECK is covered; unknown ⇒ 99).
5. **Cap.** Keep the top `MAX_OBS_PER_SUMMARY` (default 8) to stay scannable. Every kept observation
   becomes one evidence link (§4.3).

### 3.3 Theme label & keywords (deterministic)

- **Label** = the member **entity names** (looked up from `entities.name`), sorted lexicographically
  and joined with `, `, truncated to `MAX_LABEL_LEN` (default 80) on a word boundary, e.g.
  `"Auth Service, JWT Library, Redis Cache, Refresh Token Cache"`. No invented prose.
- **Keywords** = top-N TF terms across the kept observations' text, computed deterministically:
  lowercase, split on the same non-alphanumeric rule `fts5_terms_query` uses, drop tokens of length
  ≤ 1 and a fixed built-in stop-list, count, then sort by `(count desc, token asc)` and take N
  (default 12). Pure counting + lexical tie-break ⇒ deterministic. (We deliberately use raw TF, not
  TF-IDF: IDF would depend on corpus size and make the output non-local to the community.)

### 3.4 Payload JSON shape (the `context` candidate payload)

The summary is proposed as a **`context`** candidate. `approve_candidate_payload`'s `"context"` arm
calls `add_context`, which requires `key`, `title`, `content`, and a `category` from the context
CHECK set — so the payload uses exactly those field names. (See §4.1 for why `context`, not
`observation`.)

```json
{
  "key": "reflection-eval-2f9c1a7b",
  "title": "Theme: Auth Service, JWT Library, Redis Cache, Refresh Token Cache",
  "category": "architecture",
  "content": "Community theme across 4 entities: Auth Service, JWT Library, Redis Cache, Refresh Token Cache.\n\nKey facts:\n- [JWT Library] JWT refresh tokens rotate every 15 minutes and are stored hashed, never in plaintext.\n- [Auth Service] Auth endpoints rate-limited to 10 requests per second per IP to prevent brute force.\n- [Refresh Token Cache] Refresh token reuse within 5 seconds is rejected; prevents token theft during rotation.\n- [JWT Library] Token blacklist checked on every request; invalidated tokens are cached in Redis for 1 hour.\n- [Redis Cache] Redis entries expire automatically after 1 hour TTL; no manual cleanup needed.\n- [Auth Service] Session cookies are HttpOnly, Secure, SameSite=Strict; no JavaScript access.\n\nKeywords: auth, token, refresh, redis, rotate, hashed, blacklist, ttl, rate, cookies.",
  "members": ["auth-service", "jwt-library", "redis-cache", "refresh-token-cache"],
  "reflection_version": 1,
  "dedup_key": "2f9c1a7b"
}
```

- `content` is the only human-facing field; it is the extracted, arranged text above (the part that
  gets FTS-indexed via `context_fts(title, content)`).
- `members`, `reflection_version`, `dedup_key` are carried for tooling/audit; `add_context` ignores
  unknown fields (it reads only `key/title/content/category`), so no schema change is needed.
- `category` is `"architecture"` (a valid context category and the closest semantic fit). It is
  **not** `"reflection"`/`"thematic"`/`"summary"` — those are **not** in the context CHECK set and
  would be rejected at approval. (Conflict resolution: two input designs used `category:"thematic"`
  or `"summary"` / `"reflection"`; all three violate the real CHECK constraints in
  `db/schema.rs` — observations allow only `general|architecture|decision|blocker|pattern|progress|
  gotcha|learned|preference|convention|dependency|risk`, context allows only `architecture|audit|
  spec|reference|onboarding|runbook|postmortem|guide`. We use `architecture`.)

---

## 4. Candidate integration

### 4.1 Record type: **`context`** (decision)

A reflection is approved as a **`context`** record, not an `observation`.

**Why.** `approve_candidate_payload`'s `"observation"` arm calls `save_entity(observe=…)`, which
**creates/reuses a backing entity** and attaches the text to it. A community summary has no single
owning entity — forcing one would pollute the entity table and the graph with a synthetic node and
mislead PPR. The `"context"` arm calls `add_context`, which stores a free-standing keyed document,
indexes it in `context_fts(title, content)`, and enqueues an embedding job — exactly the shape of a
consolidated summary, and it is already searched by `search_keyword_memory` / `search_semantic_memory`
under `record_type = "all"` (the eval default). **No new record type, no schema migration.**

(Conflict resolution: the retrieval-value and integration angles proposed
`record_type = "observation"` plus a synthetic `community`/`comm-*` entity and `part_of` relations.
That adds synthetic graph nodes that feed back into H3 PPR and a second indexing path. We reject it:
`context` is retrievable today, is free-standing, and needs zero new wiring.)

### 4.2 `propose_candidate` call

`run_reflection` builds one `ProposeCandidateOptions` per summarizable community:

```rust
ProposeCandidateOptions {
    project_name, start_dir, grafiki_home,           // threaded from RunReflectionOptions
    source_type: "reflection".to_string(),           // free-form; validate_candidate_source_type only requires non-empty
    source: Some("reflection:louvain:v1".to_string()),
    record_type: "context".to_string(),
    payload,                                          // §3.4 JSON object
    scope: scope.as_str().to_string(),                // the run's scope (top of the chain)
    confidence,                                       // §4.4
    rationale: Some(format!(
        "Community reflection over {} entities (modularity contribution {:.4}); \
         {} source observations.", members.len(), q_contribution, evidence.len())),
    evidence,                                         // §4.3
}
```

`propose_candidate` already redacts the payload (`redact_json_value`) and the rationale
(`redact_sensitive_text`) before insert, so §4.5 is satisfied for free.

### 4.3 Evidence / provenance

One `EvidenceInput` per kept observation (§3.2), so the pending candidate carries a verbatim trail
back to every source fact. On approval, `promote_candidate_evidence` repoints these links at the new
trusted `context` record.

```rust
EvidenceInput {
    source_event_id: None,
    source_type: "reflection".to_string(),
    source: Some("reflection:louvain:v1".to_string()),
    title: Some(entity_name.clone()),     // the member that owns this observation
    excerpt: observation_content.clone(), // verbatim; redacted by propose_candidate
    uri: Some(format!("grafiki://observation/{observation_id}")),  // stable pointer to the source obs
    byte_start: None, byte_end: None, line_start: None, line_end: None,
    captured_at: None,
}
```

`evidence_links` has no FK to a trusted observation id, but the `uri` carries
`grafiki://observation/<id>` so the provenance is machine-resolvable. (`trusted_record_id` is
populated at approval by `promote_candidate_evidence` to point at the *context* record; the source
obs ids live in `uri`.)

### 4.4 Confidence

`confidence = clamp(mean(member observation confidences) × cohesion, 0.0, 1.0)`, where `cohesion` is
the community's modularity contribution mapped into `[0.5, 1.0]` (`0.5 + 0.5·min(1, Q_c/Q_ref)`,
`Q_ref = 0.3`). A tight, high-confidence community proposes near 1.0; a loose one is discounted but
never below 0.5. Deterministic (all inputs are store-derived). Passes
`validate_candidate_confidence` (must be in `[0,1]`).

### 4.5 Redaction

No new redaction code. Summary `content`, `title`, payload extras, and rationale all flow through
`propose_candidate`, which calls `redact_json_value(&mut payload)` and `redact_sensitive_text` on the
rationale before persistence. Evidence excerpts are also redacted because they are verbatim
observation text that was *already* redacted at its own ingest, and they pass through
`insert_candidate_evidence`. Belt-and-suspenders, zero added surface.

### 4.6 Idempotency / dedup key — and the unique-`key` hazard

Running `grafiki reflect` twice on an unchanged store must not create duplicate candidates, and an
approved reflection must not collide on `context.key` (which is `UNIQUE` — a second `add_context`
with the same key throws).

**Dedup key.** `dedup_key = hex(sha256( scope || '\u{1}' || sorted_member_ids.join('\u{1}') ||
'\u{1}' || sha256(content) ))[..16]`. It is a pure function of *scope + membership + summary text*,
so it is stable while the community and its facts are unchanged, and changes when either drifts.

**Context key.** `key = format!("reflection-{}-{}", scope_slug, dedup_key)`. Because the key embeds
the content hash, a *content drift* yields a *new* key (a new candidate proposing a fresh summary),
while an *unchanged* community yields the *same* key — so the unique-`key` constraint becomes the
backstop, not a crash.

**Dedup check (before propose).** `run_reflection` queries `extraction_candidates` for a
pending-or-approved candidate in this scope whose payload `dedup_key` matches; and queries `context`
for an existing row with this `key`. If either exists and `--force` is not set, the community is
**skipped** (counted in the report), and no candidate is created. With `--force`, the dedup check is
bypassed but the unique-`key` backstop still prevents a duplicate *approval*.

### 4.7 Scope & bitemporal

- Load **only** the in-scope, currently-valid subgraph: the exact SQL from `graph_search_results`
  (`relations` joined to `entities` on both endpoints, `r.valid_to IS NULL`, both scopes
  `IN (scope_chain)`). Observations are loaded with `valid_to IS NULL`. No cross-scope leakage.
- Proposed candidate's `scope` = the run's scope (top of the chain), inherited by the trusted
  `context` row. `context` is **versioned, not bitemporal** (it has `version`, no `valid_to`), which
  is fine: a reflection is a current consolidated view; supersession of stale reflections is handled
  by the dedup key minting a new keyed doc, not by `valid_to`.

### 4.8 Every file touched

**New**
- `crates/grafiki-core/src/reflection.rs` — `RunReflectionOptions`, `ReflectionReport`,
  `CommunityDetail`, `run_reflection`, and the private extractive summarizer + dedup helpers.
  (`detect_communities` + `Community` live in `graph.rs` with the other pure graph code.)
- `crates/grafiki-eval/fixtures/retrieval/grafiki_themes_v1/` — `corpus_seed.jsonl`,
  `relations.jsonl`, `queries.jsonl`, `qrels.tsv`, `dataset.json` (§5).
- `crates/grafiki-eval/src/runner/reflection.rs` — Arm E runner (§5).
- `crates/grafiki-eval/tests/reflection_regression.rs` — CI gate (§5).

**Modified**
- `crates/grafiki-core/src/graph.rs` — add `nodes()/neighbors()/weighted_degree()/total_degree()`
  accessors, `Community`, `detect_communities` (+ unit tests). PPR untouched.
- `crates/grafiki-core/src/lib.rs` — `pub use graph::{Community, detect_communities};` and
  `pub use reflection::{run_reflection, RunReflectionOptions, ReflectionReport, CommunityDetail};`
  (add `pub mod reflection;`).
- `crates/grafiki-core/src/memory.rs` — extract the in-scope subgraph loader used by
  `graph_search_results` into `fn load_scope_subgraph(connection, scope_chain) -> Result<Graph>`,
  and call it from both `graph_search_results` (refactor) and `reflection::run_reflection`. Also
  expose a small `pub(crate)` helper to load currently-valid observations for an entity (the query
  already exists inline in `graph_search_results`). No behavior change to existing search.
- `crates/grafiki-cli/src/main.rs` — add the `Reflect` subcommand (off by default; never auto-runs).
- `crates/grafiki-eval/src/runner/mod.rs` — `pub mod reflection;`
- `crates/grafiki-eval/src/main.rs` — add `--arm reflection` dispatch (optional; the CI test is the
  hard gate regardless).

### 4.9 Public API (in `reflection.rs`)

```rust
#[derive(Debug, Clone)]
pub struct RunReflectionOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub scope: String,
    pub min_community_size: usize,   // default 2
    pub max_obs_per_summary: usize,  // default 8
    pub confidence_floor: f64,       // applied after §4.4 mapping; default 0.5
    pub force: bool,                 // bypass dedup check (§4.6)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectionReport {
    pub project: String,
    pub scope: String,
    pub communities_detected: usize,         // all, incl. singletons
    pub communities_summarized: usize,       // size >= min and >=1 obs
    pub candidates_created: usize,
    pub skipped_existing: usize,             // dedup hits
    pub details: Vec<CommunityDetail>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommunityDetail {
    pub community_id: usize,
    pub member_entity_ids: Vec<String>,
    pub member_entity_names: Vec<String>,
    pub observation_count: usize,            // kept (post-cap)
    pub modularity_contribution: f64,
    pub dedup_key: String,
    pub candidate_id: Option<String>,        // None when skipped
    pub status: String,                      // "created" | "skipped_existing" | "skipped_too_small"
}

/// Orchestrator: load in-scope subgraph → detect_communities → for each
/// summarizable community, build the extractive summary and propose a `context`
/// candidate (dedup-guarded). Pending only; never auto-approves.
pub fn run_reflection(options: RunReflectionOptions) -> Result<ReflectionReport>;
```

CLI:

```rust
// crates/grafiki-cli/src/main.rs  (new subcommand; default-off, manual only)
/// Detect entity communities and propose extractive reflection summaries (pending review).
Reflect {
    #[arg(long)] project: Option<String>,
    #[arg(long, default_value = "")] scope: String,
    #[arg(long, default_value_t = 2)] min_size: usize,
    #[arg(long)] force: bool,
    #[arg(long, default_value = ".")] path: PathBuf,
    #[arg(long, value_enum, default_value_t = OutputFormat::Plain)] format: OutputFormat,
}
```

---

## 5. Eval Arm E — thematic lift

### 5.1 Thesis (the metric only moves because of consolidation)

A thematic query is graded so that **no single raw observation can fully answer it** — raw
observations are partial (grade 1) and the **community summary is the exact answer (grade 2)**. With
only raw records, nDCG@10 is capped (you can never retrieve the grade-2 doc because it doesn't
exist); once the reflection `context` doc is in the store, the same query retrieves it and nDCG@10
jumps. The arm reports **baseline (raw only)** vs **with-reflections** and asserts the delta.

This is structurally Arm D's pattern (seed → mutate the store through the real candidate gate →
re-probe), so Arm E reuses the `seed_retrieval` corpus loader and `search_memory`, plus
`propose_candidate`/`approve_candidate` for the "with-reflections" pass.

### 5.2 Fixture shape — `grafiki_themes_v1`

Reuses the **BEIR-triple** loader (`RetrievalDataset::load`) so the fixture is just the standard
files. Directory `crates/grafiki-eval/fixtures/retrieval/grafiki_themes_v1/`:

- `corpus_seed.jsonl` — entities + observations of 3–4 natural clusters (auth/token, observability,
  storage/transport, security/redaction). Same record shapes the seeder already understands
  (`entity`: `{name, entity_type, category}`; `observation`: `{name, entity_type, category, text}`).
- `relations.jsonl` — edges making the clusters detectable (`{from, to, relation}` by `doc_id`,
  relations from the schema CHECK set: `uses|depends_on|calls|produces|related_to|…`).
- `queries.jsonl` — thematic queries (`{_id, text}`).
- `qrels.tsv` — TREC `query-id <TAB> corpus-id <TAB> grade`; raw observations graded 1, the
  reflection summary doc graded 2.
- `dataset.json` — `{name, version, description}`.

**The reflection summary as a corpus doc.** The grade-2 target is itself a corpus doc of
`record_type: "context"` whose `content` is the *expected extractive summary* for that cluster (the
deterministic output of §3 for these fixtures). It is `doc_id: "comm-auth-security"` etc.

The arm runs in two passes against the *same* fixture:

- **Baseline pass:** seed via `seed_retrieval` but **skip** the `record_type == "context"` reflection
  docs (filter them out before seeding). Search every query → score → these are the "raw only"
  numbers. The grade-2 doc is unreachable ⇒ low nDCG@10.
- **With-reflections pass:** run `grafiki_core::run_reflection` on the seeded store **or** seed the
  reflection `context` docs directly (v1 uses the latter for a hermetic, model-free fixture: the
  expected summary text is committed, so the metric tests *retrievability of a consolidated doc*,
  not the summarizer's exact wording). Re-search → score → high nDCG@10.

(Conflict resolution: the retrieval-value angle authored a full themed fixture with
`category:"thematic"` observation docs. We keep its corpus/queries/qrels design but (a) make the
grade-2 doc a `context` record, not a `category:"thematic"` observation — the latter violates the
observations CHECK — and (b) drop the bespoke `communities.jsonl` shape in favor of the standard
BEIR triple the loader already parses, so no new dataset format is added.)

### 5.3 qrels design (sketch)

```
# query-id            corpus-id              grade
q-theme-auth          obs-jwt-rotate          1
q-theme-auth          obs-auth-rate-limit     1
q-theme-auth          obs-refresh-no-reuse    1
q-theme-auth          comm-auth-security      2     # the reflection context doc — only full answer
q-theme-storage       obs-sqlite-schema       1
q-theme-storage       obs-cli-offline         1
q-theme-storage       comm-storage-transport  2
q-theme-security      obs-redaction-keys      1
q-theme-security      obs-http-localhost      1
q-theme-security      comm-security-redaction 2
...
```

Each thematic query has several grade-1 partial facts and exactly one grade-2 reflection doc.

### 5.4 Runner & report

`crates/grafiki-eval/src/runner/reflection.rs`:

```rust
pub struct ReflectionArmReport {
    pub dataset_name: String,
    pub query_count: usize,
    pub baseline: AggregateScores,        // raw observations only
    pub with_reflections: AggregateScores,// + reflection context docs
    pub delta_ndcg_at_10: f64,            // with - baseline
    pub deterministic: bool,              // detect_communities run twice == same partition
}

/// Model-free: keyword mode only (no embeddings needed; the lift is purely from a
/// retrievable consolidated doc, scored by the same IR metrics as Arm A).
pub fn run_reflection_arm(dataset: &RetrievalDataset, cfg: &EvalConfig)
    -> EvalResult<ReflectionArmReport>;
```

It scores with the same `metrics::ir::evaluate` used by Arm A, and additionally seeds a tiny
synthetic graph, calls `grafiki_core::detect_communities` twice, and sets `deterministic` to
`partition_1 == partition_2`.

### 5.5 CI regression test (the hard gate)

`crates/grafiki-eval/tests/reflection_regression.rs`:

```rust
#[test]
fn reflection_arm_e_communities_lift_thematic_retrieval() {
    let ds = RetrievalDataset::load(&fixtures().join("retrieval/grafiki_themes_v1")).unwrap();
    let report = run_reflection_arm(&ds, &EvalConfig::default()).unwrap();

    // Communities must be byte-identical across runs (the trust invariant).
    assert!(report.deterministic, "community detection must be deterministic");

    // Raw observations can't fully answer a thematic query → capped baseline.
    assert!(report.baseline.macro_avg["ndcg@10"] < 0.60,
        "baseline nDCG@10 should be capped without consolidation: {}",
        report.baseline.macro_avg["ndcg@10"]);

    // The retrievable reflection doc lifts it.
    assert!(report.with_reflections.macro_avg["ndcg@10"] > 0.85,
        "reflections should lift thematic nDCG@10: {}",
        report.with_reflections.macro_avg["ndcg@10"]);

    assert!(report.delta_ndcg_at_10 > 0.30,
        "thematic lift must exceed 0.30: {}", report.delta_ndcg_at_10);
}
```

Plus a pure unit test in `graph.rs`:

```rust
#[test]
fn detect_communities_is_deterministic_and_splits_two_cliques() {
    // two 3-cliques joined by one weak edge → exactly two communities, stable across runs
}
```

### 5.6 Model-free guarantee

Arm E uses **keyword** search only and a **committed expected-summary** corpus doc, so it needs no
embedding model and runs in the fast, model-free CI matrix (like Arm D and the keyword/graph Arm-A
checks). `detect_communities` and the summarizer are pure (no clock, no RNG, no model). The
`#[cfg(feature = "llm-summaries")]` path, if ever added, is never exercised by this arm.

---

## 6. Risks & mitigations

| # | Risk | Mitigation |
|---|---|---|
| 1 | **Non-determinism** from tie-breaks / float noise (the classic Louvain failure). | Lexicographic init, visit, and target tie-breaks; moves require ∆Q > `1e-12`; fixed `MAX_SWEEPS`; dense renumber by lowest member id. Asserted by `detect_communities_is_deterministic_*` and Arm E's `deterministic` flag (§2.2, §5.5). |
| 2 | **Accidental LLM dependency** creeping into the default path. | Summarizer is template + verbatim extraction; any generative refiner is `#[cfg(feature = "llm-summaries")]` and never required by an eval. Arm E is keyword-only / model-free. |
| 3 | **Auto-writing untrusted reflections** to the store. | `run_reflection` only ever calls `propose_candidate` (status `pending`). Approval is a separate, human step (`approve_candidate`). The CLI never auto-runs. (Constraint 3.) |
| 4 | **Schema CHECK violation at approval** from an invalid category. | Payload uses `category: "architecture"` (in the context CHECK set) and `record_type: "context"`; verified against `db/schema.rs`. The two input designs' `thematic`/`summary`/`reflection` categories are rejected (§3.4). |
| 5 | **Duplicate candidates / `context.key` unique-constraint crash** on re-run. | Content-hash `dedup_key` → stable `context.key`; pre-propose dedup check across `extraction_candidates` + `context`; the `UNIQUE(key)` constraint is the backstop, not a crash path (§4.6). |
| 6 | **Giant-blob / weak-bridge mis-clustering.** | Modularity objective cuts weak bridges; `MIN_COMMUNITY_SIZE` drops residue singletons; Louvain chosen precisely for this over components/LPA (§2.1, §2.6). |
| 7 | **Cross-scope leakage** in a multi-tenant store. | Subgraph + observation loads are restricted to `scope_chain` with the exact `graph_search_results` SQL; candidate inherits the run scope (§4.7). |
| 8 | **Synthetic graph nodes polluting H3 PPR** (the rejected "community entity" approach). | `context` records are free-standing and not entities, so they never enter the relations graph or perturb PPR (§4.1). |
| 9 | **Secret leakage** through consolidated text. | All payload/rationale/evidence flow through `propose_candidate`'s `redact_json_value` / `redact_sensitive_text`; source observations were already redacted at their own ingest (§4.5). |
| 10 | **Eval that "passes" without real consolidation** (e.g. a raw obs accidentally satisfying the grade-2 doc). | Grade-2 is a distinct `context` doc absent from the baseline pass; baseline nDCG@10 is asserted *capped* (`< 0.60`) and the *delta* (`> 0.30`) is asserted, so the metric can only clear the gate when the consolidated doc is present and retrieved (§5.1, §5.5). |
| 11 | **Quadratic blow-up** on a large scope. | One sweep is `O(E)` with the maintained per-community degree table; fixed sweep cap; sub-ms at Grafiki's per-scope scale (§2.5). |
