import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { test, expect } from "@playwright/test";

// Implements [WEBSITE-E2E-WITHDRAWAL]. The site serves one statement and a
// notice at every retired URL ([WITHDRAWAL-SURFACES]). These tests enforce that
// contract against the production build: the statement is the approved copy, no
// page says anything the messaging spec forbids ([WITHDRAWAL-PROHIBITED]), and
// no retired URL 404s or offers itself for indexing.
// See docs/specs/WEBSITE-E2E-SPEC.md and docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md.

const SITE = fileURLToPath(new URL("../../_site/", import.meta.url));
const DATA = fileURLToPath(new URL("../../src/_data/", import.meta.url));

const withdrawal = JSON.parse(
  readFileSync(join(DATA, "withdrawal.json"), "utf-8"),
) as { line: string; title: string; short: string[]; full: string[] };

const retiredUrls = JSON.parse(
  readFileSync(join(DATA, "retiredUrls.json"), "utf-8"),
) as string[];

const APOLOGY = "https://www.christianfindlay.com/blog/basilisk-conformance-apology";

/** Every built HTML page, as `{ url, html }`. */
function builtPages(): { url: string; html: string }[] {
  const pages: { url: string; html: string }[] = [];
  const walk = (dir: string, prefix: string) => {
    for (const entry of readdirSync(dir)) {
      const full = join(dir, entry);
      if (statSync(full).isDirectory()) {
        walk(full, `${prefix}${entry}/`);
      } else if (entry.endsWith(".html")) {
        const url = entry === "index.html" ? prefix : `${prefix}${entry}`;
        pages.push({ url, html: readFileSync(full, "utf-8") });
      }
    }
  };
  walk(SITE, "/");
  return pages;
}

/**
 * Visible text of a built page, tags and scripts removed. Dropping an inline
 * tag leaves a gap, so whitespace before punctuation is closed up: a browser
 * renders `<a>an apology</a>.` as "an apology.", not "an apology .".
 */
function visibleText(html: string): string {
  return html
    .replace(/<script[\s\S]*?<\/script>/g, " ")
    .replace(/<style[\s\S]*?<\/style>/g, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/\s+/g, " ")
    .replace(/\s+([.,;:])/g, "$1");
}

test.describe("the statement", () => {
  test("home serves the full approved copy and is indexable", async ({ page }) => {
    await page.goto("/");

    await expect(page.locator("h1")).toHaveText(withdrawal.title);
    // Every paragraph of the approved full copy, in order.
    const paragraphs = page.locator(".notice p");
    await expect(paragraphs).toHaveCount(withdrawal.full.length);
    for (const [index, expected] of withdrawal.full.entries()) {
      await expect(paragraphs.nth(index)).toHaveText(
        visibleText(expected).trim(),
      );
    }

    await expect(page.locator('meta[name="robots"]')).toHaveAttribute(
      "content",
      "index, follow",
    );
    await expect(page.locator(`.notice a[href="${APOLOGY}"]`)).toHaveCount(1);
  });

  test("home names the four facts the message rests on", async ({ page }) => {
    await page.goto("/");
    const text = (await page.locator(".notice").innerText()).replace(/\s+/g, " ");

    expect(text).toContain("producing incorrect results");
    expect(text).toContain("removed from the python/typing conformance results");
    expect(text).toContain("not isolated to a known set of rules");
    expect(text).toContain("worse than useless");
  });
});

test.describe("retired URLs", () => {
  // A representative URL from each family the site used to serve. `/errors/` is
  // the one that matters most: shipped binaries print those links.
  const SAMPLES = [
    "/docs/",
    "/docs/rules/",
    "/errors/BSK-0001/",
    "/blog/",
    "/playground/",
    "/zh/docs/installation/",
  ];

  for (const url of SAMPLES) {
    test(`${url} serves the notice and links home`, async ({ page }) => {
      const response = await page.goto(url);
      expect(response?.status()).toBe(200);

      await expect(page.locator(".notice p").first()).toContainText(
        "Basilisk is unlisted everywhere",
      );
      await expect(page.locator('.notice a[href="/"]')).toHaveCount(1);
      await expect(page.locator(`.notice a[href="${APOLOGY}"]`)).toHaveCount(1);
    });
  }

  test("every retired URL was built", () => {
    const built = new Set(builtPages().map((p) => p.url));
    const missing = retiredUrls.filter((url) => !built.has(url));
    expect(missing, "retired URLs with no page — these would 404").toEqual([]);
  });

  test("every notice page is noindex, and only home is not", () => {
    const indexable = builtPages()
      .filter((p) => !/name="robots" content="noindex/.test(p.html))
      .map((p) => p.url);
    expect(indexable).toEqual(["/"]);
  });

  test("the sitemap offers only the statement", () => {
    const sitemap = readFileSync(join(SITE, "sitemap.xml"), "utf-8");
    const locs = [...sitemap.matchAll(/<loc>([^<]+)<\/loc>/g)].map((m) => m[1]);
    expect(locs).toEqual(["https://www.basilisk-python.dev/"]);
  });
});

test.describe("prohibited content", () => {
  // [WITHDRAWAL-PROHIBITED]. Each pattern is something the site must never say
  // again; a hit means marketing, a withdrawn figure, or an install path
  // survived the strip.
  const FORBIDDEN: { label: string; pattern: RegExp }[] = [
    { label: "a conformance or pass-rate figure", pattern: /\d+(\.\d+)?\s*%/ },
    {
      label: "install instructions",
      pattern: /\b(pip|pipx|uv tool|brew|scoop|npm)\s+install\b/i,
    },
    { label: "an editor install link", pattern: /vscode:extension/i },
    {
      label: "a marketplace or package listing",
      pattern: /marketplace\.visualstudio\.com|open-vsx\.org|pypi\.org/i,
    },
    {
      label: "a competitor comparison",
      pattern: /\b(pyright|mypy|pyrefly|zuban|ty)\b/i,
    },
    { label: "a benchmark claim", pattern: /\bbenchmark|\bfastest\b/i },
    { label: "a rule catalogue", pattern: /\bBSK-\d{4}\b/ },
  ];

  for (const { label, pattern } of FORBIDDEN) {
    test(`no page contains ${label}`, () => {
      const offenders = builtPages()
        .filter((p) => pattern.test(visibleText(p.html)))
        .map((p) => p.url);
      expect(offenders).toEqual([]);
    });
  }

  test("the apology is linked, never quoted", () => {
    for (const { url, html } of builtPages()) {
      expect(html, `${url} must link the apology`).toContain(APOLOGY);
      // Link text stays neutral; the page must not reproduce its argument.
      expect(visibleText(html), `${url} must not quote the apology`).not.toMatch(
        /I (was|am) (wrong|sorry)|in my own words/i,
      );
    }
  });
});
