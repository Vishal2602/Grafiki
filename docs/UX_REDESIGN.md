# Grafiki — UX Redesign from Scratch

> The ground-up product/UX plan: positioning, sitemap, every screen, every flow,
> onboarding, states, and build phases. References: Granola (ambient capture +
> the ledger as home), Wispr Flow (invisible tool, magic on first use), Linear
> (keyboard-first speed, inbox triage).
>
> Status: PROPOSAL — supersedes the current four-tab UI when approved.

---

## 1. Positioning (one sentence the whole UI serves)

**"Granola for your AI coding sessions."** You work with your agent normally;
Grafiki sits *around* the session, listens, and turns it into durable memory
that briefs your next session. The user never "operates" Grafiki — they *visit*
it to see what it learned and to answer "what did we decide?"

**The identity mistake in the current app:** the terminal is the home screen.
The terminal is plumbing. Granola's home isn't the microphone — it's the list
of meetings with notes that got better on their own. **Grafiki's home must be
the ledger: sessions + what was learned from each.**

## 2. Who it's for (jobs)

1. **During work** — solo dev running Claude Code daily. Job: *stop re-explaining
   context to the agent every session.* (Served invisibly: capture + briefing.)
2. **Returning** — same dev after a weekend/two weeks. Job: *"what did I decide
   about X and why?"* (Served by Home ledger + Chat.)
3. **The agent itself** — programmatic user starting a session. Job: *get briefed.*
   (Served by MCP; surfaced to the human as "briefed with n memories.")

## 3. Design principles

1. **Ambient, not administrative.** Capture is invisible; review is a 30-second
   inbox triage, not a chore screen.
2. **The memory is the product.** Every screen answers: *what did Grafiki learn,
   and can I trust it?*
3. **Honest everywhere.** Truthful capture status, truthful model status, cited
   answers, explicit abstention. Trust is the moat.
4. **Linear-fast.** Keyboard-first, ⌘K everywhere, sub-100ms interactions,
   zero dead ends (every empty state has one sentence + one CTA).
5. **One calm surface.** "Snowy Rainforest" duotone: permanent evergreen rail,
   snow paper sheet, two-tier green accent (see DESIGN.md). Calm, not
   dashboard-y.

## 4. Sitemap

```
Grafiki
├── Onboarding (first run only — 4 steps, <90s)
├── ⌘K Command palette (global)
├── HOME — "Today" session ledger              ← default screen
│   ├── Live session card        → Session (live)
│   ├── Needs-review strip (n)   → Review
│   └── Day-grouped session rows → Session (past)
├── SESSIONS
│   ├── Live: hosted terminal + "Learned this session" side peek
│   └── Past: Memories · Summary · Raw events (+ Resume)
├── MEMORY
│   ├── Chat — ask your memory (primary)
│   └── Browse — Decisions · Context/Conventions · Gotchas · Entities
├── REVIEW — the inbox (badge = pending count)
└── SETTINGS — Projects · Capture & privacy · Local AI · Agent hookups · About
```

Left rail: project switcher on top, then Home · Sessions · Memory · Review(n) ·
Settings. Rail collapses to icons below ~900px width.

## 5. Screens

### 5.0 Onboarding (first run)

Full-window, no chrome, 4 steps, skippable only where honest.

- **Step 1 — Welcome.** Brand mark, one line: *"Your AI forgets every session.
  Grafiki remembers."* Sub-line: local-first, nothing leaves this Mac.
  [Get started]
- **Step 2 — Where do you work?** Folder picker + drag-drop. On pick: create
  `.grafiki` + DB, green check with the DB path shown (transparency = trust).
  Suggest recently-used folders if detectable.
- **Step 3 — Local AI.** Probe Ollama. Three states:
  - Found + models → show installed models, pre-select best, [Continue].
  - Found, no models → one-click `ollama pull gemma3:1b` with progress bar.
  - No Ollama → [Get Ollama] link + honest skip: *"Grafiki still records and
    you can review manually; automatic memory extraction starts when a local
    model exists."*
- **Step 4 — First session.** *"Start your first captured session."* Agent
  buttons (Claude Code / Codex / Gemini / Shell). Launches Session (live) with
  a one-time overlay tip: *"Work normally. Grafiki is listening."*

Exit criterion: the user is inside a live captured session within 90 seconds.

### 5.1 Home — the ledger (the identity screen, new)

Single scrollable column (Granola-style), most-recent first.

- **Header:** project name, memory status pill, **[Resume last session]** when a
  resumable session exists (the VS Code feel).
- **Live session card** (when running): agent icon, elapsed time, cwd, last
  output line, capturing pulse dot, live "learned n so far" counter, [Open].
- **Needs-review strip:** compact chips for top pending candidates (title +
  type icon + confidence); hover → quick ✓/✕; [Review all n].
- **Timeline:** day headers ("Today", "Yesterday", "Jun 28"), then session
  rows: `9:14–10:02 · claude · ~/Project/Grafiki · 4 memories` + up to two
  memory titles inline. Click → Session (past).
- **Empty state** (brand-new project): the Step-4 launcher repeated.

Why it matters: this is the screen that proves the ambient capture works —
the "notes got better on their own" moment, every morning.

### 5.2 Session — live

- Full-bleed terminal (existing detached-PTY + reattach + revive behavior).
- Top bar: agent · cwd · truthful capture status · elapsed · [End session].
- **"Learned this session" side peek** (collapsible right panel, the Granola
  magic made visible): as heartbeat extraction proposes candidates, they slide
  in here with type icon + title; click → approve/reject inline or open Review.
- On agent start: a quiet line "Briefed the agent with n memories" (when the
  MCP briefing fires) — the loop made legible.

### 5.2b Session — the two lenses (agent chat)

There are two chats in this product and they must stay distinct:
**Memory Chat** ("what do we know?" — grounded, cited, abstains) and
**Agent chat** (the conversation with the working agent). The agent chat is
NOT a new system — it is a second renderer over the same hosted PTY session:

- **Terminal lens** — the raw PTY (today's view). Always works, any agent;
  the fallback for interactive moments (permission prompts, menus).
- **Chat lens** (default for Claude Code) — the live session rendered as a
  conversation: user prompts as bubbles, agent replies as rich text, tool
  calls collapsed to compact cards ("Edited terminal.rs +34 −12", expand for
  detail). The composer at the bottom writes into the PTY. Implementation is
  cheap because Grafiki ALREADY parses Claude Code JSONL transcripts for
  capture — the chat lens tails the live transcript file and renders turns
  with the parser we have (`transcript.rs`), rather than driving the agent
  through a fragile API. Codex/Gemini join when their transcripts parse;
  unknown agents gracefully show terminal-only.

Memory, surfaced inside agent chat (the thing no other client can do):
- Session start: system bubble *"Grafiki briefed Claude with 12 memories"*
  (expandable to the exact briefing).
- During work: extraction proposals appear as inline chips anchored at the
  conversational moment they came from ("+ Decision: pin CI to UTC — ✓/✕").
- Composer: "attach memory" — insert a cited fact into your prompt so the
  agent respects a past decision.

### 5.3 Session — past (detail)

- Header: date/time span, agent, cwd, duration, capture stats.
- Tabs: **Memories** (candidates from this session + their status),
  **Summary** (LLM/heuristic recap), **Raw** (redacted capture events, the
  audit trail).
- Actions: **[Resume this session]** (reopen terminal in cwd + `claude
  --continue`), delete session data (destructive, confirm).

### 5.4 Memory

- **Chat tab (default).** The existing grounded chat, elevated: persistent
  history per project, big input, citation chips that open the memory detail,
  model pill showing the auto-detected local model, scope filter tucked into a
  ⚙ popover. Abstains honestly; injection warnings kept.
- **Browse tab.** Segmented control: Decisions · Context & conventions ·
  Gotchas · Entities. List with search + scope/freshness filters. Row click →
  detail drawer: full content, status (active/superseded), provenance chain
  (evidence → "from session Jun 30" → jump to that session), Edit / Retire
  (supersede — never silent delete; bitemporal is a feature, show it).

### 5.5 Review — the inbox (Linear-grade triage)

- List rows: type icon, title, one-line content, confidence bar, source chip
  ("claude · Jun 30"), scope.
- **Keyboard-first:** `j/k` move, `a` approve, `r` reject, `e` edit,
  `space` toggle evidence preview, `x` select, `⇧A` bulk approve.
- Right preview pane: full payload + evidence excerpts with "open session".
- Filters: status, scope, min-confidence (all exist in backend).
- Empty state: *"Inbox zero. Grafiki proposes memories as you work."* + last
  extraction time + [Extract now].

### 5.6 Settings

Tabs, each one screen, no scroll-of-doom:
- **Projects:** list, add/init, per-project DB path, default project.
- **Capture & privacy:** source toggles, redaction profile
  (none/default/strict), blocked paths. Copy states what each profile means.
- **Local AI:** Ollama status, installed models (pick default), pull helper,
  extraction cadence (off / on session end / every 2 min).
- **Agent hookups:** one-click MCP config for Claude Code (`claude mcp add…`
  copy button) + Cursor JSON; zsh shell-hook copy; HTTP daemon under
  "Advanced".
- **About:** version, licenses, data location, export/import.

### 5.7 ⌘K Command palette (global)

Navigate; "Start Claude Code session"; "Resume last session"; "Ask memory: …"
(typed question routes into Chat); "Extract now"; "Approve next candidate";
"Switch project". Fuzzy, instant, Linear-style.

### 5.8 Ambient presence (Phase 3)

Menubar item: dot pulses while capturing; menu = live sessions, "needs review
(n)", quick-ask field. Optional, off-by-default notification: *"3 new memories
from your last session."* This is the Wispr-style end state: the window is
optional.

## 6. Key flows

- **A. First run → first memory (the aha, target <15 min):** Welcome → folder
  → model detect → start Claude Code → work → side peek pops "+1 learned" →
  click → approve 2 in Review → Home shows the session with its memories →
  Chat: "what did we decide today?" → cited answer. Hook set.
- **B. Daily return:** open app → Home shows yesterday's ledger → [Resume] →
  terminal revives, `claude --continue` → agent gets briefed via MCP → line in
  session view: "Briefed with 12 memories."
- **C. Recall:** ⌘K → type question → Chat answer with [1][2] chips → click →
  memory detail → provenance → open the exact session it came from.
- **D. Trust repair:** bad answer → click its citation → Edit or Retire the
  memory → re-ask → corrected, still cited. (Bitemporal supersession, visible.)
- **E. Agent-side (invisible):** hosted terminal runs `claude` → MCP briefing
  at session start → capture → extraction → Review. The human only ever sees
  Home fill up.
- **F. Recall → Act:** Memory Chat answers "why did we switch to thin LTO?"
  with citations → **[Continue this with Claude]** → opens a live session with
  the relevant memories pre-briefed and the question as the opening prompt.
  Memory stops being an archive and becomes a launchpad — the flow Cursor and
  Claude Code alone cannot do.

## 7. States, motion, visual language

> The full visual system lives in **`docs/DESIGN.md`** — tokens, typography
> (Newsreader serif display + Inter UI + JetBrains Mono), and the component
> inventory extracted from the user's Granola/Wispr/Linear screenshots
> (`Inspiration/`). Summary:

- **Visual:** "Snowy Rainforest" duotone — permanent Evergreen (#032113)
  rail/frame in BOTH themes, snow-paper sheet (#FAFAF7 light / green-charcoal
  dark), deep-pine structural accent + fresh-green live tier, ≤3 accent uses
  per screen; serif display for titles and hero moments; rows-not-cards;
  hairlines-not-shadows.
- **Motion:** existing framer springs are right; add: side-peek slide-in for
  new candidates, gentle count-up on "learned n", pulse on capture dot.
- **Every async surface:** skeleton → content, never spinner-only.
- **Every empty state:** one sentence + one CTA.
- **Every error:** what happened + the one next step (mirror the
  `ollama pull` hint pattern).

## 8. Build phases (all on the existing backend; no core rewrites)

- **P0 — Re-architecture of the shell:** Home ledger (queries exist:
  capture_sessions, candidates, descriptors), nav = Home/Sessions/Memory/
  Review/Settings, Review keyboard triage, Chat under Memory. This alone
  transforms the product.
- **P0.5 — Motion pass (user-added):** strip ALL legacy transitions/animations
  (press-lift on every button, spring-scale pane entrances, layout animations
  on nav) and replace with the calm set from DESIGN.md §7: 140–180ms ease-out
  fades with small translate for surface changes, CSS-only press states,
  slide-down banner entrances, capture pulse, count-up stats. Motion should be
  felt only at moments of *arrival* (new memory, new session), never on hover.
- **P1 — Onboarding wizard** (project init + Ollama probe/pull + first-session
  handoff) and the live-session side peek.
- **P2 — Session detail + agent chat lens** (past sessions: memories/summary/
  raw; live chat lens = transcript tail + bubble renderer + PTY composer —
  the same turn-rendering component serves both), ⌘K palette, Browse tab with
  provenance drawer.
- **P3 — Ambient + recall→act:** menubar presence, optional notifications,
  composer memory-attach, Flow F handoff, graph view.

## 9. Open decisions

1. Multi-project: switcher in rail from P0, or single-project until P2?
2. Menubar-first vs window-first long-term identity?
3. Session summaries: heuristic-only until the bundled-model story lands?
4. Naming on screen: "memories" vs "notes" vs "facts" (Granola says "notes";
   "memories" fits the thesis — recommend keeping).
