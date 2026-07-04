# Roadmap

## Now

- **GATE (do first): full end-to-end test of the current app** — user-driven pass over the
  whole product (onboarding → sessions/terminal → capture → extraction → Review →
  Memory/chat → Settings/theme → tray), backed by the three-layer test stack
  (`docs/TESTING.md`); log every friction point and bug before any new feature work.
  **Checklist seed: the five ranked friction hypotheses (H1–H5 + chat-lens bonus) in
  `docs/UX_CRITIQUE.md`** (2026-07-04 Norman-critic review: silent pipeline failures,
  Review's inverted error asymmetry, machine internals leaking into human surfaces).
  **✅ H1–H5 + Top-3 all fixed same day** (CHANGELOG.md "don-norman-design-critic UX
  fixes (Jul 4)"), adversarially reviewed, and three bugs the fix itself introduced
  (entity-cascade corruption, revert race, silent edit-form data loss) also fixed.
  The gate itself — a full user-driven E2E pass — is still open; these were
  critique-sourced fixes, not a completed test run.
- **Code graph + injection + honest token benchmark — scoped and PARKED behind the gate
  above** (`docs/CODE_GRAPH_PLAN.md`): Tier 1 = `grafiki map` budgeted digests,
  session-start injection, tree-sitter TS/Py, `grafiki benchmark` on real transcripts.
  Four open decisions listed in the plan doc await user confirmation; do not start until
  the E2E gate passes and the user green-lights.
- Evidence-linked memory.
- Agent query audit logs.
- Init imports for existing agent memory files.
- Review queue grouping, evidence preview, keyboard flow, and noisy-candidate actions.
- Claude, Codex, and Cursor setup docs.
- Codex, Claude Code, and Cursor transcript import.
- Workspace capture consent config.
- Terminal command metadata capture and zsh hook generation.
- Workspace file-change snapshot capture.
- Git working-tree summary capture.
- Coding-specific retrieval evals for rejected approaches, handoffs, gotchas, and active constraints.
- Desktop controls for capture source consent and blocked paths.

## Next

- IDE-native file event adapters.
- Deeper terminal hook install/uninstall UX.
- Richer git event summarizer with commit/diff chunk evidence.
- Richer transcript adapter fixtures and edge cases.
- Larger real-world retrieval eval corpus.
- First-run desktop onboarding and agent setup UX.

## Later

- SQLCipher/keychain encryption.
- VS Code extension.
- JetBrains plugin.
- Optional screen OCR with app allowlists.
- Encrypted export bundles.
- Team sync.
