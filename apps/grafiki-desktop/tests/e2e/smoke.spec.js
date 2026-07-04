// Smoke suite: boots the real app and walks the primary surfaces.
//
// DRIVER NOTE: the embedded macOS driver (tauri-plugin-wdio-webdriver 1.2)
// intermittently stalls WebDriver element-find calls for 90s+ on this app,
// and its select action doesn't fire React's change event. Every query and
// interaction below therefore goes through `browser.execute` (in-page DOM),
// which has been reliable in every run. Revisit native finds when the driver
// matures. Keyboard input via browser.keys() works and is used as-is.

const q = {
  exists: (sel) =>
    browser.execute((s) => Boolean(document.querySelector(s)), sel),
  count: (sel) =>
    browser.execute((s) => document.querySelectorAll(s).length, sel),
  text: (sel) =>
    browser.execute((s) => document.querySelector(s)?.textContent?.trim() ?? null, sel),
  attr: (sel, name) =>
    browser.execute(
      (s, a) => document.querySelector(s)?.getAttribute(a) ?? null,
      sel,
      name,
    ),
  click: (sel) =>
    browser.execute((s) => {
      const el = document.querySelector(s);
      if (!el) return false;
      el.click();
      return true;
    }, sel),
  clickByText: (sel, textContent) =>
    browser.execute(
      (s, t) => {
        const el = [...document.querySelectorAll(s)].find((node) =>
          (node.textContent ?? "").trim().includes(t),
        );
        if (!el) return false;
        el.click();
        return true;
      },
      sel,
      textContent,
    ),
  // React controlled inputs need the native value setter + an input event.
  setValue: (sel, value) =>
    browser.execute(
      (s, v) => {
        const el = document.querySelector(s);
        if (!el) return false;
        const proto =
          el instanceof HTMLTextAreaElement
            ? HTMLTextAreaElement.prototype
            : HTMLInputElement.prototype;
        Object.getOwnPropertyDescriptor(proto, "value").set.call(el, v);
        el.dispatchEvent(new Event("input", { bubbles: true }));
        return true;
      },
      sel,
      value,
    ),
  selectValue: (sel, value) =>
    browser.execute(
      (s, v) => {
        const el = document.querySelector(s);
        if (!el) return false;
        el.value = v;
        el.dispatchEvent(new Event("change", { bubbles: true }));
        return true;
      },
      sel,
      value,
    ),
  waitFor: (sel, timeoutMsg) =>
    browser.waitUntil(() => q.exists(sel), {
      timeoutMsg: timeoutMsg ?? `${sel} never appeared`,
    }),
};

/// First-run profiles boot into onboarding; configured ones restore whatever
/// pane was persisted. Either way, land on Home.
async function landOnHome() {
  await browser.waitUntil(
    async () =>
      (await q.exists(".onboarding")) || (await q.exists(".rail-nav")),
    { timeoutMsg: "neither onboarding nor the app shell appeared" },
  );

  if (await q.exists(".onboarding")) {
    await q.clickByText("button", "Get started");
    await q.waitFor(".onboarding-folder input");
    await q.setValue(".onboarding-folder input", `/tmp/grafiki-e2e-${Date.now()}`);
    await q.clickByText("button", "Create memory here");
    await browser.waitUntil(
      async () => ((await q.text(".onboarding-step h1")) ?? "").includes("Local AI"),
      { timeoutMsg: "local AI step never appeared" },
    );
    // "Continue" when Ollama is present, "Skip for now" otherwise.
    if (!(await q.clickByText("button", "Continue"))) {
      await q.clickByText("button", "Skip for now");
    }
    await browser.waitUntil(() => q.clickByText("button", "Skip — take me to the app"));
  }

  await q.click(".brand");
  await q.waitFor(".home-title", "Home never rendered");
}

describe("Grafiki desktop", () => {
  it("boots to the Home ledger", async () => {
    await landOnHome();
    // "Today" only when a real today-group of sessions exists; a fresh
    // profile (this test's onboarding run) has none, so "Home" is correct
    // (2026-07-04 fix — the title used to lie and always say "Today").
    const title = await q.text(".home-title");
    if (title !== "Today" && title !== "Home") throw new Error(`home title was ${title}`);
    const cards = await q.count(".stat-card");
    if (cards !== 3) throw new Error(`expected 3 stat cards, got ${cards}`);
    if (!(await q.exists(".ask-bar-wrap input"))) {
      throw new Error("ask bar input missing");
    }
  });

  it("navigates every rail destination", async () => {
    await landOnHome();
    const destinations = [
      ["Sessions", ".pane-kind"],
      ["Memory", ".seg-tabs"],
      ["Review", ".candidate-toolbar"],
      ["Settings", ".settings-grid"],
    ];
    for (const [label, marker] of destinations) {
      if (!(await q.clickByText(".rail-item", label))) {
        throw new Error(`rail item ${label} not found`);
      }
      await q.waitFor(marker, `${label} pane did not render ${marker}`);
    }
    await q.click(".brand");
    await q.waitFor(".home-title");
  });

  it("opens the command palette and routes a question to Memory chat", async () => {
    await landOnHome();
    await browser.keys(["Meta", "k"]);
    await q.waitFor(".palette input", "⌘K palette did not open");
    await q.setValue(".palette input", "what did we decide about testing");
    await browser.keys(["Enter"]);
    await q.waitFor(".chat-view", "Memory chat did not open from the palette");
  });

  it("Review advertises its keyboard triage", async () => {
    await landOnHome();
    await q.clickByText(".rail-item", "Review");
    await q.waitFor(".candidate-toolbar");
    const kbd = await q.count("kbd");
    if (kbd < 6) throw new Error(`expected ≥6 kbd chips, got ${kbd}`);
  });

  it("Settings switches the theme and back", async () => {
    await landOnHome();
    await q.clickByText(".rail-item", "Settings");
    await q.waitFor(".setting-row select");
    const initial = (await q.attr("html", "data-theme")) ?? "light";
    const flipped = initial === "dark" ? "light" : "dark";
    await q.selectValue(".setting-row select", flipped);
    await browser.waitUntil(
      async () => (await q.attr("html", "data-theme")) === flipped,
      { timeoutMsg: `${flipped} theme did not apply` },
    );
    await q.selectValue(".setting-row select", initial);
    await browser.waitUntil(
      async () => (await q.attr("html", "data-theme")) === initial,
      { timeoutMsg: `${initial} theme did not restore` },
    );
  });
});
