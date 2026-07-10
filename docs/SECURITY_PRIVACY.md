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

The terminal adapter stores command, cwd, exit code, duration, shell, and source scope. Output capture is explicitly selected per workspace:

- `off` stores no output or resumable output tail;
- `digest` stores only byte and line counts in capture events — no output content. (The
  separate cross-relaunch resume tail is still persisted, redacted, as in `full`;
  it lives in the terminal descriptor, not in capture events.);
- `full` stores redacted output for users who deliberately enable it.

Resumable terminal descriptors are owner-only (`0700` directory, `0600` file). Any persisted tail is consent-gated and redacted before it reaches disk.

The file watcher captures path, modified time, and size metadata for recent workspace files while ignoring common generated or private directories such as `.git`, `.grafiki`, `node_modules`, `target`, `dist`, and `build`.

The git adapter captures branch, porcelain status, diff stat, changed-file names, and the latest commit subject. It is meant to describe coding state for review, not to silently trust new memory.

The workspace config can disable launch-safe sources such as `terminal`, `files`, `git`, or `transcripts`, add blocked paths/apps, and keep terminal output capture set to `off`. The passive transcript watcher re-reads this policy on every pass and fails closed when consent is absent or revoked. Screen/browser/audio remain off unless explicitly enabled.

## Redaction

Capture ingest redacts obvious secrets before writing to SQLite:

- API-key assignment patterns,
- token/password/secret assignment patterns,
- private key blocks,
- common provider token prefixes,
- JWT-looking strings,
- bare high-entropy tokens (≥24 mixed alphanumeric/base64 chars) on lines that name a
  secret-ish word ("key", "secret", "token", …) — catches prose like "the signing key
  is a3f9c2e8…" that has no prefix and no `key=value` shape.

Known limit: a bare secret on a line with NO secret-naming word nearby still passes
(there is no reliable content-free way to tell it from a build hash). Treat redaction
as recall-biased defense in depth, not a guarantee.

Structured payloads and metadata are recursively redacted before serialization. Secret-named keys such as `password` and `client_secret` redact nested scalar values even when the value has no recognizable token prefix. A redaction marks the event as redacted and escalates ordinary privacy levels to `sensitive`.

This is a safety net, not a full replacement for dedicated scanners such as gitleaks or TruffleHog.

## Local Interfaces

- MCP is read-only by default. Mutation/curation tools require the explicit `--allow-write` capability.
- Non-local HTTP binds require a token, authenticate before allocating request bodies, cap concurrent connections and request metadata, and use typed client errors.
- A daemon is immutably bound to one project/root. Callers cannot switch projects through query or JSON `project/path` parameters.
- HTTP transcript imports canonicalize their input and reject path traversal, symlink escapes, and files outside the daemon project root.
- Daemon stop verifies the recorded executable, project, instance identity, and health response before signaling a PID.

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
