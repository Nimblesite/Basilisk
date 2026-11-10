import { readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";

// Tests [WEBSITE-E2E] / [WEBSITE-E2E-PURPOSE]: the published benchmark must
// identify the measured unit as a complete file and link every fixture source.
// See docs/specs/WEBSITE-E2E-SPEC.md.
const FIXTURE_DIR = fileURLToPath(
  new URL("../../../benchmarks/fixtures/", import.meta.url),
);
const FIXTURE_FILES = readdirSync(FIXTURE_DIR)
  .filter((filename) => filename.endsWith(".py"))
  .sort();
const FIXTURE_SOURCE_ROOT =
  "https://github.com/Nimblesite/Basilisk/blob/main/benchmarks/fixtures/";

test("benchmark table reports whole-file timings and links every fixture", async ({
  page,
}) => {
  await page.goto("/docs/benchmarks/");

  await expect(page.locator(".releases-intro")).toContainText(
    "Each row is one complete Python fixture file",
  );
  await expect(page.locator(".releases-intro")).toContainText(
    "It does not measure one typing rule",
  );

  const rows = page.locator(".benchmark-table tbody tr");
  const links = rows.locator("th a");
  await expect(rows).toHaveCount(FIXTURE_FILES.length);
  await expect(links).toHaveCount(FIXTURE_FILES.length);
  expect(await links.allTextContents()).toEqual(FIXTURE_FILES);
  const hrefs = await links.evaluateAll((anchors) =>
    anchors.map((anchor) => (anchor as HTMLAnchorElement).href),
  );
  expect(hrefs).toEqual(
    FIXTURE_FILES.map((filename) => `${FIXTURE_SOURCE_ROOT}${filename}`),
  );

  await expect(rows.first().locator("th, td")).toHaveCount(9);
  for (const value of await rows.locator("td").allTextContents()) {
    expect(value.trim()).toMatch(/^(?:\d+\.\d ms|—)$/);
  }
});
