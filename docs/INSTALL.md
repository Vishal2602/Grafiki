# Installing Grafiki

Grafiki ships two things: the **`grafiki` CLI** (also the MCP server for AI agents)
and an optional **desktop app**. The CLI is the core; the desktop app is a memory
console on top of it.

> Released binaries are built with real semantic search (`fastembed` + `sqlite-vec`).
> If you build from source with the default features you get the lightweight
> deterministic embedding provider instead — run with `--features fastembed,sqlite-vec`
> (or set `GRAFIKI_EMBEDDING_PROVIDER=fastembed`) for real embeddings.

### Semantic-search model (offline)

Real semantic search uses the MiniLM model, cached at `~/.grafiki/models/fastembed`
(or `$GRAFIKI_HOME/models/fastembed`). It downloads automatically on first use. For
airgapped/offline machines, pre-download it while online:

```bash
grafiki embeddings prefetch
```

If the model can't be loaded offline, Grafiki falls back to the deterministic
provider and `grafiki embeddings status` explains how to pre-download it. The
deterministic provider (`GRAFIKI_EMBEDDING_PROVIDER=deterministic`) needs no model
and works fully offline.

## CLI

### Homebrew (recommended)

```bash
brew tap <owner>/grafiki
brew install grafiki        # installs the `grafiki` binary
```

### From a release tarball

```bash
# pick your target: aarch64-apple-darwin, x86_64-apple-darwin, x86_64-unknown-linux-gnu
curl -L https://github.com/<owner>/grafiki/releases/latest/download/grafiki-aarch64-apple-darwin.tar.gz | tar -xz
sudo mv grafiki /usr/local/bin/
```

A standalone downloaded binary is quarantined by macOS; clear it with
`xattr -d com.apple.quarantine ./grafiki` (Homebrew does this for you).

### From source

```bash
cargo install --path crates/grafiki-cli --features fastembed,sqlite-vec
```

## Desktop app (macOS)

### Homebrew Cask (recommended)

```bash
brew tap <owner>/grafiki
brew install --cask grafiki
```

`brew install --cask` strips the quarantine flag, so the app opens cleanly even
before Developer ID signing/notarization is configured.

### Download the DMG

1. Grab `Grafiki_<version>_aarch64.dmg` from the [Releases](https://github.com/<owner>/grafiki/releases) page.
2. Open the DMG and drag **Grafiki** to Applications.
3. **If macOS blocks it** ("Apple could not verify…"): open
   **System Settings → Privacy & Security**, scroll to the Grafiki notice, and
   click **Open Anyway**. (This step disappears once the build is signed and
   notarized — see [PRODUCTION_RELEASE.md](PRODUCTION_RELEASE.md).)

## Connect an AI agent (MCP)

After `grafiki init`, point your MCP client at:

```bash
grafiki mcp --project <name> --path /path/to/repo
```

`grafiki init` prints the exact command for your project.

## Linux / Windows

- **Linux:** the CLI works natively (Homebrew on Linux, the release tarball, or
  `cargo install`). No Gatekeeper equivalent.
- **Windows:** the CLI builds from source with `cargo build -p grafiki-cli`. The
  desktop app and signed installers are not yet produced for Windows.
