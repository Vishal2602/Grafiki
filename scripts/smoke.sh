#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
HOME_DIR="$TMP/home"
SOURCE_DIR="$TMP/source"
TARGET_DIR="$TMP/target"
HTTP_DIR="$TMP/http"
DAEMON_DIR="$TMP/daemon"
PORT="${GRAFIKI_SMOKE_PORT:-19710}"
DAEMON_PORT="${GRAFIKI_SMOKE_DAEMON_PORT:-19711}"

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

mkdir -p "$HOME_DIR" "$SOURCE_DIR" "$TARGET_DIR" "$HTTP_DIR" "$DAEMON_DIR"
cd "$ROOT"

echo "== cargo test =="
cargo test

echo "== cli export/import =="
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- init source --path "$SOURCE_DIR" --format json > "$TMP/source-init.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- init target --path "$TARGET_DIR" --format json > "$TMP/target-init.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- capture config show --project source --path "$SOURCE_DIR" --format json > "$TMP/source-capture-config.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- capture config set --project source --path "$SOURCE_DIR" --terminal true --files true --git true --add-blocked-path secrets --remove-blocked-path target --terminal-output off --format json > "$TMP/source-capture-config-set.json"
git -C "$SOURCE_DIR" init -q
printf 'Auth Service rotates refresh tokens in this smoke repo.\n' > "$SOURCE_DIR/auth.md"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- start --project source --path "$SOURCE_DIR" --type codex --goal "Smoke CLI" --scope source/core --format json > "$TMP/source-start.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- decide "Use smoke tests" --project source --path "$SOURCE_DIR" --reasoning "Repeatable checks protect the project" --scope source/core --format json > "$TMP/source-decision.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- save "Database" --project source --path "$SOURCE_DIR" --type service --scope source/core --format json > "$TMP/source-database.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- save "Auth Service" --project source --path "$SOURCE_DIR" --type service --observe "JWT refresh uses rotating tokens" --category architecture --relate database:depends_on --scope source/core --format json > "$TMP/source-auth.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- candidates propose --project source --path "$SOURCE_DIR" --type context --source-type "connector:smoke" --source "ticket-42" --scope source/core --confidence 0.9 --payload '{"key":"candidate-cli","title":"Candidate CLI","category":"reference","content":"CLI candidate review keeps extracted memory untrusted until approval."}' --format json > "$TMP/source-candidate.json"
SOURCE_CANDIDATE_ID="$(node -e "const fs=require('fs'); console.log(JSON.parse(fs.readFileSync(process.argv[1], 'utf8')).candidate.id)" "$TMP/source-candidate.json")"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- candidates edit "$SOURCE_CANDIDATE_ID" --project source --path "$SOURCE_DIR" --confidence 0.95 --payload '{"key":"candidate-cli","title":"Candidate CLI Edited","category":"reference","content":"CLI candidate review supports editing before approval."}' --format json > "$TMP/source-candidate-edit.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- candidates approve "$SOURCE_CANDIDATE_ID" --project source --path "$SOURCE_DIR" --format json > "$TMP/source-candidate-approve.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- candidates list --project source --path "$SOURCE_DIR" --status approved --scope source/core --format json > "$TMP/source-candidates-approved.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- ask "What should coding agents know about rotating tokens?" --project source --path "$SOURCE_DIR" --scope source/core --agent codex --format json > "$TMP/source-ask.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- agent-activity --project source --path "$SOURCE_DIR" --scope source/core --format json > "$TMP/source-agent-activity.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- auto-capture --project source --path "$SOURCE_DIR" --scope source/core --source smoke-cli --format json > "$TMP/source-auto-capture.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- capture start --project source --path "$SOURCE_DIR" --scope source/core --source-app codex --format json > "$TMP/source-capture-start.json"
SOURCE_CAPTURE_ID="$(node -e "const fs=require('fs'); console.log(JSON.parse(fs.readFileSync(process.argv[1], 'utf8')).capture.id)" "$TMP/source-capture-start.json")"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- capture ingest --project source --path "$SOURCE_DIR" --capture "$SOURCE_CAPTURE_ID" --scope source/core --type transcript --source codex --title "Agent asked Grafiki" --text "Coding agent asked Grafiki about rotating tokens and captured the answer automatically." --format json > "$TMP/source-capture-ingest.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- capture ingest --project source --path "$SOURCE_DIR" --capture "$SOURCE_CAPTURE_ID" --scope source/core --type terminal --source zsh --title "Redaction smoke" --text "OPENAI_API_KEY=sk-testsecretsecretsecretsecret" --format json > "$TMP/source-capture-redacted.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- capture ingest --project source --path "$SOURCE_DIR" --capture "$SOURCE_CAPTURE_ID" --scope source/core --type ide --source editor --title "Edited auth.md" --payload '{"file":"auth.md","action":"created"}' --format json > "$TMP/source-capture-ide.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- capture summarize --project source --path "$SOURCE_DIR" --capture "$SOURCE_CAPTURE_ID" --scope source/core --format json > "$TMP/source-capture-summary.json"
printf '{"timestamp":"2026-05-31T00:00:00Z","type":"event_msg","payload":{"type":"user_message","message":"What did we learn about rotating tokens?"}}\n{"timestamp":"2026-05-31T00:00:02Z","type":"event_msg","payload":{"type":"agent_message","message":"Rotating tokens need evidence-linked memory for future agents."}}\n' > "$TMP/source-codex-transcript.jsonl"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- capture import-transcripts --project source --path "$SOURCE_DIR" --agent codex --input "$TMP/source-codex-transcript.jsonl" --scope source/core --summarize --format json > "$TMP/source-transcript-import.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- capture terminal-command --project source --path "$SOURCE_DIR" --scope source/core --cmd "cargo test -p grafiki-core" --cwd "$SOURCE_DIR" --exit-code 0 --duration-ms 42 --shell zsh --format json > "$TMP/source-terminal-command.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- capture watch-files --project source --path "$SOURCE_DIR" --scope source/core --since-seconds 86400 --limit 10 --summarize --format json > "$TMP/source-watch-files.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- capture git-summary --project source --path "$SOURCE_DIR" --scope source/core --summarize --format json > "$TMP/source-git-summary.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- capture shell-hook --project source --path "$SOURCE_DIR" --scope source/core > "$TMP/source-shell-hook.sh"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- export --project source --path "$SOURCE_DIR" --scope source/core --format json --output "$TMP/export.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- export --project source --path "$SOURCE_DIR" --scope source/core --format md --output "$TMP/export.md"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- export --project source --path "$SOURCE_DIR" --scope source/core --format dot --output "$TMP/export.dot"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- export --project source --path "$SOURCE_DIR" --scope source/core --format graphml --output "$TMP/export.graphml"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- export --project source --path "$SOURCE_DIR" --scope source/core --format html --output "$TMP/export.html"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- export --project source --path "$SOURCE_DIR" --scope source/core --format wiki --output "$TMP/wiki" > "$TMP/wiki.out"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- import "$TMP/export.json" --project target --path "$TARGET_DIR" --format json > "$TMP/import.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- search rotating --project target --path "$TARGET_DIR" --scope source/core --format json > "$TMP/import-search.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- embeddings status --project target --path "$TARGET_DIR" --scope source/core --format json > "$TMP/embedding-status.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- embeddings rebuild --project target --path "$TARGET_DIR" --scope source/core --limit 20 --format json > "$TMP/embedding-rebuild.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- embeddings status --project target --path "$TARGET_DIR" --scope source/core --format json > "$TMP/embedding-status-after.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- search rotating --project target --path "$TARGET_DIR" --scope source/core --mode semantic --format json > "$TMP/import-semantic-search.json"
GRAFIKI_HOME="$HOME_DIR" GRAFIKI_EMBEDDING_PROVIDER=missing-provider cargo run -q -p grafiki-cli -- search rotating --project target --path "$TARGET_DIR" --scope source/core --mode keyword --format json > "$TMP/search-invalid-provider-keyword.json"
GRAFIKI_HOME="$HOME_DIR" GRAFIKI_EMBEDDING_PROVIDER=missing-provider cargo run -q -p grafiki-cli -- search rotating --project target --path "$TARGET_DIR" --scope source/core --mode semantic --format json > "$TMP/search-invalid-provider-semantic.json"
GRAFIKI_HOME="$HOME_DIR" GRAFIKI_EMBEDDING_PROVIDER=missing-provider cargo run -q -p grafiki-cli -- embeddings status --project target --path "$TARGET_DIR" --scope source/core --format json > "$TMP/status-invalid-provider.json"
GRAFIKI_HOME="$HOME_DIR" GRAFIKI_EMBEDDING_PROVIDER=fastembed cargo run -q -p grafiki-cli -- embeddings status --project target --path "$TARGET_DIR" --scope source/core --format json > "$TMP/status-fastembed-unavailable.json"

grep -q 'rotating tokens' "$TMP/import-search.json"
grep -q '.grafiki.capture.json' "$TMP/source-init.json"
grep -q '"terminal": true' "$TMP/source-capture-config.json"
grep -q '"terminal_output": "off"' "$TMP/source-capture-config-set.json"
grep -q '"secrets"' "$TMP/source-capture-config-set.json"
grep -q 'rotating tokens' "$TMP/source-ask.json"
grep -q '"agent": "codex"' "$TMP/source-ask.json"
grep -q '"audit_id":' "$TMP/source-ask.json"
grep -q 'What should coding agents know' "$TMP/source-agent-activity.json"
grep -q 'grafiki_candidate_propose' "$TMP/source-ask.json"
grep -q 'Auto-captured coding session snapshot' "$TMP/source-auto-capture.json"
grep -q 'auth.md' "$TMP/source-auto-capture.json"
grep -q 'Capture session started' "$TMP/source-capture-start.json"
grep -q 'Capture event ingested' "$TMP/source-capture-ingest.json"
grep -q 'REDACTED' "$TMP/source-capture-redacted.json"
grep -q '"redacted": true' "$TMP/source-capture-redacted.json"
grep -q 'Automatic coding capture summary' "$TMP/source-capture-summary.json"
grep -q '"evidence":' "$TMP/source-capture-summary.json"
grep -q 'Imported 2 codex transcript events' "$TMP/source-transcript-import.json"
grep -q '"events_imported": 2' "$TMP/source-transcript-import.json"
grep -q 'source-codex-transcript.jsonl' "$TMP/source-transcript-import.json"
grep -q 'cargo test -p grafiki-core' "$TMP/source-terminal-command.json"
grep -q '"source_type": "terminal"' "$TMP/source-terminal-command.json"
grep -q 'File changes captured into raw events' "$TMP/source-watch-files.json"
grep -q 'auth.md' "$TMP/source-watch-files.json"
grep -q 'Git working-tree snapshot captured' "$TMP/source-git-summary.json"
grep -q '"source_type": "git"' "$TMP/source-git-summary.json"
grep -q 'add-zsh-hook precmd __grafiki_precmd' "$TMP/source-shell-hook.sh"
grep -q 'Candidate updated for review' "$TMP/source-candidate-edit.json"
grep -q 'editing before approval' "$TMP/source-candidate-edit.json"
grep -q 'Candidate approved into trusted memory' "$TMP/source-candidate-approve.json"
grep -q 'candidate-cli' "$TMP/source-candidates-approved.json"
grep -q '"pending": 5' "$TMP/embedding-status.json"
grep -q '"processed": 5' "$TMP/embedding-rebuild.json"
grep -q '"pending": 0' "$TMP/embedding-status-after.json"
grep -q '"embedded": 5' "$TMP/embedding-status-after.json"
grep -q '"embeddable_records": 5' "$TMP/embedding-status-after.json"
grep -q '"indexed_records": 5' "$TMP/embedding-status-after.json"
grep -q '"fresh_records": 5' "$TMP/embedding-status-after.json"
grep -q '"missing_or_stale_records": 0' "$TMP/embedding-status-after.json"
grep -q '"semantic_available": true' "$TMP/import-semantic-search.json"
grep -q '"score":' "$TMP/import-semantic-search.json"
grep -q 'rotating tokens' "$TMP/search-invalid-provider-keyword.json"
grep -q 'Semantic search is unavailable' "$TMP/search-invalid-provider-semantic.json"
grep -q '"provider": "unknown"' "$TMP/status-invalid-provider.json"
grep -q 'unknown embedding provider' "$TMP/status-invalid-provider.json"
grep -q 'fastembed provider requires building' "$TMP/status-fastembed-unavailable.json"
grep -q '<graphml' "$TMP/export.graphml"
grep -q '<svg' "$TMP/export.html"
test -s "$TMP/wiki/index.md"

echo "== http api =="
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- init http --path "$HTTP_DIR" --format json > "$TMP/http-init.json"
git -C "$HTTP_DIR" init -q
printf 'HTTP smoke rotates refresh tokens in this repo.\n' > "$HTTP_DIR/http.md"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- serve --project http --path "$HTTP_DIR" --host 127.0.0.1 --port "$PORT" > "$TMP/server.log" 2>&1 &
SERVER_PID=$!

for _ in {1..50}; do
  if curl -fsS "http://127.0.0.1:$PORT/health" > "$TMP/health.json" 2>/dev/null; then
    break
  fi
  sleep 0.1
done

curl -fsS -X POST "http://127.0.0.1:$PORT/api/sessions/start" -H 'Content-Type: application/json' -d '{"type":"codex","goal":"Smoke HTTP","scope":"http/core"}' > "$TMP/http-start.json"
curl -fsS "http://127.0.0.1:$PORT/api/capture/config" > "$TMP/http-capture-config.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/capture/config" -H 'Content-Type: application/json' -d '{"terminal":true,"files":true,"git":true,"add_blocked_paths":["private-notes"],"terminal_output":"off"}' > "$TMP/http-capture-config-set.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/capture/auto" -H 'Content-Type: application/json' -d '{"scope":"http/core","source":"smoke-http"}' > "$TMP/http-auto-capture.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/capture/start" -H 'Content-Type: application/json' -d '{"scope":"http/core","source_app":"codex"}' > "$TMP/http-capture-start.json"
HTTP_CAPTURE_ID="$(node -e "const fs=require('fs'); console.log(JSON.parse(fs.readFileSync(process.argv[1], 'utf8')).capture.id)" "$TMP/http-capture-start.json")"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/capture/ingest" -H 'Content-Type: application/json' -d "{\"capture\":\"$HTTP_CAPTURE_ID\",\"scope\":\"http/core\",\"source_type\":\"screen\",\"source\":\"desktop\",\"title\":\"Review pane visible\",\"text\":\"Screen showed Grafiki memory review during HTTP smoke.\"}" > "$TMP/http-capture-ingest.json"
printf '{"timestamp":"2026-05-31T00:00:00Z","type":"event_msg","payload":{"type":"user_message","message":"Import my HTTP transcript."}}\n{"timestamp":"2026-05-31T00:00:02Z","type":"event_msg","payload":{"type":"agent_message","message":"HTTP transcript import keeps coding-agent history reviewable."}}\n' > "$TMP/http-codex-transcript.jsonl"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/capture/import-transcripts" -H 'Content-Type: application/json' -d "{\"scope\":\"http/core\",\"agent\":\"codex\",\"input\":\"$TMP/http-codex-transcript.jsonl\",\"summarize\":true}" > "$TMP/http-transcript-import.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/capture/terminal-command" -H 'Content-Type: application/json' -d '{"scope":"http/core","command":"npm run build","cwd":"'"$HTTP_DIR"'","exit_code":0,"duration_ms":55,"shell":"zsh"}' > "$TMP/http-terminal-command.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/capture/watch-files" -H 'Content-Type: application/json' -d '{"scope":"http/core","since_seconds":86400,"limit":10,"summarize":true}' > "$TMP/http-watch-files.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/capture/git-summary" -H 'Content-Type: application/json' -d '{"scope":"http/core","summarize":true}' > "$TMP/http-git-summary.json"
curl -fsS "http://127.0.0.1:$PORT/api/capture/status?scope=http/core" > "$TMP/http-capture-status.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/capture/summarize" -H 'Content-Type: application/json' -d "{\"capture\":\"$HTTP_CAPTURE_ID\",\"scope\":\"http/core\"}" > "$TMP/http-capture-summary.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/sessions/handoff" -H 'Content-Type: application/json' -d '{}' > "$TMP/http-handoff.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/entities/save" -H 'Content-Type: application/json' -d '{"name":"Database","entity_type":"service","scope":"http/core"}' > "$TMP/http-db.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/entities/save" -H 'Content-Type: application/json' -d '{"name":"Auth Service","entity_type":"service","observe":"HTTP smoke stores rotating tokens","category":"architecture","scope":"http/core","relate":"database:depends_on"}' > "$TMP/http-auth.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/decisions" -H 'Content-Type: application/json' -d '{"title":"Use HTTP API","reasoning":"Agents need local access","scope":"http/core"}' > "$TMP/http-decision.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/state" -H 'Content-Type: application/json' -d '{"key":"http-api","title":"Build HTTP API","status":"in-progress","priority":"high","scope":"http/core"}' > "$TMP/http-state.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/context/add" -H 'Content-Type: application/json' -d '{"key":"prd","title":"PRD","category":"spec","scope":"http/core","content":"HTTP context smoke."}' > "$TMP/http-context.json"
curl -fsS "http://127.0.0.1:$PORT/api/search?q=rotating&scope=http/core" > "$TMP/http-search.json"
curl -fsS "http://127.0.0.1:$PORT/api/ask?q=What%20should%20agents%20know%20about%20rotating%20tokens%3F&scope=http/core&agent=cursor" > "$TMP/http-ask.json"
curl -fsS "http://127.0.0.1:$PORT/api/agent-queries?scope=http/core" > "$TMP/http-agent-queries.json"
for _ in {1..30}; do
  curl -fsS "http://127.0.0.1:$PORT/api/embeddings/status?scope=http/core" > "$TMP/http-embedding-status.json"
  if grep -q '"embedded": 5' "$TMP/http-embedding-status.json"; then
    break
  fi
  sleep 0.2
done
curl -fsS -X POST "http://127.0.0.1:$PORT/api/embeddings/rebuild" -H 'Content-Type: application/json' -d '{"scope":"http/core","limit":20}' > "$TMP/http-embedding-rebuild.json"
curl -fsS "http://127.0.0.1:$PORT/api/search?q=rotating&scope=http/core&mode=semantic" > "$TMP/http-semantic-search.json"
curl -fsS "http://127.0.0.1:$PORT/api/graph/auth-service?depth=1" > "$TMP/http-graph.json"
curl -fsS "http://127.0.0.1:$PORT/api/context/prd" > "$TMP/http-context-show.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/memory/update" -H 'Content-Type: application/json' -d '{"type":"context","id":"prd","content":"HTTP context smoke updated through generic maintenance."}' > "$TMP/http-record-update.json"
curl -fsS "http://127.0.0.1:$PORT/api/memory/context/prd?scope=http/core" > "$TMP/http-record-detail.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/memory/delete" -H 'Content-Type: application/json' -d '{"type":"state","id":"http-api"}' > "$TMP/http-record-delete.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/candidates/propose" -H 'Content-Type: application/json' -d '{"type":"context","source_type":"connector:smoke","source":"issue-7","scope":"http/core","confidence":0.8,"payload":{"key":"http-candidate","title":"HTTP Candidate","category":"reference","content":"HTTP candidate approval stores trusted context."}}' > "$TMP/http-candidate.json"
HTTP_CANDIDATE_ID="$(node -e "const fs=require('fs'); console.log(JSON.parse(fs.readFileSync(process.argv[1], 'utf8')).candidate.id)" "$TMP/http-candidate.json")"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/candidates/edit" -H 'Content-Type: application/json' -d "{\"id\":\"$HTTP_CANDIDATE_ID\",\"confidence\":0.88,\"payload\":{\"key\":\"http-candidate\",\"title\":\"HTTP Candidate Edited\",\"category\":\"reference\",\"content\":\"HTTP candidate editing preserves the review step before approval.\"}}" > "$TMP/http-candidate-edit.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/candidates/propose" -H 'Content-Type: application/json' -d '{"type":"state","source_type":"connector:smoke","source":"issue-noise","scope":"http/core","confidence":0.2,"payload":{"key":"http-noisy-candidate","title":"HTTP noisy candidate"}}' > "$TMP/http-candidate-noisy.json"
HTTP_NOISY_CANDIDATE_ID="$(node -e "const fs=require('fs'); console.log(JSON.parse(fs.readFileSync(process.argv[1], 'utf8')).candidate.id)" "$TMP/http-candidate-noisy.json")"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/candidates/bulk" -H 'Content-Type: application/json' -d "{\"action\":\"reject\",\"ids\":[\"$HTTP_NOISY_CANDIDATE_ID\"],\"rationale\":\"Smoke bulk reject\"}" > "$TMP/http-candidate-bulk.json"
curl -fsS "http://127.0.0.1:$PORT/api/candidates?status=pending&scope=http/core" > "$TMP/http-candidates-pending.json"
curl -fsS -X POST "http://127.0.0.1:$PORT/api/candidates/approve" -H 'Content-Type: application/json' -d "{\"id\":\"$HTTP_CANDIDATE_ID\"}" > "$TMP/http-candidate-approve.json"
curl -fsS "http://127.0.0.1:$PORT/api/memory/context/http-candidate?scope=http/core" > "$TMP/http-candidate-detail.json"

grep -q 'rotating tokens' "$TMP/http-search.json"
grep -q '"screen": false' "$TMP/http-capture-config.json"
grep -q '"private-notes"' "$TMP/http-capture-config-set.json"
grep -q '"terminal_output": "off"' "$TMP/http-capture-config-set.json"
grep -q 'rotating tokens' "$TMP/http-ask.json"
grep -q '"agent": "cursor"' "$TMP/http-ask.json"
grep -q 'What should agents know' "$TMP/http-agent-queries.json"
grep -q 'For new uncertain facts' "$TMP/http-ask.json"
grep -q 'Auto-captured coding session snapshot' "$TMP/http-auto-capture.json"
grep -q 'http.md' "$TMP/http-auto-capture.json"
grep -q 'Capture event ingested' "$TMP/http-capture-ingest.json"
grep -q 'Imported 2 codex transcript events' "$TMP/http-transcript-import.json"
grep -q '"events_imported": 2' "$TMP/http-transcript-import.json"
grep -q 'npm run build' "$TMP/http-terminal-command.json"
grep -q 'File changes captured into raw events' "$TMP/http-watch-files.json"
grep -q 'http.md' "$TMP/http-watch-files.json"
grep -q 'Git working-tree snapshot captured' "$TMP/http-git-summary.json"
grep -q 'Review pane visible' "$TMP/http-capture-status.json"
grep -q 'Automatic coding capture summary' "$TMP/http-capture-summary.json"
grep -q 'Grafiki Handoff' "$TMP/http-handoff.json"
grep -q 'Smoke HTTP' "$TMP/http-handoff.json"
grep -q '"pending": 0' "$TMP/http-embedding-status.json"
grep -q '"embedded": 5' "$TMP/http-embedding-status.json"
grep -q '"semantic_available": true' "$TMP/http-semantic-search.json"
grep -q '"score":' "$TMP/http-semantic-search.json"
grep -q 'depends_on' "$TMP/http-graph.json"
grep -q 'HTTP context smoke' "$TMP/http-context-show.json"
grep -q 'Context updated' "$TMP/http-record-update.json"
grep -q 'HTTP context smoke updated through generic maintenance' "$TMP/http-record-detail.json"
grep -q '"record_type": "context"' "$TMP/http-record-detail.json"
grep -q 'State item deleted' "$TMP/http-record-delete.json"
grep -q 'Candidate updated for review' "$TMP/http-candidate-edit.json"
grep -q 'review step before approval' "$TMP/http-candidate-edit.json"
grep -q '"succeeded": 1' "$TMP/http-candidate-bulk.json"
grep -q 'http-candidate' "$TMP/http-candidates-pending.json"
grep -q 'Candidate approved into trusted memory' "$TMP/http-candidate-approve.json"
grep -q 'HTTP candidate editing preserves the review step before approval' "$TMP/http-candidate-detail.json"

kill "$SERVER_PID" 2>/dev/null || true
wait "$SERVER_PID" 2>/dev/null || true
unset SERVER_PID

echo "== daemon =="
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- init daemon --path "$DAEMON_DIR" --format json > "$TMP/daemon-init.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- daemon start --project daemon --path "$DAEMON_DIR" --port "$DAEMON_PORT" --token smoke-token --format json > "$TMP/daemon-start.json"
for _ in {1..50}; do
  if curl -fsS "http://127.0.0.1:$DAEMON_PORT/health" > "$TMP/daemon-health.json" 2>/dev/null; then
    break
  fi
  sleep 0.1
done
if curl -fsS "http://127.0.0.1:$DAEMON_PORT/api/status" > "$TMP/daemon-unauthorized.json" 2>/dev/null; then
  echo "Expected token-protected daemon request to fail" >&2
  exit 1
fi
curl -fsS -H "Authorization: Bearer smoke-token" "http://127.0.0.1:$DAEMON_PORT/api/status" > "$TMP/daemon-authorized.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- daemon status --project daemon --path "$DAEMON_DIR" --format json > "$TMP/daemon-status.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- daemon stop --project daemon --path "$DAEMON_DIR" --format json > "$TMP/daemon-stop.json"

grep -q '"already_running": false' "$TMP/daemon-start.json"
grep -q '"project": "daemon"' "$TMP/daemon-authorized.json"
grep -q '"running": true' "$TMP/daemon-status.json"
grep -q '"stopped": true' "$TMP/daemon-stop.json"

if GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- serve --project daemon --path "$DAEMON_DIR" --host 0.0.0.0 --allow-non-local --port "$DAEMON_PORT" > "$TMP/nonlocal.out" 2> "$TMP/nonlocal.err"; then
  echo "Expected non-local bind without token to fail" >&2
  exit 1
fi
grep -q 'Non-local HTTP binds require --token' "$TMP/nonlocal.err"

echo "== mcp =="
printf 'MCP smoke rotates refresh tokens in this repo.\n' > "$HTTP_DIR/mcp.md"
printf '{"timestamp":"2026-05-31T00:00:00Z","type":"event_msg","payload":{"type":"user_message","message":"Import my MCP transcript."}}\n{"timestamp":"2026-05-31T00:00:02Z","type":"event_msg","payload":{"type":"agent_message","message":"MCP transcript import keeps agent history reviewable."}}\n' > "$HTTP_DIR/mcp-codex-transcript.jsonl"
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"grafiki_start","arguments":{"goal":"Smoke MCP","scope":"mcp/core"}}}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"grafiki_handoff","arguments":{}}}' \
  '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"grafiki_save","arguments":{"name":"Database","entity_type":"service","scope":"mcp/core"}}}' \
  '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"grafiki_save","arguments":{"name":"Auth Service","entity_type":"service","observe":"MCP smoke stores rotating tokens","category":"architecture","scope":"mcp/core","relate":"database:depends_on"}}}' \
  '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"grafiki_update_record","arguments":{"type":"context","id":"prd","content":"MCP record maintenance updated context."}}}' \
  '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"grafiki_record","arguments":{"type":"context","id":"prd","scope":"http/core"}}}' \
  '{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"grafiki_state_set","arguments":{"key":"mcp-delete","title":"MCP delete smoke","status":"in-progress","scope":"mcp/core"}}}' \
  '{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"grafiki_delete_record","arguments":{"type":"state","id":"mcp-delete"}}}' \
  '{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"grafiki_candidate_propose","arguments":{"type":"state","source_type":"mcp-smoke","source":"thread-99","scope":"mcp/core","confidence":0.6,"payload":{"key":"mcp-candidate","title":"MCP candidate smoke"}}}}' \
  '{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"grafiki_candidate_list","arguments":{"status":"pending","scope":"mcp/core"}}}' \
  '{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"grafiki_search","arguments":{"query":"rotating","scope":"mcp/core"}}}' \
  '{"jsonrpc":"2.0","id":13,"method":"tools/call","params":{"name":"grafiki_ask","arguments":{"question":"What should agents know about rotating tokens?","scope":"mcp/core","agent":"claude-code"}}}' \
  '{"jsonrpc":"2.0","id":14,"method":"tools/call","params":{"name":"grafiki_agent_activity","arguments":{"scope":"mcp/core"}}}' \
  '{"jsonrpc":"2.0","id":15,"method":"tools/call","params":{"name":"grafiki_auto_capture","arguments":{"scope":"mcp/core","source":"smoke-mcp","limit":10}}}' \
  '{"jsonrpc":"2.0","id":16,"method":"tools/call","params":{"name":"grafiki_capture_start","arguments":{"scope":"mcp/core","source_app":"codex"}}}' \
  '{"jsonrpc":"2.0","id":17,"method":"tools/call","params":{"name":"grafiki_capture_ingest","arguments":{"scope":"mcp/core","source_type":"transcript","source":"codex","title":"MCP transcript capture","text":"MCP captured a coding agent transcript event."}}}' \
  '{"jsonrpc":"2.0","id":18,"method":"tools/call","params":{"name":"grafiki_capture_events","arguments":{"scope":"mcp/core","limit":5}}}' \
  '{"jsonrpc":"2.0","id":19,"method":"tools/call","params":{"name":"grafiki_capture_summarize","arguments":{"scope":"mcp/core","limit":5}}}' \
  "{\"jsonrpc\":\"2.0\",\"id\":20,\"method\":\"tools/call\",\"params\":{\"name\":\"grafiki_capture_import_transcripts\",\"arguments\":{\"agent\":\"codex\",\"input\":\"$HTTP_DIR/mcp-codex-transcript.jsonl\",\"scope\":\"mcp/core\",\"summarize\":true}}}" \
  '{"jsonrpc":"2.0","id":21,"method":"tools/call","params":{"name":"grafiki_capture_config","arguments":{}}}' \
  '{"jsonrpc":"2.0","id":22,"method":"tools/call","params":{"name":"grafiki_capture_config_set","arguments":{"add_blocked_paths":["mcp-private"],"terminal_output":"off"}}}' \
  "{\"jsonrpc\":\"2.0\",\"id\":23,\"method\":\"tools/call\",\"params\":{\"name\":\"grafiki_capture_terminal_command\",\"arguments\":{\"scope\":\"mcp/core\",\"command\":\"cargo fmt\",\"cwd\":\"$HTTP_DIR\",\"exit_code\":0,\"duration_ms\":10,\"shell\":\"zsh\"}}}" \
  '{"jsonrpc":"2.0","id":24,"method":"tools/call","params":{"name":"grafiki_capture_watch_files","arguments":{"scope":"mcp/core","since_seconds":86400,"limit":10,"summarize":true}}}' \
  '{"jsonrpc":"2.0","id":25,"method":"tools/call","params":{"name":"grafiki_capture_git_summary","arguments":{"scope":"mcp/core","summarize":true}}}' \
  | GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli -- mcp --project http --path "$HTTP_DIR" > "$TMP/mcp.out"

grep -q 'required' "$TMP/mcp.out"
grep -q 'Grafiki Handoff' "$TMP/mcp.out"
grep -q 'MCP record maintenance updated context' "$TMP/mcp.out"
grep -q 'State item deleted' "$TMP/mcp.out"
grep -q 'mcp-candidate' "$TMP/mcp.out"
grep -q 'grafiki_candidate_edit' "$TMP/mcp.out"
grep -q 'grafiki_candidate_bulk' "$TMP/mcp.out"
grep -q 'rotating tokens' "$TMP/mcp.out"
grep -q 'grafiki_ask' "$TMP/mcp.out"
grep -q 'grafiki_agent_activity' "$TMP/mcp.out"
grep -q 'claude-code' "$TMP/mcp.out"
grep -q 'grafiki_auto_capture' "$TMP/mcp.out"
grep -q 'mcp.md' "$TMP/mcp.out"
grep -q 'grafiki_capture_ingest' "$TMP/mcp.out"
grep -q 'grafiki_capture_import_transcripts' "$TMP/mcp.out"
grep -q 'Imported 2 codex transcript events' "$TMP/mcp.out"
grep -q 'grafiki_capture_config' "$TMP/mcp.out"
grep -q 'mcp-private' "$TMP/mcp.out"
grep -q 'grafiki_capture_terminal_command' "$TMP/mcp.out"
grep -q 'cargo fmt' "$TMP/mcp.out"
grep -q 'grafiki_capture_watch_files' "$TMP/mcp.out"
grep -q 'File changes captured into raw events' "$TMP/mcp.out"
grep -q 'grafiki_capture_git_summary' "$TMP/mcp.out"
grep -q 'Git working-tree snapshot captured' "$TMP/mcp.out"
grep -q 'MCP transcript capture' "$TMP/mcp.out"
grep -q 'Automatic coding capture summary' "$TMP/mcp.out"
grep -q 'For new uncertain facts' "$TMP/mcp.out"

echo "smoke ok: $TMP"
