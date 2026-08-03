import { test, expect } from "@playwright/test";

// Implements [WEBSITE-E2E-SMOKE]: homepage positioning, proof and mobile
// usability checks on the production build. Tests [WEBSITE-E2E] /
// [WEBSITE-E2E-PURPOSE]; see docs/specs/WEBSITE-E2E-SPEC.md.

test.describe("homepage positioning", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("leads with the Python type checker and language server", async ({
    page,
  }) => {
    await expect(page).toHaveTitle(
      "Basilisk — Fast Python Type Checker & Language Server",
    );
    await expect(page.locator("h1")).toHaveCount(1);
    await expect(page.locator("h1")).toHaveText(
      "The only Python type checker that scores 100% on the official Python typing suite. And the fastest we’ve benchmarked.",
    );
    await expect(page.locator(".hero__subheadline")).toContainText(
      "Basilisk is an open-source Python type checker and language server built in Rust.",
    );
    await expect(
      page.locator('a[href="vscode:extension/Nimblesite.basilisk"]'),
    ).toHaveCount(2);

    const description = await page
      .locator('meta[name="description"]')
      .getAttribute("content");
    expect(description).toContain("Python type checker and language server");
    expect(description?.length).toBeGreaterThanOrEqual(150);
    expect(description?.length).toBeLessThanOrEqual(160);
  });

  test("carries a proof link beside each headline claim", async ({ page }) => {
    // The two comparative claims in the hero are only publishable while they
    // are linked to the source that grades them: the conformance claim to the
    // official python/typing results, the speed claim to our benchmark and its
    // methodology. The false-positive count is asserted at 0 because that is a
    // ratchet; the caught count is left open because upstream adds test cases.
    await expect(page.locator(".hero__subheadline")).toContainText(
      "only checker that passes every file of the official python/typing conformance suite",
    );
    await expect(page.locator(".hero__subheadline")).toContainText(
      /\d+ required errors caught and 0 false positives/,
    );
    await expect(page.locator(".hero__subheadline")).toContainText(
      "lowest median cold full-file CLI time of any checker in our published benchmark",
    );
    await expect(
      page.locator('.hero__subheadline a[href*="github.com/python/typing"]'),
    ).toHaveCount(1);
    await expect(
      page.locator('.hero__subheadline a[href="/docs/benchmarks/"]'),
    ).toHaveCount(1);
  });

  test("shows only linked and scoped headline proof", async ({ page }) => {
    await expect(page.locator(".hero__proof .stat-card")).toHaveCount(2);
    await expect(page.locator(".hero__proof")).toContainText(
      "Only listed checker with a perfect official score",
    );
    await expect(page.locator(".hero__proof")).toContainText(
      "Fastest in our published cold-check benchmark",
    );
    await expect(
      page.locator('.hero__proof a[href*="github.com/python/typing"]'),
    ).toHaveCount(1);
    await expect(
      page.locator('.hero__proof-cta a[href="/docs/benchmarks/"]'),
    ).toBeVisible();

    const body = await page.locator("body").innerText();
    expect(body).not.toContain("Strict by default");
    expect(body).not.toContain("Every diagnostic");
    expect(body).not.toContain("One binary");
  });

  test("publishes matching social and software metadata", async ({ page }) => {
    await expect(page.locator('meta[property="og:title"]')).toHaveAttribute(
      "content",
      "Basilisk — Fast Python Type Checker & Language Server",
    );
    await expect(page.locator('meta[property="og:image"]')).toHaveAttribute(
      "content",
      "https://www.basilisk-python.dev/assets/images/og-image.png",
    );
    await expect(page.locator('meta[property="og:image:width"]')).toHaveAttribute(
      "content",
      "1200",
    );
    await expect(page.locator('meta[property="og:image:height"]')).toHaveAttribute(
      "content",
      "630",
    );

    const jsonLd = await page
      .locator('script[type="application/ld+json"]')
      .textContent();
    const graph = JSON.parse(jsonLd ?? "{}")["@graph"];
    const webpage = graph.find((item: { "@type": string }) =>
      item["@type"] === "WebPage",
    );
    const software = graph.find((item: { "@type": string }) =>
      item["@type"] === "SoftwareApplication",
    );

    expect(webpage.dateModified).toBeTruthy();
    expect(software.applicationSubCategory).toBe(
      "Python type checker and language server",
    );
    expect(software.programmingLanguage).toBe("Rust");
    expect(software).not.toHaveProperty("softwareRequirements");
  });
});

test.describe("homepage mobile usability", () => {
  test.skip(
    ({ isMobile }) => !isMobile,
    "the iPhone SE project guards the narrowest supported viewport",
  );

  test("fits the viewport and keeps visible calls to action tappable", async ({
    page,
  }) => {
    await page.goto("/");

    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth - window.innerWidth,
    );
    expect(overflow).toBeLessThanOrEqual(0);

    const buttonHeights = await page.locator(".btn").evaluateAll((buttons) =>
      buttons
        .filter((button) => {
          const style = getComputedStyle(button);
          return style.display !== "none" && style.visibility !== "hidden";
        })
        .map((button) => button.getBoundingClientRect().height),
    );
    expect(buttonHeights.length).toBeGreaterThan(0);
    for (const height of buttonHeights) {
      expect(height).toBeGreaterThanOrEqual(48);
    }
  });
});
