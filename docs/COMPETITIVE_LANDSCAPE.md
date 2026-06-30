# Grafiki — Competitive & Research Landscape (agent memory, 2026)

**Date:** 2026-06-29 · **Method:** mnemosyne source read directly from a clone of
`github.com/mnemosyne-oss/mnemosyne`; the rest via Exa deep search + source reads.
**Evidence tags:** `[V]` vendor/marketing · `[I]` independent · `[1P]` primary (code, issue,
paper, license). Treat all single-vendor benchmark numbers as `[V]` unless independently
reproduced.

Grafiki = local-first **Rust** + SQLite **bitemporal** knowledge-graph memory for AI coding
agents: FTS5 + all-MiniLM-L6-v2 dense + RRF hybrid retrieval; human-in-the-loop candidate-review
pipeline with provenance; deterministic conflict detection + arbitration + supersession (H2);
Personalized-PageRank graph retrieval (H3); cross-encoder rerank (H4); deterministic
community-summary reflection (H5); TREC/BEIR-grade eval harness.

---

## 1. Strategic verdict — where Grafiki actually wins

The market splits cleanly on one axis: **LLM-on-write vs. deterministic-on-write.**

- **LLM-on-write** (Mem0, Zep/Graphiti, Letta, Cognee, Hindsight, LangMem): an LLM extracts/merges
  facts on every ingest. Pays per-fact latency + token cost, is non-deterministic, non-auditable,
  and accumulates junk. An independent harness (**MemBench/agentbench**, [I]
  https://news.ycombinator.com/item?id=46032521 · https://github.com/fastpaca/agentbench) found
  Mem0/Zep **14–77× more expensive and 31–33% *less* accurate** at fact recall than simply passing
  the full history. A Mem0 production audit found **97.8% of 10,134 entries were junk** ([1P]
  https://github.com/mem0ai/mem0/issues/4573).
- **Deterministic-on-write** (Grafiki; mnemosyne is the only close sibling): rule-based ingest +
  human/gate review, no LLM required. Reproducible, auditable, cheap, local.

**Grafiki's defensible wedge** (no competitor occupies all four corners):
1. **Single-binary, torch-free, embedded** (Rust + SQLite). txtai/Memary/Letta/Zep all need
   Postgres/Neo4j/Faiss + a Python/torch runtime or a server process.
2. **Bitemporal + *deterministic* conflict resolution.** Zep/Graphiti has the bitemporal model but
   pays an LLM per contradiction check; Mem0 V3 *removed* conflict resolution (ADD-only); Letta/
   LangMem/Memary/txtai have none. Grafiki's H2 is the only deterministic, auditable arbiter.
3. **Provenance + human-in-the-loop candidate gate** — the structural antidote to the "97.8% junk"
   failure mode every LLM-on-write system exhibits.
4. **Coding-agent domain focus.** Most rivals are conversational-assistant memory; only Cipher/
   ByteRover and Grafiki target coding agents specifically.

**Do NOT benchmark-chase LoCoMo/LongMemEval** (see §4 — both are saturated/contested). Grafiki
should publish **tokens-per-retrieval** (like Mem0 does) and win on **temporal/contradiction tasks**
where deterministic bitemporal resolution structurally beats embedding similarity.

---

## 2. mnemosyne — the close sibling (verified from source)

**What:** `github.com/mnemosyne-oss/mnemosyne` (mirror of AxDSan/mnemosyne), MIT, Python, PyPI
`mnemosyne-memory` (~21K downloads/mo — real, used). "Zero-dependency, sub-ms, SQLite-backed
universal AI memory." Same thesis as Grafiki, different language, further along on the cognitive layer.

**Architecture — BEAM (Bilevel Episodic-Associative Memory):** 3 SQLite tiers — *working* (FTS5,
TTL eviction, auto-injected pre-LLM) → *episodic* (long-term, sqlite-vec + FTS5, populated by a
`sleep()` consolidation job) → *TripleStore* (temporal KG, `valid_from`/`as_of`). Hybrid score =
**0.5·vector + 0.3·FTS5 + 0.2·importance**.

### Tested, MIT-licensed code worth borrowing (verified by direct file read)
> Note: an Exa agent doing *web-only* research reported Weibull/MMR are absent from AxDSan/mnemosyne
> (they're under-documented in the README, and a *separate* project **Mnemo** / `Methux/mnemo` also
> uses per-tier Weibull). But `mnemosyne/core/weibull.py` (183 LoC) and `mnemosyne/core/mmr.py`
> (95 LoC) **are present in the cloned `mnemosyne-oss/mnemosyne` tree** — code ≠ docs. Both projects
> are fair game.

| # | Technique (file) | What it is | Port to Grafiki | Determinism |
|---|---|---|---|---|
| 1 | **MIB binary vectors** (`binary_vectors.py`) | sign-bit binarize (`x>0→1`), `packbits` → 384-d f32 to 48 B (**32×**), Hamming via XOR+popcount **in SQLite**, no ANN. Cites Moorcheh ITS `arXiv:2601.11557`. | **Two-stage retrieval:** binary popcount coarse filter → re-rank top-K with full dense + cross-encoder. Dodges the binary recall cliff. `u64::count_ones()` in Rust. | ✅ |
| 2 | **Per-type Weibull decay** (`weibull.py`) | hazard by memory type: shape `k`<1 slow (preference η≈4380 h), `k`=1 exponential, `k`>1 fast (request η≈72 h). Ready param table. | Grafiki's missing forgetting/decay + temporal-recency boost (M-tier). Pure math. | ✅ |
| 3 | **Veracity Bayesian confidence** (`veracity_consolidation.py`) | `confidence = 1 − 0.7ⁿ` (n=mention count) + tiers (stated 1.0/inferred 0.7/tool 0.5/imported 0.6). | Grafiki's "calibrated candidate confidence" M-item, as a formula. | ✅ |
| 4 | **Query-intent adaptive weights** (`query_intent.py`) | regex → intent (temporal/factual/entity/preference/procedural) → shifts vector/FTS weights. | Grafiki's RRF weights are static; make them intent-adaptive. | ✅ |
| 5 | **MMR diversity rerank** (`mmr.py`) + **polyphonic recall** | `λ·rel − (1−λ)·maxSim`; 4 voices (vector/graph/fact/**temporal**) + diversity penalty. | Add a temporal voice + diversity penalty to Grafiki's RRF. | ✅ |
| 6 | **Episodic Gist+Fact graph** (`episodic_graph.py`) | "zero-LLM rule-based gist + pattern-based fact extraction." Cites REMem `arXiv:2602.13530` (real, ICLR'26). | Extends H5 reflection with deterministic episode summaries. | ✅ |
| 7 | **XChaCha20-Poly1305 client-side encrypted sync** (`sync.py`) | bidirectional delta sync; server sees only metadata. | A ship-ready feature Grafiki lacks; fits local-first. | ✅ |

### mnemosyne's benchmark claims — NOT credible quality evidence `[I]`
- **98.9% LongMemEval Recall@5** is on the **oracle split**, which is *saturated*: neuromcp 99.9%,
  QMG 98.6%, YourMemory 95.8% (https://github.com/xiaowu0162/LongMemEval/issues/32,46,42). The
  benchmark community flagged it rewards verbatim/vector retrieval without reasoning (MemPalace #314).
  And it used *dense* bge-small, **not** the MIB binary path.
- **MIB binary path is weak at scale: 20% recall@10 at 10M** ([1P] README) — the binary recall cliff.
  So headline recall and headline 32× compression are *different configs*. → borrow MIB only as a
  coarse first stage, never as the sole index.
- **BEAM 65.2%** uses a *different judge* (DeepSeek V4 + Llama 3.3 70B) than the 73.4% (Hindsight,
  Llama-4-Maverick) it's tabled against — apples-to-oranges (their own disclosure). The "BEAM ICLR
  2026" badge implies an affiliation that doesn't exist (BEAM = Tavakoli et al., `arXiv:2510.27246`;
  mnemosyne's "BEAM" architecture name is a *collision*).
- **Self-bypass harness bugs:** PR #90 — the harness "shipped four hardcoded paths that produced
  answers WITHOUT going through `BeamMemory.recall()`"; PR #79 — silent substring fallback. Pre-fix
  numbers were contaminated (their own docs now say so). **Biggest red flag.**

**Takeaway:** ignore the leaderboard; steal the 7 deterministic techniques above.

---

## 3. Competitor matrix

| System | Lang / store | Local-first | License | Conflict / temporal | Headline benchmark `[V]` | Fatal flaw |
|---|---|---|---|---|---|---|
| **Grafiki** | Rust / SQLite (1 file) | ✅ embedded, torch-free | — | **deterministic** bitemporal arbitration + supersession | (adopt §4) | unproven on public benches; small ecosystem |
| **mnemosyne** | Python / SQLite | ✅ | MIT | "timeline + importance" (under-specified) | LongMemEval 98.9% R@5* | benches contaminated (PR#90); binary recall cliff |
| **Mem0** | Py/TS / vec+graph+KV | ⚠️ needs LLM+vec DB | Apache-2.0 | V3 = **ADD-only, removed** UPDATE/DELETE | LoCoMo 91.6 / LongMemEval 94.8 | 97.8% junk audit; LLM-on-write; ~389 open issues |
| **Zep / Graphiti** | Py / Neo4j-FalkorDB-Kuzu | ❌ server | Graphiti Apache-2.0; Zep commercial | **bitemporal** (4 timestamps) + LLM contradiction check | LongMemEval 71.2; DMR 94.8 | LLM-on-write; server+graph DB; LoCoMo dispute |
| **Letta (MemGPT)** | Py / Postgres+pgvector | ❌ server | Apache-2.0 | agent/LLM-managed; none deterministic | LoCoMo ~74 | breaks prefix caching (43.9% hit→cost blowup); DB timeout ~30 mems; whole-platform |
| **Cognee** | Py / graph+vec DBs | ⚠️ local→cloud | Apache-2.0 | LLM ECL pipeline | — | LLM-heavy ingest; open-core upsell |
| **Hindsight** (vectorize-io) | Py / Postgres | ❌ LLM-on-write | MIT | LLM belief-update at "reflect" | LongMemEval 91.4 / LoCoMo 89.6 | "independent" repro is by co-authors; LLM-on-write |
| **txtai** | Py / SQLite+Faiss+NetworkX | ✅ (small tier) | Apache-2.0 | **none** (search framework) | BEIR (self-run) | not a memory layer; SQLite=small tier→Postgres; torch dep; 1 maintainer |
| **Memary** | Py / Neo4j-FalkorDB | ❌ server | MIT | freq+recency ("PageRank"=marketing); none | none | **dormant since Oct 2024**; retention PR #46 never merged |
| **LangMem** | Py / LangGraph store | ❌ | MIT | LLM-merge; none | **none published** | pre-1.0 (v0.0.30); dependabot-only; LangChain lock-in |

\* saturated oracle split — see §2/§4.

**Honorable mentions (verified real):** A-Mem (NeurIPS'25, `2502.12110`, Zettelkasten self-organizing);
MemoryOS (`2506.06326`, hierarchical OS); **Cipher/ByteRover** (`campfirein/byterover-cli`, MCP-native,
**coding-agent** focus — closest niche rival); Memori (GibsonAI, SQL-based); Honcho (Plastic Labs,
peer+reasoning); ChromaDB (vector DB, not memory).

**Recurring competitor flaws (all `[1P]`/`[I]`):**
- Mem0 V3 ADD-only ⇒ contradictory facts coexist & rank stale-first (#4956, #4896 closed not-planned →
  community wrote `mem0-temporal-hygiene` workaround); silent extraction failures (#3009, #5245);
  hallucinated "ghost" memories (#4099).
- Letta self-edits mutate the prompt prefix every turn → **prefix-cache hit ~43.9% vs ~93.4%** →
  "profitable vs unsustainable GPU economics" ([I] tensormesh).
- Memary HN critique: "overloading the term knowledge graph… similarity search over complete responses."

---

## 4. The benchmark trap — what to adopt, what to avoid

| Benchmark | Paper / repo | Use for Grafiki? | Caveat |
|---|---|---|---|
| **LongMemEval** | `arXiv:2410.10813` (ICLR'25), xiaowu0162 | partial | **oracle split saturated** (many systems 96–99.9% R@5); rewards verbatim retrieval, not reasoning (MemPalace #314). Conversational, not coding. |
| **LoCoMo** | snap-research | **avoid as headline** | independent audit (`dial481/locomo-audit` + Penfield): **6.4% gold answers wrong** (93.57% ceiling), category sizes vary 8.8×, LLM judge accepts **62.8%** of intentionally-wrong answers; subject of the unresolved Zep↔Mem0 methodology war. |
| **BEAM** | "Beyond a Million Tokens" `arXiv:2510.27246` (ICLR'26), `mohammadtavakoli78/BEAM`, MIT | **yes** | real, scales to 10M tokens, 10 abilities (IE/MR/TR/ABS/CR/KU/EO/IF/PF/SUM); cross-judge comparisons are apples-to-oranges. |
| **MemBench/agentbench** | `fastpaca/agentbench` ([I]) | **yes — cost axis** | the cost+accuracy critique of LLM-on-write; gives Grafiki its "cheaper *and* more accurate" framing. |
| **DMR** | (Zep) | no | widely viewed as saturated/easy. |

**Recommendation:** keep Grafiki's deterministic, model-free CI harness (it's a genuine strength — no
vendor has reproducible model-free gates). Add (a) a **BEAM** integration arm (model-free retrieval
recall subset), (b) a **tokens-per-retrieval** metric published alongside nDCG, (c) a bespoke
**temporal-contradiction** suite (Grafiki's home turf), and explicitly *not* anchor marketing to
LoCoMo/LongMemEval absolute scores.

---

## 5. Research map (verified arXiv IDs) — tagged for a deterministic, local-first system

**Foundational memory architectures**
- MemGPT / Letta — `2310.08560` — OS-tiered virtual context, paging. [needs-LLM for paging decisions]
- Generative Agents — `2304.03442` — memory stream + importance·recency·relevance retrieval + reflection. [retrieval scoring = deterministic-OK; reflection = needs-LLM]
- A-MEM — `2502.12110` (NeurIPS'25) — Zettelkasten link-on-write. [needs-LLM]
- MemoryBank — `2305.10250` (AAAI) — Ebbinghaus forgetting curve. [deterministic-OK]
- MemoryOS — `2506.06326` — short/mid/long hierarchical. [mixed]

**Graph / KG retrieval & consolidation**
- HippoRAG — `2405.14831`; HippoRAG2 "From RAG to Memory" — `2502.14802` — **PPR over KG** (Grafiki's H3 lineage). [deterministic-OK]
- Microsoft GraphRAG — `2404.16130` — Leiden community detection + LLM community summaries (global/local). [detection deterministic-OK; summaries needs-LLM — Grafiki H5 already does the deterministic-extractive variant]
- LightRAG — `2410.05779` (EMNLP'25) — dual-level retrieval. [mixed]
- RAPTOR — `2401.18059` (ICLR'24) — recursive summary tree. [needs-LLM]
- REMem — `2602.13530` (ICLR'26, HippoRAG group) — episodic gist+fact reasoning. [partly deterministic]
- Survey "From Storage to Experience" — `2605.06716` (ACL'26) + live list `github.com/FeishuLuo/Evolving-LLM-Agent-Memory-Survey`
- Survey "Graph-based Agent Memory: Taxonomy" — `2602.05665`

**Temporal / conflict / supersession** (Grafiki's H2 home turf)
- Zep/Graphiti — `2501.13956` — 4-timestamp bitemporal invalidate-don't-delete (validates Grafiki; replace its per-fact LLM with deterministic rules). [needs-LLM as built; pattern deterministic-OK]
- "Temporal Validity in Retrieval Memory" — `2606.26511` — argues **deterministic supersession beats RAG by construction** (third-party endorsement of Grafiki's thesis).
- "Not All Memories Age the Same" — `2604.26970` — per-predicate volatility → tune decay/recency. [deterministic-OK]
- Ritter et al., **functional-relation contradiction** — EMNLP 2008 (`aclanthology.org/D08-1002.pdf`) — `is_functional` predicate uniqueness; canonicalize before flagging. [deterministic-OK — highest-value zero-ML win]
- ROME `2202.05262` / MEMIT `2210.07229` — **out of scope** (edit model weights, not a store); cite only to scope out.

**Retrieval / embeddings**
- Moorcheh ITS "From HNSW to Information-Theoretic Binarization" — `2601.11557` (real but **0 citations, vendor-authored, unvetted**) — basis of MIB.
- Binary/scalar quantization — Cohere int8/binary (Mar'24); HF embedding-quantization blog — ~96% quality retained **with int8 rescoring** (the part mnemosyne's pure-binary skips). [deterministic-OK]
- Matryoshka — `2205.13147`; truncation-robustness critique `2605.16608`. [deterministic-OK]
- Rank-without-GPT (open listwise rerankers) — `2312.02969`; BGE reranker (Grafiki H4). [cross-encoder deterministic-OK; listwise-LLM needs-LLM]
- NLI contradiction (DeBERTa-v3, ~92% SNLI) runnable in Rust via `ort`/ONNX or `candle` (DeBERTa-v3 merged, PR #2743), ~8 ms CPU — a **scored signal**, not an LLM. [deterministic-reproducible]

---

## 6. Steal-like-an-artist plan — mapped to Grafiki's M-tier

Ranked by leverage. All keep the local-first / deterministic invariant.

1. **Two-stage binary-vector retrieval** (mnemosyne MIB + Cohere/HF rescoring lesson). Binary
   popcount coarse filter in SQLite → re-rank top-K with dense + H4 cross-encoder. 32× smaller hot
   index, no recall cliff. **Effort: M. Det: ✅.** (`binary_vectors.py`; `2601.11557`; Cohere/HF)
2. **Per-type Weibull decay + temporal-recency retrieval voice** (mnemosyne `weibull.py` + Mnemo +
   `2604.26970`). Implements the M-tier forgetting/decay + temporal-aware retrieval items; per-predicate
   volatility tunes it. Reinforce-on-access. **Effort: S. Det: ✅.**
3. **`is_functional` predicate uniqueness + polarity/numeric conflict checks** (Ritter EMNLP'08).
   Single-value predicates → deterministic supersession routed to the H2 gate; canonicalize objects
   first. Strengthens H2 with zero ML. **Effort: S. Det: ✅. — DONE** (model-free Stage 1.2 slot
   detector wired into `propose_candidate` on the default build; `conflict_detector="slot"`; the
   `fastembed` embedding detector is now a fallback. Single-token-value guard prevents prose
   mis-parse. Polarity/numeric checks remain deferred.)
4. **Veracity Bayesian confidence** `1 − 0.7ⁿ` + source tiers (mnemosyne `veracity_consolidation.py`)
   → the M-tier "calibrated candidate confidence" + active-learning review ordering. **Effort: S. Det: ✅.**
5. **Optional local NLI contradiction signal** (DeBERTa-v3 via `candle`/`ort`) — feature-gated, scored
   evidence into the H2 gate (not a truth oracle, not an LLM). **Effort: M. Det: reproducible.**
6. **Query-intent adaptive RRF weights + MMR diversity + temporal voice** (mnemosyne `query_intent.py`,
   `mmr.py`, polyphonic). Make Grafiki's static RRF adaptive + de-duplicated. **Effort: S–M. Det: ✅.**
7. **XChaCha20-Poly1305 client-side encrypted sync** (mnemosyne `sync.py`) — a genuinely new feature;
   server sees only metadata. **Effort: M. Det: ✅.**
8. **Publish tokens-per-retrieval + a BEAM arm** (Mem0 framing; MemBench cost critique; BEAM repo).
   Reframes the comparison onto Grafiki's structural advantage. **Effort: M.**

**Conceptual framings to adopt** (free, improve legibility): Hindsight's **Retain/Recall/Reflect** +
4 memory networks; LangMem's **semantic/episodic/procedural** typing; Letta's self-edited
always-resident **core block** as an MCP "always-injected" tier; a **`memory doctor`** that audits the
KG for conflicts/staleness (Letta `/doctor`).

**Anti-patterns to avoid** (learned from competitor failures): LLM-on-write (junk + cost + non-determinism);
LLM-arbitrated DELETE that empties the store (Mem0 #4536); prompt-prefix mutation that kills caching
(Letta); whole-platform lock-in (Letta/LangMem); Postgres/Neo4j hard dependency (Zep/Memary/txtai-at-scale);
benchmark-chasing LoCoMo.

---

## 7. Key references
mnemosyne `github.com/mnemosyne-oss/mnemosyne` · BEAM `arXiv:2510.27246` / `github.com/mohammadtavakoli78/BEAM` ·
LongMemEval `arXiv:2410.10813` / `github.com/xiaowu0162/LongMemEval` · LoCoMo audit `github.com/dial481/locomo-audit` ·
MemBench `github.com/fastpaca/agentbench` · Mem0 `arXiv:2504.19413` / `github.com/mem0ai/mem0` ·
Zep/Graphiti `arXiv:2501.13956` · Letta `github.com/letta-ai/letta` · Hindsight `arXiv:2512.12818` /
`github.com/vectorize-io/hindsight` · Cognee `github.com/topoteretes/cognee` · HippoRAG `2405.14831`/`2502.14802` ·
GraphRAG `2404.16130` · LightRAG `2410.05779` · RAPTOR `2401.18059` · REMem `2602.13530` ·
Moorcheh ITS `2601.11557` · Matryoshka `2205.13147` · Ritter EMNLP'08 `aclanthology.org/D08-1002.pdf` ·
Temporal-validity `2606.26511` · Adaptive-decay `2604.26970` · Surveys `2605.06716`, `2602.05665`.
