# Grafiki Launch Plan

## Positioning

Grafiki is Granola for coding agents: local-first memory that captures coding work, produces reviewable memory candidates, preserves evidence, and lets Claude, Codex, Cursor, and other MCP clients ask what happened before.

## Launch Demo

1. Open an old project with no current agent context.
2. Run `grafiki init` and show imported `CLAUDE.md`, Cursor rules, Cline memory bank files, and git history as review candidates.
3. Approve one decision/context candidate.
4. Ask `grafiki ask "Why did we choose this architecture?"`.
5. Show the cited evidence and Agent Activity log.
6. Open Grafiki Desktop and show Review plus Agent Activity.

## GitHub Launch Gate

- README explains the product in under 30 seconds.
- Claude, Codex, and Cursor setup paths are documented.
- `grafiki ask` returns cited memory.
- Review candidates show evidence.
- Desktop launches with one main pane and inspector hidden until needed.
- Smoke scripts pass.

## Launch Assets

- 60-90 second video.
- README GIF: init -> review -> ask -> cited answer.
- Comparison table against `CLAUDE.md`, Cursor rules, generic MCP memory, and screen recorders.
- Short privacy statement above the fold.
