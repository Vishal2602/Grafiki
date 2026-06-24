# Grafiki Client Setup

This guide covers the current local alpha setup for MCP clients and HTTP-based tooling.

## Build The Binary

From the repository root:

```bash
cargo build -p grafiki-cli
```

The development binary is:

```text
target/debug/grafiki
```

For examples below, replace `/path/to/Grafiki` with the absolute path to this repository or your project repository.

## Initialize A Project

```bash
target/debug/grafiki init grafiki --path /path/to/Grafiki
```

Grafiki stores project data in `~/.grafiki/<project>.db` by default. For isolated tests, set `GRAFIKI_HOME`.

## MCP Client Configuration

Use this command for MCP clients that accept a command plus arguments:

```bash
/path/to/Grafiki/target/debug/grafiki mcp --project grafiki --path /path/to/Grafiki
```

Generic MCP JSON shape:

```json
{
  "mcpServers": {
    "grafiki": {
      "command": "/path/to/Grafiki/target/debug/grafiki",
      "args": ["mcp", "--project", "grafiki", "--path", "/path/to/Grafiki"]
    }
  }
}
```

Available MCP tools:

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

Useful first prompt for an agent:

```text
Use grafiki_start with scope "grafiki/core" and goal "<current task>". Then call grafiki_ask with the current task before broad repository exploration, passing your agent name as "agent" when possible. Grafiki logs the question and returned memory locally for audit. Check grafiki_capture_config before automatic capture if the workspace is unfamiliar. Start grafiki_capture_start, stream transcript/IDE/screen/file events with grafiki_capture_ingest as work happens, call grafiki_capture_import_transcripts for Codex, Claude Code, Cursor, or generic transcript files, and call grafiki_capture_terminal_command, grafiki_capture_watch_files, or grafiki_capture_git_summary when those facts are available. Then call grafiki_capture_summarize near the end so the user can review pending memory candidates with evidence links. Save decisions or durable observations as you work, and use grafiki_candidate_propose when a memory should be reviewed by the user first.
```

At the end of a coding turn, agents can call `grafiki_auto_capture` to inspect the git working tree and place a concise session snapshot into the Memory Review queue. That keeps automatic capture reviewable instead of silently trusting inferred memory.

For fuller automatic capture, agents should keep a capture session open and ingest raw events by type:

```text
transcript: user/agent messages or assistant summaries
ide: active file, cursor, diagnostics, edits, symbols
screen: OCR/window/screenshot metadata from the desktop app
file/git: file writes, diff chunks, branch/status events
terminal: commands, exit status, important output
```

Transcript import can read a specific file or the default local history folder for the agent:

```bash
target/debug/grafiki capture import-transcripts --agent codex --path /path/to/Grafiki --scope grafiki/core --summarize
target/debug/grafiki capture import-transcripts --agent claude-code --input /path/to/transcript.jsonl --path /path/to/Grafiki --scope grafiki/core --summarize
```

Terminal, file, and git adapters capture coding metadata into the same raw ledger:

```bash
target/debug/grafiki capture config show --path /path/to/Grafiki
target/debug/grafiki capture config set --path /path/to/Grafiki --terminal true --files true --git true --add-blocked-path secrets --terminal-output off
target/debug/grafiki capture terminal-command --path /path/to/Grafiki --scope grafiki/core --cmd "cargo test" --cwd /path/to/Grafiki --exit-code 0 --duration-ms 1200 --shell zsh
target/debug/grafiki capture watch-files --path /path/to/Grafiki --scope grafiki/core --since-seconds 300 --limit 50 --summarize
target/debug/grafiki capture git-summary --path /path/to/Grafiki --scope grafiki/core --summarize
target/debug/grafiki capture shell-hook --path /path/to/Grafiki --scope grafiki/core
```

The shell hook prints a zsh hook you can source manually. It captures command, cwd, exit code, duration, and shell metadata. It does not capture stdout by default.

Workspace capture consent is stored at `.grafiki.capture.json` in the project root. Launch-safe defaults keep git, transcript, terminal, file, IDE, and system metadata enabled; screen/browser/audio capture stay off unless the user opts in.

## HTTP Daemon

Foreground server:

```bash
target/debug/grafiki serve --project grafiki --path /path/to/Grafiki --port 9700
```

Background daemon:

```bash
target/debug/grafiki daemon start --project grafiki --path /path/to/Grafiki --port 9700 --token local-dev-token
target/debug/grafiki daemon status --project grafiki --path /path/to/Grafiki
target/debug/grafiki daemon stop --project grafiki --path /path/to/Grafiki
```

Grafiki binds to `127.0.0.1` by default. Non-local binds require `--allow-non-local` plus `--token` or `GRAFIKI_HTTP_TOKEN`.

When a token is configured, pass one of:

```bash
curl -H "Authorization: Bearer local-dev-token" http://127.0.0.1:9700/api/status
curl -H "X-Grafiki-Token: local-dev-token" http://127.0.0.1:9700/api/status
curl "http://127.0.0.1:9700/api/status?token=local-dev-token"
```

## HTTP Quick Checks

```bash
curl http://127.0.0.1:9700/health
curl -H "Authorization: Bearer local-dev-token" "http://127.0.0.1:9700/api/status?scope=grafiki/core"
curl -H "Authorization: Bearer local-dev-token" "http://127.0.0.1:9700/api/ask?q=What%20should%20I%20know%20before%20editing%20retrieval%3F&scope=grafiki/core"
curl -H "Authorization: Bearer local-dev-token" "http://127.0.0.1:9700/api/agent-queries?scope=grafiki/core"
curl -H "Authorization: Bearer local-dev-token" "http://127.0.0.1:9700/api/search?q=sqlite&scope=grafiki/core"
curl -H "Authorization: Bearer local-dev-token" "http://127.0.0.1:9700/api/memory/context/phase1-prd?scope=grafiki/core"
curl -X POST -H "Authorization: Bearer local-dev-token" -H 'Content-Type: application/json' "http://127.0.0.1:9700/api/memory/update" -d '{"type":"context","id":"phase1-prd","content":"Updated trusted context."}'
curl -X POST -H "Authorization: Bearer local-dev-token" -H 'Content-Type: application/json' "http://127.0.0.1:9700/api/memory/delete" -d '{"type":"state","id":"obsolete-work"}'
curl -X POST -H "Authorization: Bearer local-dev-token" -H 'Content-Type: application/json' "http://127.0.0.1:9700/api/candidates/propose" -d '{"type":"decision","source_type":"connector:github","source":"issue-42","scope":"grafiki/core","confidence":0.8,"payload":{"title":"Review connector output","reasoning":"Imported data stays untrusted until approval."}}'
curl -H "Authorization: Bearer local-dev-token" "http://127.0.0.1:9700/api/candidates?status=pending&scope=grafiki/core"
curl -X POST -H "Authorization: Bearer local-dev-token" -H 'Content-Type: application/json' "http://127.0.0.1:9700/api/candidates/edit" -d '{"id":"<candidate-id>","confidence":0.9,"payload":{"title":"Edited reviewed decision","reasoning":"Human review corrected the extraction."}}'
curl -X POST -H "Authorization: Bearer local-dev-token" -H 'Content-Type: application/json' "http://127.0.0.1:9700/api/candidates/bulk" -d '{"action":"reject","ids":["<candidate-id>"],"rationale":"Noisy extraction."}'
curl -X POST -H "Authorization: Bearer local-dev-token" -H 'Content-Type: application/json' "http://127.0.0.1:9700/api/candidates/approve" -d '{"id":"<candidate-id>"}'
curl -H "Authorization: Bearer local-dev-token" "http://127.0.0.1:9700/api/capture/config"
curl -X POST -H "Authorization: Bearer local-dev-token" -H 'Content-Type: application/json' "http://127.0.0.1:9700/api/capture/config" -d '{"terminal":true,"files":true,"git":true,"add_blocked_paths":["secrets"],"terminal_output":"off"}'
curl -X POST -H "Authorization: Bearer local-dev-token" -H 'Content-Type: application/json' "http://127.0.0.1:9700/api/capture/import-transcripts" -d '{"agent":"codex","input":"/path/to/session.jsonl","scope":"grafiki/core","summarize":true}'
curl -X POST -H "Authorization: Bearer local-dev-token" -H 'Content-Type: application/json' "http://127.0.0.1:9700/api/capture/terminal-command" -d '{"scope":"grafiki/core","command":"cargo test","cwd":"/path/to/Grafiki","exit_code":0,"duration_ms":1200,"shell":"zsh"}'
curl -X POST -H "Authorization: Bearer local-dev-token" -H 'Content-Type: application/json' "http://127.0.0.1:9700/api/capture/watch-files" -d '{"scope":"grafiki/core","since_seconds":300,"limit":50,"summarize":true}'
curl -X POST -H "Authorization: Bearer local-dev-token" -H 'Content-Type: application/json' "http://127.0.0.1:9700/api/capture/git-summary" -d '{"scope":"grafiki/core","summarize":true}'
curl -H "Authorization: Bearer local-dev-token" "http://127.0.0.1:9700/api/report?scope=grafiki/core&format=md"
curl -X POST -H "Authorization: Bearer local-dev-token" -H 'Content-Type: application/json' "http://127.0.0.1:9700/api/sessions/handoff" -d '{}'
```

## Verification

Run the full local check:

```bash
scripts/smoke.sh
```

This covers the CLI memory loop, export/import, transcript/terminal/file/git capture adapters, HTTP API, daemon lifecycle, token checks, and MCP tool calls.
