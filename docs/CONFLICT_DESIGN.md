# DESIGN: Grafiki H2 — Automated Conflict / Contradiction Resolution

**Status:** Proposed (implementation-ready)
**Crate:** `crates/grafiki-core` (detection + resolution); `crates/grafiki-eval` (Arm D proof); `crates/grafiki-cli` + MCP surface (gate UX)
**Depends on:** existing bitemporal supersession, candidate lifecycle, and embedding APIs in `grafiki-core`
**Owner:** Lead engineer, memory
**Version target:** v1 deterministic-first + model-free; v2 optional NLI/LLM escalation layer behind a feature flag
**Companion:** `docs/EVAL_DESIGN.md` (H1) — this doc adds the supersession / knowledge-update arm to that harness.

---

## 1. Goals and principles

**What H2 claims.** When a new fact contradicts or updates a fact already in trusted memory, Grafiki (a) **detects** the conflict, (b) **routes** it through the human candidate gate (auto-applying only the structurally-certain cases), (c) **resolves** it by bitemporal supersession — closing the old fact's validity window and linking what replaced it, never hard-deleting — and (d) thereafter surfaces the **new** fact and suppresses the **stale** one (or abstains if the only evidence was retracted).

**Principles (load-bearing; each maps to a hard design constraint):**

1. **Deterministic-first.** Detection MUST resolve, with no LLM, every case Grafiki can structurally see: same **state key**, same **decision topic** (via `supersedes`), same **entity attribute / relation** (subject+predicate+scope). The cross-system survey is unambiguous that *winner selection is the most deterministic-able part* — Graphiti reduces it to a hard recency rule; Mem0/MemGPT leave it to LLM "freshness" judgment and pay for it in precision. We adopt the deterministic half Grafiki already has (durable bitemporal store + provenance) that base-Mem0/MemGPT *lack*.
2. **Embedding similarity is a candidate-generation GATE, never a contradiction signal.** Cosine similarity measures *relatedness*, which is necessary-but-not-sufficient for *contradiction*; negated/antonymous sentences ("X is alive" vs "X is dead") routinely get *higher* cosine than unrelated-true pairs (arXiv:2504.16318, arXiv:2403.05440). Embeddings only answer "which trusted facts are even about the same thing"; the conflict *decision* is made by the deterministic structural layer.
3. **Resolve via the EXISTING bitemporal supersession — invalidate, never delete.** Every resolution sets the older row's `valid_to = new.valid_from` (observations/relations) or flips `status='superseded'` + `superseded_by` (decisions), exactly the Graphiti pattern (set old `t_invalid = new t_valid`, retain & queryable). This is what makes even an imperfect auto-apply *safe*: every decision is reversible and the history stays point-in-time queryable.
4. **Route through the CANDIDATE GATE.** A new `conflict` candidate type lets a human confirm before trusted memory mutates. Auto-apply is restricted to the structurally-certain deterministic cases (key/single-valued-slot/explicit-supersede); everything model-flagged or ambiguous is surfaced for review, mirroring the SOTA "soft-mark, route to review" posture for NLI/LLM verdicts.
5. **Arbitrate on metadata, not LLM freshness.** Winner selection is a deterministic policy over **recency (`valid_from`/`captured_at`), source priority, and confidence** — explicitly *not* an LLM's judgment of which fact is "fresher." Confidence/source-priority arbitration is an open differentiator: **no surveyed system (Graphiti, Mem0, Mem0^g, A-Mem, MemGPT) uses explicit source-trust or confidence scores to pick winners** — they all reduce to recency or LLM judgment.
6. **Reuse Grafiki primitives.** Build on `log_decision`'s supersession path (`memory.rs:1358–1382`), `valid_to` soft-invalidation (`delete_observation` `memory.rs:2036`, `delete_relation` `memory.rs:2181`), the candidate lifecycle (`propose_candidate` `memory.rs:3254`, `approve_candidate_payload` `memory.rs:5627`), and the embedding gate (`search_semantic_memory` `memory.rs:6759`, `cosine_similarity` `embeddings.rs:648`).

**Non-goals (v1):** any LLM/NLI on the hot path; confidence-weighted arbitration *learning* (the policy is fixed weights in v1); semantic conflict detection on `state` (state has no embeddings — see §3); auto-applying any model-flagged conflict.

---

## 2. Architecture

### 2.1 Where the conflict hook fires

Two insertion points, both inside the existing candidate lifecycle so trusted memory never mutates without passing the gate.

**(A) Detection at propose time — `propose_candidate` (`memory.rs:3254`) and `propose_capture_candidates` (`memory.rs:3944`).**
After validation and redaction, before the candidate INSERT (the in-place-mutation slot demonstrated by `redact_json_value` at `memory.rs:3275`), run `detect_conflicts(candidate)`:

1. Gate: retrieve related trusted facts of the **same `record_type` and same scope chain** via the embedding gate (§5, Stage 0).
2. Deterministic core (§5, Stage 1): for each retrieved candidate-vs-trusted pair, apply key / slot / explicit-supersede checks.
3. On a deterministic hit, **pre-record the conflict** by populating the candidate's existing `trusted_record_type` / `trusted_record_id` columns (`schema.rs:436–437`; struct fields `memory.rs:647–648` — today only set at approval) and stamp a structured `rationale`. Set an internal `conflict_kind` (auto-safe vs review).
4. Emit a `conflict_detected` event (new event type, §2.3).

This makes the conflict *visible at propose time* without mutating trusted memory — the candidate is the staging area.

**(B) Resolution at approve time — `approve_candidate_payload` (`memory.rs:5627`, dispatch at `memory.rs:5634`).**
Add a match arm for the `conflict` record type (and conflict-annotated standard types) that, inside the existing approval transaction, runs the supersession write against the linked `trusted_record_id` (§3), then returns `(trusted_record_type, trusted_record_id)` exactly as the other arms do so the status flip + evidence promotion at `memory.rs:3413–3423` is unchanged.

**(C) State-specific hook — `upsert_state` (`memory.rs:4046`).**
State conflicts are *silently lost today*: the `INSERT ... ON CONFLICT(key) DO UPDATE` at `memory.rs:4060–4087` is destructive last-writer-wins with no history. v1 adds a pre-`ON CONFLICT` comparison of `excluded.*` vs the existing row (read via `state_id_for_key` `memory.rs:4864`); on a material change it emits the `conflict_detected` event and (when the write arrives via the candidate path) routes through the gate. The destructive overwrite is retained as the *applied* behavior for the auto-safe key case (key supersession is the most auto-applicable layer), but the prior value is captured into the conflict event payload for auditability.

### 2.2 New candidate type

Add `"conflict"` to:
- `validate_candidate_record_type` `RECORD_TYPES` (`memory.rs:5114`), and
- the schema CHECK `proposed_record_type IN ('entity','observation','decision','context','state')` (`schema.rs:427–428`) → add `'conflict'`.

A `conflict` candidate's payload carries: `{ target_record_type, target_record_id, new_value, mechanism, arbitration: {winner, basis, recency, source_priority, confidence}, conflict_kind }`. Standard-type candidates may *also* carry a conflict annotation (the populated `trusted_record_*` columns) when the update is itself a normal record (e.g. an observation that supersedes another); the `conflict` type is reserved for cases where the proposal is *purely* a supersession decision.

### 2.3 Schema changes

Minimal and additive; the heavy lifting reuses existing columns.

1. **Candidate CHECK** extended with `'conflict'` (`schema.rs:427`). No new candidate columns — `trusted_record_type`/`trusted_record_id` (`schema.rs:436–437`) already exist and are repurposed to mean "this candidate conflicts with trusted record X."
2. **Events vocabulary** (`events.event_type` CHECK, `schema.rs:355–364`): add `conflict_detected` and `record_superseded`. (`decision_superseded` and `observation_invalidated` already exist; `record_superseded` generalizes the supersede-link for observations/relations/context which currently emit only invalidation, not a *what-replaced-it* link.)
3. **Supersede-link for non-decision records.** Decisions record what replaced them (`superseded_by` FK, `schema.rs:211`); observations/relations/context have `valid_to` but **no link recording WHAT superseded them**. v1 records this link *in the `record_superseded` event payload* (`target_id` = old, `summary`/payload = new id + basis) rather than adding nullable `superseded_by` columns to three tables. This keeps the migration to two CHECK edits. (Adding `superseded_by` columns to observations/relations/context is a deferred v2 normalization — see §8.)
4. **Migration** runs through the existing schema-migration runner (M4c).

No change to `valid_from`/`valid_to` on observations (`schema.rs:163–164`) or relations (`schema.rs:191–192`) — they are the resolution substrate as-is.

---

## 3. Resolution: bitemporal supersession by mechanism

Resolution is always **soft**: close the old window or flip status; never `DELETE`. The mechanism is chosen by the conflicting record's type, reusing the verified write paths.

| Mechanism | Old-fact fate (verified path) | Supersede-link | Event |
|---|---|---|---|
| **decision** | `UPDATE decisions SET status='superseded', superseded_by=<new>` (`memory.rs:1359–1368`) — already end-to-end via `log_decision(supersedes=)` | native `superseded_by` FK | `decision_superseded` (exists) |
| **observation** | set `valid_to = new.valid_from` on old id (`delete_observation` pattern `memory.rs:2036–2039`, `WHERE id=? AND valid_to IS NULL`) | `record_superseded` event payload | `record_superseded` (new) |
| **relation** | set `valid_to = new.valid_from` (`delete_relation` `memory.rs:2181–2182`) | `record_superseded` event payload | `record_superseded` (new) |
| **state** | `ON CONFLICT(key) DO UPDATE` overwrite (`memory.rs:4060–4087`) — auto-safe key case only; prior value captured in event | event payload | `conflict_detected` + `state_changed` (exists) |
| **context** | key/version overwrite (`update_context`, version+checksum) | event payload | `record_superseded` (new) |

**Critical bitemporal rule (the Graphiti move):** the old row's `valid_to` is set to the **new fact's `valid_from`** (= its `captured_at`), not to "now." This makes the timelines abut exactly, so point-in-time queries before `valid_from` still return the old fact and queries after return the new one. The transaction-time audit (created/event rows) is separate and untouched. This is the bi-temporal contract Graphiti uses and that base-Mem0/MemGPT discard when they hard-overwrite.

**The decision asymmetry that the eval must respect:** decision FTS does **not** filter `status` (`memory.rs:7289`-region search), so a superseded decision is still *returned by search* — its suppression is **status-based**, asserted via `status=='superseded'`/`superseded_by!=null`, not absence. Observation search **does** filter `valid_to IS NULL` (`memory.rs:7255`, `1949`, `1970`), so a superseded observation is cleanly excluded. The resolution layer and the eval both branch on this.

---

## 4. Layered detection strategy

Deterministic-first, recall-gated, model-optional. The three deterministic layers (Stage 1) are the only ones eligible for auto-apply; everything else routes to review.

### Stage 0 — Candidate generation (cheap, recall-optimized, NOT a contradiction signal)
For an incoming candidate, retrieve the top-k related trusted facts of the **same `record_type` + same `scope_chain`** via the embedding gate: call `search_semantic_memory` (`memory.rs:6759`) or replicate `search_json_vector_memory` (scans `embedding_vectors`, `cosine_similarity` per row `memory.rs:6845`). Optimize for **recall**; widen with `hybrid_search_results` (`memory.rs:6944`, keyword+semantic) to catch paraphrase/vocab gaps. The similarity threshold only *narrows the set the deterministic checks run on* — it never decides conflict. **Note:** `embedding_vectors` has no `state` row type (`schema.rs:319–320`), so state uses a non-embedding key match (Stage 1.1), not the gate.

### Stage 1 — Deterministic resolution (auto-apply, non-destructive), in priority order

**1.1 Key supersession (LWW) — cost ≈ 0, most auto-applicable.**
For `state` (UNIQUE key, `schema.rs:254`) and `context` (keyed), a new value for an existing key supersedes by definition. Auto-safe. *False-positive guard:* require the normalized key to match exactly; do not collapse distinct facts onto a coarse key. Single-writer local store means no clock/concurrency hazard (vector clocks unnecessary).

**1.2 Structural slot — same subject + same predicate/attribute + same scope, incompatible value.**
For entities/relations/observations normalized to `(subject, predicate, value)`: same subject+predicate after normalization with a different value is a conflict **iff the attribute is single-valued**. Auto-safe for single-valued + normalized; else review. *False-positive guards (the cardinality gate is the whole game):*
- **Per-attribute cardinality registry** — `current_employer`, `timezone`, `marital_status`, `status` → single-valued (different value = conflict); `speaks_language`, `tag`, `visited` → multi-valued (different value = coexist, **never** a conflict). Unknown cardinality → review, not auto-apply.
- **Value normalization** before compare (`"NYC"`≡`"New York"`, units, timezones, casing/whitespace) so equal values don't look different.
- **Subject coreference confidence** — low-confidence subject match → review.

**1.3 Temporal scoping — distinguish succession from contradiction.**
Two facts holding in **different valid-time windows** are *succession*, not contradiction: close the old window (`valid_to = new.valid_from`), open the new. Auto-safe (reversible bookkeeping). *False-positive guard:* never compare cross-window facts as simultaneous (this is what stops "job change" / "moved city" from being flagged as a conflict); use `captured_at`/`valid_from`, never ingestion order. *False-negative guard:* if timestamps are missing/wrong, fall back to Stage 1.2 on the slot.

### Stage 2 — Optional model escalation (DEFERRED to v2; default to review)
For candidates clearly about the same slot that **don't** reduce to a key/slot/temporal rule (free-text, paraphrased, subtle), optionally run **NLI first** (cheaper, e.g. `roberta-large-mnli`) then **LLM only if needed**. Given NLI's documented **low precision on the contradiction label**, lexical-overlap/"not"→contradiction bias, and negation/antonym mishandling (EMNLP 2022; Stress-Test NLI), **any model verdict is a soft flag only**: it produces a `review`-kind `conflict` candidate, never an auto-apply. It mirrors Mem0^g/Graphiti using an LLM to *mark* an edge invalid within a temporal model, not to delete. Clearly flagged in the candidate (`conflict_kind=model_flagged`, `detector=nli|llm`) so the gate UI and eval can separate deterministic from model-driven precision.

### Auto-apply matrix
| Layer | Auto-apply? | Routes to review when |
|---|---|---|
| Key (1.1) | **Yes** (keep old version) | coarse/unstable key; multi-writer concurrency |
| Slot (1.2) | **Yes** if single-valued + normalized | cardinality unknown; normalization uncertain; low coref |
| Temporal (1.3) | **Yes** (reversible) | timestamps missing/ambiguous |
| NLI/LLM (Stage 2, v2) | **No** | always — soft-flag → review |

---

## 5. Arbitration policy (winner selection)

Deterministic, metadata-only, evaluated **only after** detection has established the two facts are about the same slot. Recency is the default; source priority and confidence break or override it. **No LLM freshness judgment.**

**Inputs (all already on Grafiki records or candidate metadata):** `valid_from`/`captured_at` (recency); `source_type` (`propose_candidate` validates it, `memory.rs:5148`) → source-priority rank; `confidence` (`schema.rs:431`, validated `memory.rs:5191`).

**Policy (lexicographic, v1 fixed weights):**
1. **Source priority** — if the incoming and trusted facts have *different* source-priority tiers and the trusted fact is from a strictly higher tier (e.g. `manual`/`human-confirmed` > `transcript` > `auto-extract`), **do not auto-supersede**; route to review. (Protects a human-confirmed fact from being silently overwritten by a low-trust auto-extraction — the differentiator no surveyed system implements.)
2. **Recency** — same tier: **newest `valid_from` wins** (the Graphiti/LongMemEval "latest value is correct" expectation). This is the common case and is auto-applied.
3. **Confidence tie-break** — equal recency (or within an epsilon window): higher `confidence` wins; if still tied, route to review.

The arbitration result (`winner`, `basis ∈ {recency|source_priority|confidence}`) is stamped into the candidate payload and the `record_superseded`/`conflict_detected` event so every supersession is explainable.

---

## 6. Candidate-gate UX (CLI / MCP surface)

The conflict path is a first-class extension of the existing candidate lifecycle (`propose → list → approve|reject|edit|bulk`), so all surfaces already exist; v1 adds conflict-aware fields and filters.

**CLI (`grafiki-cli`):**
- `grafiki candidates list --type conflict [--scope …]` — reuses `list_candidates` (`memory.rs:3306`); each conflict candidate renders: **new value**, the **trusted record it conflicts with** (`trusted_record_type:trusted_record_id`), **mechanism**, **arbitration** (`winner` + `basis`), **conflict_kind** (`auto_safe|review|model_flagged`), and the **detector** (`key|slot|temporal|nli|llm`).
- `grafiki candidates approve <id>` → runs the §3 supersession (soft-invalidate + link) via the new `approve_candidate_payload` arm; prints `superseded <old_id> -> <new_id> (basis=recency)`.
- `grafiki candidates reject <id>` → leaves both facts live; the trusted fact is untouched.
- `grafiki candidates diff <id>` (new, thin) → shows old-vs-new side by side for the conflicting slot.
- **Auto-applied conflicts are surfaced, not hidden:** `grafiki events --type record_superseded|decision_superseded` lists what was auto-resolved, each reversible by re-opening the window (the soft model makes undo trivial).

**MCP tools:** extend `grafiki_candidate_list`/`grafiki_candidate_approve`/`grafiki_candidate_reject` to accept/return the conflict fields above; add `conflict_kind` and `trusted_record_*` to the returned candidate object. No new tool is strictly required for v1, keeping the MCP surface stable.

**Gate policy:** `conflict_kind=auto_safe` (Stage 1 key/single-valued-slot/temporal that passed arbitration on recency within the same source tier) **may** be auto-applied without a human, controlled by a config flag `conflict.auto_apply_safe` (default **on** for key/temporal, configurable for slot). `conflict_kind=review` and `model_flagged` are **always** held pending. This is the precise embodiment of "auto-apply only the safe deterministic cases."

---

## 7. EVAL plan — Arm D: Supersession & Knowledge-Update

Drop-in arm for `crates/grafiki-eval`, same seams as `docs/EVAL_DESIGN.md` (`dataset.rs`/`seed.rs`/`metrics`/`report.rs`). New runner `runner/supersession.rs`, fixture `fixtures/supersession/grafiki_updates_v1/`, metric reuse in `metrics/classify.rs` + `metrics/stats.rs`, CI gate `tests/supersession.rs`. **v1 is keyword-mode + status/timestamp assertions only — offline, deterministic, model-free** (no embedding download); semantic/hybrid are the *same assertions* behind `--features fastembed` (nightly).

### 7.1 Falsifiable claim, per item
Ingest the original fact at `t0`; ingest the superseding fact at `t1 > t0` through a real mechanism; probe. Assert four orthogonal things on `search_memory` (`SearchReport{results,…}`) + `ask_memory` (`AgentMemoryBriefing{answer,…}`):
1. **NEW surfaced** — superseding record in `relevant_memory`/`results` at rank ≤ k.
2. **STALE suppressed** — mechanism-specific: excluded from search (observation/state/context, since obs filters `valid_to IS NULL` `memory.rs:7255`) **OR** present-but-flagged `status='superseded'` (decision, since FTS doesn't filter status `memory.rs:7289`).
3. **Answer carries NEW not STALE** — `new_required_tokens` ⊆ normalized `briefing.answer`; `stale_forbidden_tokens` ∩ answer = ∅.
4. **Abstention on retraction** — for `retraction` items, answer == the exact sentence `"I do not have trusted memory for this yet."` (`memory.rs:2394`) and no stale assertion.

Everything is decidable from `status`/`valid_to`/ids + literal token match. No judge.

### 7.2 Dataset (`fixtures/supersession/grafiki_updates_v1/updates.jsonl`)
One supersession item per line: `item_id`, `category ∈ {knowledge_update, preference_update, state_transition, decision_reversal, retraction, distractor_noise}`, `mechanism ∈ {observation, decision, state, context}`, ordered `events[]` (each `t_index`, `captured_at`, `record`; the updater carries `supersedes_t_index`), and `assert: {new_required_tokens, stale_forbidden_tokens, expect_abstain}`. Load-time validation (fail-loud): tokens disjoint and each literally present in its event's record; retraction items have empty `new_required_tokens` and a replacement-less updater; `distractor_noise` items are two coexisting non-conflicting facts (the negative class). Per-item isolated temp `GRAFIKI_HOME` (same isolation contract as `seed.rs`). v1 ≈ 24–30 hand-authored items: knowledge_update ×8 (observation/context), preference_update ×4 (state/observation), state_transition ×3 (state), decision_reversal ×4 (decision status-flip path), retraction ×4 (observation/state), distractor_noise ×4–6 (observation/decision).

### 7.3 Metrics (exact)
Per item: `{new_surfaced, stale_suppressed, answer_has_new, answer_has_stale, abstained, is_negative, expect_abstain, new_rank}`.

- **Primary — supersession pass-rate** (macro over non-negative, non-retraction items):
  `PASS ⟺ new_surfaced ∧ stale_suppressed ∧ answer_has_new ∧ ¬answer_has_stale`. A conjunction, so "new shown but stale also shown" is a fail — that is the point of H2.
- **Stale-leak rate** (safety co-metric, must trend to 0): `mean(answer_has_stale ∨ ¬stale_suppressed)`, with a per-item stale-leak list `(item_id, mechanism, leaked_token)` used as the CI hard gate (fails the build like a secret leak).
- **Conflict precision/recall/F1** (does Grafiki *recognize* the contradiction): TP = supersession item where `stale_suppressed` fired; FN = stale stayed live (missed conflict); FP = `distractor_noise` item where a still-true fact got suppressed (**false supersession**); TN = `distractor_noise` both live. `precision=TP/(TP+FP)`, `recall=TP/(TP+FN)`, `false_supersession_rate=FP/(FP+TN)`. Reuses `classify::Counts`.
- **Retraction-abstain accuracy** (never blended): `mean(abstained ∧ ¬answer_has_stale)` over `retraction`.
- **Diagnostics:** mean `new_rank` (MRR via `reciprocal_rank` over a 1-doc qrel), per-mechanism breakdown (surfaces that decisions suppress by *status* not *exclusion*), per-category breakdown, retrieval coverage.
- **Uncertainty:** bootstrap 95% CI (`stats::bootstrap_ci`, B=2000, seeded) on pass-rate, stale-leak-rate, false-supersession-rate; per-item pass vector feeds `stats::paired_*` for future A/B (auto-apply on vs off).

### 7.4 CLI / baseline / CI
- `grafiki-eval run --arm supersession --dataset fixtures/supersession/grafiki_updates_v1 --mode keyword`.
- `report.md`/`results.json` add a `supersession` block (headline pass-rate+CI, stale-leak list, conflict P/R/F1, false-supersession rate, retraction-abstain accuracy, per-mechanism/per-category tables; state v1 = keyword + status assertions, model-free).
- `baseline.json`:
  ```json
  "supersession": { "min_pass_rate": 0.90, "max_stale_leaks": 0,
                    "max_false_supersession_rate": 0.0, "min_retraction_abstain_acc": 1.0 }
  ```
- **CI gate** (`tests/supersession.rs`, keyword-only, fast matrix): `stale_leak_list.is_empty()`, `false_supersession_rate == 0.0`, `retraction_abstain_acc == 1.0`, `pass_rate >= floor`; wired into `eval-gate` with `--fail-on-regression`.

### 7.5 External-dataset adapters (deferred, schema already compatible)
- **LongMemEval `knowledge-update`** (arXiv:2410.10813) — the sharper instrument for KU (LoCoMo has *no* KU category; it folds fact-change into temporal-reasoning, so it is weaker/indirect here). Map the old/new answer-bearing sessions → two events; later session gets `supersedes_t_index`; `answer`→`new_required_tokens`, prior span→`stale_forbidden_tokens`; `_abs` → `retraction`/abstain.
- **MemoryAgentBench Conflict-Resolution** (arXiv:2507.05257, MIT) — override-stream items → ordered events via transcript→candidate→approve (also exercises extraction recall); "no spurious update" items → `distractor_noise` (false-supersession). Runner + metrics reused; only a new loader.
- **LoCoMo** (arXiv:2402.17753) — category-5 adversarial/unanswerable → `retraction`/abstain via `qa.evidence` dia_ids.

---

## 8. v1 vs deferred scope

**v1 (deterministic, model-free):**
- Detection: embedding gate (Stage 0) + Stage 1 key / single-valued-slot / temporal core, with cardinality registry + value normalization + temporal scoping guards.
- Resolution: soft supersession on decisions (native), observations, relations, context, state — `valid_to = new.valid_from` / status flip; supersede-link via `record_superseded` event payload.
- Candidate gate: `conflict` type, `conflict_detected`/`record_superseded` events, CLI+MCP conflict fields, auto-apply for `auto_safe` (key/temporal default-on; slot configurable).
- Arbitration: source-priority → recency → confidence (fixed lexicographic policy).
- Eval Arm D with CI hard gate.

**Deferred (v2+):**
- **Stage 2 model escalation** (NLI then LLM) — soft-flag-only, behind feature flag; eval reports its precision separately.
- **Normalized supersede-link columns** (`superseded_by` on observations/relations/context) replacing the event-payload link.
- **State embeddings** (add `state` to `embedding_vectors` CHECK) for semantic state-conflict detection (today state is key-only).
- **Entity bitemporal soft-delete** — entities have no `valid_to` (`schema.rs:133–145`); conflict on an entity *attribute* is handled via its observations/relations in v1; whole-entity supersession is deferred.
- **Learned/weighted arbitration** and multi-writer concurrency (vector clocks) — unnecessary for the single-writer local store today.

---

## 9. Risks

1. **False supersession (precision risk).** A multi-valued attribute mistaken for single-valued, or a normalization miss, silently invalidates a true fact. *Mitigation:* cardinality registry gates auto-apply (unknown → review); the soft model makes every error reversible; `false_supersession_rate == 0.0` is a CI hard gate on the `distractor_noise` negative class.
2. **Missed conflict (recall risk).** Paraphrase below the gate threshold, or same attribute under two predicate strings (`works_at` vs `employer`), never enters the same slot. *Mitigation:* hybrid (kw+semantic) gate optimized for recall; predicate normalization; `conflict_recall` tracked, FN list reported.
3. **Embedding-as-contradiction-signal regression.** A future contributor "simplifies" detection to "high cosine ⇒ conflict." *Mitigation:* the gate is structurally separate from the decision in code and doc; antonym/negation items belong in the fixture to catch it.
4. **Decision-search status leak.** Decisions are returned by FTS even when superseded (`memory.rs:7289`); a naive caller could surface a stale decision. *Mitigation:* suppression is asserted by status in the eval, and the briefing layer must filter `status='superseded'`; called out as a per-mechanism eval breakdown.
5. **Source-priority over-blocking.** Tier 1 (source priority → review) could route too many benign updates to review, hurting auto-apply throughput. *Mitigation:* default tiers are coarse (human-confirmed > everything else); tune via the per-category pass-rate and review-queue depth.
6. **State history loss.** v1 retains destructive `ON CONFLICT` overwrite for the auto-safe key case; prior value lives only in the event payload, not a queryable window. *Mitigation:* event payload captures it; full state bitemporal versioning is deferred (§8).
7. **Migration safety.** Two CHECK edits on `extraction_candidates` and `events`. *Mitigation:* additive-only, run through the M4c migration runner; round-trips through export/import (`ExportDecision.superseded_by` already round-trips, `memory.rs:618`).

---

## 10. References

**Internals (verified, this codebase):** `log_decision` supersession `crates/grafiki-core/src/memory.rs:1321–1406` (status flip + `decision_superseded` event `1358–1382`); `valid_to` soft-invalidation `delete_observation:2036`, `delete_relation:2181`, obs search filter `valid_to IS NULL` `:1949/1970/7255`; decision FTS does *not* filter status `~:7289`; candidate lifecycle `propose_candidate:3254`, `approve_candidate_payload:5627` (dispatch `:5634`), `validate_candidate_record_type:5114`; `upsert_state` destructive overwrite `:4046/4060–4087`; embedding gate `search_semantic_memory:6759` (`cosine_similarity` `embeddings.rs:648`, `rank_by_embedding:357`, `hybrid_search_results:6944`); abstention sentence `:2394`; schema `extraction_candidates` CHECK `db/schema.rs:427–434`, `trusted_record_*` `:436–437`, `events.event_type` CHECK `:355–364`, obs `valid_from/valid_to` `:163–164`, decisions `status`/`superseded_by` `:209–211`, `embedding_vectors` (no state) `:318–334`.

**SOTA agent-memory:** Zep/Graphiti temporal edge invalidation, recency-wins, bi-temporal soft-invalidate (arXiv:2501.13956); Mem0 retrieve-then-ADD/UPDATE/DELETE/NOOP, Mem0^g mark-invalid (arXiv:2504.19413); A-Mem (no contradiction detection; cite for linking only) (arXiv:2502.12110); MemGPT/Letta hard-overwrite core memory (arXiv:2310.08560); LongMemEval knowledge-update split = answer latest value (arXiv:2410.10813); LoCoMo (no KU category) (arXiv:2402.17753); MemoryAgentBench conflict-resolution (arXiv:2507.05257).

**Contradiction detection:** NLI low-precision-on-contradiction (Springer 10.1007/978-3-030-91244-4_25), word-overlap bias (EMNLP 2022 `2022.emnlp-main.725`), negation/antonym failures (Stress-Test NLI `C18-1198`); cosine ≠ contradiction (arXiv:2504.16318, arXiv:2403.05440); KG conflicting-triple alignment/fusion + cardinality (Dagstuhl TGDK.3.1.3); LWW / versioned-value (Fowler, Versioned Value); bitemporal modeling (Temporal database).

**Companion:** `docs/EVAL_DESIGN.md` (H1 harness this arm extends).