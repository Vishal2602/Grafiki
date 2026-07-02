# Code Graph + Injection + Benchmark — full scope

*Scoped 2026-07-02, after the graphify competitive research (see `COMPETITIVE_LANDSCAPE.md`
and the memory note `graphify-competitor`). This document is the source of truth for the
initiative; nothing starts until this scope is agreed.*

## Why (one paragraph)

Graphify (76k stars in 3 months, YC S26) proved the market wants "agents query a compact
graph instead of re-reading files" and that a crisp token-reduction number sells — even
though their benchmark never runs an agent (it divides corpus size by subgraph size).
Grafiki already owns the harder half of this: a real knowledge graph with PPR retrieval,
an MCP server, session capture with real transcripts, and a hosted terminal we can inject
into. The initiative: close the code-structure gap, add the injection path, and publish an
HONEST measured token number graphify structurally cannot match. Differentiation thesis:
**one graph that holds code structure AND decisions AND time** — "this module talks to that
one, and here's the session where we decided why."

## What already exists (do not rebuild)

| Piece | Where | State |
|---|---|---|
| Code→graph extraction (Rust via `syn`) | `code_index.rs` (M-E4) | Shipped, behind `code-index` feature, Rust-only, `part_of` edges only |
| Graph retrieval (Personalized PageRank, HippoRAG-style) | `graph.rs` (H3), fused into hybrid search RRF | Shipped |
| MCP server (save/decide/search…, read-only mode, injection-detection at retrieval boundary) | `grafiki mcp` in the CLI | Shipped |
| Hosted terminal with PTY write access (types into live agent sessions) | desktop `terminal.rs` | Shipped |
| Session capture + extraction cursor + real transcripts | capture pipeline, `transcript.rs` | Shipped |
| Entity/relation tables, scopes, evidence links, bitemporal timestamps | `memory.rs` / schema | Shipped |

## Tier 1 — build now (~1 week of sessions)

### T1.1 Token-budgeted graph answers — `grafiki map` + MCP tool
- New core fn: seed PPR from query terms → ranked subgraph → render as a compact digest
  (nodes + edges + linked decisions/context), **cut at a token budget** (default ~2k,
  4-chars/token estimate is fine for cutting).
- Surfaces: `grafiki map "<question>"` CLI command; `grafiki_map` MCP tool; desktop uses it
  for the session-start brief (T1.2).
- The digest interleaves CODE nodes and MEMORY records when both are relevant — this is the
  differentiator; never ship a code-only digest if decisions link to those entities.
- Acceptance: on Grafiki's own repo, "how does capture extraction work" returns a ≤2k-token
  digest naming the right functions/files AND the relevant shipped decisions; deterministic
  given the same graph.

### T1.2 Session-start injection (hosted terminal)
- On spawning/reviving a claude session, Grafiki types a one-page brief: project map
  headline (top communities/god-nodes), top pending/approved decisions for this scope, and
  the instruction "query grafiki (MCP `grafiki_map`/search) before reading files".
- Once-guarded like the existing handoff-prompt injection (sessionStorage + single-line).
- Configurable: Settings toggle (default ON for claude sessions, OFF for bare shells);
  never inject when not capturing.
- Skill/CLAUDE.md snippet generator: `grafiki install-skill` writes the "query before you
  read" guidance for non-hosted agents (mirrors graphify's nudge layer).
- Acceptance: live session visibly consults grafiki before file exploration on a seeded
  question; injection adds ≤1.5k tokens.

### T1.3 Multi-language extraction — tree-sitter, TypeScript + Python first
- **Posture decision to confirm:** M-E4 chose `syn` to stay pure-Rust. tree-sitter crates
  compile C grammars via `cc` — accepted here as the cost of multi-language (graphify's 36
  grammars prove the approach; still a single static binary, just a slower build).
- Architecture: keep the existing write path (entities + `part_of` relations, idempotent
  upserts, deterministic import — NOT candidate-gated); add a `tree_sitter` extractor
  backend beside `walk_item`, one grammar module per language.
- Languages in Tier 1: **TypeScript/TSX, JavaScript, Python** (+ Rust stays on `syn`).
  Extract: functions, classes/interfaces, methods, exported consts, imports (file→file
  `imports` edges — cheap and high-value, unlike full call resolution).
- Feature flag: promote `code-index` to default-on once TS+Py land.
- Acceptance: indexing Grafiki's own repo graphs BOTH src-tauri (Rust) and App.tsx (TS);
  eval-style fixture tests per language (fixture file → expected entities/edges).

### T1.4 `grafiki benchmark` — the honest number
- Methodology (documented in the output itself):
  - (A) **with memory**: replay N real questions against `grafiki map`/ask; count actual
    prompt+response tokens of the digest an agent would consume.
  - (B) **without memory**: measure what the cold agent actually did — from OUR captured
    transcripts, sum the tool-result tokens of the exploration (greps + file reads) that
    answered the same question the first time. Real usage, not a strawman.
  - Report per-question tokens, median/mean reduction, AND the caveat table (small repos
    ≈ 1×, like graphify honestly admits) AND the non-derivable class: questions whose
    answer exists only in session memory (reduction = ∞ / "not re-derivable").
- Print a one-line summary after every extraction run (graphify's trick; it converts).
- Output: `grafiki benchmark` (plain/md/json) + a committed `worked/` example with real
  inputs and outputs so anyone can reproduce.
- Acceptance: number is reproducible from the committed example; README section drafted.

## Tier 2 — after Tier 1 ships and is re-assessed (+2–3 weeks)

- **T2.1 Languages wave 2:** Go, Java, C/C++, Ruby, PHP, shell (~½ day each once T1.3
  plumbing exists).
- **T2.2 Cross-file `calls` edges (M-E4b):** per-language import resolution + same-file
  binding heuristics; confidence-tagged edges (borrow graphify's EXTRACTED/INFERRED idea —
  it maps cleanly onto our existing confidence machinery). Enables "what breaks if I
  change X".
- **T2.3 Graph view pane in desktop** (already on the UX backlog): render the scope's
  subgraph, click node → session/memory detail. Snowy-Rainforest styling.
- **T2.4 Community clustering** (Leiden or label-propagation) for map headlines and a
  `GRAPH_REPORT`-style export.
- **T2.5 Repo watcher:** re-index changed files on session start / git HEAD change
  (content-hash cache like graphify's SHA256 skip).

## Tier 3 — explicitly OUT of scope (their moat of grind, not our fight)

- The remaining ~28 tree-sitter grammars (R, Erlang, …) — add on demand only.
- PDFs / images / video / audio ingestion into the code graph.
- Obsidian export, PR-triage dashboard, FalkorDB/Neo4j push, HTML graph file.
- Multi-repo "global graph" (our multi-project switcher backlog item covers the need
  differently).

## Decisions needed before starting (confirm with user)

1. **tree-sitter accepted?** (changes the pure-Rust build posture; recommendation: yes)
2. **Injection default-on** for hosted claude sessions? (recommendation: yes, with toggle)
3. **Benchmark baseline** = replayed real exploration from our transcripts (recommendation)
   vs. graphify-style whole-corpus ratio (only as a clearly-labeled secondary number).
4. Sequencing: this displaces "live in the app + polish" for ~a week. Accepted?

## Risks / honesty notes

- Injection ADDS tokens; the win must be measured end-to-end or we're graphify.
- An agent may ignore the nudge and read files anyway — the benchmark must measure real
  behavior, and the nudge text will need iteration.
- Tree-sitter grammar quality varies; fixture tests per language are non-negotiable.
- Deterministic import bypasses the review gate by design (M-E4 decision) — keep code
  entities in the `code` scope so they never pollute decision/context review flows.
