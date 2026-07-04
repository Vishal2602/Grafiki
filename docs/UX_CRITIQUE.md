# UX critique — don-norman-design-critic, 2026-07-04

*Product-wide critique run before the E2E test gate; its five hypotheses ARE the E2E
checklist. Full agent output preserved verbatim below. Companion to docs/UX_REDESIGN.md
(what we built) and docs/DESIGN.md (the visual system).*

**✅ STATUS (2026-07-04, same day): all of Top-3 and H1–H5 fixed, live-verified, and
adversarially reviewed** — see CHANGELOG.md "don-norman-design-critic UX fixes (Jul 4)".
The adversarial review pass then found and fixed three real data-integrity bugs the
undo/edit-form fixes themselves introduced (entity-cascade corruption, no concurrency
guard, a silent-data-loss edit-field mismatch) — those are documented in the CHANGELOG
too, not repeated here. One item is an explicit known limitation, not fixed: undoing an
approval that superseded an older record doesn't restore the older record's prior
status. The critique text below is the original, unedited finding — kept for the
reasoning, not as a live status.

# Grafiki UX Critique — through Norman's lenses

Read: `docs/UX_REDESIGN.md`, `docs/DESIGN.md`, `apps/grafiki-desktop/src/Onboarding.tsx`, `apps/grafiki-desktop/src/App.tsx` (HomePane ~1112–1303, TerminalPane ~1324–1770, Review pane ~2250–2744, SettingsPane), `screenshots/mcp-e2e-proof.png` (the Settings/Capture Consent screen), and `src-tauri/src/terminal.rs` capture-hint plumbing.

## Is this the right problem? (Five Whys reframe)

Yes — mostly. The UX_REDESIGN doc already did the Norman move once: it correctly demoted the terminal from identity screen to plumbing and made the ledger the home ("the notes got better on their own" is the DVR-not-VCR reframe, done well). The remaining wrong-problem risk is subtler: the design invests in making **review** pleasant (Linear-grade triage, kbd chips) when the fundamental first-week problem is making the **pipeline legible** — a user can't value the inbox until they understand *why* items appear in it and *what approving does*. The stated problem is "triage fast"; the fundamental problem is "trust what the machine learned." Several details below show the app optimizing the former while silently failing the latter.

And per Norman's fault-line relocation: every confusion below is a design defect, not a user defect. "Horrifying? Yes, but why do we label it 'human error'? It is design error."

---

## 1. Conceptual model — where the first-timer's model breaks

The real pipeline is: **session → captured events → extraction → candidate → approve → trusted memory → briefs the agent**. The app never draws this chain anywhere; the user must assemble it from vocabulary shards:

- Onboarding says "decisions and gotchas" and "shows up on Home for review" (Onboarding.tsx:78–80, 164–166). Good, human words.
- Home ledger rows then say `4 captured events` (App.tsx:1268) — plumbing units nobody asked for.
- Review calls the same things "candidates" and renders `record_type / status / 47% / date` as a raw metadata string (App.tsx:2657), with a "noisy" tag and an "Open trusted memory" action whose disabled state is unexplained until you click it and get "Approve this candidate before opening it as trusted memory" (App.tsx:2472).
- Pressing `e` — advertised as a first-class shortcut — drops the user into a **raw JSON payload textarea plus a 0–1 decimal confidence field** (App.tsx:2676–2701). This is a database record editor wearing a memory-app costume. A person who thinks they're fixing a note's wording now faces `{"title": ...}` and a parse error on failure.
- "Scope" is a free-text filter with placeholder "global or project/module" (App.tsx:2547) — pure knowledge-in-the-head. The user must already know what scope strings exist. (Your own QA history shows this exact field caused the empty-queue bug.)
- The titlebar pill "0/0 fresh" is cryptic to anyone who didn't build the freshness system.

**The model break moment:** the user approves something and *nothing tells them what changed in the world*. The loop's payoff — "this now briefs your agent" — is the entire reason review exists, and it is absent at the point of action. Norman's maxim applies exactly: "It is the designer's job to take complex requirements and make them so understandable and appropriate that they are pronounced 'simple.'" The pipeline's complexity is legitimate (it's the task's complexity); the failure is that it's hidden rather than structured.

## 2. The consent moment — honest, dark-pattern-adjacent, or neither?

Verdict: **the checkbox itself is defensible; the surrounding copy is where the honesty leaks.**

For the checkbox (Onboarding.tsx:106–116): checked-by-default is not automatically a dark pattern. The test is whether the default matches the user's evident intent and whether the state is visible and reversible. Here the user just read "Grafiki listens to your coding sessions" on step 1, the data stays local, it's per-workspace, the copy is plain, and it points to Settings → Capture Consent (which really exists). Capture *is* the product; opting in is why they downloaded it. That said, a pre-checked box the user never touches is evidence of a default, not of consent — and it's positioned as fine print under a folder picker while the user's attention is on the path field and the "Create memory here" button. Norman would say: make the choice *the content of the step*, not an accessory to it.

Three genuine honesty defects that undermine the consent story:

1. **The launcher lies.** TerminalPane's start screen: "everything in this session is captured automatically, no setup" (App.tsx:1670). This is stated unconditionally — it is false when consent is off or the folder is uninitialized, and it contradicts the consent gate just built. This one sentence is more dark-pattern-adjacent than the checkbox.
2. **False affordances in the consent panel.** The Settings consent panel shows nine source toggles — including Screen, Browser, Audio, IDE, System — that (per grafiki-core) have no implementation behind them. In a *consent* surface, non-functional toggles are worse than clutter: they inflate the perceived surveillance surface ("this thing can record my *audio*?") and they're controls that do nothing. Signifiers must signal real capabilities.
3. **Daily legibility is under-weighted.** The live capture state is a 12px muted suffix on a toolbar meta line (App.tsx:1690–1692) and a bare "capturing / not capturing" on the Home live card (App.tsx:1186) with no reason attached (the `capture_hint` is plumbed to the terminal toolbar but not to the Home card). A privacy-critical status should get feedback proportional to its importance — the fresh-green pulse dot convention from DESIGN.md is right; the *off* state is the one that's nearly invisible, and it's the one that costs the user a whole session of lost memory.

## 3. Review queue — error design is inverted

Norman's design-for-error lens: prevent, tolerate, make recovery easy. The queue does the opposite on its most dangerous path.

- **Wrong approve is the highest-blast-radius error in the product** — an approved candidate becomes trusted memory that gets injected into future agent sessions (the project's own memory notes record a false fact planted in a real project this way). Yet approve is one keystroke (`a`), instant, with no undo affordance and no statement of consequence. Recovery requires knowing to go find the memory in Browse and retire it — knowledge in the head, several screens away.
- **Meanwhile reject — the *safe* action — costs a modal with a mandatory rationale textarea** (App.tsx:2310–2328). The friction asymmetry is exactly backwards: caution is expensive and risk is cheap.
- **Wrong reject is unrecoverable from the UI**: every action button is `disabled={candidate.status !== "pending"}` (App.tsx:2661–2669), so a rejected candidate can be viewed but never resurrected.
- **Bulk approve** ("Approve Selected", "Approve Group", "Select Pending" → one click) commits dozens of machine-generated facts with no preview of contents, no confirmation, no batch undo (App.tsx:2579–2582, 2623–2625). Bulk *reject* gets a rationale modal pre-filled with "Bulk review cleanup" — again the inverted asymmetry. Norman on exactly this economics: "…you make and get errors along the way, and the cost of repairing the error more than makes up for all the savings."

What's good and should stay: kbd chips advertised inline (App.tsx:2514–2533), evidence chips with excerpts, the "All hidden below X — lower Min Confidence" honesty hint (App.tsx:2563–2567), Select Noisy as a triage accelerant, hover-focus + guarded shortcuts while modals are open. The bones of Linear-grade triage are genuinely there.

One broken window: the empty state is "No candidates in this view." (App.tsx:2739) — a dead end that violates DESIGN.md rule 5 and the UX_REDESIGN spec ("Inbox zero… + [Extract now]").

## 4. Home ledger — ledger skeleton, telemetry flesh

Structure: correct. Live card → resume banner → pending banner (with the first pending *title* quoted — the best sentence on the screen) → day-grouped rows → ask bar. That is a ledger, not a dashboard.

Content: not yet. A session row reads `claude · 4 captured events · 2 memories · 9:14` (App.tsx:1262–1275). "Captured events" is Grafiki telling you about itself; the UX_REDESIGN spec (§5.1) explicitly promised "up to two memory titles inline" — the implementation dropped the one element that delivers the "notes got better on their own" proof. Without titles, Home answers "what ran" but not "what did Grafiki learn for me today." The three stat cards are Wispr-ish decoration; count-up on every visit also violates the P0.5 rule (motion only at arrival). Keep them, but they're not the identity — the memory titles are.

Also: the serif page title is hardcoded "Today" (App.tsx:1155) even when today is empty and the newest group is "Yesterday" — a small mapping falsehood on the hero element.

---

## Top 3 issues, ranked by user impact

**1. The silent-failure first week: pipeline preconditions fail invisibly at the moment of value.**
Three independent gates (consent on, folder initialized, Ollama model present) each silently produce the same symptom: Home stays empty, "memories this week: 0", and the product appears to not work. The Ollama explanation lives only in one onboarding screen the user saw once; the not-capturing state is a 12px suffix; the launcher copy actively claims capture is automatic.
*Fix:* one truthful pipeline-status object surfaced as knowledge in the world: on Home's empty/quiet states and the session toolbar, render the chain with the first broken link named — "Capturing ✓ → Extraction paused: no local model → [Get Ollama]". Reuse the `capture_hint` pattern (terminal.rs already produces honest reasons) for extraction and init too. Delete the false sentence at App.tsx:1670 and gate it on actual state. Pass `capture_hint` through to the Home live card.

**2. Review's error asymmetry: the dangerous action is the cheapest and the least reversible.**
*Fix, in priority order:* (a) undo — approve shows a toast "Approved 'pin CI to UTC' — now briefs your agent · Undo", where undo retires/supersedes (the bitemporal backend already supports supersession; expose it here); bulk approve gets a batch summary + batch undo. (b) Allow re-opening a rejected candidate back to pending — reject becomes tolerable instead of terminal. (c) Demote the reject rationale to an optional inline field so the safe action is at least as cheap as the risky one. (d) State the consequence at the point of action once per session ("Approved memories are injected into your agent's next session").

**3. Machine internals leak into human surfaces, breaking the conceptual model.**
JSON payload editor + decimal confidence on `e`; `record_type / status / 47%` metadata strings; "captured events" counts on Home rows; free-text scope filter; "0/0 fresh" pill; unimplemented Screen/Audio/Browser toggles inside the consent panel.
*Fix:* form-based edit (Title, Content, Scope-as-dropdown-of-existing-scopes) with JSON behind an "advanced" disclosure; drop human-editable confidence entirely (a human edit *is* full confidence); replace event counts with the two inline memory titles per the §5.1 spec; hide unimplemented capture sources or mark them "coming — not active"; give the fresh pill a tooltip or a name.

## Top 5 first-week friction hypotheses (E2E checklist, testable)

1. **H1 — No-model dead end:** A user who skips Ollama at onboarding step 3 will complete 2+ sessions, see zero memories anywhere, find no in-app explanation after that screen, and conclude Grafiki is broken. *Test:* fresh install, skip step 3, run a real session; assert some persistent surface states why extraction is off and offers the fix.
2. **H2 — Consent-off session loss:** With the onboarding checkbox unchecked, a user starts a session from the launcher (whose copy says capture is automatic), works 30 minutes, and never notices the "not capturing" suffix. *Test:* eye-trackable proxy — does any element ≥14px or with accent color communicate the off state? Assert launcher copy matches actual capture state.
3. **H3 — Unrecoverable triage errors:** Approve a wrong candidate with `a`, then try to undo within 30 seconds without leaving Review — predicted fail. Reject a good candidate, then try to resurrect it — predicted hard fail (buttons disabled for non-pending). *Test:* both paths, timed.
4. **H4 — Filter-induced "empty" queue:** Typing a plausible-but-wrong scope string (or a leftover min-confidence) hides real pendings; user sees "0/12 candidates" and trusts the zero, while the rail badge says 12. *Test:* set scope to a near-miss string, assert the badge/queue disagreement is explained on-screen (the min-confidence case has its hint; scope does not).
5. **H5 — The `e` shortcut abandonment:** Ask a first-week user to fix one typo in a pending memory. Predicted: they either abandon at the JSON textarea or corrupt the payload and hit a raw parse error. *Test:* task completion rate + error copy quality on invalid JSON.

(Bonus, worth one E2E case: in the chat lens, an agent permission prompt renders only in the hidden terminal lens — App.tsx:1739–1742 warns about it in muted text, but the session *appears hung*. Assert the chat lens surfaces an active prompt or auto-flips.)

## What's working — keep it

The Granola reframe of Home, the honest `capture_hint` plumbing from the backend up, the pending-title quoted in the suggestion banner, kbd chips as first-class UI, the min-confidence "all hidden" disclosure, evidence chips, consent stored per-workspace with a real Settings section behind the onboarding promise, and the DESIGN.md rule set itself (rules 4–6 are exactly the discipline most apps lack — the Review empty state just needs to obey them).

Norman's frame: this product's moat is stated in its own principles doc — "Honest everywhere. Trust is the moat." The three top issues are all, at root, the same defect: places where the interface's story and the system's state diverge. Close those and the conceptual model teaches itself.
