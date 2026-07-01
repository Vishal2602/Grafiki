# DESIGN: Grafiki — "Chat with your memory" (local, grounded RAG)

**Status:** Phases 1–2 DONE + Phase 3 desktop chat pane DONE; the app-bundled runtime and Phase 4 proposed.
**Update 2026-07-01 (Phase 3 UI):** the **desktop chat pane** shipped — a `chat_with_memory` Tauri
command (`apps/grafiki-desktop/src-tauri/src/lib.rs`, same Ollama+extractive-fallback logic as the
CLI), an `chatWithMemory` API binding, and a `ChatPane` React view (new `"chat"` nav item) that shows
grounded answers, clickable **citations** (open the cited record), a "Use local AI" toggle
(default `gemma3:1b`), and the abstain/injection notices. Verified: `cargo check`/clippy clean +
frontend `tsc && vite build` clean (live rendering is user-verified). REMAINING for the full
self-contained vision: an app-bundled model runtime (download the .gguf through the setup UI, no
separate Ollama) — implements the same `ChatProvider` seam.
**Update 2026-07-01 (Phase 2):** local-model generation shipped via **Ollama** —
`chat::build_grounded_messages` (the anti-hallucination system prompt: answer only from the numbered
memories, cite `[n]`, abstain with `NO_MEMORY_ANSWER`, treat memories as untrusted data) +
`chat::OllamaProvider` (raw-HTTP `/api/chat`, non-streaming, default model `gemma3:1b`, no new deps /
no in-process runtime). `ChatProvider::generate` is now fallible; CLI `grafiki chat --model gemma3:1b
[--ollama-url …]` uses it and **falls back to the extractive answer** (with a note) if the model is
unreachable. Verified with a mock-server round-trip test (grounded prompt sent, answer parsed). A
fully app-bundled runtime (no separate Ollama) can implement the same `ChatProvider` later — that's
the remaining piece of the "download-through-the-UI, self-contained" vision.
**Update 2026-06-30:** `grafiki_core::chat` shipped — `ChatProvider` seam +
`ExtractiveProvider` (deterministic, model-free) + `chat`/`chat_with_provider` in `memory.rs`
(retrieve → ground → generate → cite; abstains with `NO_MEMORY_ANSWER` when nothing is relevant;
injection-scans snippets; logs to the reuse-salience audit). Surfaces: CLI `grafiki chat "<q>"` and
MCP `grafiki_chat` (read-only-safe). The local-model provider is Phase 2 — it drops into the same
`ChatProvider` seam without touching retrieval or the surfaces.
**Crates:** `grafiki-core` (retrieval reuse + `chat` module), `grafiki-cli` (CLI + MCP surface),
`apps/grafiki-desktop` (chat UI + model-download setup)
**Depends on:** the existing retrieval stack (`ask_memory`, `hybrid_search_results`, Graph/Rerank),
the redaction trust boundary, the M-E5 injection guard, and the fastembed model-download pattern.
**North star:** `~/.claude/.../memory/grafiki-vision.md` — goal #3 ("chat with your memory", à la
Granola) and a "better than the big players" differentiator (local + cited answers).

---

## 1. What it is

A chat box (and tool) that lets a user — or an agent — **ask Grafiki's memory a question in
plain language and get a grounded, cited answer.** It is retrieval-augmented generation (RAG)
where the *retrieval* half is Grafiki's existing, high-quality engine and the *generation* half is
a small, **local** language model. Nothing leaves the machine.

Two hard product constraints, both flowing from the user's origin pain (long sessions hallucinate):
1. **Grounded, never invented.** The answer must be built ONLY from retrieved memory. If the memory
   doesn't contain it, the honest answer is "I don't have that in your memory yet." Anti-hallucination
   is the whole point — a chat that fabricates would betray the reason Grafiki exists.
2. **Cited.** Every answer names the memories it used (record ids / titles). Grafiki already tracks
   provenance (`evidence_links`, ids); surfacing it makes answers auditable — a concrete edge over
   cloud memory products that return unsourced prose.

## 2. Principles

- **Local-first / private by default.** The model runs on the user's machine (goal from the vision).
  A cloud key is at most an opt-in, never the default. "Your memory never leaves your house, and
  neither does the AI reading it" is a selling point Mem0/Zep (cloud services) cannot match.
- **Reuse the retrieval engine as-is.** The hard 80% (hybrid + graph + rerank + scope) is built and
  eval-gated. Chat is a thin, well-tested layer on top — not a new retrieval system.
- **Pluggable generation behind a trait.** A `ChatProvider` abstraction so the answerer can be:
  the calling agent (works today, no model), a local model, or (opt-in) a cloud model — without
  touching retrieval or the surfaces.
- **Treat retrieved memory as DATA, not instructions.** Run the M-E5 injection guard over retrieved
  snippets before they enter the prompt; the system prompt states the memory is untrusted content.
- **Deterministic where it can be; model only for phrasing.** Retrieval, citation selection, and the
  "insufficient evidence → abstain" decision are deterministic; the model only phrases the answer.

## 3. Architecture

```
question ─▶ retrieve (ask_memory/hybrid, scope-aware)  ─▶ [Memory hits + ids]
                                                            │
              M-E5 injection scan over snippets ◀───────────┘
                                                            │
         build grounded prompt (system + memory-as-context + question)
                                                            │
                                 ChatProvider.answer(prompt) ─▶ answer text
                                                            │
        attach citations (ids actually referenced) + abstain-if-empty  ─▶ ChatReply
```

- **`grafiki_core::chat`** (new): `ChatRequest { question, scope, limit }`, `ChatReply { answer,
  citations: Vec<Citation>, used_memory: bool }`, `Citation { record_type, id, title }`. Assembles
  the grounded prompt from a retrieval call; enforces the abstain contract (no hits ⇒ the fixed
  "not in memory yet" sentence, no model call); post-checks that cited ids were actually retrieved.
- **`ChatProvider` trait**: `fn answer(&self, prompt: &ChatPrompt) -> Result<String>`. Impls:
  - `AgentProvider` (default, no model): returns the assembled grounded briefing for the *calling
    agent* to phrase — i.e. the MCP path works day one with the agent's own model.
  - `LocalModelProvider` (feature `chat`): a small instruct GGUF model via a Rust llama.cpp binding
    (candidate: `llama-cpp-2`) — CPU-friendly, offline. Model chosen for size/license (candidate:
    a ~1–3B permissively-licensed instruct model, e.g. Qwen2.5-1.5B-Instruct / Llama-3.2-1B-Instruct),
    downloaded on setup to the managed cache dir (reuse the `fastembed_cache_dir` + progress pattern).
  - (Later, opt-in) `CloudProvider` behind an API key; `OllamaProvider` if a local Ollama is present.

## 4. Surfaces

- **MCP `grafiki_chat`** — question in, grounded briefing + citations out. With `AgentProvider` this
  makes "chat with your memory from inside Claude Code/Cursor" work immediately, no local model.
- **CLI `grafiki chat "<question>"`** — for scripts and the desktop sidecar; `--scope`, `--json`.
- **Desktop chat UI** — a chat pane in `grafiki-desktop` that drives the CLI sidecar with the local
  model, plus a first-run "Download local AI" setup step (progress bar) mirroring the existing
  embedding-model download.

## 5. Phases

1. **Chat core + MCP/CLI, `AgentProvider` (no new model).** `grafiki_core::chat`, grounded-prompt
   assembly, abstain contract, citations, injection guard; `grafiki_chat` MCP tool + `grafiki chat`
   CLI. Ships value immediately (agent-driven chat) and is fully model-free/CI-testable.
2. **`LocalModelProvider` (feature `chat`).** llama.cpp binding + model download-on-setup + a small
   default model; best-effort/no-op without the model (default build unaffected, like `fastembed`).
3. **Desktop chat UI + setup flow.** Chat pane + guided local-AI download with progress + citation
   rendering.
4. **Hardening + eval.** A "grounded-QA" eval arm: fixed memory → questions with known answers →
   assert (a) answer contains the grounded fact, (b) abstains when the fact is absent, (c) never
   asserts a `stale_forbidden` token — a model-free CI gate on the deterministic retrieval+abstain
   layer, with the model path measured under a nightly feature.

## 6. Risks

1. **Hallucination.** *Mitigation:* grounded system prompt ("answer only from the memory below; if
   absent, say so"), deterministic abstain-on-no-hits, citation post-check (drop cited ids that
   weren't retrieved), and an eval arm that fails on invented facts.
2. **Model size / laptop performance.** *Mitigation:* small quantized model; streamed tokens in the
   UI; the feature is optional (agent-driven path needs no local model at all).
3. **llama.cpp build complexity / portability (incl. Windows).** *Mitigation:* feature-gated like
   `fastembed`; the default build never compiles it; sidecar built in release CI per-platform.
4. **Model licensing / redistribution.** *Mitigation:* pick a permissively-licensed model, download
   on setup (not bundled), document provenance.
5. **Prompt-injection via poisoned memory.** *Mitigation:* M-E5 injection scan on retrieved snippets;
   memory framed as untrusted data in the system prompt.

## 7. Why this is a "better than the big players" story

Local (private) + **cited** (auditable) + grounded (won't invent) + built on a bitemporal store that
knows what's current vs. stale. Mem0/Zep are cloud, return unsourced prose, and don't model
supersession. Pair the shipped feature with the eval harness (a published grounded-QA + supersession
benchmark) to make the claim with numbers, not adjectives.
