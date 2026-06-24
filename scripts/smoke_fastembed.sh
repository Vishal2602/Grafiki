#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
HOME_DIR="$TMP/home"
PROJECT_DIR="$TMP/project"
FEATURES="${GRAFIKI_FASTEMBED_FEATURES:-fastembed sqlite-vec}"

mkdir -p "$HOME_DIR" "$PROJECT_DIR"
cd "$ROOT"

echo "== fastembed sqlite-vec smoke =="
echo "This may download the MiniLM model and ONNX Runtime assets on first run."

GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli --features "$FEATURES" -- init fastembed-smoke --path "$PROJECT_DIR" --format json > "$TMP/init.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli --features "$FEATURES" -- start --project fastembed-smoke --path "$PROJECT_DIR" --type codex --goal "Fastembed smoke" --scope fastembed-smoke/core --format json > "$TMP/start.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli --features "$FEATURES" -- save "Auth Service" --project fastembed-smoke --path "$PROJECT_DIR" --type service --observe "JWT refresh uses rotating tokens and replay protection" --category architecture --scope fastembed-smoke/core --format json > "$TMP/auth.json"
GRAFIKI_HOME="$HOME_DIR" cargo run -q -p grafiki-cli --features "$FEATURES" -- context add auth-guide --project fastembed-smoke --path "$PROJECT_DIR" --title "Auth Guide" --category guide --scope fastembed-smoke/core --content "Refresh token rotation protects sessions from replay." --format json > "$TMP/context.json"

GRAFIKI_HOME="$HOME_DIR" GRAFIKI_EMBEDDING_PROVIDER=fastembed cargo run -q -p grafiki-cli --features "$FEATURES" -- embeddings status --project fastembed-smoke --path "$PROJECT_DIR" --scope fastembed-smoke/core --format json > "$TMP/status-before.json"
GRAFIKI_HOME="$HOME_DIR" GRAFIKI_EMBEDDING_PROVIDER=fastembed cargo run -q -p grafiki-cli --features "$FEATURES" -- embeddings rebuild --project fastembed-smoke --path "$PROJECT_DIR" --scope fastembed-smoke/core --limit 20 --format json > "$TMP/rebuild.json"
GRAFIKI_HOME="$HOME_DIR" GRAFIKI_EMBEDDING_PROVIDER=fastembed cargo run -q -p grafiki-cli --features "$FEATURES" -- embeddings status --project fastembed-smoke --path "$PROJECT_DIR" --scope fastembed-smoke/core --format json > "$TMP/status-after.json"
GRAFIKI_HOME="$HOME_DIR" GRAFIKI_EMBEDDING_PROVIDER=fastembed cargo run -q -p grafiki-cli --features "$FEATURES" -- search "session replay prevention" --project fastembed-smoke --path "$PROJECT_DIR" --scope fastembed-smoke/core --mode semantic --format json > "$TMP/search.json"

grep -q '"provider": "fastembed"' "$TMP/status-after.json"
grep -q '"vector_backend": "json+sqlite-vec"' "$TMP/status-after.json"
grep -q '"pending": 0' "$TMP/status-after.json"
grep -q '"semantic_available": true' "$TMP/search.json"
grep -q 'replay' "$TMP/search.json"

echo "fastembed smoke ok: $TMP"
