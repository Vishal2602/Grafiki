// WebdriverIO e2e suite for the Grafiki desktop app (Tauri 2, macOS-native via
// the embedded driver provider — no external driver, no cloud dependency).
//
// Prereqs:
//   1. npm run dev              → serves tauri.conf.json build.devUrl
//   2. npm run build            → dist/ for packaged/debug builds
//   3. cargo build -p grafiki-desktop
//   4. wdio run wdio.conf.js
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

  // First page load pays vite's cold module-transform tax (the debug binary
  // loads devUrl) — absorb it once here instead of inside the first spec.
  before: async () => {
    await browser
      .$(".rail-nav")
      .waitForExist({ timeout: 120_000 })
      .catch(() => Promise.resolve()); // fresh profiles show onboarding instead
  },
  connectionRetryTimeout: 60_000,
  connectionRetryCount: 2,
  logLevel: "warn",
};
