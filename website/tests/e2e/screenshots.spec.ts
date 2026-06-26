import { test, expect, type Page } from "@playwright/test";

import { SHOTS } from "../../screenshots/shots.mjs";

// Implements [WEBSITE-SCREENSHOTS-VERIFY]: every CLI screenshot the docs embed
// must actually exist and render (non-zero pixels), so a missing or zero-byte
// regeneration is caught in CI rather than shipping a broken image.
// See docs/specs/WEBSITE-SCREENSHOTS-SPEC.md.

// Derived from the generator manifest so the test can never drift from the set of
// images we actually produce: `e00*` shots live on the rule docs, the rest
// (cli-demo / cli-clean) on the homepage before/after demo.
const RULE_STEMS = SHOTS.map((s) => s.name).filter((n) => n.startsWith("e0"));
const HOME_STEMS = SHOTS.map((s) => s.name).filter((n) => !n.startsWith("e0"));

const stemOf = (src: string): string => src.split("/").pop()!.replace(/\.png$/, "");

// Assert a single screenshot <img> has decoded to real pixels.
const expectRendered = async (page: Page, src: string): Promise<void> => {
  const img = page.locator(`img[src="${src}"]`).first();
  await img.scrollIntoViewIfNeeded();
  const state = await img.evaluate((el: HTMLImageElement) =>
    el.decode().then(
      () => ({ ok: true, w: el.naturalWidth, h: el.naturalHeight }),
      () => ({ ok: false, w: 0, h: 0 }),
    ),
  );
  expect(state.ok, `screenshot should decode: ${src}`).toBe(true);
  expect(state.w, `screenshot should have pixel width: ${src}`).toBeGreaterThan(0);
  expect(state.h, `screenshot should have pixel height: ${src}`).toBeGreaterThan(0);
};

// Collect the stems of every screenshot-asset <img> referenced on a page, failing
// fast on any request that did not return 200.
const screenshotStemsOn = async (page: Page, path: string): Promise<string[]> => {
  const failed: string[] = [];
  page.on("response", (r) => {
    if (r.url().includes("/assets/images/") && r.url().endsWith(".png") && r.status() !== 200) {
      failed.push(`${r.url()} → ${r.status()}`);
    }
  });
  await page.goto(path);
  expect(failed, `image requests must all succeed: ${failed.join(", ")}`).toHaveLength(0);

  const srcs = await page.locator('img[src*="/assets/images/"]').evaluateAll((imgs) =>
    imgs.map((el) => (el as HTMLImageElement).getAttribute("src") ?? ""),
  );
  return srcs.map(stemOf);
};

test.describe("CLI screenshots render", () => {
  test("rule docs embed every rule screenshot and each renders", async ({ page }) => {
    const seen = new Set<string>();
    for (const path of ["/docs/rules/missing-annotations/", "/docs/rules/type-safety/"]) {
      const stems = await screenshotStemsOn(page, path);
      for (const stem of stems) {
        if (stem.startsWith("e0")) {
          await expectRendered(page, `/assets/images/${stem}.png`);
          seen.add(stem);
        }
      }
    }
    for (const stem of RULE_STEMS) {
      expect(seen.has(stem), `rule docs must embed ${stem}.png`).toBe(true);
    }
  });

  test("homepage before/after demo renders both CLI screenshots", async ({ page }) => {
    const stems = await screenshotStemsOn(page, "/");
    for (const stem of HOME_STEMS) {
      expect(stems.includes(stem), `homepage must embed ${stem}.png`).toBe(true);
    }

    // The "before" panel (cli-demo) is visible; reveal "after" (cli-clean), which
    // sits in a lazy, initially-hidden tab panel, before asserting it renders.
    await expectRendered(page, "/assets/images/cli-demo.png");
    await page.locator('.demo-tab[data-tab="after"]').click();
    await expectRendered(page, "/assets/images/cli-clean.png");
  });
});
