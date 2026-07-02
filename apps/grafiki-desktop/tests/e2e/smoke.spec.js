// Smoke suite: boots the real app and walks the primary surfaces.
// Selectors are the app's real classes (see src/styles.css) — no test doubles.

/// First-run state shows onboarding; a configured machine shows Home. Both are
/// valid boots — this helper lands us on Home either way.
async function landOnHome() {
  // Whichever arrives first: onboarding (fresh profile) or the app shell
  // (which restores whatever pane was persisted — NOT necessarily Home).
  const getStarted = $("button=Get started");
  const rail = $(".rail-nav");
  await browser.waitUntil(
    async () => (await getStarted.isExisting()) || (await rail.isExisting()),
    { timeoutMsg: "neither onboarding nor the app shell appeared" },
  );

  if (await getStarted.isExisting()) {
    // Drive onboarding with a disposable project folder.
    await getStarted.click();
    const folder = $(".onboarding-folder input");
    await folder.waitForExist();
    await folder.setValue(`/tmp/grafiki-e2e-${Date.now()}`);
    await $("button=Create memory here").click();
    // Local AI step: either "Continue" (Ollama found) or "Skip for now".
    const cont = $("button=Continue");
    const skip = $("button=Skip for now");
    await browser.waitUntil(
      async () => (await cont.isExisting()) || (await skip.isExisting()),
    );
    await ((await cont.isExisting()) ? cont : skip).click();
    // First-session step: skip straight to the app.
    await $("button=Skip — take me to the app").click();
  }

  // Navigate deterministically — the brand mark always goes Home.
  await $(".brand").click();
  await $(".home-title").waitForExist();
}

describe("Grafiki desktop", () => {
  it("boots to the Home ledger", async () => {
    await landOnHome();
    expect(await $(".home-title").getText()).toBe("Today");
    // The stat strip renders all three counters.
    expect(await $$(".stat-card")).toHaveLength(3);
    // The ask bar is pinned and ready.
    await expect($(".ask-bar-wrap input")).toExist();
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
      await $(`button*=${label}`).click();
      await $(marker).waitForExist({
        timeoutMsg: `${label} pane did not render ${marker}`,
      });
    }
    // Back home via the brand mark.
    await $(".brand").click();
    await $(".home-title").waitForExist();
  });

  it("opens the command palette and routes a question to Memory chat", async () => {
    await landOnHome();
    await browser.keys(["Meta", "k"]);
    const paletteInput = $(".palette input");
    await paletteInput.waitForExist({ timeoutMsg: "⌘K palette did not open" });
    // Free text that matches no command becomes an Ask-memory row.
    await paletteInput.setValue("what did we decide about testing");
    await browser.keys(["Enter"]);
    // Lands in the Memory pane with the question asked.
    await $(".chat-view").waitForExist();
  });

  it("Review advertises its keyboard triage", async () => {
    await landOnHome();
    await $("button*=Review").click();
    await $(".candidate-toolbar").waitForExist();
    const kbd = await $$("kbd");
    // j/k a r e v space — at least six advertised keys.
    expect(kbd.length).toBeGreaterThanOrEqual(6);
  });

  it("Settings switches the theme and back", async () => {
    await landOnHome();
    await $("button*=Settings").click();
    const select = $(".setting-row select");
    await select.waitForExist();
    await select.selectByVisibleText("Dark");
    await browser.waitUntil(
      async () =>
        (await $("html").getAttribute("data-theme")) === "dark",
      { timeoutMsg: "dark theme did not apply" },
    );
    await select.selectByVisibleText("Light");
    await browser.waitUntil(
      async () =>
        (await $("html").getAttribute("data-theme")) === "light",
      { timeoutMsg: "light theme did not restore" },
    );
  });
});
