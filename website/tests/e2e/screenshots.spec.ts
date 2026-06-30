import { test, expect, type Page } from "@playwright/test";

// Implements [WEBSITE-SCREENSHOTS-VERIFY]: every CLI screenshot the docs embed
// must actually exist and render (non-zero pixels), so a missing or zero-byte
// regeneration is caught in CI rather than shipping a broken image.
// Tests [WEBSITE-SCREENSHOTS] / [WEBSITE-SCREENSHOTS-PURPOSE]: the committed,
// real-binary CLI screenshots actually render on the production site, so the
// automated-screenshot pipeline's output is enforced in CI.
// See docs/specs/WEBSITE-SCREENSHOTS-SPEC.md.

// Every shot in the manifest is a rule shot embedded on its per-code
// /errors/<code>/ page; their rendering is asserted in errors.spec.ts.

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
  test("rule docs embed worked-example screenshots and none are broken", async ({ page }) => {
    for (const path of ["/docs/rules/missing-annotations/", "/docs/rules/type-safety/"]) {
      const stems = await screenshotStemsOn(page, path);
      const ruleShots = stems.filter((stem) => stem.startsWith("e0"));
      expect(ruleShots.length, `${path} should embed rule screenshots`).toBeGreaterThan(0);
      for (const stem of ruleShots) {
        await expectRendered(page, `/assets/images/${stem}.png`);
      }
    }
  });

  // Implements [VSIX-EDITOR-SCREENSHOTS-VERIFY]: real VS Code editor screenshots
  // embedded on the feature docs — assert each is present on its embedding page and
  // decodes to non-zero pixels, so a missing/zero-byte capture fails CI (capture
  // itself never runs in CI, per [GITHUB-NO-ARTIFACTS]).
  // Mirrors the [VSIX-EDITOR-SCREENSHOTS-SET] table (Image -> Embedded on): each
  // captured PNG must surface on the docs page that documents the feature.
  const EDITOR_SHOTS: Array<{ path: string; image: string }> = [
    { path: "/docs/install-vscode/", image: "vscode-diagnostics.png" },
    { path: "/docs/refactoring/", image: "vscode-quickfix.png" },
    { path: "/docs/", image: "vscode-module-explorer.png" },
    { path: "/docs/quick-start/", image: "vscode-hover.png" },
  ];

  for (const { path, image } of EDITOR_SHOTS) {
    test(`${path} embeds the ${image} editor screenshot`, async ({ page }) => {
      const stems = await screenshotStemsOn(page, path);
      expect(stems.includes(image.replace(/\.png$/, "")), `${path} must embed ${image}`).toBe(true);
      await expectRendered(page, `/assets/images/${image}`);
    });
  }
});
