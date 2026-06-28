# Grafiki Research Landscape & Gap Analysis

*Prepared for Grafiki maintainers — June 2026*

---

## 1. Executive Summary

Grafiki is, as of mid-2026, a **strong, provenance-first memory substrate** that has deliberately and competently built the **storage / retrieval / governance** half of the agent-memory stack the research literature assumes, while omitting the **active "cognitive" half** — reflect, consolidate, decay, reconcile, traverse — that the 2024–2026 frontier treats as essential.

What Grafiki already does at or near SOTA:

- **Bitemporal data layer.** Per-fact `valid_from`/`valid_to` soft-deletes (`observations`, `relations`), decision supersession (`status IN active|superseded|revisit|revoked`, `superseded_by`), and an append-only event log — the foundation Jensen & Snodgrass and SQL:2011 formalize, and the same substrate Zep/Graphiti builds on.
- **Modern hybrid retrieval.** SQLite FTS5 (BM25-style lexical) + all-MiniLM-L6-v2 dense embeddings (via `fastembed`), fused with Reciprocal Rank Fusion — exactly the sparse+dense+RRF recipe shown to generalize on BEIR.
- **Human-in-the-loop curation with lineage.** `extraction_candidates` → approve/edit/reject → trusted, with `evidence_links` — a manual, high-precision belief-revision gate that most academic systems lack, and which is *safer* than parametric editing (ROME/MEMIT) because updates are external and reversible.
- **Memory-as-a-Service done right.** Local-first, per-project, no telemetry, redact-before-write, exposed over MCP (`grafiki_search`, `grafiki_ask`, `grafiki_record`, `grafiki_candidate_*`, etc.) — the governance/data-residency posture the MaaS position paper calls the open frontier.

**The 5–7 highest-leverage things Grafiki is missing** (detailed and prioritized in §3):

1. **An evaluation harness.** There is *no* benchmark today. Without one, every other improvement is unmeasurable. Adopt LongMemEval + LoCoMo (memory), a BEIR-style retrieval split, and a SWE-bench-derived "does memory lift resolved-rate" test.
2. **Automated conflict resolution / contradiction detection.** Supersession is purely manual. Graphiti-style "compare new fact to semantically-related edges, then invalidate (don't delete)" plus deterministic, metadata-driven arbitration is the single biggest capability gap.
3. **Reflection / consolidation.** Grafiki never synthesizes raw observations into higher-level insights (Generative Agents) or community summaries (GraphRAG). This blocks corpus-level "what are the themes / what did we decide about X" briefings.
4. **Graph-aware retrieval.** Grafiki *stores* a graph but *retrieves* with flat lexical+dense RRF only. Adding k-hop expansion / Personalized PageRank (HippoRAG) over the existing relations table is low-effort, high-value for multi-hop queries.
5. **A reranking stage.** A small cross-encoder or LLM listwise reranker over the RRF top-k would sharply raise the precision of cited briefings — the standard second stage every production RAG pipeline has and Grafiki lacks.
6. **Forgetting / decay & salience.** Memory only grows. A MemoryBank-style decay + reinforcement signal (driven by the existing agent-query audit log) keeps the store healthy and rankings fresh.
7. **Code-structure awareness.** For a coding-agent memory layer specifically, the absence of any AST / def-ref / call-graph index (RepoGraph) is a notable miss; retrieval can't do symbol-level multi-hop the way the SWE-bench leaders do.

Net: Grafiki should treat **evaluation first**, then **conflict resolution + graph-aware retrieval + reranking** as the highest-ROI cluster, because all three reuse infrastructure it already has (bitemporal store, relations table, RRF pipeline, candidate gate).

---

## 2. Per-Area Sections

### 2.1 Long-term / persistent memory for LLM agents

**Overview.** The field splits into OS/RAG-style external memory hierarchies (MemGPT/Letta, MemoryBank, Mem0) and cognitively-inspired architectures mirroring human episodic/semantic/procedural memory (Generative Agents, CoALA, A-MEM, HippoRAG, MIRIX). The shared pattern: **capture → extract/structure → consolidate/reflect → retrieve**. The 2024–2026 shift is from "can we store it" to *disciplined memory operations* (ADD/UPDATE/DELETE/NOOP, conflict resolution, decay) and rigorous long-horizon evaluation.

**Key papers.**

| Title | Authors | Year | Venue | ID |
|---|---|---|---|---|
| Generative Agents: Interactive Simulacra of Human Behavior | Park, O'Brien, Cai, Morris, Liang, Bernstein | 2023 | ACM UIST | arXiv:2304.03442 |
| MemGPT: Towards LLMs as Operating Systems | Packer, Wooders, Lin, Fang, Patil, Stoica, Gonzalez | 2023 | arXiv (→ Letta) | arXiv:2310.08560 |
| MemoryBank: Enhancing LLMs with Long-Term Memory | Zhong, Guo, Gao, Ye, Wang | 2023 | AAAI 2024 | arXiv:2305.10250 |
| Cognitive Architectures for Language Agents (CoALA) | Sumers, Yao, Narasimhan, Griffiths | 2023 | TMLR | arXiv:2309.02427 |
| HippoRAG: Neurobiologically Inspired Long-Term Memory | Gutiérrez, Shu, Gu, Yasunaga, Su | 2024 | NeurIPS | arXiv:2405.14831 |
| A Survey on the Memory Mechanism of LLM-based Agents | Zhang et al. | 2024 | arXiv | arXiv:2404.13501 |
| A-MEM: Agentic Memory for LLM Agents | Xu, Liang, Mei, Gao, Tan, Zhang | 2025 | NeurIPS | arXiv:2502.12110 |
| Mem0: Production-Ready AI Agents with Scalable Long-Term Memory | Chhikara, Khant, Aryan, Singh, Yadav | 2025 | arXiv | arXiv:2504.19413 |
| AriGraph: KG World Models with Episodic Memory | Anokhin, Semenov, Sorokin, Evseev, Burtsev, Burnaev | 2024 | arXiv | arXiv:2407.04363 |
| MIRIX: Multi-Agent Memory System | Wang, Chen et al. | 2025 | arXiv | arXiv:2507.07957 |

**SOTA techniques.** Tiered/virtual context management (MemGPT); memory-ops loop with ADD/UPDATE/DELETE/NOOP conflict resolution (Mem0); reflection/consolidation of raw observations into insights (Generative Agents); importance+recency+relevance retrieval scoring; self-organizing linked notes with evolution (A-MEM); decay + reinforcement (MemoryBank); typed memory modules with module-specific policies (CoALA, MIRIX).

**Grafiki today vs. SOTA.** Grafiki matches the *external structured memory + hybrid retrieval* core (close to Mem0/A-MEM/MemGPT) and aligns with the CoALA episodic/semantic/procedural taxonomy, with governance differentiators (curation, provenance, audit, local-first) most academic systems lack. It trails on the active layer: no reflection, no decay, no automated conflict resolution, no graph-aware retrieval, no typed-module differentiation (observations/decisions are largely undifferentiated stores vs. MIRIX's six typed modules), and no eval.

---

### 2.2 Context Management for LLM Coding Agents

**Overview.** Real repos exceed any context window, so the field converged on three layers: (1) repo-level retrieval/representation (lexical+embedding RAG, AST/graph indices, iterative retrieve-generate); (2) agentic harnesses (SWE-agent, OpenHands, AutoCodeRover, Aider) vs. the minimalist counter-current (Agentless); (3) persistent/project memory (CLAUDE.md/cursor-rules, distilled trajectory "experience" memory). SWE-bench and variants are the evaluation backbone. The frontier is "context engineering": graph-enhanced retrieval, just-in-time exploration, and reusing past trajectories as memory.

**Key papers.**

| Title | Authors | Year | Venue | ID |
|---|---|---|---|---|
| SWE-bench: Can LMs Resolve Real-World GitHub Issues? | Jimenez, Yang, Wettig, Yao, Pei, Press, Narasimhan | 2023 | ICLR 2024 | arXiv:2310.06770 |
| RepoCoder: Repo-Level Completion via Iterative Retrieval & Generation | Zhang, Chen, Zhang, Liu et al. | 2023 | EMNLP | arXiv:2303.12570 |
| SWE-agent: Agent-Computer Interfaces Enable Automated SE | Yang, Jimenez, Wettig, Lieret, Yao, Narasimhan, Press | 2024 | NeurIPS | arXiv:2405.15793 |
| CodeAct: Executable Code Actions Elicit Better LLM Agents | Wang, Chen, Yuan, Zhang et al. | 2024 | ICML | arXiv:2402.01030 |
| AutoCodeRover: Autonomous Program Improvement | Zhang, Ruan, Fan, Roychoudhury | 2024 | ISSTA | arXiv:2404.05427 |
| OpenHands: Open Platform for AI Software Developers | Wang, Li, Lieret, Yang et al. | 2024 | ICLR 2025 | arXiv:2407.16741 |
| Agentless: Demystifying LLM-Based SE Agents | Xia, Deng, Dunn, Zhang | 2024 | FSE/PACMSE 2025 | arXiv:2407.01489 |
| RepoGraph: Repository-Level Code Graph | Ouyang, Yu, Ma, Xiao, Zhang et al. | 2024 | ICLR 2025 | arXiv:2410.14684 |
| SWE-Gym: Training SE Agents and Verifiers | Pan, Cao, Wang, Yang et al. | 2024 | ICML 2025 | arXiv:2412.21139 |
| SWE-smith: Scaling Data for SE Agents | Yang, Lieret, Jimenez et al. | 2025 | NeurIPS | arXiv:2504.21798 |
| SWE-Bench Pro: Long-Horizon SE Tasks | Scale AI (Deng et al.) | 2025 | arXiv | arXiv:2509.16941 |

**SOTA techniques.** Iterative retrieve-then-generate conditioned on draft output (RepoCoder); structure-aware/graph retrieval over AST/def-ref graphs (RepoGraph, LocAgent, CodexGraph); ACI design (SWE-agent); localization-first pipelines (Agentless, AutoCodeRover); just-in-time context + compaction + sub-agent isolation; experience/trajectory memory (ExpeRepair, SWE-Exp, MemGovern); long-context↔RAG routing (Self-RAG, RAPTOR, GraphRAG); inference-time scaling with verifiers (SWE-Gym, DARS).

**Grafiki today vs. SOTA.** Grafiki's generic primitives map well (hybrid RAG ≈ RepoCoder; its entity/relation graph ≈ a code graph; MCP exposure lets any harness query it; curation ≈ MemGovern/CLAUDE.md). Missing for code specifically: code-structure-aware indexing (no AST/call/def-ref graph), iterative output-conditioned re-retrieval (it's single-shot), trajectory/experience memory + reflection, learned/verifier reranking, and any code-specific eval to prove memory lifts resolved-rate.

---

### 2.3 Knowledge Graphs from/for Code and Text (GraphRAG, KG construction)

**Overview.** Two convergent threads: structural code graphs (code property graphs, RepoGraph) and LLM-built KGs from text. The frontier is **GraphRAG**: extract entities/relations, organize into a graph (often with community detection + hierarchical summaries), and retrieve via traversal / Personalized PageRank / subgraph selection rather than flat vectors — improving multi-hop and global ("themes") questions. Parallel work pushes from hand-crafted ontologies toward schema-guided and autonomous schema induction.

**Key papers.**

| Title | Authors | Year | Venue | ID |
|---|---|---|---|---|
| Modeling and Discovering Vulnerabilities with Code Property Graphs | Yamaguchi, Golde, Arp, Rieck | 2014 | IEEE S&P | DOI:10.1109/SP.2014.44 |
| REBEL: Relation Extraction By End-to-end Language generation | Huguet Cabot, Navigli | 2021 | Findings of EMNLP | ACL 2021.findings-emnlp.204 |
| Unifying LLMs and Knowledge Graphs: A Roadmap | Pan, Luo, Wang, Chen, Wang, Wu | 2023 | arXiv → IEEE TKDE | arXiv:2306.08302 |
| From Local to Global: A GraphRAG Approach | Edge, Trinh, Cheng et al. (Microsoft) | 2024 | arXiv | arXiv:2404.16130 |
| G-Retriever: RAG for Textual Graph QA | He, Tian, Liu et al. | 2024 | NeurIPS | arXiv:2402.07630 |
| HippoRAG | Gutiérrez, Shu, Gu, Yasunaga, Su | 2024 | NeurIPS | arXiv:2405.14831 |
| LightRAG: Simple and Fast RAG | Guo, Xia, Yu, Ao, Huang | 2024 | arXiv | arXiv:2410.05779 |
| RepoGraph | Ouyang, Yu, Ma et al. | 2024 | ICLR 2025 | arXiv:2410.14684 |
| Graph RAG: A Survey | Peng, Zhu, Liu, Bo, Shi, Hong, Zhang, Tang | 2024 | arXiv | arXiv:2408.08921 |
| AutoSchemaKG: Autonomous KG Construction via Dynamic Schema Induction | Bai, Fan et al. | 2025 | arXiv | arXiv:2505.23628 |
| When to use Graphs in RAG: A Comprehensive Analysis | DEEP-PolyU et al. | 2025 | arXiv | arXiv:2506.05690 *(authors unverified)* |
| LLM-empowered Knowledge Graph Construction: A Survey | (survey) | 2025 | arXiv | arXiv:2510.20345 *(authors unverified)* |

**SOTA techniques.** LLM triple extraction (schema-guided/competency-question-guided); autonomous schema induction (AutoSchemaKG); Leiden community detection + LLM community summaries; local vs. global search; Personalized PageRank (HippoRAG); subgraph retrieval via Prize-Collecting Steiner Tree (G-Retriever); dual-level indexing with incremental updates (LightRAG); code property / def-ref / call graphs; hybrid graph+lexical+dense fusion with provenance on triples.

**Grafiki today vs. SOTA.** Grafiki embodies the lexical+dense backbone GraphRAG sits on, plus a candidate-with-provenance pipeline resembling schema-guided extract-then-verify. Missing: no graph-structure-aware retrieval (no PPR, no PCST, no k-hop expansion); no community detection / hierarchical summaries (so no global "themes" briefings or consolidation); apparently fixed schema vs. LLM-induced; no triple-level conflict resolution beyond bitemporal supersession; and no code property/def-ref graph. **Lowest-effort high-value adds: k-hop/PPR expansion on the existing `relations` table, and Leiden community summaries for global briefings.**

---

### 2.4 RAG + Hybrid Lexical+Dense Retrieval + Fusion

**Overview.** Retrieval has converged on a multi-stage pattern: cheap first-stage recall, then expensive reranking. First-stage families — sparse/lexical (BM25, SPLADE), dense bi-encoder (DPR, Contriever, MiniLM), late-interaction (ColBERT) — have complementary failure modes, so **hybrid sparse+dense fused by RRF** is standard, usually followed by a cross-encoder or LLM listwise reranker. RAG itself matured from retrieve-then-generate (Lewis 2020) into adaptive/agentic variants (Self-RAG, CRAG).

**Key papers.**

| Title | Authors | Year | Venue | ID |
|---|---|---|---|---|
| Retrieval-Augmented Generation for Knowledge-Intensive NLP | Lewis, Perez, Piktus et al. | 2020 | NeurIPS | arXiv:2005.11401 |
| Dense Passage Retrieval (DPR) | Karpukhin, Oğuz, Min et al. | 2020 | EMNLP | arXiv:2004.04906 |
| ColBERT: Late Interaction over BERT | Khattab, Zaharia | 2020 | SIGIR | arXiv:2004.12832 |
| ColBERTv2 | Santhanam, Khattab, Saad-Falcon, Potts, Zaharia | 2022 | NAACL | arXiv:2112.01488 |
| SPLADE v2: Sparse Lexical and Expansion Model | Formal, Lassance, Piwowarski, Clinchant | 2021 | arXiv | arXiv:2109.10086 |
| Reciprocal Rank Fusion Outperforms Condorcet… | Cormack, Clarke, Buettcher | 2009 | SIGIR | DOI:10.1145/1571941.1572114 |
| BEIR: Heterogeneous Benchmark for Zero-shot IR | Thakur, Reimers, Rücklé, Srivastava, Gurevych | 2021 | NeurIPS D&B | arXiv:2104.08663 |
| Self-RAG | Asai, Wu, Wang, Sil, Hajishirzi | 2023 | ICLR 2024 | arXiv:2310.11511 |
| Corrective RAG (CRAG) | Yan, Gu, Zhu, Ling, Zhang, Yu | 2024 | arXiv | arXiv:2401.15884 |
| RankGPT: LLMs as Re-Ranking Agents | Sun, Yan, Ma et al. | 2023 | EMNLP | arXiv:2304.09542 |
| RAG for LLMs: A Survey | Gao, Xiong, Gao et al. | 2023 | arXiv | arXiv:2312.10997 |
| Contriever: Unsupervised Dense IR via Contrastive Learning | Izacard, Caron, Hosseini, Riedel, Bojanowski, Joulin, Grave | 2021 | TMLR 2022 | arXiv:2112.09118 |

**SOTA techniques.** Multi-stage recall→rerank; hybrid sparse+dense fused by RRF (or calibrated convex/α score fusion); learned sparse (SPLADE); late interaction (ColBERTv2); cross-encoder rerankers (bge-reranker, monoT5) and LLM listwise rerankers (RankGPT, RankZephyr); adaptive/agentic RAG (Self-RAG gating, CRAG grading); query rewriting/decomposition (HyDE, RAG-Fusion); distillation for on-device latency.

**Grafiki today vs. SOTA.** Grafiki implements the modern hybrid core (FTS5 + all-MiniLM + RRF, with `grafiki_ask`/briefings as retrieve-then-generate). Missing the second stage and adaptive layer: (1) **no reranker** — a small cross-encoder or distilled listwise reranker over the RRF top-k would sharply raise cited-evidence precision; (2) **fixed/unweighted fusion** — no query-conditioned or learned weights, frozen off-the-shelf encoder (no SPLADE/domain tuning); (3) **no adaptive/corrective retrieval** (Self-RAG gating, CRAG grading map onto its absent conflict/confidence handling); (4) **no retrieval eval**.

---

### 2.5 Temporal / Bitemporal Knowledge Representation & Temporal KGs

**Overview.** Three traditions Grafiki sits on: (1) bitemporal DB theory (valid time vs. transaction time; Jensen & Snodgrass; SQL:2011); (2) temporal KG representation/reasoning (embeddings: HyTE, DE-SimplE, TNTComplEx; reasoners: RE-GCN, CyGNet, Know-Evolve; interpolation vs. extrapolation); (3) belief revision/truth maintenance (AGM, justification-based TMS) and modern analogues (knowledge editing ROME/MEMIT; agentic temporal memory Zep/Graphiti). The frontier: **bi-temporal KGs with LLM contradiction detection and edge invalidation** (Graphiti) and the recognition that LLMs should *not* be trusted to track freshness — favor deterministic conflict resolution.

**Key papers.**

| Title | Authors | Year | Venue | ID |
|---|---|---|---|---|
| Semantics of Time-Varying Information (BCDM) | Jensen, Snodgrass | 1996 | Information Systems | DOI:10.1016/0306-4379(96)00017-8 |
| The TSQL2 Temporal Query Language | Snodgrass (ed.) et al. | 1995 | Kluwer (→ SQL:2011) | ISBN:978-0-7923-9614-6 |
| Truth Maintenance Systems (justification-based) | Doyle | 1979 | Artificial Intelligence | DOI:10.1016/0004-3702(79)90008-0 |
| On the Logic of Theory Change (AGM) | Alchourrón, Gärdenfors, Makinson | 1985 | J. Symbolic Logic | DOI:10.2307/2274239 |
| HyTE: Hyperplane-based Temporally aware KG Embedding | Dasgupta, Ray, Talukdar | 2018 | EMNLP | ACL:D18-1225 |
| DE-SimplE: Diachronic Embedding for TKG Completion | Goel, Kazemi, Brubaker, Poupart | 2020 | AAAI | arXiv:1907.03143 |
| Know-Evolve: Deep Temporal Reasoning for Dynamic KGs | Trivedi, Dai, Wang, Song | 2017 | ICML | arXiv:1705.05742 |
| ROME: Locating and Editing Factual Associations in GPT | Meng, Bau, Andonian, Belinkov | 2022 | NeurIPS | arXiv:2202.05262 |
| MEMIT: Mass-Editing Memory in a Transformer | Meng, Sharma, Andonian, Belinkov, Bau | 2022 | ICLR 2023 | arXiv:2210.07229 |
| Zep: A Temporal KG Architecture for Agent Memory (Graphiti) | Rasmussen, Paliychuk, Beauvais, Ryan, Chalef | 2025 | arXiv | arXiv:2501.13956 |
| A Survey on Temporal KG: Representation Learning & Applications | Cai, Mao, Zhou, Long, Wu, Lan | 2024 | arXiv | arXiv:2403.04782 |
| A Survey on Temporal KG Embedding | (various) | 2024 | Knowledge-Based Systems v304 | DOI:10.1016/j.knosys.2024.112454 |
| Don't Ask the LLM to Track Freshness | (preprint) | 2026 | arXiv | arXiv:2606.01435 *(unverified)* |
| Governing Evolving Memory in LLM Agents (SSGM) | (preprint) | 2026 | arXiv | arXiv:2603.11768 *(unverified)* |
| A Comprehensive Study of Knowledge Editing (KnowEdit/EasyEdit) | Zhang, Yao, Tian et al.; Wang et al. | 2024 | arXiv; ACL Demo | arXiv:2401.01286 |
| History Matters: Temporal Knowledge Editing | Yin, Jiang, Yang, Wan | 2024 | AAAI | arXiv:2312.05497 |

**SOTA techniques.** Bi-temporal per-edge modeling (valid + ingestion/transaction time, never delete — close validity windows); contradiction detection via hybrid retrieval + LLM comparison, then edge invalidation; deterministic metadata-driven conflict resolution (newest-by-valid-time, source attribution, confidence); justification/provenance-based retraction (TMS) + AGM minimal change with entrenchment; time-aware embeddings/scoring (HyTE, DE-SimplE, TNTComplEx); extrapolation reasoning (RE-GCN, CyGNet); community detection over the temporal graph; knowledge-editing eval discipline (reliability/generality/locality/portability).

**Grafiki today vs. SOTA.** Grafiki implements the bitemporal data layer (valid_from/valid_to soft-deletes = valid time; event log + supersession ≈ a history/transaction axis; evidence links ≈ TMS justifications), and its candidate→trusted pipeline is a manual, high-precision, *reversible* belief-revision gate — safer than ROME/MEMIT weight editing. Missing: (1) **contradiction detection** (Graphiti auto-compares + invalidates; Grafiki only supersedes on explicit human action); (2) **belief-revision/conflict policy** for two contradictory *trusted* facts (no entrenchment, no deterministic newest-by-valid-time arbitration, no provenance-triggered retraction of dependents); (3) **temporally-blind retrieval** — RRF ranks by lexical/semantic match, not validity-at-query-time, so it can surface stale facts; (4) **no temporal reasoning/forecasting, no community tier, no first-class queryable transaction-time axis**; (5) **no eval** (KnowEdit/LongMemEval/TimeQA would measure update reliability, locality, temporal-answer correctness).

---

### 2.6 Human-in-the-loop curation, provenance/lineage, trust of machine-extracted facts

**Overview.** Four threads: (1) HITL curation + active learning; (2) provenance/lineage standards (W3C PROV: Entity/Activity/Agent); (3) weak supervision / data programming (Snorkel) modeling source reliability; (4) trust/credibility scoring — KBP confidence calibration, truth discovery, fact verification (FEVER), LLM uncertainty (semantic entropy). The 2024–2026 frontier fuses LLMs with HITL: LLMs draft/validate, humans adjudicate, with explicit provenance and calibrated confidence routing what needs review.

**Key papers.**

| Title | Authors | Year | Venue | ID |
|---|---|---|---|---|
| Data Programming: Creating Large Training Sets, Quickly | Ratner, De Sa, Wu, Selsam, Ré | 2016 | NeurIPS | arXiv:1605.07723 |
| Snorkel: Rapid Training Data Creation with Weak Supervision | Ratner, Bach, Ehrenberg, Fries, Wu, Ré | 2017 | PVLDB 11(3) | arXiv:1711.10160 |
| PROV-DM: The PROV Data Model | Moreau, Missier et al. (W3C) | 2013 | W3C Rec | w3.org/TR/prov-dm/ |
| FEVER: Fact Extraction and VERification | Thorne, Vlachos, Christodoulopoulos, Mittal | 2018 | NAACL-HLT | arXiv:1803.05355 |
| Resolving Conflicts by Truth Discovery & Source Reliability | Li, Gao, Meng, Su, Zhao, Fan, Han | 2014 | SIGMOD | DOI:10.1145/2588555.2610509 |
| Active Learning Literature Survey | Settles | 2009 | UW-Madison TR 1648 | minds.wisconsin.edu/handle/1793/60660 |
| A Survey of Human-in-the-loop for ML | Wu, Xiao, Sun, Zhang, Ma, He | 2022 | FGCS v135 | arXiv:2108.00941 |
| A Survey of Confidence Estimation & Calibration in LLMs | Geng, Cai, Wang, Koeppl, Nakov, Gurevych | 2024 | NAACL | arXiv:2311.08298 |
| Detecting Hallucinations Using Semantic Entropy | Farquhar, Kossen, Kuhn, Gal | 2024 | Nature v630 | DOI:10.1038/s41586-024-07421-0 |
| A Survey on Deep Active Learning | Li, Yang, Zhao et al. | 2024 | arXiv | arXiv:2405.00334 |
| KG Validation by Integrating LLMs and HITL | Tsaneva, Dessì, Osborne, Sabou | 2025 | IP&M 62(5) | DOI:10.1016/j.ipm.2025.104145 |
| Traceable LLM-based Validation of Statements in KGs | Boros et al. | 2024 | arXiv | arXiv:2409.07507 |
| Factuality of Large Language Models: A Survey | Wang, Liu, Yue et al. | 2024 | arXiv | arXiv:2402.02420 |

**SOTA techniques.** Candidate→approve/edit/reject loops with provenance (Grafiki has this); weak supervision modeling per-source accuracy (Snorkel); active learning to prioritize review (uncertainty + representativeness); confidence calibration (Platt/temperature/isotonic); LLM uncertainty (semantic entropy, self-consistency, token-entropy); FEVER-style evidence verification; truth discovery / data fusion; W3C PROV lineage; LLM-as-validator + human adjudication; traceable validation.

**Grafiki today vs. SOTA.** Grafiki nails the HITL pattern (candidates → human → trusted, with `evidence_links` and event log — a manual quality gate + lineage), conceptually aligned with PROV but not yet PROV-conformant (limiting portability). Missing: (1) **no calibrated confidence/trust score** on candidates (calibration + semantic entropy would auto-prioritize and flag risky auto-extractions); (2) **no active-learning prioritization** (every candidate competes equally for scarce attention); (3) **no conflict resolution** (truth discovery + FEVER-style evidence verification); (4) **no weak-supervision source-reliability model** across its multiple capture channels (transcripts/terminal/git/snapshots).

---

### 2.7 Agent Interoperability / Tool Protocols + Memory-as-a-Service

**Overview.** Lineage from API-calling (Toolformer, Gorilla) and reason+act (ReAct) through tool-learning at scale (ToolLLM, BFCL) to standardized layers. MCP (Nov 2024) became the de-facto agent-tool standard over JSON-RPC 2.0 (Tools/Resources/Prompts), spawning a security literature (tool poisoning, indirect prompt injection, over-privilege) and complementary A2A/ACP/ANP agent-to-agent protocols. A recent thread reframes memory itself as a governed, independently-served module ("Memory-as-a-Service") — exactly Grafiki's niche.

**Key papers.**

| Title | Authors | Year | Venue | ID |
|---|---|---|---|---|
| ReAct: Synergizing Reasoning and Acting in LMs | Yao, Zhao, Yu, Du, Shafran, Narasimhan, Cao | 2022 | ICLR 2023 | arXiv:2210.03629 |
| Toolformer: LMs Can Teach Themselves to Use Tools | Schick, Dwivedi-Yu, Dessì et al. | 2023 | NeurIPS | arXiv:2302.04761 |
| Gorilla: LLM Connected with Massive APIs | Patil, Zhang, Wang, Gonzalez | 2023 | NeurIPS 2024 | arXiv:2305.15334 |
| ToolLLM: Mastering 16000+ Real-world APIs | Qin, Liang, Ye, Zhu, Yan, Lu et al. | 2023 | ICLR 2024 | arXiv:2307.16789 |
| Tool Learning with Foundation Models | Qin, Hu, Lin, Chen, Ding, Cui et al. | 2023 | ACM Comp. Surveys | arXiv:2304.08354 |
| MCP: Landscape, Security Threats, Future Directions | Hou, Zhao, Wang, Wang | 2025 | arXiv | arXiv:2503.23278 |
| A Survey of Agent Interoperability Protocols (MCP/ACP/A2A/ANP) | Ehtesham, Singh et al. | 2025 | arXiv | arXiv:2505.02279 |
| A Survey of AI Agent Protocols | Yang, Chai, Song, Qi, Wen, Li et al. | 2025 | arXiv | arXiv:2504.16736 |
| Memory as a Service (MaaS) | Li et al. | 2025 | arXiv | arXiv:2506.22815 |
| MCPTox: Benchmark for Tool Poisoning on MCP Servers | (under review) | 2025 | arXiv | arXiv:2508.14925 |
| MCP-Bench: Benchmarking Tool-Using Agents via MCP | (see arXiv) | 2025 | arXiv | arXiv:2508.20453 |

*(Note: the survey listed MemGPT here under arXiv:2310.08391; the correct identifier is arXiv:2310.08560, used in §2.1.)*

**SOTA techniques.** JSON-RPC 2.0 Tools/Resources/Prompts over stdio/HTTP (Grafiki's interface); ReAct control loops; retrieval-augmented tool selection (Gorilla/ToolLLM); OS-style tiered memory (MemGPT); governed service-oriented memory modules (Mem0/MaaS); tool-use security hardening (least-privilege, metadata sanitization, injection defenses, capability auditing, signed registries); layered protocol stacks (MCP + A2A/ACP/ANP); function-calling reliability (schema-constrained decoding, argument self-verification).

**Grafiki today vs. SOTA.** Grafiki *is* a local-first, per-project MaaS exposed over MCP (stdio JSON-RPC in `crates/grafiki-cli/src/main.rs`, dispatching `tools/list`/`tools/call` to ~40 `grafiki_*` tools), with the governance posture (provenance, audit log, curation) the MaaS paper calls the frontier. Missing: (1) **MCP-specific security hardening** — it ingests *untrusted* transcripts/terminal output into a tool-exposed store, so tool-metadata sanitization + indirect-prompt-injection defenses + least-privilege capability auditing matter; (2) **reflection/consolidation + self-organizing linking** (A-MEM) and **conflict resolution**; (3) **no eval** (MCP-Bench/BFCL for tool quality, MCPTox for poisoning resistance); (4) **single-server only** — no A2A/ANP path for cross-agent sharing of curated memory.

---

### 2.8 Privacy / local-first / secret redaction + on-device retrieval

**Overview.** Grafiki sits at the intersection of (1) secret/PII detection-and-redaction (regex+entropy scanners → ML/LLM classifiers; SecretBench) and (2) on-device retrieval (small distilled encoders — MiniLM, BGE/GTE/Nomic — Matryoshka-truncatable, running local). A critical under-appreciated thread: **embeddings leak** — Vec2Text/GEIA reconstruct most original text (incl. names/secrets) from a stored vector, directly implicating Grafiki's local vector store. Consensus: hybrid detection, local-first inference, and treating embeddings + their index as sensitive material.

**Key papers.**

| Title | Authors | Year | Venue | ID |
|---|---|---|---|---|
| Sentence-BERT | Reimers, Gurevych | 2019 | EMNLP | arXiv:1908.10084 |
| MiniLM: Deep Self-Attention Distillation | Wang, Wei, Dong, Bao, Yang, Zhou | 2020 | NeurIPS | arXiv:2002.10957 |
| Presidio: PII detection/redaction framework | Microsoft | 2019 | open-source | github.com/microsoft/presidio |
| SecretBench: A Dataset of Software Secrets | Basak, Neil, Reaves, Williams | 2023 | MSR | arXiv:2303.06729 |
| Text Embeddings Reveal (Almost) As Much As Text (Vec2Text) | Morris, Kuleshov, Shmatikov et al. | 2023 | EMNLP | arXiv:2310.06816 |
| MTEB: Massive Text Embedding Benchmark | Muennighoff, Tazi, Magne, Reimers | 2022 | EACL 2023 | arXiv:2210.07316 |
| C-Pack / BGE | Xiao, Liu, Zhang, Muennighoff et al. | 2024 | SIGIR | arXiv:2309.07597 |
| GTE: General Text Embeddings via Multi-stage Contrastive Learning | Li, Zhang, Zhang, Long, Xie, Zhang | 2023 | arXiv | arXiv:2308.03281 |
| Nomic Embed | Nussbaum, Morris, Duderstadt, Mulyar | 2024 | arXiv | arXiv:2402.01613 |
| Matryoshka Representation Learning | Kusupati et al. | 2022 | NeurIPS | arXiv:2205.13147 |
| Secret Breach Detection in Source Code with LLMs | Rahman, Ahmed, Wahab, Sohan, Shahriyar | 2025 | ESEM | arXiv:2504.18784 |
| GEIA: Generative Embedding Inversion Attack | Li, Xu, Mehrotra et al. | 2023 | ACL Findings | arXiv:2305.03010 |
| Privacy-Preserving RAG on Local Devices | (multiple) | 2025 | medRxiv | DOI:10.1101/2025.10.20.25337146 *(unverified)* |
| Mitigating Privacy Risks in RAG via Local Private Entity Perturbation | Zeng et al. | 2025 | IP&M | DOI:10.1016/j.ipm.2025.104150 *(unverified)* |

**SOTA techniques.** Hybrid secret detection (regex candidates + entropy gating + ML/LLM classification); live credential verification (TruffleHog); detect-then-anonymize PII (Presidio); small distilled encoders selected via MTEB; Matryoshka truncation + int8/binary quantization to shrink on-disk vectors; fully offline RAG; treating embeddings as sensitive (access control + DP/perturbation vs. Vec2Text/GEIA); data minimization at capture.

**Grafiki today vs. SOTA.** Grafiki is genuinely local-first (data under `~/.grafiki`, no telemetry, redact-before-write at `propose_candidate`/`ingest_capture_event`, metadata-only default, on-device MiniLM via fastembed). Missing: (1) **regex/prefix-only detection** (no entropy gating, no ML/LLM verification — lags gitleaks/TruffleHog and the LLaMA+regex SOTA on SecretBench); (2) **no PII detection** (Presidio-style detect-then-anonymize is the obvious gap); (3) **no benchmark** (SecretBench/FPSecretBench for redaction, MTEB for embedding choice); (4) **unprotected embeddings** — vectors stored as JSON TEXT in `embedding_vectors` with no awareness Vec2Text/GEIA can invert them; (5) **no live verification, static rule set** (no TOML-configurable rule packs).

---

## 3. GAP ANALYSIS — What Grafiki Is Missing / Could Adopt

Grouped and prioritized. Each gap: *what it is → paper(s)/technique → concrete landing in Grafiki's architecture* (SQLite KG + candidates + hybrid retrieval + MCP).

### HIGH leverage

**H1 — Evaluation harness (there is none today).**
*What:* Grafiki cannot prove its memory helps; every other change is unmeasurable.
*Draw from:* LongMemEval (arXiv:2410.10813), LoCoMo (arXiv:2402.17753), BEIR (arXiv:2104.08663), KnowEdit (arXiv:2401.01286), SWE-bench Verified / SWE-Gym (arXiv:2310.06770, arXiv:2412.21139), SecretBench (arXiv:2303.06729), MTEB (arXiv:2210.07316).
*Landing:* A new `grafiki-eval` crate (or `crates/grafiki-core/eval`) that (a) loads a LongMemEval/LoCoMo conversation, replays it through the capture→candidate→trusted pipeline, then scores `grafiki_ask` answers (single-hop/multi-hop/temporal/update); (b) holds a fixed query→relevant-record gold set to report nDCG@10 / recall@k on the FTS5+RRF pipeline (BEIR-style); (c) runs SecretBench through the redactor for precision/recall; (d) MTEB-subset score to justify the embedding model. This is the *prerequisite* for H2–H5. See §4.

**H2 — Automated conflict resolution / contradiction detection.**
*What:* Supersession is manual only; two contradictory trusted facts can coexist; stale facts surface.
*Draw from:* Zep/Graphiti (arXiv:2501.13956), Mem0 ADD/UPDATE/DELETE/NOOP (arXiv:2504.19413), deterministic-freshness (arXiv:2606.01435 *unverified*), truth discovery (DOI:10.1145/2588555.2610509), AGM (DOI:10.2307/2274239), FEVER (arXiv:1803.05355).
*Landing:* On each new candidate, run the existing hybrid retrieval to find the top-k semantically-related *trusted* observations/relations; an LLM (or rule) flags contradiction; instead of deleting, set the older row's `valid_to = new.valid_from` (the bitemporal store already supports this — reuse the supersession path). Critically, **route the decision through the candidate gate**, not silently: emit a `conflict` candidate type so a human confirms invalidation (preserving Grafiki's trust posture). Arbitration uses deterministic metadata (newest valid_time, source reliability, confidence) rather than asking the LLM to judge freshness.

**H3 — Graph-aware retrieval (k-hop / Personalized PageRank).**
*What:* Grafiki *stores* a graph (`relations` table) but retrieves with flat lexical+dense RRF; multi-hop/relational queries degrade.
*Draw from:* HippoRAG (arXiv:2405.14831), G-Retriever PCST (arXiv:2402.07630), GraphRAG (arXiv:2404.16130), "When to use Graphs in RAG" (arXiv:2506.05690 *unverified*).
*Landing:* Add a third retrieval arm: seed from the FTS5+dense top-k entities, expand 1–2 hops over `relations` (respecting `valid_from`/`valid_to`), and run Personalized PageRank over that subgraph; feed its node ranking as a third list into the existing RRF fusion (lexical + dense + graph). No new store needed — it's a SQL recursive query + an in-memory PPR over the bitemporally-filtered edges. Gate behind a feature flag and measure via H1.

**H4 — Reranking stage over RRF top-k.**
*What:* No second-stage precision boost on cited briefings.
*Draw from:* RankGPT (arXiv:2304.09542), ColBERTv2 (arXiv:2112.01488), bge-reranker (arXiv:2309.07597).
*Landing:* After RRF produces top-N (e.g., 50), run a small local cross-encoder reranker on (query, record) pairs before `grafiki_ask` builds the cited answer. Keep it optional/on-device to preserve local-first latency; distilled listwise (RankZephyr-style) if a small LLM is already loaded for extraction.

**H5 — Reflection / consolidation + community summaries.**
*What:* Grafiki never synthesizes raw observations into higher-level insights, blocking global "themes / what did we decide about X" briefings.
*Draw from:* Generative Agents reflection (arXiv:2304.03442), GraphRAG Leiden+community summaries (arXiv:2404.16130), LightRAG dual-level (arXiv:2410.05779), A-MEM (arXiv:2502.12110).
*Landing:* A periodic (or `grafiki_*`-triggered) consolidation job that runs Leiden community detection over `relations`, then LLM-summarizes each community into a new first-class record — but **enters it as a candidate** (provenance = the source observation IDs) so it inherits the trust gate. `grafiki_ask` then does local search (entity-anchored) for specific questions and community-summary map-reduce for global ones.

### MEDIUM leverage

**M1 — Forgetting / decay & salience.**
*What:* Memory only grows; rankings don't reflect staleness or reuse.
*Draw from:* MemoryBank Ebbinghaus curve (arXiv:2305.10250), Mem0 DELETE/NOOP, SSGM governance (arXiv:2603.11768 *unverified*).
*Landing:* Add a `salience`/`last_accessed`/`access_count` column (or derive from the existing **agent-query audit log**); fold a recency+importance+reuse term into ranking (à la Generative Agents) and into a *soft* decay that proposes low-salience, long-unused candidates for archival via the candidate gate — never hard-deletes.

**M2 — Temporal-aware retrieval (validity-at-query-time).**
*What:* RRF ignores valid-time; stale facts rank equally.
*Draw from:* HyTE (ACL:D18-1225), DE-SimplE (arXiv:1907.03143), TKG surveys (arXiv:2403.04782).
*Landing:* Cheap first step — a valid-time filter/boost in the SQL query (`valid_to IS NULL OR valid_to > now`) plus a recency weight in fusion. Later, diachronic time-aware scoring if H1 shows temporal-QA gaps.

**M3 — Calibrated confidence + active-learning prioritization for candidates.**
*What:* Every candidate competes equally for scarce human attention; no trust score.
*Draw from:* Confidence calibration survey (arXiv:2311.08298), semantic entropy (DOI:10.1038/s41586-024-07421-0), active learning (Settles; arXiv:2405.00334).
*Landing:* Attach a confidence score to each `extraction_candidate` (semantic-entropy over multi-sample extraction, or source-reliability prior); sort `grafiki_candidate_list` by uncertainty×representativeness so reviewers see the highest-value items first; auto-flag low-confidence/auto-extracted items.

**M4 — Code-structure-aware indexing.**
*What:* No AST/def-ref/call graph; can't do symbol-level multi-hop the SWE-bench leaders use.
*Draw from:* RepoGraph (arXiv:2410.14684), code property graphs (DOI:10.1109/SP.2014.44), AutoCodeRover (arXiv:2404.05427).
*Landing:* A capture-time pass (tree-sitter) that emits code entities (functions/classes/files) and def-ref/call/contains relations into the *existing* entity/relation tables — so H3's graph retrieval immediately works over code symbols too. Provenance links back to file snapshots Grafiki already captures.

**M5 — MCP security hardening.**
*What:* Grafiki ingests untrusted transcripts/terminal output into a tool-exposed store.
*Draw from:* MCP threat taxonomy (arXiv:2503.23278), MCPTox (arXiv:2508.14925).
*Landing:* Sanitize/escape ingested content that flows back through `tools/list`/`tools/call` metadata; indirect-prompt-injection guards on captured text; least-privilege capability split (read vs. write/curate tools); add MCPTox-style poisoning tests to H1.

**M6 — PII detection + higher-recall secret detection.**
*What:* Regex/prefix-only secrets, no PII.
*Draw from:* Presidio (github.com/microsoft/presidio), SecretBench (arXiv:2303.06729), LLM secret detection (arXiv:2504.18784).
*Landing:* Add entropy gating + TOML-configurable rule packs (gitleaks-style) and an optional local PII recognizer at the existing redaction trust boundary; measure against SecretBench/FPSecretBench via H1.

### LOW leverage (worthwhile, lower urgency)

**L1 — Protect the vector store against embedding inversion.**
*Draw from:* Vec2Text (arXiv:2310.06816), GEIA (arXiv:2305.03010), local entity perturbation (DOI:10.1016/j.ipm.2025.104150 *unverified*).
*Landing:* Treat `embedding_vectors` with the same access controls/redaction as raw text; optionally quantize (int8/binary, Matryoshka — arXiv:2205.13147) which also shrinks footprint; document the risk.

**L2 — Embedding-model upgrade path.**
*Draw from:* BGE-small (arXiv:2309.07597), GTE (arXiv:2308.03281), Nomic Embed long-context (arXiv:2402.01613), MTEB (arXiv:2210.07316).
*Landing:* BGE-small is a near drop-in for all-MiniLM at the same 384-dim; Nomic's 8192-token window better fits long transcripts. The `embedding_vectors` table already records `provider/model/dimension`, so multi-model coexistence + re-embedding is feasible. Justify any switch via MTEB + H1.

**L3 — Adaptive / corrective retrieval & learned fusion.**
*Draw from:* Self-RAG (arXiv:2310.11511), CRAG (arXiv:2401.15884), SPLADE (arXiv:2109.10086).
*Landing:* A "should I retrieve / is this evidence sufficient" gate in `grafiki_ask`, and query-conditioned fusion weights instead of fixed RRF; longer-term, learned-sparse (SPLADE) for the lexical arm.

**L4 — Formal W3C PROV lineage + agent-to-agent sharing.**
*Draw from:* PROV-DM (w3.org/TR/prov-dm/), interop surveys (arXiv:2505.02279, arXiv:2504.16736), MaaS (arXiv:2506.22815).
*Landing:* Map `evidence_links`/event log onto PROV Entity/Activity/Agent for portable, queryable lineage; expose curated memory over A2A/ANP for cross-agent sharing once single-server value is proven.

**Suggested sequencing:** H1 → (H2 + H3 + H4 in parallel, all reuse existing infra) → H5 → M-tier. Conflict resolution (H2) and graph retrieval (H3) are the highest ROI because they directly exploit Grafiki's two strongest existing assets — the bitemporal supersession machinery and the relations table — that are currently underused at query time.

---

## 4. Evaluation & Benchmarks

Grafiki has **no benchmark today**; this is the top gap. Recommended, layered harness:

**A. Agent-memory QA (does memory answer correctly over long horizons).**
- **LongMemEval** (arXiv:2410.10813) — ~115k-token interactive memory across info extraction, multi-session reasoning, **temporal reasoning**, **knowledge updates**, abstention. *Most directly tests Grafiki's conflict/update gaps.* Zep reports up to +18.5% here — a comparable target.
- **LoCoMo** (arXiv:2402.17753) — ~300-turn / 35-session conversational QA (single-hop, multi-hop, temporal, open-domain).
- *Adaptation:* replay each conversation through capture → candidate → (auto-approve for eval) → trusted, then score `grafiki_ask`. Report per-category accuracy; isolate the *contradiction/update* slice to validate H2.

**B. Retrieval quality (does the right record surface).**
- **BEIR** (arXiv:2104.08663) methodology + **MTEB** (arXiv:2210.07316) for encoder choice; **TREC DL / MS MARCO** for reranker tuning.
- *Adaptation:* build a project-specific gold set (query → relevant observation/decision IDs) and report recall@k and nDCG@10 for lexical-only, dense-only, RRF, RRF+graph (H3), and RRF+rerank (H4) — the only way to know each addition actually helps.
- RAG answer quality: **RAGAS/ARES**-style faithfulness/context-precision/citation correctness on `grafiki_ask`.

**C. Does memory help coding agents (the metric this whole space is judged on).**
- **SWE-bench Verified** / **SWE-bench Lite** (arXiv:2310.06770), **SWE-Gym** (arXiv:2412.21139) executable env, **SWE-Bench Pro** (arXiv:2509.16941) for long-horizon, **SWE-rebench** (arXiv:2505.20411) for decontamination.
- *Adaptation:* A/B a coding agent (Claude Code / OpenHands via MCP) **with vs. without** Grafiki on a repo subset; report Δ resolved-rate, Δ tokens, Δ steps. This is the headline number that justifies the project. Repo-level retrieval can additionally be checked with **RepoEval** (from RepoCoder, arXiv:2303.12570) and **CrossCodeEval**.

**D. Temporal correctness & fact-update discipline.**
- **TimeQA / TempLAMA / TEMPREASON** (return the value valid at a queried time), **KnowEdit** (arXiv:2401.01286) for reliability/generality/locality/portability of updates, **DMR** (Deep Memory Retrieval, MemGPT-vs-Zep).

**E. Trust, redaction & security.**
- **SecretBench** (arXiv:2303.06729) + **FPSecretBench** + secrets-in-issues (arXiv:2410.23657) for redaction precision/recall (closest to Grafiki's NL transcript capture); **FEVER** (arXiv:1803.05355) for evidence-grounded candidate verification; **ECE/Brier/AUROC** for candidate confidence calibration; **MCPTox** (arXiv:2508.14925) + **MCP-Bench** (arXiv:2508.20453) + **BFCL** for tool poisoning resistance and tool quality; embedding-inversion eval (Vec2Text/GEIA, arXiv:2310.06816 / arXiv:2305.03010) to quantify vector-store leakage.

---

## 5. References

1. Park et al., *Generative Agents: Interactive Simulacra of Human Behavior*, UIST 2023 — arXiv:2304.03442
2. Packer et al., *MemGPT: Towards LLMs as Operating Systems*, 2023 — arXiv:2310.08560 *(survey §2.7 cited arXiv:2310.08391; that id is incorrect)*
3. Zhong et al., *MemoryBank: Enhancing LLMs with Long-Term Memory*, AAAI 2024 — arXiv:2305.10250
4. Sumers et al., *Cognitive Architectures for Language Agents (CoALA)*, TMLR 2023 — arXiv:2309.02427
5. Gutiérrez et al., *HippoRAG*, NeurIPS 2024 — arXiv:2405.14831
6. Zhang et al., *A Survey on the Memory Mechanism of LLM-based Agents*, 2024 — arXiv:2404.13501
7. Xu et al., *A-MEM: Agentic Memory for LLM Agents*, NeurIPS 2025 — arXiv:2502.12110
8. Chhikara et al., *Mem0: Production-Ready AI Agents with Scalable Long-Term Memory*, 2025 — arXiv:2504.19413
9. Anokhin et al., *AriGraph*, 2024 — arXiv:2407.04363
10. Maharana et al., *Evaluating Very Long-Term Conversational Memory (LoCoMo)*, ACL 2024 — arXiv:2402.17753
11. Wu et al., *LongMemEval*, ICLR 2025 — arXiv:2410.10813
12. Wang et al., *MIRIX: Multi-Agent Memory System*, 2025 — arXiv:2507.07957
13. Jimenez et al., *SWE-bench*, ICLR 2024 — arXiv:2310.06770
14. Zhang et al., *RepoCoder*, EMNLP 2023 — arXiv:2303.12570
15. Yang et al., *SWE-agent*, NeurIPS 2024 — arXiv:2405.15793
16. Wang et al., *CodeAct: Executable Code Actions*, ICML 2024 — arXiv:2402.01030
17. Zhang et al., *AutoCodeRover*, ISSTA 2024 — arXiv:2404.05427
18. Wang et al., *OpenHands*, ICLR 2025 — arXiv:2407.16741
19. Xia et al., *Agentless*, FSE/PACMSE 2025 — arXiv:2407.01489
20. Ouyang et al., *RepoGraph*, ICLR 2025 — arXiv:2410.14684
21. Pan et al., *SWE-Gym*, ICML 2025 — arXiv:2412.21139
22. Yang et al., *SWE-smith*, NeurIPS 2025 — arXiv:2504.21798
23. Deng et al. (Scale AI), *SWE-Bench Pro*, 2025 — arXiv:2509.16941
24. *SWE-rebench*, 2025 — arXiv:2505.20411
25. Yamaguchi et al., *Modeling and Discovering Vulnerabilities with Code Property Graphs*, IEEE S&P 2014 — DOI:10.1109/SP.2014.44
26. Huguet Cabot & Navigli, *REBEL*, Findings of EMNLP 2021 — ACL:2021.findings-emnlp.204
27. Pan et al., *Unifying LLMs and Knowledge Graphs: A Roadmap*, 2023 — arXiv:2306.08302
28. Edge et al. (Microsoft), *From Local to Global: A GraphRAG Approach*, 2024 — arXiv:2404.16130
29. He et al., *G-Retriever*, NeurIPS 2024 — arXiv:2402.07630
30. Guo et al., *LightRAG*, 2024 — arXiv:2410.05779
31. Peng et al., *Graph RAG: A Survey*, 2024 — arXiv:2408.08921
32. Bai, Fan et al., *AutoSchemaKG*, 2025 — arXiv:2505.23628
33. *When to use Graphs in RAG: A Comprehensive Analysis*, 2025 — arXiv:2506.05690 *(authors unverified)*
34. *LLM-empowered Knowledge Graph Construction: A Survey*, 2025 — arXiv:2510.20345 *(authors unverified)*
35. Lewis et al., *Retrieval-Augmented Generation for Knowledge-Intensive NLP*, NeurIPS 2020 — arXiv:2005.11401
36. Karpukhin et al., *Dense Passage Retrieval (DPR)*, EMNLP 2020 — arXiv:2004.04906
37. Khattab & Zaharia, *ColBERT*, SIGIR 2020 — arXiv:2004.12832
38. Santhanam et al., *ColBERTv2*, NAACL 2022 — arXiv:2112.01488
39. Formal et al., *SPLADE v2*, 2021 — arXiv:2109.10086
40. Cormack et al., *Reciprocal Rank Fusion*, SIGIR 2009 — DOI:10.1145/1571941.1572114
41. Thakur et al., *BEIR*, NeurIPS 2021 D&B — arXiv:2104.08663
42. Asai et al., *Self-RAG*, ICLR 2024 — arXiv:2310.11511
43. Yan et al., *Corrective RAG (CRAG)*, 2024 — arXiv:2401.15884
44. Sun et al., *RankGPT*, EMNLP 2023 — arXiv:2304.09542
45. Gao et al., *Retrieval-Augmented Generation for LLMs: A Survey*, 2023 — arXiv:2312.10997
46. Izacard et al., *Contriever*, TMLR 2022 — arXiv:2112.09118
47. Jensen & Snodgrass, *Semantics of Time-Varying Information (BCDM)*, Information Systems 1996 — DOI:10.1016/0306-4379(96)00017-8
48. Snodgrass (ed.), *The TSQL2 Temporal Query Language*, Kluwer 1995 — ISBN:978-0-7923-9614-6
49. Doyle, *A Truth Maintenance System*, Artificial Intelligence 1979 — DOI:10.1016/0004-3702(79)90008-0
50. Alchourrón, Gärdenfors, Makinson, *On the Logic of Theory Change (AGM)*, J. Symbolic Logic 1985 — DOI:10.2307/2274239
51. Dasgupta et al., *HyTE*, EMNLP 2018 — ACL:D18-1225
52. Goel et al., *DE-SimplE*, AAAI 2020 — arXiv:1907.03143
53. Trivedi et al., *Know-Evolve*, ICML 2017 — arXiv:1705.05742
54. Meng et al., *ROME*, NeurIPS 2022 — arXiv:2202.05262
55. Meng et al., *MEMIT*, ICLR 2023 — arXiv:2210.07229
56. Rasmussen et al., *Zep / Graphiti: A Temporal KG Architecture for Agent Memory*, 2025 — arXiv:2501.13956
57. Cai et al., *A Survey on Temporal Knowledge Graph*, 2024 — arXiv:2403.04782
58. *A Survey on Temporal KG Embedding*, Knowledge-Based Systems v304, 2024 — DOI:10.1016/j.knosys.2024.112454
59. *Don't Ask the LLM to Track Freshness*, 2026 — arXiv:2606.01435 *(unverified)*
60. *Governing Evolving Memory in LLM Agents (SSGM)*, 2026 — arXiv:2603.11768 *(unverified)*
61. Zhang, Yao, Tian et al., *A Comprehensive Study of Knowledge Editing (KnowEdit / EasyEdit)*, 2024 — arXiv:2401.01286
62. Yin et al., *History Matters: Temporal Knowledge Editing*, AAAI 2024 — arXiv:2312.05497
63. Ratner et al., *Data Programming*, NeurIPS 2016 — arXiv:1605.07723
64. Ratner et al., *Snorkel*, PVLDB 11(3) 2017 — arXiv:1711.10160
65. Moreau, Missier et al., *PROV-DM: The PROV Data Model*, W3C Rec 2013 — w3.org/TR/prov-dm/
66. Thorne et al., *FEVER*, NAACL-HLT 2018 — arXiv:1803.05355
67. Li et al., *Resolving Conflicts by Truth Discovery & Source Reliability*, SIGMOD 2014 — DOI:10.1145/2588555.2610509
68. Settles, *Active Learning Literature Survey*, UW-Madison TR 1648, 2009 — minds.wisconsin.edu/handle/1793/60660
69. Wu et al., *A Survey of Human-in-the-loop for ML*, FGCS v135 2022 — arXiv:2108.00941
70. Geng et al., *A Survey of Confidence Estimation and Calibration in LLMs*, NAACL 2024 — arXiv:2311.08298
71. Farquhar et al., *Detecting Hallucinations Using Semantic Entropy*, Nature v630 2024 — DOI:10.1038/s41586-024-07421-0
72. Li et al., *A Survey on Deep Active Learning*, 2024 — arXiv:2405.00334
73. Tsaneva et al., *KG Validation by Integrating LLMs and HITL*, IP&M 62(5) 2025 — DOI:10.1016/j.ipm.2025.104145
74. Boros et al., *Traceable LLM-based Validation of Statements in KGs*, 2024 — arXiv:2409.07507
75. Wang et al., *Factuality of Large Language Models: A Survey*, 2024 — arXiv:2402.02420
76. Yao et al., *ReAct*, ICLR 2023 — arXiv:2210.03629
77. Schick et al., *Toolformer*, NeurIPS 2023 — arXiv:2302.04761
78. Patil et al., *Gorilla*, NeurIPS 2024 — arXiv:2305.15334
79. Qin et al., *ToolLLM*, ICLR 2024 — arXiv:2307.16789
80. Qin et al., *Tool Learning with Foundation Models*, ACM Computing Surveys 2023 — arXiv:2304.08354
81. Hou et al., *MCP: Landscape, Security Threats, Future Directions*, 2025 — arXiv:2503.23278
82. Ehtesham, Singh et al., *A Survey of Agent Interoperability Protocols (MCP/ACP/A2A/ANP)*, 2025 — arXiv:2505.02279
83. Yang et al., *A Survey of AI Agent Protocols*, 2025 — arXiv:2504.16736
84. Li et al., *Memory as a Service (MaaS)*, 2025 — arXiv:2506.22815
85. *MCPTox: Benchmark for Tool Poisoning on MCP Servers*, 2025 — arXiv:2508.14925
86. *MCP-Bench: Benchmarking Tool-Using LLM Agents via MCP*, 2025 — arXiv:2508.20453
87. Reimers & Gurevych, *Sentence-BERT*, EMNLP 2019 — arXiv:1908.10084
88. Wang et al., *MiniLM*, NeurIPS 2020 — arXiv:2002.10957
89. Microsoft, *Presidio*, 2019 — github.com/microsoft/presidio
90. Basak et al., *SecretBench*, MSR 2023 — arXiv:2303.06729
91. Morris et al., *Text Embeddings Reveal (Almost) As Much As Text (Vec2Text)*, EMNLP 2023 — arXiv:2310.06816
92. Muennighoff et al., *MTEB*, EACL 2023 — arXiv:2210.07316
93. Xiao et al., *C-Pack / BGE*, SIGIR 2024 — arXiv:2309.07597
94. Li et al., *GTE*, 2023 — arXiv:2308.03281
95. Nussbaum et al., *Nomic Embed*, 2024 — arXiv:2402.01613
96. Kusupati et al., *Matryoshka Representation Learning*, NeurIPS 2022 — arXiv:2205.13147
97. Rahman et al., *Secret Breach Detection in Source Code with LLMs*, ESEM 2025 — arXiv:2504.18784
98. Li et al., *GEIA: Generative Embedding Inversion Attack*, ACL Findings 2023 — arXiv:2305.03010
99. *Privacy-Preserving RAG on Local Devices*, medRxiv 2025 — DOI:10.1101/2025.10.20.25337146 *(unverified)*
100. Zeng et al., *Mitigating Privacy Risks in RAG via Local Private Entity Perturbation*, IP&M 2025 — DOI:10.1016/j.ipm.2025.104150 *(unverified)*
101. *Secret-leak-in-issues benchmark*, 2024 — arXiv:2410.23657

*Multi-hop QA sets cited throughout (HotpotQA, 2WikiMultiHopQA, MuSiQue), classic IR/QA sets (MS MARCO, Natural Questions, TriviaQA, KILT), persona/multi-session sets (MSC, PerLTQA), temporal KG datasets (ICEWS14/05-15, GDELT, YAGO11k, Wikidata12k), temporal-QA sets (TimeQA, TempLAMA, TEMPREASON), and tool benchmarks (BFCL, ToolBench/arXiv:2307.16789, APIBench) are standard community resources referenced by the works above rather than separate primary citations.*

---

**Bottom line for maintainers:** Grafiki has built the hard, trustworthy foundation — a bitemporal KG, hybrid retrieval, and a provenance-first human-in-the-loop gate — that the literature *assumes*. The next phase is activating that foundation: build the eval harness first (H1), then layer conflict resolution (H2), graph-aware retrieval (H3), reranking (H4), and consolidation (H5), all of which reuse the relations table, supersession machinery, RRF pipeline, and candidate gate you already have.