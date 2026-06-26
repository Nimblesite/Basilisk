import { test, expect, type Page } from "@playwright/test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { SHOTS } from "../../screenshots/shots.mjs";

// Implements [WEBSITE-ERROR-PAGES-VERIFY]: every diagnostic the CLI deep-links to
// must resolve to a rendered /errors/<code>/ page. The checker prints
// `see: https://www.basilisk-python.dev/errors/BSK-XXXX` for each diagnostic, so a
// missing page is a broken "learn more" link shown to real users.
// See docs/specs/WEBSITE-ERROR-PAGES-SPEC.md.

type Rule = { code: string; severity: string; summary: string };

// The generated data the pages are built from — the single source of truth.
const RULES: Rule[] = JSON.parse(
  readFileSync(new URL("../../src/_data/rules.json", import.meta.url), "utf8"),
);

// Worked-example shots, keyed by the code each one triggers (shot.expect).
const EXAMPLE_CODES = SHOTS.filter(
  (s) => /^e\d+$/.test(s.name) && /^BSK-[EW]\d{4}$/.test(s.expect),
).map((s) => ({ code: s.expect, stem: s.name }));

const expectRendered = async (page: Page, src: string): Promise<void> => {
  const img = page.locator(`img[src="${src}"]`).first();
  await img.scrollIntoViewIfNeeded();
  const state = await img.evaluate((el: HTMLImageElement) =>
    el.decode().then(() => el.naturalWidth, () => 0),
  );
  expect(state, `screenshot should decode: ${src}`).toBeGreaterThan(0);
};

test.describe("error reference pages", () => {
  test("every diagnostic code has a built /errors/ page", () => {
    const missing = RULES.map((r) => r.code).filter(
      (code) => !existsSync(join(process.cwd(), "_site", "errors", code, "index.html")),
    );
    expect(missing, `codes with no /errors/ page: ${missing.join(", ")}`).toHaveLength(0);
    expect(RULES.length).toBeGreaterThanOrEqual(155); // every CLI-linked code, at least
  });

  test("a sampled page renders its code, title and severity", async ({ page }) => {
    for (const code of ["BSK-E0001", "BSK-W0014", "BSK-E0099", RULES[RULES.length - 1].code]) {
      const rule = RULES.find((r) => r.code === code)!;
      await page.goto(`/errors/${code}/`);
      await expect(page.locator("h1.error-title code")).toHaveText(code);
      await expect(page).toHaveTitle(new RegExp(code));
      await expect(page.locator(`.badge--${rule.severity}`)).toBeVisible();
    }
  });

  test("the /errors/ index links every diagnostic code", async ({ page }) => {
    await page.goto("/errors/");
    const linked = await page
      .locator('.error-list a[href^="/errors/BSK-"]')
      .evaluateAll((els) => els.length);
    expect(linked).toBe(RULES.length);
  });

  // Each worked example must actually surface on its error page (desktop only —
  // rendering is viewport-independent and this navigates ~30 pages).
  test("every worked example renders on its error page", async ({ page, isMobile }) => {
    test.skip(!!isMobile, "viewport-independent; run once on desktop");
    for (const { code, stem } of EXAMPLE_CODES) {
      await page.goto(`/errors/${code}/`);
      await expectRendered(page, `/assets/images/${stem}.png`);
    }
  });
});
