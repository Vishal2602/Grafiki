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
  activeWithin: (sel) =>
    browser.execute((s) => {
      const root = document.querySelector(s);
      return Boolean(root && document.activeElement && root.contains(document.activeElement));
    }, sel),
};

const isolatedProject = `/tmp/grafiki-e2e-${Date.now()}`;

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
    await q.setValue(".onboarding-folder input", isolatedProject);
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

async function ensureIsolatedProject() {
  const currentProject = await browser.execute(() =>
    localStorage.getItem("grafiki.desktop.projectRoot"),
  );
  if (currentProject === isolatedProject) return;

  await q.clickByText(".rail-item", "Settings");
  await q.waitFor(".settings-grid");
  await q.setValue(".settings-grid .field-label input", isolatedProject);
  await q.clickByText(".settings-grid button", "Initialize");
  await browser.waitUntil(
    async () => ((await q.text(".notice.good")) ?? "").includes("initialized"),
    { timeoutMsg: "isolated E2E project did not initialize" },
  );
}

describe("Grafiki desktop", () => {
  before(async () => {
    await landOnHome();
    await ensureIsolatedProject();
  });

  it("boots to the Home ledger", async () => {
    await landOnHome();
    // "Today" only when a real today-group of sessions exists; a fresh
    // profile (this test's onboarding run) has none, so "Home" is correct
    // (2026-07-04 fix — the title used to lie and always say "Today").
    const title = await q.text(".home-title");
    if (title !== "Today" && title !== "Home") throw new Error(`home title was ${title}`);
    await browser.waitUntil(async () => (await q.count(".stat-card")) === 3, {
      timeoutMsg: "the outgoing animated Home pane did not unmount",
    });
    const cards = await q.count(".stat-card");
    if (cards !== 3) throw new Error(`expected 3 stat cards, got ${cards}`);
    if (!(await q.exists(".ask-bar-wrap input"))) {
      throw new Error("ask bar input missing");
    }
  });

  it("navigates every rail destination", async () => {
    await landOnHome();
    // The eyebrow (.pane-kind) was retired in the modal→page overhaul; every
    // non-home destination now renders a .pane-header with its title.
    const destinations = [
      ["Sessions", ".pane-header"],
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

  it("keeps keyboard focus inside the accessible command palette", async () => {
    await landOnHome();
    await browser.keys(["Meta", "k"]);
    await q.waitFor('.palette[role="dialog"][aria-modal="true"]');
    if (!(await q.activeWithin(".palette"))) {
      throw new Error("palette did not move focus inside the dialog");
    }
    for (let index = 0; index < 12; index += 1) {
      await browser.keys(["Tab"]);
      if (!(await q.activeWithin(".palette"))) {
        throw new Error(`focus escaped the palette after ${index + 1} Tab presses`);
      }
    }
    await browser.keys(["Escape"]);
    await browser.waitUntil(async () => !(await q.exists(".palette")), {
      timeoutMsg: "Escape did not close the palette",
    });
  });

  it("exposes trusted search, agent activity, and manual memory capture", async () => {
    await landOnHome();
    await q.clickByText(".rail-item", "Memory");
    await q.waitFor(".chat-view");

    await q.clickByText(".memory-surface-actions button", "New memory");
    await q.waitFor(".manual-memory-form");
    if (!(await q.exists('.manual-memory-form option[value="decision"]'))) {
      throw new Error("manual decision capture is missing");
    }
    if (!(await q.exists('.manual-memory-form option[value="context"]'))) {
      throw new Error("manual context capture is missing");
    }

    const title = `E2E decision ${Date.now()}`;
    await q.setValue('.manual-memory-form input:not([placeholder="All memory"])', title);
    await q.setValue(".manual-memory-form textarea", "Keep this unique decision as trusted memory.");
    await q.clickByText(".manual-memory-form button", "Save memory");
    await browser.waitUntil(
      () =>
        browser.execute(
          (expected) =>
            [...document.querySelectorAll(".data-row-button")].some((row) =>
              (row.textContent ?? "").includes(expected),
            ),
          title,
        ),
      { timeoutMsg: "the IPC-created decision did not reappear in the Decisions list" },
    );

    await q.clickByText('[role="tab"]', "Search");
    await q.waitFor(".trusted-search-form");
    const modes = await q.count('.trusted-search-form select option');
    if (modes < 6) throw new Error(`trusted search filters missing; found ${modes} options`);
    await q.setValue(".trusted-search-query input", title);
    await q.clickByText(".trusted-search-form button", "Search");
    await browser.waitUntil(
      () =>
        browser.execute(
          (expected) =>
            [...document.querySelectorAll(".data-row-button")].some((row) =>
              (row.textContent ?? "").includes(expected),
            ),
          title,
        ),
      { timeoutMsg: "the IPC-created decision was not returned by exact search" },
    );

    await q.clickByText('[role="tab"]', "Agent activity");
    await q.waitFor('.mem-tab-panel[role="tabpanel"]');
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
    await q.waitFor(".settings-grid");
    // Theme lives under the About tab (docs/UX_REDESIGN.md §5.6 tab layout).
    await q.clickByText(".seg-tab", "About");
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
