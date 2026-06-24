# Security Policy

Grafiki stores coding context locally, including project memory, raw capture events, and agent query logs. Treat this data as sensitive.

## Supported Versions

Grafiki is currently pre-1.0. Security fixes apply to the latest mainline release or snapshot.

## Reporting A Vulnerability

Until a public security email is configured, please open a private security advisory in GitHub if available. If not, open an issue with minimal detail and request a private channel.

Do not publish exploit details for:

- captured secrets,
- database leakage,
- path traversal,
- local HTTP auth bypass,
- MCP command abuse,
- desktop capture permission bypass.

## Security Design

- Local-first storage under `~/.grafiki`.
- No telemetry by default.
- Non-local HTTP binds require explicit token configuration.
- Capture ingest redacts obvious secrets before persistence.
- Screen capture is explicit/manual in the desktop app.
- Agent queries are logged locally for audit.

## Known Pre-1.0 Gaps

- SQLCipher/keychain encryption is planned but not complete.
- Signing/notarization requires Apple Developer credentials.
- Redaction is a first-pass heuristic and must not be treated as a complete secrets scanner.
