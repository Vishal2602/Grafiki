# Grafiki

> Granola for coding agents. Local-first memory that captures coding decisions, cites evidence, and lets AI coding agents ask what happened before.

Grafiki is a local-first memory layer for AI-assisted software development. It is not a generic notes app and not a broad screen recorder. It is a coding flight recorder: it stores project decisions, observations, active state, handoffs, raw coding capture events, reviewable memory candidates, and cited answers for AI agents.

The core loop is now implemented: project init, first-run import, sessions, decisions, entity graph memory, scoped search, coding-memory retrieval fixtures, candidate review with grouping/edit/bulk/noisy actions, evidence-linked capture summaries, workspace capture consent config, Codex/Claude Code/Cursor transcript import, terminal command capture, workspace file-change snapshots, git working-tree summaries, `grafiki ask`, agent query audit logs, raw capture sessions/events, HTTP, MCP, Tauri desktop, reports, analysis, export, and JSON import.

## Why Grafiki Exists

AI coding agents lose context. They rediscover old decisions, repeat rejected approaches, and ask the user to explain the same project history again. Grafiki gives Claude, Codex, Cursor, and other MCP clients one shared local memory they can query before touching code.

The killer workflow:

```bash
grafiki init grafiki --path .
grafiki ask "What should I know before changing the desktop UI?" --scope grafiki/desktop
grafiki agent-activity --scope grafiki/desktop
```

Grafiki answers with trusted project memory and evidence links back to capture events, imported memory files, git snapshots, or reviewed candidates.

## Privacy Posture

- Local-first SQLite database under `~/.grafiki`.
- No telemetry by default.
- Workspace capture settings live in `.grafiki.capture.json`.
- Screen capture is explicit/manual in the desktop app.
- Capture ingest redacts obvious secrets before persistence.
- Automatically extracted knowledge is proposed as candidates first; the user approves or rejects it.
- Agent questions are logged locally so you can see what the agent asked and what memory it received.

## Install

See [docs/INSTALL.md](docs/INSTALL.md) for the full guide. The short version:

```bash
brew tap <owner>/grafiki
brew install grafiki            # the `grafiki` CLI + MCP server
brew install --cask grafiki     # optional desktop app (macOS)
```

Released binaries include real semantic search (`fastembed` + `sqlite-vec`).

## Quickstart

```bash
cargo build -p grafiki-cli
target/debug/grafiki init grafiki --path .
target/debug/grafiki ask "What should I know before editing retrieval?" --scope grafiki/core
target/debug/grafiki mcp --project grafiki --path .
```

For the desktop app:

```bash
INSTALL_TO_APPLICATIONS=1 scripts/smoke_desktop.sh
open /Applications/Grafiki.app
```

## Comparison

| Capability | Grafiki | CLAUDE.md / Cursor rules | Generic MCP memory | Screen recorders |
|---|---:|---:|---:|---:|
| Local-first | yes | yes | mixed | mixed |
| Agent-facing MCP | yes | no | yes | mixed |
| Coding-aware capture | yes | manual | no | broad/noisy |
| Reviewable candidates | yes | no | rarely | no |
| Evidence-linked answers | yes | no | rarely | rarely |
| Agent query audit log | yes | no | rarely | no |
| Screen capture default | off/manual | n/a | n/a | often on |

## Development

```bash
cargo test
scripts/smoke.sh
cargo run -p grafiki-cli -- init grafiki --path .
cargo run -p grafiki-cli -- start --type codex --goal "Implement the next Phase 1 command" --scope grafiki/core
cargo run -p grafiki-cli -- ask "What should I know before changing the desktop UI?" --scope grafiki/desktop
cargo run -p grafiki-cli -- agent-activity --scope grafiki/desktop
cargo run -p grafiki-cli -- capture config show
cargo run -p grafiki-cli -- capture config set --terminal true --files true --git true --terminal-output off
cargo run -p grafiki-cli -- auto-capture --scope grafiki/core --source codex-session
cargo run -p grafiki-cli -- capture start --scope grafiki/core --source-app codex
cargo run -p grafiki-cli -- capture ingest --scope grafiki/core --type transcript --source codex --title "Agent event" --text "Agent captured a coding transcript event."
cargo run -p grafiki-cli -- capture import-transcripts --agent codex --scope grafiki/core --summarize
cargo run -p grafiki-cli -- capture terminal-command --scope grafiki/core --cmd "cargo test" --cwd . --exit-code 0 --duration-ms 1200 --shell zsh
cargo run -p grafiki-cli -- capture watch-files --scope grafiki/core --since-seconds 300 --summarize
cargo run -p grafiki-cli -- capture git-summary --scope grafiki/core --summarize
cargo run -p grafiki-cli -- capture shell-hook --scope grafiki/core
cargo run -p grafiki-cli -- capture summarize --scope grafiki/core
cargo run -p grafiki-cli -- decide "Use SQLite WAL" --reasoning "Keep local reads responsive during writes" --scope grafiki/core
cargo run -p grafiki-cli -- save "Auth Service" --type service --observe "JWT refresh uses rotating tokens" --category architecture --scope grafiki/core
cargo run -p grafiki-cli -- save "Database" --type service --scope grafiki/core
cargo run -p grafiki-cli -- save "Auth Service" --type service --scope grafiki/core --relate database:depends_on
cargo run -p grafiki-cli -- search rotating --scope grafiki/core
cargo run -p grafiki-cli -- search "token refresh design" --scope grafiki/core --mode hybrid --format json
cargo run -p grafiki-cli -- embeddings rebuild --scope grafiki/core
GRAFIKI_EMBEDDING_PROVIDER=fastembed cargo run -p grafiki-cli --features fastembed -- embeddings rebuild --scope grafiki/core
cargo run -p grafiki-cli -- graph auth-service --depth 1 --format dot
cargo run -p grafiki-cli -- report --scope grafiki/core --format md
cargo run -p grafiki-cli -- report --scope grafiki/core --format md --output grafiki-report.md
cargo run -p grafiki-cli -- analyze --scope grafiki/core --output grafiki-analysis.md
cargo run -p grafiki-cli -- export --scope grafiki/core --format json --output grafiki-export.json
cargo run -p grafiki-cli -- export --scope grafiki/core --format md --output grafiki-export.md
cargo run -p grafiki-cli -- export --scope grafiki/core --format dot --output grafiki-export.dot
cargo run -p grafiki-cli -- export --scope grafiki/core --format graphml --output grafiki-export.graphml
cargo run -p grafiki-cli -- export --scope grafiki/core --format html --output grafiki-export.html
cargo run -p grafiki-cli -- export --scope grafiki/core --format wiki --output grafiki-wiki
cargo run -p grafiki-cli -- import grafiki-export.json
cargo run -p grafiki-cli -- embeddings status --scope grafiki/core
cargo run -p grafiki-cli -- serve --project grafiki --path . --port 9700
cargo run -p grafiki-cli -- daemon start --project grafiki --path . --port 9700 --token local-dev-token
cargo run -p grafiki-cli -- daemon status --project grafiki --path .
cargo run -p grafiki-cli -- daemon stop --project grafiki --path .
cargo run -p grafiki-cli -- mcp --project grafiki --path .
cargo run -p grafiki-cli -- state set memory-loop --title "Build memory loop" --status in-progress --priority high --scope grafiki/core
cargo run -p grafiki-cli -- state list --scope grafiki/core
cargo run -p grafiki-cli -- status --scope grafiki/core
cargo run -p grafiki-cli -- events --scope grafiki/core --last 10
cargo run -p grafiki-cli -- log --scope grafiki/core --last 10
cargo run -p grafiki-cli -- handoff --format md
cargo run -p grafiki-cli -- end --summary "Finished current task" --accomplishments "implemented command,tested command"
cargo run -p grafiki-cli -- scope-chain open-insurance/backend/enrichment
```

## HTTP API

`grafiki serve` binds to `127.0.0.1:9700` by default and exposes a small local API. Binding to a non-local interface requires `--allow-non-local` plus `--token` or `GRAFIKI_HTTP_TOKEN`.
Use `grafiki daemon start/status/stop` to manage the same HTTP API as a background process.
When a token is configured, pass it as `Authorization: Bearer <token>`, `X-Grafiki-Token: <token>`, or a `token` query parameter.

## Embeddings

Grafiki defaults to deterministic local embeddings so the core workflow is fast, offline, and testable. For real local semantic embeddings, build with the `fastembed` feature and set `GRAFIKI_EMBEDDING_PROVIDER=fastembed`. Add the `sqlite-vec` feature when you want the embedding worker and semantic search to maintain a local vector index. The first run may download the MiniLM model and ONNX Runtime assets through fastembed.

```bash
GRAFIKI_EMBEDDING_PROVIDER=fastembed cargo run -p grafiki-cli --features fastembed -- embeddings rebuild --scope grafiki/core
GRAFIKI_EMBEDDING_PROVIDER=fastembed cargo run -p grafiki-cli --features "fastembed sqlite-vec" -- search "token refresh design" --scope grafiki/core --mode semantic
```

For an optional end-to-end check of the real local embedding stack, run `scripts/smoke_fastembed.sh`. It may download model/runtime assets on first use, so it is kept separate from the default smoke script.

## Desktop App

The Tauri desktop app lives in `apps/grafiki-desktop`. It is a Macro-inspired memory console with a left rail, top status strip, inspector, command palette, launcher, URL-synced multi-pane layout, native project folder picker, scoped search with mode/type/scope filters and embedding freshness controls, session controls with real history, direct handoff/completion actions, handoff review, editable session records, native local-daemon controls, Capture Consent settings for source toggles/blocked paths, real memory capture, transcript import from Codex/Claude Code/Cursor histories, a Memory Review pane grouped by source/day with keyboard focus flow, evidence previews, noisy-candidate selection, and candidate edit/approve/reject/bulk promotion into trusted memory, a focused relations ledger, real decisions/context/state list panes, inline maintenance for context/state, detail-view editing/deletion for decisions/entities/observations/relations/context/state plus session editing, and detail/provenance panes.

```bash
cd apps/grafiki-desktop
npm install
npm run dev
npm run build
npm run tauri -- dev
npm run tauri:build:debug
npm run tauri:build:release
```

The web dev server runs on `http://127.0.0.1:1420`. The Tauri shell exposes local commands that read and write through `grafiki-core` for project status, embedding status, reports, search, graph traversal, memory detail loading, project init, sessions, decisions, observations, entities, state, context, relations, handoffs, JSON import/export, embedding maintenance, and local HTTP daemon start/status/stop. The app bundle includes the `grafiki` CLI as a sidecar so daemon controls can work from the installed app instead of depending on the development workspace.

Debug desktop bundles are produced at:

```text
target/debug/bundle/macos/Grafiki.app
target/debug/bundle/dmg/Grafiki_0.1.0_aarch64.dmg
```

The macOS bundle includes the custom Grafiki icon generated from `apps/grafiki-desktop/src-tauri/icons/icon.png`. If the app icon changes, regenerate the platform icon set with:

```bash
cd apps/grafiki-desktop
npm run tauri -- icon /path/to/source-icon.png
npm run tauri -- build --debug
```

For a repeatable debug release build with DMG verification:

```bash
scripts/build_desktop_debug.sh
INSTALL_TO_APPLICATIONS=1 scripts/build_desktop_debug.sh
```

For a repeatable desktop smoke check that runs core tests, rebuilds the bundle, launches the app, and verifies daemon-status wiring:

```bash
scripts/smoke_desktop.sh
INSTALL_TO_APPLICATIONS=1 scripts/smoke_desktop.sh
```

```bash
curl http://127.0.0.1:9700/health
curl "http://127.0.0.1:9700/api/status?scope=grafiki/core"
curl "http://127.0.0.1:9700/api/ask?q=What%20should%20I%20know%20before%20changing%20the%20desktop%20UI%3F&scope=grafiki/desktop"
curl "http://127.0.0.1:9700/api/search?q=rotating&scope=grafiki/core&mode=keyword"
curl "http://127.0.0.1:9700/api/embeddings/status?scope=grafiki/core"
curl -X POST http://127.0.0.1:9700/api/embeddings/rebuild -H 'Content-Type: application/json' -d '{"scope":"grafiki/core","limit":100}'
curl "http://127.0.0.1:9700/api/search?q=rotating&scope=grafiki/core&mode=semantic"
curl "http://127.0.0.1:9700/api/report?scope=grafiki/core&format=md"
curl "http://127.0.0.1:9700/api/export?scope=grafiki/core&format=json"
curl "http://127.0.0.1:9700/api/graph/auth-service?depth=1"
curl "http://127.0.0.1:9700/api/events?scope=grafiki/core&limit=10"
curl "http://127.0.0.1:9700/api/log?scope=grafiki/core&limit=10"
curl "http://127.0.0.1:9700/api/context?scope=grafiki/core"
curl "http://127.0.0.1:9700/api/context/phase1-prd"
curl "http://127.0.0.1:9700/api/memory/context/phase1-prd?scope=grafiki/core"
curl -X POST http://127.0.0.1:9700/api/sessions/start -H 'Content-Type: application/json' -d '{"type":"codex","goal":"Continue implementation","scope":"grafiki/core"}'
curl -X POST http://127.0.0.1:9700/api/sessions/handoff -H 'Content-Type: application/json' -d '{}'
curl -X POST http://127.0.0.1:9700/api/entities/save -H 'Content-Type: application/json' -d '{"name":"Auth Service","entity_type":"service","observe":"JWT refresh uses rotating tokens","category":"architecture","scope":"grafiki/core"}'
curl -X POST http://127.0.0.1:9700/api/decisions -H 'Content-Type: application/json' -d '{"title":"Use SQLite WAL","reasoning":"Keep local reads responsive","scope":"grafiki/core"}'
curl -X POST http://127.0.0.1:9700/api/state -H 'Content-Type: application/json' -d '{"key":"memory-loop","title":"Build memory loop","status":"in-progress","priority":"high","scope":"grafiki/core"}'
curl -X POST http://127.0.0.1:9700/api/context/add -H 'Content-Type: application/json' -d '{"key":"phase1-prd","title":"Phase 1 PRD","category":"spec","scope":"grafiki/core","content":"Core memory loop requirements."}'
curl -X POST http://127.0.0.1:9700/api/memory/update -H 'Content-Type: application/json' -d '{"type":"context","id":"phase1-prd","content":"Updated core memory loop requirements."}'
curl -X POST http://127.0.0.1:9700/api/memory/delete -H 'Content-Type: application/json' -d '{"type":"state","id":"memory-loop"}'
curl -X POST http://127.0.0.1:9700/api/candidates/propose -H 'Content-Type: application/json' -d '{"type":"decision","source_type":"connector:github","source":"issue-42","scope":"grafiki/core","confidence":0.8,"payload":{"title":"Review connector output","reasoning":"Imported data stays untrusted until approval."}}'
curl "http://127.0.0.1:9700/api/candidates?status=pending&scope=grafiki/core"
curl -X POST http://127.0.0.1:9700/api/candidates/edit -H 'Content-Type: application/json' -d '{"id":"<candidate-id>","confidence":0.9,"payload":{"title":"Edited reviewed decision","reasoning":"Human review corrected the extraction."}}'
curl -X POST http://127.0.0.1:9700/api/candidates/bulk -H 'Content-Type: application/json' -d '{"action":"reject","ids":["<candidate-id>"],"rationale":"Noisy extraction."}'
curl -X POST http://127.0.0.1:9700/api/candidates/approve -H 'Content-Type: application/json' -d '{"id":"<candidate-id>"}'
curl -X POST http://127.0.0.1:9700/api/capture/auto -H 'Content-Type: application/json' -d '{"scope":"grafiki/core","source":"codex-session"}'
curl "http://127.0.0.1:9700/api/capture/config"
curl -X POST http://127.0.0.1:9700/api/capture/config -H 'Content-Type: application/json' -d '{"terminal":true,"files":true,"git":true,"add_blocked_paths":["secrets"],"terminal_output":"off"}'
curl -X POST http://127.0.0.1:9700/api/capture/import-transcripts -H 'Content-Type: application/json' -d '{"scope":"grafiki/core","agent":"codex","input":"/path/to/session.jsonl","summarize":true}'
curl -X POST http://127.0.0.1:9700/api/capture/terminal-command -H 'Content-Type: application/json' -d '{"scope":"grafiki/core","command":"cargo test","cwd":"/path/to/project","exit_code":0,"duration_ms":1200,"shell":"zsh"}'
curl -X POST http://127.0.0.1:9700/api/capture/watch-files -H 'Content-Type: application/json' -d '{"scope":"grafiki/core","since_seconds":300,"limit":50,"summarize":true}'
curl -X POST http://127.0.0.1:9700/api/capture/git-summary -H 'Content-Type: application/json' -d '{"scope":"grafiki/core","summarize":true}'
curl -X POST http://127.0.0.1:9700/api/context/update -H 'Content-Type: application/json' -d '{"key":"phase1-prd","content":"Updated core memory loop requirements."}'
curl -X POST http://127.0.0.1:9700/api/context/delete -H 'Content-Type: application/json' -d '{"key":"phase1-prd"}'
curl -X POST http://127.0.0.1:9700/api/import -H 'Content-Type: application/json' --data-binary @grafiki-export.json
curl -X POST http://127.0.0.1:9700/api/sessions/end -H 'Content-Type: application/json' -d '{"summary":"Finished current task","status":"completed"}'
```

## MCP

`grafiki mcp` runs a stdio JSON-RPC server for MCP-compatible clients. The first tool surface includes:

```text
grafiki_start
grafiki_end
grafiki_handoff
grafiki_status
grafiki_ask
grafiki_agent_activity
grafiki_auto_capture
grafiki_capture_start
grafiki_capture_stop
grafiki_capture_ingest
grafiki_capture_import_transcripts
grafiki_capture_config
grafiki_capture_config_set
grafiki_capture_terminal_command
grafiki_capture_watch_files
grafiki_capture_git_summary
grafiki_capture_status
grafiki_capture_events
grafiki_capture_summarize
grafiki_search
grafiki_candidate_propose
grafiki_candidate_list
grafiki_candidate_edit
grafiki_candidate_bulk
grafiki_candidate_approve
grafiki_candidate_reject
grafiki_record
grafiki_update_record
grafiki_delete_record
grafiki_save
grafiki_decide
grafiki_state_set
grafiki_embeddings_status
grafiki_embeddings_process
grafiki_report
grafiki_graph
grafiki_export
```

## Documents

- [PRD.md](PRD.md) - final product requirements and phased plan
- [PROJECT_STATUS.md](PROJECT_STATUS.md) - current progress and remaining work
- [docs/CLIENT_SETUP.md](docs/CLIENT_SETUP.md) - MCP and HTTP client setup examples
- [docs/DESKTOP_APP_PLAN.md](docs/DESKTOP_APP_PLAN.md) - Tauri desktop app direction and first shell scope
- [docs/PRODUCTION_RELEASE.md](docs/PRODUCTION_RELEASE.md) - local build, release build, signing, and notarization notes
- [docs/SEMANTIC_SEARCH_PLAN.md](docs/SEMANTIC_SEARCH_PLAN.md) - Phase 3 semantic search implementation plan
- [docs/LAUNCH_PLAN.md](docs/LAUNCH_PLAN.md) - GitHub launch checklist and demo shape
- [docs/SECURITY_PRIVACY.md](docs/SECURITY_PRIVACY.md) - local-first capture, redaction, and audit posture
- [docs/DEMO_SCRIPT.md](docs/DEMO_SCRIPT.md) - launch demo scenarios
- [docs/ROADMAP.md](docs/ROADMAP.md) - now/next/later roadmap
- [Grafiki_Spec.md](Grafiki_Spec.md) - original complete technical specification
