# Grafiki Production Release Notes

Grafiki can now build and launch as an installed macOS desktop app, but the current verified artifact is still a local debug build. A public production release needs Apple signing and notarization credentials that cannot be created inside this workspace.

## Verified Local Build

Use this when you want the app installed locally on this Mac:

```bash
INSTALL_TO_APPLICATIONS=1 scripts/smoke_desktop.sh
```

This runs Rust tests, prepares the bundled CLI sidecar, builds the Tauri app and DMG, verifies the DMG checksum, installs `/Applications/Grafiki.app`, launches the app, and checks daemon-status wiring.

Last verified locally: 2026-05-31, after evidence-linked candidates, agent query audit logs, init imports, capture redaction, and the desktop Agent Activity pane were verified. The debug bundle launched successfully from the build output.

## Release Build Command

Use this once Apple signing/notarization is configured:

```bash
cd apps/grafiki-desktop
npm run tauri:build:release
```

Release bundles are written under:

```text
target/release/bundle/macos/Grafiki.app
target/release/bundle/dmg/
```

## Production Gate

Before calling a release production-ready for other Macs:

- Build without `--debug`.
- Confirm the `grafiki` CLI sidecar is present in `Grafiki.app/Contents/MacOS/`.
- Sign with a Developer ID Application certificate.
- Notarize and staple the app/DMG with Apple.
- Run the installed app smoke on a clean macOS user account.
- Confirm daemon non-local binds still require explicit token configuration.
- Keep the debug build for local testing only.

## Current Limitation

The remaining external blocker is Apple distribution identity: Developer ID certificate, notarization credentials, and any desired update signing keys. The codebase is prepared for the release build path, but those secrets must be supplied by the project owner.
