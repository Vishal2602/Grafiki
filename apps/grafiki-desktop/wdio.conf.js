// WebdriverIO e2e suite for the Grafiki desktop app (Tauri 2, macOS-native via
// the embedded driver provider — no external driver, no cloud dependency).
//
// Prereqs (npm run test:e2e does all three):
//   1. npm run build            → dist/ the debug binary embeds
//   2. cargo build -p grafiki-desktop
//   3. wdio run wdio.conf.js
//
// The debug binary hosts the automation server (tauri-plugin-wdio-webdriver,
// debug builds only), so tests drive the REAL app: real Rust backend, real DB
// resolution, real terminal registry.

export const config = {
  runner: "local",
  specs: ["./tests/e2e/**/*.spec.js"],
  maxInstances: 1,

  services: [
    [
      "tauri",
      {
        appBinaryPath: "../../target/debug/grafiki-desktop",
        driverProvider: "embedded",
      },
    ],
  ],

  capabilities: [{ browserName: "tauri" }],

  framework: "mocha",
  mochaOpts: { ui: "bdd", timeout: 90_000 },
  reporters: ["spec"],
  waitforTimeout: 15_000,
  connectionRetryTimeout: 60_000,
  connectionRetryCount: 2,
  logLevel: "warn",
};
