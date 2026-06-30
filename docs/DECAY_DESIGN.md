# M-E1 + M-E2 — Forgetting/decay & temporal-aware retrieval

**Status:** design, build-ready. **Depends on:** the hybrid RRF fusion (`add_hybrid_scores` /
`hybrid_search_results`), the bitemporal `observations.valid_from`, and the `agent_queries` audit
log (`returned_ids` = JSON array of `"type:id"`). **Borrows (MIT, deterministic):** mnemosyne
`weibull.py` per-type decay + `query_intent`/recency framing; `arXiv:2604.26970` (per-predicate
volatility); MemoryBank `2305.10250` (Ebbinghaus). See `docs/COMPETITIVE_LANDSCAPE.md` §6.

Combines two roadmap items that share the recency machinery:
- **M-E2 temporal-aware retrieval** — recent currently-valid facts should outrank topically-similar
  but older ones.
- **M-E1 forgetting/decay & salience** — reinforce records the agent actually reuses (from the audit
  log); de-emphasize stale, never-touched ones.

## Hard constraints
Deterministic & local-first (no LLM, no clock-dependent CI flakiness); **opt-in and off by default**
so existing eval baselines (Arm A/D/E) and `grafiki_dev_v1` numbers are byte-unchanged; never
hard-deletes (M-E1 archival is gate-mediated and deferred — see §5).

## 1. Pure core — `grafiki_core::decay`
```rust
/// Weibull survival freshness in [0,1]: exp(-(age_hours/eta)^k). Future/zero age → 1.0.
/// k<1 ⇒ slow (long-lived: preference/profile); k=1 ⇒ exponential; k>1 ⇒ fast (events).
pub fn weibull_freshness(age_hours: f64, k: f64, eta: f64) -> f64;

/// Per observation-category (k, eta_hours), adapted from mnemosyne's table to Grafiki's
/// CHECK categories (general/architecture/decision/blocker/pattern/progress/gotcha/learned/
/// preference/convention/dependency/risk). Unknown/non-observation types → a neutral default.
pub fn decay_params(category: &str) -> (f64, f64);

/// Reuse salience in [0,1] from the audit log: blends access volume (log1p, normalized) with
/// the freshness of the most recent access. Zero accesses → 0.
pub fn reuse_salience(access_count: u64, last_access_age_hours: f64) -> f64;
```
All pure, no I/O, no clock → unit-tested for monotonicity + determinism.

## 2. Fusion integration (the boost)
- Add **one** opt-in field to `SearchMemoryOptions`: `temporal_weight: f64` (default `0.0`). All
  existing construction sites pass `0.0` (mechanical; compiler-enforced).
- In `search_memory`'s fused arms (Hybrid / Graph / Rerank only), when `temporal_weight > 0`, build a
  precomputed `boost: HashMap<(record_type, id), f64>` over the union of candidate keys, then pass it
  to `hybrid_search_results`, which adds `boost.get(key)` to each fused score **before** the sort +
  truncate. When the map is empty (weight 0) the fusion is byte-identical to today → **`hybrid_search_results`
  stays pure** (takes a `&HashMap`, does no I/O). The Hybrid/Rerank arms add no candidates beyond
  lexical+dense, so the precomputed map is complete for them; the **Graph arm recomputes** the map over
  the wider union that also includes its PPR-discovered records, so a recent/reused multi-hop
  observation is boost-eligible too (not just the lexical/dense hits).
- **Boost formula** (interpretable, on the RRF scale): for each candidate,
  `boost = temporal_weight · UNIT · (0.6·recency + 0.4·salience)` where `UNIT = 1/(RRF_K+1) ≈ 0.0217`
  (one rank-0 RRF unit), `recency = weibull_freshness(age, decay_params(category))`,
  `salience = reuse_salience(...)`. So `temporal_weight = 1.0` ≈ "a fully-fresh, highly-reused record
  gains ~one RRF rank." Recency dominates (0.6) over salience (0.4).
- **Timestamps** loaded per type from the candidate set: `observations.valid_from` (+ its `category`),
  `decisions.created_at`, `context.updated_at`; types without a meaningful time (entity) → recency 0.
- **Salience** loaded with one query:
  `SELECT je.value, COUNT(*), MAX(aq.created_at) FROM agent_queries aq, json_each(aq.returned_ids) je
   WHERE aq.scope IN (chain) GROUP BY je.value` → `(count, last_access)` per `"type:id"`.
- **Reference "now"** = DB `strftime('%s','now')`. CI determinism is preserved by the eval asserting
  *relative ordering* (recent > old), which is stable for any now ≥ the seeded timestamps — not exact
  metric values against a frozen baseline.

## 3. Surfaces
- CLI: `grafiki search --temporal-weight <f64>` (default 0.0); `grafiki ask` likewise.
- Library: `SearchMemoryOptions.temporal_weight`.

## 4. Eval Arm F — temporal lift (`grafiki_temporal_v1`, model-free, keyword mode)
Seed N topically-near currently-valid observations with controlled `valid_from` (recent vs old) +
audit-log rows giving one of them reuse. Query the shared topic. Assert: (a) baseline
(`temporal_weight=0`) ranks by lexical score only; (b) with `temporal_weight>0` the **recent** (and the
**reused**) record's rank strictly improves vs baseline; (c) determinism: two runs identical. CI test
`temporal_arm_recent_and_reused_rank_higher`. No baseline.json coupling (relative assertions only).

## 5. Deferred to M-E1b (documented, not silently dropped)
Soft-decay **archival candidate** generation (propose retiring stale + never-accessed + low-confidence
observations through the candidate gate, setting `valid_to` on approval) needs a new gate "retire"
action + careful UX, so it ships as a separate increment. v1 delivers the scoring + salience +
temporal-ranking integration that the archival job will later reuse.

## 6. Risks
| Risk | Mitigation |
|---|---|
| CI baseline drift / flakiness | off by default (`temporal_weight=0`); eval uses relative-ordering, not frozen metrics |
| Boost dominates lexical relevance | scaled to one RRF unit; recency-weighted 0.6; capped by `temporal_weight` |
| Non-determinism from `now()` | pure decay fns take explicit age; eval asserts ordering stable ∀ now ≥ seeds |
| Salience query cost | single `json_each` GROUP BY over `agent_queries`, scoped + indexed |
| Reviving superseded facts | only `valid_to IS NULL` rows are retrieved already; recency just reorders the live set |
