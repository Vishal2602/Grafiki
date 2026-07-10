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

In `auto` mode (`GRAFIKI_EMBEDDING_PROVIDER=auto`), if the model can't be loaded
Grafiki falls back to the deterministic provider and `embeddings status` notes how
to pre-download it. The deterministic provider
(`GRAFIKI_EMBEDDING_PROVIDER=deterministic`, the default) needs no model and works
fully offline. Note: if `HF_HOME` is set in your environment it overrides the cache
location above — unset it (or point it at the same directory) to keep the model pinned.

## CLI

### From source (recommended — the only path that works today)

```bash
git clone https://github.com/Vishal2602/Grafiki && cd Grafiki
cargo install --path crates/grafiki-cli --features fastembed,sqlite-vec
```

### Homebrew (not yet available)

No `homebrew-grafiki` tap has been published and no signed release binaries
exist yet. Once a tap is published, installation will look like:

```bash
brew tap Vishal2602/grafiki
brew install grafiki        # installs the `grafiki` binary
```

### From a release tarball (not yet available)

No tagged GitHub release has been published yet, so there is no tarball to
download. Once one exists:

```bash
# pick your target: aarch64-apple-darwin, x86_64-apple-darwin, x86_64-unknown-linux-gnu
curl -L https://github.com/Vishal2602/Grafiki/releases/latest/download/grafiki-aarch64-apple-darwin.tar.gz | tar -xz
sudo mv grafiki /usr/local/bin/
```

A standalone downloaded binary is quarantined by macOS; clear it with
`xattr -d com.apple.quarantine ./grafiki` (Homebrew does this for you).

## Desktop app (macOS)

### Build from source (works today)

No signed release or Homebrew cask is published yet. Build the app locally:

```bash
git clone https://github.com/Vishal2602/Grafiki && cd Grafiki
cd apps/grafiki-desktop
npm install
npm run tauri:build:release
```

See [PRODUCTION_RELEASE.md](PRODUCTION_RELEASE.md) for the full build/signing
notes, or [scripts/build_desktop_debug.sh](../scripts/build_desktop_debug.sh)
for a repeatable debug build.

### Homebrew Cask (not yet available)

No `homebrew-grafiki` tap has been published and no signed DMG has been
released. Once one exists:

```bash
brew tap Vishal2602/grafiki
brew install --cask grafiki
```

`brew install --cask` strips the quarantine flag, so the app opens cleanly even
before Developer ID signing/notarization is configured.

### Download the DMG (not yet available)

No tagged GitHub release has been published yet, so there is no DMG to
download. Once one exists:

1. Grab `Grafiki_<version>_aarch64.dmg` from the [Releases](https://github.com/Vishal2602/Grafiki/releases) page.
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

`grafiki init` prints the exact read-only command for your project. Add
`--allow-write` only when a trusted client intentionally needs to save, approve,
update, or retire memory.

## Linux / Windows

- **Linux:** the CLI works natively (Homebrew on Linux, the release tarball, or
  `cargo install`). No Gatekeeper equivalent.
- **Windows:** the CLI builds from source with `cargo build -p grafiki-cli`. The
  desktop app and signed installers are not yet produced for Windows.
