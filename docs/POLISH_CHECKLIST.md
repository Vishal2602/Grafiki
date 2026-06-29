# Grafiki — "Fully Works + Full Polish" Checklist

Tracks the remaining work after the 9 production-hardening milestones (M1–M5, CI,
Distribution) landed on branch `production-hardening`. Current state: builds, 56
tests pass, fmt + clippy `-D warnings` clean, smoke green, frontend + desktop build.

> **Status (Sections A + B complete):** A1-A5 and B6-B10 done, adversarially reviewed, and committed on `production-hardening`. Section E (capability roadmap) documented from RESEARCH_LANDSCAPE.md, to be planned next. Each milestone verified (tests + fmt + clippy -D warnings + smoke + frontend/desktop build).

Effort key: **S** ≈ <½ day · **M** ≈ ½–2 days · **L** ≈ multi-day.

---

## A. Blocks "fully works" (functional gaps a real user hits)

- [x] **A1 — Replace `window.prompt`/`confirm` in the desktop UI.** Tauri's webview can
  suppress them, so reject / delete / import-with-rationale silently no-op. Use the
  Tauri dialog plugin or in-app modals. `apps/grafiki-desktop/src/App.tsx` (reject ~L1871,
  and other prompt/confirm sites). **M**
- [x] **A2 — Capture-event dedup on re-ingest.** Re-running `capture import-transcripts`
  / re-ingest duplicates everything. Add a `content_hash` column (migration **v2** — runner
  exists in `db/schema.rs`) + `INSERT OR IGNORE`; dedup transcripts/file snapshots by content,
  but still allow legitimately-repeated terminal commands (per-source-type policy). `memory.rs`
  `ingest_capture_event`. **M**
- [x] **A3 — Offline-first embedding model.** Release builds ship `fastembed`, which downloads
  MiniLM at runtime on first use → first run offline/airgapped fails. Bundle/vendor the ONNX
  model, or show a clear "downloading model…" state + offline error. `embeddings.rs` + release
  workflow. **M**
- [x] **A4 — Desktop daemon auth token.** The app launches the local daemon with an empty token
  (unauthenticated). Generate a random token on launch and thread it through. `lib.rs` (~L1854),
  `api.ts` (~L1070). **S**
- [x] **A5 — Finish desktop delete/update coverage.** Some record types still return "not
  available in the desktop alpha". Wire every type the CLI/MCP can edit/delete to the UI.
  `lib.rs` delete/update commands. **S–M** (verify against current code)

## B. Full polish (UI/UX completeness)

- [x] **B6 — Async Tauri commands.** export / screencapture / daemon control / 5k-file
  auto-capture walk run synchronously and freeze the window → `async` / `spawn_blocking`. `lib.rs`. **M**
- [x] **B7 — Accessibility finish.** Real `role="dialog"` + aria-modal + focus-trap + Escape on
  Launcher & CommandPalette. (focus-visible, dark mode, reduced-motion already done.) `App.tsx`. **M**
- [x] **B8 — Lifecycle hygiene.** Stop the app-started daemon on quit (`RunEvent::ExitRequested`);
  bound screenshot retention + fix whole-second filename collisions. `lib.rs` (~L1366). **S**
- [x] **B9 — State round-trip completeness.** export/import still drops state
  `details`/`blockers`/`depends_on` (needs `StateItem` field additions that ripple into the
  status view). `memory.rs`. **M**
- [x] **B10 — Small UX bugs.** min-confidence free-text can hide all candidates; duplicate-pane
  uses active not clicked pane; `titleForPane` crash on tampered URL hash; restrictive CSP
  (`tauri.conf.json`). `App.tsx`. **S each**

## C. Hardening leftovers (P1, not user-visible)

- [x] **C11 — HTTP token hygiene.** Daemon passes `--token` via argv (visible in `ps`) and accepts
  `?token=`; move to env/stdin and drop the query param. `main.rs`. **S**
- [ ] **C12 — Embedding housekeeping.** Delete orphan `vec0` rows on record delete; prune stale
  vectors on provider/dimension switch; reuse the ONNX provider in the daemon worker instead of
  rebuilding it every 2s. `memory.rs` / `main.rs`. **S–M**
- [ ] **C13 — `redaction_profile`.** Stored/advertised but unused — implement none/default/strict
  or remove it from config/CLI/MCP. `memory.rs` + `project.rs`. **M**
- [ ] **C14 — Misc audit P2s.** MCP stdin size cap + protocol-version negotiation; chunked
  `Transfer-Encoding`; daemon PID-reuse race; entity `LIKE` wildcard escaping; empty/colliding
  slug ids; `add_context` can't update an existing key. **S each**

## D. External / ops (mostly you, not code)

- [ ] **D15 — Encryption at rest (SQLCipher + OS keychain).** Perms (0600/0700) done; this is the
  real thing, and must land after the migration runner (it exists). **L**
- [ ] **D16 — Apple Developer ID ($99/yr).** Only blocker to a Gatekeeper-clean DMG; release
  workflow + signing config already wired and dormant.
- [ ] **D17 — Push to a GitHub remote.** CI + release workflows only run once the branch is pushed.
- [ ] **D18 — Cross-platform.** macOS-hardcoded `screencapture`/HOME/`kill`; Windows desktop build.
  Linux CLI already works. **M**

## E. Capability roadmap (research-gap features vs SOTA)

Derived from the literature survey in [RESEARCH_LANDSCAPE.md](RESEARCH_LANDSCAPE.md)
(85 papers, full citations + concrete landing per item there). These are *new
capabilities*, not polish — a multi-week program, sequenced after B. Suggested
order: **H1 → (H2 + H3 + H4 in parallel) → H5 → M-tier**. H2/H3 are highest-ROI
because they reuse Grafiki's two most underused assets (bitemporal supersession +
the `relations` table).

### E-High leverage

- [~] **H1 — Evaluation harness (prerequisite for all of E).** **v1 LANDED** as the
  `crates/grafiki-eval` crate (design in [EVAL_DESIGN.md](EVAL_DESIGN.md)): TREC/BEIR-grade
  IR metrics (linear-gain nDCG@k / Recall@k / MRR / MAP / Success / Judged) **proven against
  `pytrec_eval` to 1e-9** (`tests/metrics_oracle.rs`); **Arm A** retrieval (keyword/semantic/
  hybrid, paired permutation + Holm) over a hand-authored Grafiki-native BEIR triple; **Arm C**
  redaction P/R/F1/F2 + leak gate over a synthetic, leak-safe corpus; bootstrap CIs; provenance;
  `results.json` + `report.md`; and a deterministic, model-free **CI eval-gate** (keyword + redaction
  vs a committed `baseline.json`). First run already surfaced a real gap: entity keyword search uses
  `name LIKE %query%` not FTS, so entities are unretrievable by NL queries ([memory.rs:7184](../crates/grafiki-core/src/memory.rs#L7184)).
  **Remaining (v1.5/v2/v3):** Arm B memory-QA replay (capture→candidate→trusted→ask, needs MiniLM);
  judged LongMemEval/LoCoMo + external BEIR/SecretBench adapters; SWE-bench memory-lift A/B. **L**
- [x] **H2 — Automated conflict / contradiction resolution.** **DONE** (design in
  [CONFLICT_DESIGN.md](CONFLICT_DESIGN.md); adversarially reviewed, all 15 findings fixed).
  `grafiki_core::conflict` — cardinality-gated slot/key/temporal detection + metadata arbitration
  (source-priority → recency → confidence, parsed timestamps, so a low-trust auto-extraction never
  silently overwrites a human fact). **Observation supersession via the candidate gate**
  (`supersedes` → `valid_to = new.valid_from`, honoring `captured_at`, entity/scope-guarded,
  source-type stamped — migration v4), closing the append-only gap (decisions already had native
  supersedes). **Automated semantic detection** (`detect_observation_conflict`, real-model builds):
  a new observation about an existing entity auto-gets a `supersedes` hint via same-entity embedding
  similarity, routed to review. Proven by **Eval Arm D** (CI-gated, model-free): pass-rate 1.0,
  **0 stale leaks**, false-supersession 0.0 (incl. a non-vacuous guard), retraction-abstain 1.0.
  **Future (v2, optional):** dedicated `conflict` candidate type (CHECK table rebuild); NLI/LLM
  escalation; relation/state/context supersession arms; CLI/MCP conflict fields. (Graphiti, Mem0.) **L**
- [ ] **H3 — Graph-aware retrieval.** Add a 3rd RRF arm: seed from FTS5+dense top-k
  entities, expand 1–2 hops over `relations` (respect `valid_from`/`valid_to`), run
  Personalized PageRank, fuse its ranking. SQL recursive query + in-memory PPR, no new
  store. (HippoRAG, G-Retriever, GraphRAG.) **M**
- [ ] **H4 — Reranking stage.** Small local cross-encoder (or listwise LLM) reranker over
  RRF top-N before `grafiki_ask` builds the cited answer. (RankGPT, bge-reranker.) **M**
- [ ] **H5 — Reflection / consolidation + community summaries.** Periodic job: Leiden
  community detection over `relations` → LLM-summarize each community → enter as a
  **candidate** (provenance = source observation ids). Enables global "themes / what did
  we decide about X" briefings. (Generative Agents, GraphRAG, LightRAG.) **L**

### E-Medium leverage

- [ ] **M-E1 — Forgetting / decay & salience.** `salience`/`last_accessed`/`access_count`
  (derive from the agent-query audit log); recency+importance+reuse term in ranking;
  soft decay proposes archival candidates (never hard-delete). (MemoryBank.) **M**
- [ ] **M-E2 — Temporal-aware retrieval.** Valid-time filter/boost in the SQL + recency
  weight in fusion; later diachronic scoring. (HyTE, TKG surveys.) **S→M**
- [ ] **M-E3 — Calibrated candidate confidence + active-learning review order.** Confidence
  per `extraction_candidate` (semantic entropy / source prior); sort
  `grafiki_candidate_list` by uncertainty×representativeness. **M**
- [ ] **M-E4 — Code-structure indexing.** Tree-sitter capture pass emits code
  entities + def-ref/call/contains relations into the existing entity/relation tables
  (so H3 works over symbols). (RepoGraph, code property graphs.) **M**
- [ ] **M-E5 — MCP security hardening.** Indirect-prompt-injection guards on ingested
  transcripts/terminal flowing back through tool metadata; read vs write/curate
  capability split; MCPTox-style poisoning tests in H1. **M**
- [ ] **M-E6 — PII + higher-recall secret detection.** Entropy gating + configurable rule
  packs (gitleaks-style) + optional local PII recognizer (Presidio) at the redaction
  boundary; measure vs SecretBench. **M**

### E-Low leverage

- [ ] **L-E1 — Embedding-inversion protection** (access-control `embedding_vectors`;
  int8/binary/Matryoshka quantization). **S→M**
- [ ] **L-E2 — Embedding-model upgrade path** (BGE-small drop-in @384-dim, or Nomic
  long-context; the table already records provider/model/dimension). **S→M**
- [ ] **L-E3 — Adaptive / corrective / learned retrieval** (Self-RAG/CRAG gate,
  query-conditioned fusion weights, SPLADE learned-sparse). **M→L**
- [ ] **L-E4 — Formal W3C PROV lineage + agent-to-agent sharing** (map `evidence_links`
  onto PROV; expose curated memory over A2A/ANP later). **M**

---

## Recommended path to the finish line

**Section A (A1–A5) + B6–B8** ≈ 1.5–2 weeks → makes it feel finished and never
silently misbehave. Then C/D as ongoing hardening, plus the $99 signing whenever a
clean DMG is wanted.

**Convention:** one milestone per PR/commit, each verified (tests + `cargo fmt --check`
+ `cargo clippy --all-targets -D warnings` + `scripts/smoke.sh` + frontend/desktop build)
before committing — same as M1–Distribution.
