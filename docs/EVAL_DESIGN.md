# DESIGN: `grafiki-eval` — Grafiki Evaluation Harness (H1)

**Status:** Proposed (implementation-ready)
**Crate:** `crates/grafiki-eval` (new workspace member)
**Depends on:** `grafiki-core` (path dep, in-process), `clap`, `serde`, `serde_json`
**Owner:** Lead engineer, eval
**Version target:** v1 self-contained + offline; extensible to v2 (judged QA / external datasets) and v3 (SWE-bench memory-lift)

---

## 1. Goals and principles

**What we are building.** A new Rust workspace crate that measures Grafiki's memory layer with TREC/BEIR-grade rigor, runs entirely in-process against `grafiki-core`'s public APIs, ships deterministic offline fixtures, and gates CI on regressions.

**What "good" looks like:**

1. **Rigor / correctness first.** Metrics match the world standard *exactly*. The single biggest correctness risk is the nDCG gain convention: we implement **linear-gain TREC nDCG** (gain = grade, discount = `1/log2(rank+1)`), the convention `pytrec_eval`/BEIR/MTEB report, and we **prove it** against `pytrec_eval` on a frozen fixture (one-time, offline). We never ship the exponential `2^rel−1` variant as the default, or our numbers are not comparable to BEIR/MTEB.
2. **Reproducibility by construction.** Every run pins and records: `grafiki-core` version + git hash, embedding model id (`all-MiniLM-L6-v2`) + dim (384), RRF params (`RRF_K=45`, kw weight `1.10`, semantic `1.00`, `CROSS_SOURCE_BONUS=0.018`, `text_match_boost`), dataset version, RNG seed, bootstrap iterations. This is `lm-evaluation-harness`'s central lesson (arXiv:2405.14782): *the harness, not the model, is the dominant source of irreproducibility.*
3. **Self-contained v1.** No network, no model download, no API keys, no external datasets. Grafiki-native fixtures + gold sets checked into the repo. Keyword retrieval + redaction are fully deterministic and run in the existing fast CI `test` matrix.
4. **Honest headline.** Grafiki's `ask_memory` has **no LLM generator** (`format_agent_memory_answer` templates status + top-5 snippets). So the honest headline for a *retriever* is **retrieval quality** (does the right evidence reach the briefing) + **abstention** + **redaction safety**, not a faux "QA accuracy." Judged free-text QA is a clearly-labeled v2 layer behind a feature flag.
5. **Multi-dimensional reporting** (HELM lesson): report retrieval quality **and** latency/cost **and** redaction safety side-by-side; never collapse to one number. Always report uncertainty (bootstrap 95% CI) and, for system comparisons, paired significance.
6. **Extensible.** The dataset schema is BEIR-shaped and LongMemEval-shaped from day one, so thin adapters can ingest LongMemEval / LoCoMo / BEIR / SecretBench / MemoryAgentBench later without touching the metrics or runner core. SWE-bench memory-lift is a fully-specified but deferred arm.

**Non-goals (v1):** bundling external corpora (heterogeneous licenses), LLM-judge in CI (nondeterministic, judge-bias), the sqlite-vec ANN recall arm, and the SWE-bench A/B.

---

## 2. Architecture

### 2.1 Crate layout

```
crates/grafiki-eval/
  Cargo.toml                 # path dep on grafiki-core; clap, serde, serde_json
  src/
    lib.rs                   # orchestrator: load fixtures -> seed/run -> score -> report
    main.rs                  # `grafiki-eval` CLI (clap, mirrors grafiki-cli conventions)
    config.rs                # EvalConfig: seed, bootstrap N, modes, paths, baseline, provenance
    dataset.rs               # typed loaders: BEIR triple + LongMemEval-shaped JSONL + redaction JSONL
    seed.rs                  # deterministic corpus seeder (ingest -> approve -> embed) into temp GRAFIKI_HOME
    metrics/
      mod.rs
      ir.rs                  # ndcg_at_k, recall_at_k, precision_at_k, mrr, map, success_at_k, judged_at_k
      classify.rs            # precision/recall/F1/F-beta, confusion matrix (redaction, abstention)
      stats.rs               # bootstrap_ci, paired_bootstrap, paired_permutation, holm
    runner/
      mod.rs
      retrieval.rs           # Arm A: keyword vs semantic vs hybrid over frozen store
      memory_qa.rs           # Arm B: capture->candidate->trusted->ask replay + abstention
      redaction.rs           # Arm C: redaction P/R/F1 over labeled corpus
    report.rs                # results.json + report.md emitters (provenance embedded)
  fixtures/
    retrieval/
      grafiki_dev_v1/
        corpus_seed.jsonl    # records to ingest (record_type, payload, scope, captured_at)
        queries.jsonl        # {_id, text}
        qrels.tsv            # query-id \t doc-id \t grade   (header row; grade 0/1/2)
        dataset.toml         # version, description, gold-set provenance
    memory_qa/
      grafiki_sessions_v1/
        sessions.jsonl       # LongMemEval-shaped: sessions w/ dates, turns, has_answer
        questions.jsonl      # {question_id, question_type, question, answer, evidence_ids, abstain}
    redaction/
      corpus_v1.jsonl        # {text, json_payload?, gold_secrets:[{literal,type}], benign:bool, context}
    baselines/
      baseline.json          # committed expected scores + tolerances for CI gating
    oracle/
      pytrec_eval_fixture/   # tiny (qrels, run) + frozen pytrec_eval outputs for the parity test
  tests/
    metrics_oracle.rs        # asserts Rust IR metrics == frozen pytrec_eval outputs (~1e-6)
    ci_regression.rs         # keyword-retrieval + redaction on fixture vs baseline.json
```

### 2.2 How it calls `grafiki-core` (in-process, verified signatures)

No subprocess, no network. The crate depends on `grafiki-core` by path and calls its public functions. Each run uses a **fresh temp `GRAFIKI_HOME`** (via `tempfile`, exactly as `scripts/smoke.sh` does) so runs are hermetic. Verified APIs:

- **Retrieval:** `grafiki_core::memory::search_memory(SearchMemoryOptions{ project_name, start_dir, grafiki_home, query, record_type: "all", mode, scope, limit })` → `SearchReport{ semantic_available, fallback, results: Vec<SearchResult> }`. `SearchResult` has `{ record_type, id, score: Option<f64>, evidence: Vec<EvidenceLink> }`. Doc id = `format!("{}:{}", record_type, id)`.
- **Ask:** `ask_memory(AskMemoryOptions{ question, scope, limit, agent: Some("eval"), .. })` → `AgentMemoryBriefing{ answer, relevant_memory: Vec<SearchResult>, semantic_available, fallback, audit_id, .. }`.
- **Capture→candidate→trusted:** `ingest_capture_event(IngestCaptureEventOptions{ scope, source_type, text, payload, privacy_level, redacted: false, captured_at, .. })` → `CaptureEventReport`; then `propose_capture_candidates(..)` → pending `ExtractionCandidate` rows; then per-candidate `approve_candidate(ApproveCandidateOptions{ id, .. })` → `CandidateMutationReport` (auto-approver), or `reject_candidate(..)`.
- **Redaction:** `redact_sensitive_text(&mut String) -> bool` is **private** (`fn`, L5858). **Required API addition (v1):** add to `grafiki-core` a small, well-scoped public wrapper:

  ```rust
  /// Public eval/test seam over the redaction trust boundary.
  /// Returns the redacted text and whether any redaction fired.
  pub fn redact_text(input: &str) -> (String, bool) {
      let mut s = input.to_string();
      let changed = redact_sensitive_text(&mut s);
      (s, changed)
  }
  ```
  Rationale: lets Arm C score the redactor *directly* (cleaner than reading records back through the ingest path). The harness still keeps an **indirect** path (ingest a secret-bearing event with `redacted:false`, read the stored record back) as a second test that the *trust boundary side-effects* fire (e.g. `privacy_level` escalates to `sensitive`).

### 2.3 Determinism contract

- **Frozen corpus.** For retrieval, all three `SearchMode`s run against a *byte-identical* store: snapshot the seeded SQLite DB once (or run a deterministic `seed.rs` that ingests fixed records, approves all candidates, builds embeddings) and reuse it for every mode. Never let modes run against different DB states.
- **Rank-derived run scores.** RRF-fused scores aren't globally comparable across modes; the run dict uses **rank-derived scores** (e.g. `score = limit − rank`) so metrics depend only on *order*, and tie-breaks follow Grafiki's documented order (score desc → best_rank asc → record_type → id).
- **`semantic_available` guard.** Record it per query. If embeddings aren't built, Hybrid silently falls back to Keyword (see `SearchMode::Hybrid` fallback in `memory.rs`); a "hybrid==keyword" result would then be a config artifact, not a finding. The runner **fails loud** if a semantic/hybrid arm runs with `semantic_available=false`.

### 2.4 CLI

Mirrors `grafiki-cli` conventions (clap derive, `--format json`):

```
grafiki-eval run --arm retrieval --dataset fixtures/retrieval/grafiki_dev_v1 \
                 --mode all --format md --out target/eval --seed 42 --bootstrap 2000
grafiki-eval run --arm redaction --dataset fixtures/redaction/corpus_v1.jsonl --format json
grafiki-eval run --arm memory-qa --dataset fixtures/memory_qa/grafiki_sessions_v1 \
                 --approver auto-all|oracle --mode hybrid
grafiki-eval run --arm retrieval --baseline fixtures/baselines/baseline.json --fail-on-regression
grafiki-eval validate-metrics   # runs the pytrec_eval-parity oracle test
```

`--arm {retrieval|redaction|memory-qa|all}`; `--mode {keyword|semantic|hybrid|all}`; `--approver {auto-all|oracle|reject-all}`; `--baseline <file> --fail-on-regression`; `--format {json|md}`; `--out <dir>`; `--seed <n>`; `--bootstrap <n>`.

---

## 3. The eval arms

### Arm A — Retrieval quality (keyword vs semantic vs hybrid)

**Purpose:** Prove (or refute) that Hybrid RRF beats Keyword and Semantic alone, and serve as the tuning instrument for the non-standard fusion constants.

**Document model.** Doc id = `(record_type, record_id)` for `record_type ∈ {entity, observation, decision, context}` — the four FTS-indexed + embedding-indexed types. `relations` and `state` are **not** directly searched → excluded. Serialize id as `"observation:01J...ULID"`.

**Input data format (BEIR triple, checked in):**
- `corpus_seed.jsonl`: one record/line → `{record_type, payload, scope, captured_at}`, ingested + approved by `seed.rs` so the *frozen store* is the corpus.
- `queries.jsonl`: `{"_id":"q1","text":"..."}`.
- `qrels.tsv`: TAB-separated, header `query-id\tcorpus-id\tscore`; `score` ∈ {0,1,2} (graded).

**Procedure:**
1. `seed.rs` builds the frozen store once (ingest → `propose_capture_candidates` → `approve_candidate` all → embeddings). Snapshot it.
2. For each query × each `SearchMode ∈ {Keyword, Semantic, Hybrid}`: `search_memory(.. mode, limit=20, record_type="all" ..)`.
3. Convert `SearchReport.results` → run dict `{qid: {"type:id": rank_score}}` (rank-derived).
4. Score with the metrics module against `qrels`.
5. **Compare:** mode × metric table; per-query nDCG@10 vectors feed a **paired permutation test** (Keyword vs Hybrid, Semantic vs Hybrid) with **Holm** correction on the single pre-registered primary metric (nDCG@10). Report p-value, mean delta, bootstrap 95% CI on the delta.

**Gold-set construction (pooling).** Build qrels by **pooling** the union of top-20 from all three modes per query, human-grade only that pool (0/1/2), plus a handful of **author-planted target records** (query authored *from* a target) to mitigate self-fulfilling-pool bias. Track **Judged@10** as a completeness guardrail.

**Metrics (headline + set):** **nDCG@10 (primary)**, nDCG@{1,3,5}, Recall@{5,10,20}, MRR@10, MAP, Success@{1,5}, Precision@{5,10}, Judged@10. Break down **per mode AND per record-type** (entity vs observation/decision/context) so an aggregate win can't hide a per-type regression.

**Tuning hook:** expose `RRF_K`, source weights, `CROSS_SOURCE_BONUS`, `text_match_boost` as sweepable params with nDCG@10 as the objective — turns the eval into the instrument that justifies/fixes the hand-tuned constants.

**v1 self-contained fixture:** 30–50 dev queries over the frozen Grafiki-native store. **Extension point:** thin adapter `dataset::beir::load(dir)` ingests external `corpus.jsonl/queries.jsonl/qrels.tsv` (SciFact/FiQA/NFCorpus are closest in spirit) — format already native, only licensing keeps them out of v1.

### Arm B — Memory-QA replay (capture → candidate → trusted → ask)

**Purpose:** Evaluate the *full loop* end-to-end with a deterministic, judge-free score (retrieval recall over evidence) plus abstention, and isolate candidate-extraction recall from retrieval recall.

**Input data format (LongMemEval-shaped JSONL, checked in):**
- `sessions.jsonl`: each session `{session_id, date, turns:[{role, content, has_answer?}]}`.
- `questions.jsonl`: `{question_id, question_type ∈ {info-extraction, multi-session, temporal, knowledge-update, abstention}, question, answer (gold short), evidence_ids (gold session/turn ids), abstain: bool}`. `abstain:true` (LongMemEval `_abs` analog) has **no** evidence.

**Procedure (replay mapping):**
1. **Isolate** each instance into its own `scope`/project; snapshot SQLite per instance so haystacks don't bleed.
2. **Ingest** each session turn via `ingest_capture_event(source_type="transcript", text=..., captured_at=session.date, ..)`, **threading the session date into the bitemporal `valid_from`** (temporal questions are unanswerable otherwise) and **threading the source session id through `EvidenceLink.source`** so retrieved records map back to gold haystack sessions. This also exercises the redactor on real-ish text.
3. **Candidate gate (two arms to isolate extraction recall):**
   - `--approver auto-all`: `approve_candidate` for *every* proposed candidate (baseline).
   - `--approver oracle`: approve only candidates whose evidence overlaps gold evidence ids. **The gap between the two = how much candidate-extraction recall (not retrieval) costs you.**
4. **Ask:** `ask_memory(question, scope, agent="eval")` → `AgentMemoryBriefing`.

**Scoring — three layers:**
- **(A) Retrieval (primary, judge-free, CI-safe):** from `relevant_memory` (and audited `returned_ids`), compute Recall@k / nDCG@k / MRR against gold evidence ids threaded at ingest. Directly evaluates FTS5+MiniLM+RRF across all three modes.
- **(B) Abstention:** for `abstain:true` items, **correct iff the briefing refuses** — `format_agent_memory_answer` emits "I do not have trusted memory for this yet…" when status+search are empty. Any fabricated/non-empty answer = miss. Report **abstention accuracy separately**; never blend with answerable accuracy (the dominant reporting mistake).
- **Knowledge-update / supersession:** insert the superseding fact as a *later* session; assert the briefing surfaces the **new** fact and **NOT** the stale one (`valid_to`/supersedes is the mechanism under test).

**Metrics:** answerable Recall@k / nDCG@k / MRR (macro, per question_type); abstention accuracy (separate); supersession pass-rate; plus latency/ingest cost. Optional deterministic answer check: normalized substring/required-fact match on gold short answer.

**v1 self-contained fixture:** ~10–20 small synthetic multi-session conversations authored in the LongMemEval-shaped schema, run across all three modes (so the loop doubles as a hybrid-beats-keyword regression). **Extension points:** `dataset::longmemeval::load` (HF `xiaowu0162/longmemeval-cleaned`: `question_id`/`_abs`→`abstain`, `answer_session_ids`→`evidence_ids`, `haystack_dates`→`captured_at`); `dataset::locomo::load` (`adymaharana/locomo`: `qa.evidence` dia_ids → evidence_ids, `category==5` → `abstain`); MemoryAgentBench conflict items → supersession stress. **(B-judged, v2, feature-flagged):** feed `briefing.answer`/snippets to a fixed external reader LLM and score with the LongMemEval GPT-4o-2024-08-06 judge (temp=0, prompt+model hash recorded) — the honest way to publish a real LongMemEval/LoCoMo number given Grafiki has no generator. Cache judge calls keyed by `(question_id, answer_hash)`.

### Arm C — Redaction precision/recall/F1 by secret type

**Purpose:** Safety-critical regression gate on Grafiki's substitution redactor (`redact_sensitive_text` → assignment/PEM/token-prefix/JWT passes). The redactor *substitutes*, not span-emits, so scoring is **input→output diff**, not span-vs-span.

**Detection unit.** Per planted secret instance with known `literal` + `type`. After `redact_text(input)`:
- **TP:** the secret literal is gone from output AND replaced by a marker. **Type-correct TP** additionally requires the marker label maps to `type` (e.g. `sk-ant-…` → `[REDACTED_ANTHROPIC_KEY]`).
- **FN (LEAK):** the literal (or recoverable substring) survives verbatim — the worst-case failure (credential persisted to SQLite).
- **FP (over-redaction):** a benign item was modified.

**Input data format (`corpus_v1.jsonl`, checked in):**
```json
{"text":"export OPENAI_API_KEY=sk-proj-AAAA...", "gold_secrets":[{"literal":"sk-proj-AAAA...","type":"openai"}], "benign":false, "context":"env"}
{"text":"commit 9f1c2ab... merged", "gold_secrets":[], "benign":true, "context":"git"}
{"json_payload":{"client_secret":"..."}, "gold_secrets":[...], "benign":false, "context":"json"}
```

**Procedure:** for each case run `redact_text` (and, for JSON, the ingest path so `redact_json_value` fires); diff input vs output; classify TP/FN/FP; record marker→type for the confusion matrix; emit a **leak list** (every surviving secret).

**Metrics:** per-type and overall **Precision = TP/(TP+FP)**, **Recall = TP/(TP+FN)**, **F1 = 2PR/(P+R)**, and **F2 = 5PR/(4P+R)** (leak-is-worse-than-over-redaction view, Presidio convention). **Strict scoring is primary** (any residual leak = FN; partial redaction is failure). Fuzzy/lenient overlap (Jaro-Winkler≥0.7) is a **secondary diagnostic** only. Plus a marker-vs-type **confusion matrix** and the **leak list** as the hard gate. Co-report **precision as first-class** (over-redaction corrupts memory + poisons FTS/embedding indexing → release-blocking too; GitGuardian imbalanced-accuracy fallacy — never report accuracy alone).

**v1 self-contained fixture (~150–300 cases, leak-safe synthetic only):**
- **Positives** (~10–20 format-valid synthetic instances per supported type, regenerated from templates each run so the redactor can't memorize literals): Anthropic `sk-ant-`, OpenAI `sk-`(≥20), Stripe `sk_live_/sk_test_/pk_live_/rk_live_`, GitHub `ghp_/gho_/ghu_/ghs_/ghr_/github_pat_`, GitLab `glpat-`, Slack `xoxb-/xoxp-/xoxa-/xapp-`, AWS `AKIA…` (use the AWS doc example `AKIAIOSFODNN7EXAMPLE`), Google `AIza…`, JWT (3 base64url parts), PEM private-key blocks, assignment-style `password/api_key/client_secret` in `=`, `:`, and JSON forms — each embedded in transcript/terminal/git/env/JSON contexts.
- **Negatives** (benign-but-secret-looking, ~equal count): UUIDs, 40-hex git SHAs, content hashes, base64 blobs, high-entropy test names, file paths, **near-miss prefixes** that probe the length/format guards (short `sk-`, `AKIA` mid-word, 2-part dotted non-JWT, an `api_key` word with no `=`/`:`).

These near-misses double as labeled rows that **quantify known gaps** (e.g. OpenAI `sk-` branch also catches non-OpenAI `sk-` → label FP; JWT requires all 3 parts >8 chars → short JWT FN; `AKIA` has no trailing-charset check → potential FP). **NEVER** paste a real key; **never** run live verification on the corpus. **Extension point:** SecretBench (arXiv:2303.06729) is access-gated (real secrets, data-protection agreement) → defer; reuse `presidio-evaluator` (MIT) as the scorer only if/when the redactor expands to PII.

---

## 4. Metrics module — exact formulas

Notation: query `q` with graded judgments `rel(d)` (0 = non-relevant/unjudged); ranked list 1-indexed (rank 1 = top); cutoff `k`; relevance threshold `t` (default 1); `R_q` = #docs with `rel≥t`. **Compute every metric per-query, then macro-average** (mean over queries) — what trec_eval/BEIR/MTEB report. Never micro-average. Report mean + per-query min/median.

**DCG@k (LINEAR gain — USE THIS, BEIR/MTEB/trec_eval convention):**
```
DCG@k = Σ_{i=1..k} rel(d_i) / log2(i + 1)
```
Discount at rank 1 = `1/log2(2)=1`, rank 2 = `1/log2(3)≈0.6309`, rank 3 = `0.5`.

**IDCG@k:** sort all judged docs for `q` by descending grade, take top k, compute DCG@k over that ideal order.

**nDCG@k = DCG@k / IDCG@k**, range [0,1]. **Edge case:** if `IDCG@k = 0` (no relevant docs), **nDCG@k := 0**. Binary relevance is just the special case where grades ∈ {0,1}; same formula — do **not** switch formulas.

**Exponential (Burges) variant — DO NOT default, document only:** `DCG@k = Σ (2^{rel(d_i)} − 1)/log2(i+1)`. Expose behind `GainKind::Exponential`; never the CI default. State the chosen convention in every report.

**Recall@k** `= |{relevant in top k}| / R_q`. Edge: `R_q = 0 ⇒ Recall@k := 0` (or excluded; document the choice). If `R_q > k`, max achievable < 1 (capped-recall subtlety).

**Precision@k** `= |{relevant in top k}| / k` (denominator is `k`, not `min(k, retrieved)` — matches trec_eval `P.k`).

**MRR(@k)** `= (1/|Q|) Σ_q 1/rank_q`, `rank_q` = rank of first doc with `rel≥t` (contributes 0 if none in top k). **Edge: empty results ⇒ RR=0.**

**AP (one query)** `= (1/R_q) Σ_{i: d_i relevant} Precision@i`. **`R_q=0 ⇒ AP:=0`.** **MAP** = mean AP over queries. Rewards getting *all* relevant high.

**Success@k / Hit@k** = 1 if ≥1 doc with `rel≥t` in top k else 0; averaged → success rate.

**Judged@k** (diagnostic, not quality) = fraction of top-k that carry a judgment. Low ⇒ gold set too sparse ⇒ nDCG/Recall unreliable.

**Ties:** impose a deterministic, documented tie-break (score desc → best_rank asc → record_type → id) and apply the same when building the run, so scores are stable.

**Classification metrics (redaction & abstention):** `Precision=TP/(TP+FP)`, `Recall=TP/(TP+FN)`, `F1=2PR/(P+R)` (define 0 when `P+R=0`), **`F_β=(1+β²)PR/(β²P+R)`** (β=2 = recall-weighted). Report per-type + overall + confusion matrix.

**Uncertainty (lm-eval default):** **bootstrap CI** — treat per-item scores `s_1..s_N` as a sample; for `b=1..B` (B≥2000, fixed seed) resample N with replacement, recompute aggregate `m_b`; 95% CI = 2.5th/97.5th percentiles of `{m_b}`; SE = std. **Paired comparison** (same items, two modes): **paired bootstrap** over per-item deltas `d_i=s_i(A)−s_i(B)` (resample indices once, apply to both) → CI on mean(d). **Paired permutation test** on per-query nDCG@10 deltas (assumption-free; preferred over paired t-test) → p-value. **Holm** correction over the family of pairwise tests on the primary metric. For binary pass/fail A/B (memory-lift, v3): **McNemar** on discordant pairs `(|b−c|−1)²/(b+c)`.

**Validation (mandatory):** `tests/metrics_oracle.rs` feeds a frozen `(qrels, run)` fixture to the Rust module and asserts equality to frozen `pytrec_eval` outputs (computed once offline, checked in) to **~1e-6**. Until this passes, the numbers are not "BEIR-comparable." Also keep one hand-computed nDCG@3 example as a sanity unit test.

---

## 5. Reports and CI

**`results.json` (machine; HELM `config → raw → aggregated` triple, lm-eval provenance):**
```jsonc
{
  "provenance": {
    "grafiki_core_version": "0.1.0", "git_hash": "…",
    "embedding_model": "all-MiniLM-L6-v2", "embedding_dim": 384,
    "rrf": { "k": 45.0, "kw_weight": 1.10, "sem_weight": 1.00, "cross_source_bonus": 0.018 },
    "dataset": "grafiki_dev_v1", "dataset_version": "1.0.0",
    "seed": 42, "bootstrap": 2000, "convention": "ndcg_linear_gain_trec"
  },
  "arm": "retrieval",
  "per_mode": {
    "hybrid": {
      "aggregate": { "ndcg@10": {"mean":0.71,"ci95":[0.66,0.76],"se":0.025}, "recall@10": {...}, "mrr@10": {...}, "judged@10": 0.93 },
      "per_record_type": { "observation": {...}, "entity": {...} },
      "per_query": [ {"qid":"q1","ndcg@10":0.83}, ... ]
    },
    "keyword": { ... }, "semantic": { ... }
  },
  "comparisons": [
    { "a":"hybrid","b":"keyword","metric":"ndcg@10","mean_delta":0.06,"p_value":0.004,"p_holm":0.012,"ci95":[0.02,0.10] }
  ],
  "cost": { "ingest_ms": 1820, "ask_latency_ms_p50": 14 }
}
```
`per_instance` arrays (redaction leak list, per-question abstention) included for audit (HELM transparency bar).

**`report.md` (human):** headline table (mode × {nDCG@10, Recall@10, MRR@10, MAP}) with CIs; the paired-comparison verdict ("Hybrid > Keyword on nDCG@10, Δ=0.06, p_holm=0.012"); per-record-type and per-question-type breakdowns; redaction per-type P/R/F1/F2 + **leak list**; abstention accuracy (separate); cost/latency; any regressions vs baseline. State the metric convention explicitly.

**CI wiring (`.github/workflows/ci.yml`).** Split fast/deterministic (CI) from slow/non-deterministic (nightly), mirroring how `ci.yml` already isolates the fastembed build:

1. **Add to the existing fast `test` matrix** (no model download): `cargo test -p grafiki-eval` runs `metrics_oracle.rs` + `ci_regression.rs` (keyword-retrieval + redaction on the committed fixture).
2. **New `eval-gate` job** (Ubuntu, fast): `grafiki-eval run --arm retrieval --mode keyword --baseline fixtures/baselines/baseline.json --fail-on-regression` **and** `--arm redaction --fail-on-regression`. **Gate rules:** any **FN (secret leak)** on the positive corpus → **build fails**; redaction precision below threshold → fail; nDCG@10 drop > tolerance (e.g. 0.02 absolute, or below baseline CI lower bound) → fail. Deterministic (seeded, tiny fixture, keyword-only — no model) so it runs on every PR like `cargo test`.
3. **`baseline.json`** committed; updated deliberately via PR when a metric change is intended and reviewed.
4. **Nightly `eval-semantic` job** (feature-flagged, `--features fastembed`): downloads MiniLM, runs semantic+hybrid retrieval and the memory-QA loop; appends headline metrics + git hash to a committed results-history artifact so trend regressions are visible across releases. (v2-judged QA stays behind a separate flag + cached judge.)

Sketch added to `ci.yml`:
```yaml
  eval-gate:
    name: Eval gate (retrieval + redaction, deterministic)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Metric oracle + regression tests
        run: cargo test -p grafiki-eval
      - name: Retrieval/redaction regression gate
        run: |
          cargo run -p grafiki-eval -- run --arm retrieval --mode keyword \
            --baseline crates/grafiki-eval/fixtures/baselines/baseline.json --fail-on-regression
          cargo run -p grafiki-eval -- run --arm redaction \
            --baseline crates/grafiki-eval/fixtures/baselines/baseline.json --fail-on-regression
```

---

## 6. v1 scope vs deferred

**v1 (build first — self-contained, offline, deterministic, no external datasets, no API keys, in the fast CI matrix):**
1. **Metrics module** — linear-gain TREC nDCG@k, Recall@k, Precision@k, MRR, MAP, Success@k, Judged@k, P/R/F1/F2, bootstrap + paired-bootstrap + paired-permutation + Holm — with the **pytrec_eval parity oracle test** + a hand-computed example.
2. **Arm A retrieval** over a 30–50-query Grafiki-native frozen fixture, **Keyword mode in CI** (deterministic, no model), all three modes available locally; mode × metric table + paired permutation (hybrid vs each baseline, Holm).
3. **Arm C redaction** — synthetic corpus, input→output diff scorer, per-type P/R/F1/F2 + confusion matrix + **leak list**; primary = recall + leak count, precision co-reported. Requires the small `redact_text` public wrapper in `grafiki-core`.
4. **`results.json` + `report.md`** with full provenance.
5. **One CI regression gate** (keyword retrieval + redaction): zero leaks + precision ≥ threshold + nDCG@10 within tolerance.

**v1.5 (self-contained but needs the MiniLM download → nightly/on-demand, `fastembed` feature):** Arm A **semantic + hybrid**; **Arm B memory-QA** retrieval + abstention scoring (deterministic, judge-free) with `auto-all` vs `oracle` approver split and supersession assertions.

**v2 (gated on LLM dependency / network, feature-flagged, never in PR-blocking CI):** judged end-to-end QA (external reader + LongMemEval GPT-4o-2024-08-06 judge, temp=0, prompt+model hash recorded, cached) to publish real LongMemEval/LoCoMo numbers; external-dataset adapters (`longmemeval`, `locomo` with LLM-judge + category split, BEIR `SciFact/FiQA/NFCorpus`, MemoryAgentBench conflict items); sqlite-vec ANN-vs-exact recall@k arm.

**v3 (deferred — hard arm, specified now, built later):** **SWE-bench memory-lift A/B.** Same agent+model with-vs-without `ask_memory`, on SWE-bench Verified (500). Headline: `Δresolved = resolved_rate(with) − resolved_rate(without)` (pass@1, FAIL_TO_PASS green + PASS_TO_PASS green) + token/cost delta. Significance: **McNemar** on per-instance pass/fail discordant pairs + paired bootstrap. Requires a real coding-agent harness, per-repo Docker test execution, a defined memory-population protocol, and confounder control — out of scope for a self-contained first crate.

**Extension points (documented seams):** `dataset.rs` carries trait `DatasetLoader` with impls `beir`, `longmemeval`, `locomo`, `memoryagentbench`, `secretbench` (field mappings specified in §3); `metrics` `GainKind` enum already supports the exponential variant for cross-library comparison; `runner` arms are independent so a `swe_bench` arm slots in without touching A/B/C.

---

## 7. References

- **LongMemEval** — Wu et al., ICLR 2025. **arXiv:2410.10813**. Code (MIT): `github.com/xiaowu0162/LongMemEval`. Data: HF `xiaowu0162/longmemeval`, `xiaowu0162/longmemeval-cleaned`. Judge: GPT-4o-2024-08-06, >97% human agreement.
- **LoCoMo** — Maharana et al., ACL 2024. **arXiv:2402.17753**. Code/data: `github.com/snap-research/locomo` (`data/locomo10.json`); HF `adymaharana/locomo`. Official metric SQuAD token-F1; downstream LLM-judge.
- **MemoryAgentBench** — **arXiv:2507.05257**. Data (MIT): HF `ai-hyz/MemoryAgentBench`.
- **Survey on Memory Mechanism of LLM Agents** — Zhang et al. **arXiv:2404.13501** (framing only).
- **BEIR** — Thakur et al. **arXiv:2104.08663**. Code (Apache-2.0): `github.com/beir-cellar/beir`. nDCG@10 + Recall@100 convention; corpus/queries/qrels triple.
- **MTEB** — Muennighoff et al. **arXiv:2210.07316**. Code (Apache-2.0): `github.com/embeddings-benchmark/mteb`. Model under test: `sentence-transformers/all-MiniLM-L6-v2` (Apache-2.0, 384-dim).
- **trec_eval / pytrec_eval** — Van Gysel & de Rijke, **arXiv:1805.01597**. `github.com/cvangysel/pytrec_eval`; `github.com/usnistgov/trec_eval` (`m_ndcg_cut.c` = linear-gain reference). Also `ir_measures` (terrier-org), `ranx` (AmenRa, `ndcg_burges` = exponential).
- **RRF** — Cormack, Clarke, Grossman, SIGIR 2009 (`cormacksigir09-rrf.pdf`).
- **lm-evaluation-harness** — Biderman et al., **arXiv:2405.14782** ("Lessons from the Trenches…"). Code (MIT): `github.com/EleutherAI/lm-evaluation-harness`.
- **HELM** — Liang et al. **arXiv:2211.09110**. Code (Apache-2.0): `github.com/stanford-crfm/helm`.
- **SecretBench** — Basak et al., MSR 2023. **arXiv:2303.06729**. `github.com/setu1421/SecretBench` (labels MIT; data access-gated).
- **Secret-detection tool comparison** — **arXiv:2307.00714** (Jaro-Winkler≥0.7 / Gestalt≥0.6 fuzzy-match TP rule).
- **Microsoft Presidio (presidio-research)** — `github.com/microsoft/presidio-research` (MIT). Token-level P/R, F-β (β=2) for PII.
- **detect-secrets** — `github.com/Yelp/detect-secrets` (Apache-2.0). **GitGuardian** precision/recall framing (imbalanced-accuracy fallacy).
- **SWE-bench** — Jimenez et al. **arXiv:2310.06770**; **SWE-bench Verified** (OpenAI, 500-instance). HF `princeton-nlp/SWE-bench`, `princeton-nlp/SWE-bench_Verified`.

---

### Implementation notes for the engineer starting now
1. Add `crates/grafiki-eval` to `Cargo.toml` `[workspace] members`; path-dep `grafiki-core`.
2. Add `pub fn redact_text(&str) -> (String, bool)` to `grafiki-core` (§2.2) — the only required core change for v1.
3. Build `metrics/ir.rs` + `tests/metrics_oracle.rs` **first**; the parity test is the gate that earns "BEIR-comparable."
4. Author the three fixtures; pool retrieval qrels from all three modes' top-20 + planted records.
5. Wire the `eval-gate` job into `.github/workflows/ci.yml` (deterministic keyword + redaction only).

**Relevant absolute paths:**
- New crate (to create): `/Users/vishalsunilkumar/Documents/Project/Grafiki/crates/grafiki-eval/`
- Core change: `/Users/vishalsunilkumar/Documents/Project/Grafiki/crates/grafiki-core/src/memory.rs` (redactors `fn redact_sensitive_text` L5858; RRF consts L6960; `search_memory` L2207; `ask_memory` L2295; `ingest_capture_event` L3641; `approve_candidate` L3381; `propose_capture_candidates` L3944)
- CI to extend: `/Users/vishalsunilkumar/Documents/Project/Grafiki/.github/workflows/ci.yml`
- Workspace manifest: `/Users/vishalsunilkumar/Documents/Project/Grafiki/Cargo.toml`