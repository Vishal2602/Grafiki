# Contributing

Grafiki is building local-first memory for AI coding agents. The product focus is narrow: capture coding work, create reviewable evidence-linked memory, and let agents ask for cited context.

## Development

```bash
cargo test
scripts/smoke.sh
cd apps/grafiki-desktop && npm run build
```

For desktop verification:

```bash
scripts/smoke_desktop.sh
```

## Product Principles

- Keep capture scoped to coding work.
- Keep screen/OCR capture explicit and opt-in.
- Preserve evidence for generated memory.
- Treat automatic extraction as candidates until reviewed.
- Prefer small, agent-friendly interfaces over large tool surfaces.

## Pull Requests

- Include tests for core behavior changes.
- Update docs when CLI, HTTP, MCP, or desktop behavior changes.
- Avoid unrelated refactors in feature PRs.
- Do not add telemetry, cloud sync, or broad capture defaults without a privacy review.
