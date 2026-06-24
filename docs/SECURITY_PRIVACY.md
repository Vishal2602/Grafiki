# Security And Privacy Model

Grafiki should feel like a coding flight recorder, not surveillance.

## Defaults

- Data stays local under `~/.grafiki`.
- No telemetry by default.
- Agent memory is review-first when generated automatically.
- Workspace capture consent lives in `.grafiki.capture.json` beside the `.grafiki` marker.
- Screen capture is explicit/manual.
- Audio capture is out of scope.
- Terminal capture stores command metadata by default, not full stdout.

## Capture Sources

Default-safe launch sources:

- git status/history/diff summaries,
- agent transcript adapters,
- terminal command metadata,
- IDE/file activity metadata,
- manual screenshot capture.

Later opt-in sources:

- screen OCR,
- browser allowlist capture,
- richer IDE diagnostics.

## Terminal And File Capture

The terminal adapter stores command, cwd, exit code, duration, shell, and source scope. Full command output is intentionally off by default.

The file watcher captures path, modified time, and size metadata for recent workspace files while ignoring common generated or private directories such as `.git`, `.grafiki`, `node_modules`, `target`, `dist`, and `build`.

The git adapter captures branch, porcelain status, diff stat, changed-file names, and the latest commit subject. It is meant to describe coding state for review, not to silently trust new memory.

The workspace config can disable launch-safe sources such as `terminal`, `files`, `git`, or `transcripts`, add blocked paths/apps, and keep terminal output capture set to `off`. Screen/browser/audio remain off unless explicitly enabled.

## Redaction

Capture ingest redacts obvious secrets before writing to SQLite:

- API-key assignment patterns,
- token/password/secret assignment patterns,
- private key blocks,
- common provider token prefixes,
- JWT-looking strings.

This is a safety net, not a full replacement for dedicated scanners such as gitleaks or TruffleHog.

## Auditability

Every `grafiki ask` call writes a local audit log containing:

- agent/client,
- question,
- scope,
- returned memory ids,
- retrieval mode,
- fallback note,
- latency,
- timestamp.

The desktop Agent Activity pane exposes this log.
