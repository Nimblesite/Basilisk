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
      "A Python type checker that scores 99.3% on the official Python typing suite. And the fastest we’ve benchmarked.",
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
    // The comparative claims in the hero are only publishable while they are
    // linked to the source that grades them: the conformance score to our
    // reproducible run of the official suite, the open-failure note to the
    // upstream change that caused it, the speed claim to our benchmark and its
    // methodology. The score is asserted exactly because it is what the last
    // committed harness run measured; while it is below 100% the note beside
    // it is mandatory.
    await expect(page.locator(".hero__subheadline")).toContainText(
      "It scores 99.3% on the official python/typing conformance suite",
    );
    await expect(page.locator(".hero__subheadline")).toContainText(
      "one file currently fails and a fix is in progress",
    );
    await expect(page.locator(".hero__subheadline")).toContainText(
      "lowest median cold full-file CLI time of any checker in our published benchmark",
    );
    await expect(
      page.locator('.hero__subheadline a[href="/docs/conformance/"]'),
    ).toHaveCount(1);
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
      "Official typing conformance score",
    );
    await expect(page.locator(".hero__proof")).toContainText(
      "Fastest in our published cold-check benchmark",
    );
    // Both counts are exact: they are what the last committed harness run
    // measured (the gate currently tolerates the single upstream-caused false
    // positive — see coverage-thresholds.json). The "perfect score" card copy
    // returns only when the fresh run is perfect again.
    await expect(page.locator(".hero__proof")).toContainText(
      "0 missed required errors and 1 false positives",
    );
    await expect(page.locator(".hero__proof-cta")).toContainText(
      "performance is self-measured and reproducible",
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

  test("serves the declared social image at its declared size", async ({
    page,
    request,
  }) => {
    const src = await page
      .locator('meta[property="og:image"]')
      .getAttribute("content");
    const response = await request.get(src ?? "");
    expect(response.status()).toBe(200);

    // PNG IHDR: width and height are big-endian uint32 at byte offsets 16 and 20.
    const png = await response.body();
    expect(png.readUInt32BE(16)).toBe(1200);
    expect(png.readUInt32BE(20)).toBe(630);
  });
});

// The Chinese homepage is a translation of the English one, not a separate
// pitch: same sections, same data gates, same proof links. These tests fail if
// the two drift apart in structure or if a zh superlative escapes its gate.
test.describe("Chinese homepage", () => {
  const skeleton = (page: import("@playwright/test").Page) =>
    page.evaluate(() =>
      Array.from(
        document.querySelectorAll(
          "main section, main .stat-card, main .problem__bullets li, main .btn",
        ),
      ).map((element) => element.className),
    );

  test("mirrors the English page structure section for section", async ({
    page,
  }) => {
    await page.goto("/");
    const english = await skeleton(page);
    await page.goto("/zh/");
    const chinese = await skeleton(page);

    expect(english.length).toBeGreaterThan(0);
    expect(chinese).toEqual(english);
  });

  test("renders both headline claims beside their proof", async ({ page }) => {
    await page.goto("/zh/");

    await expect(page.locator("h1")).toHaveCount(1);
    await expect(page.locator("h1")).toContainText(
      "在官方 Python typing 套件中取得 99.3% 的 Python 类型检查器。",
    );
    await expect(page.locator(".hero__headline-accent")).toHaveText(
      "也是我们测过最快的。",
    );

    await expect(page.locator(".hero__subheadline")).toContainText(
      "它在官方 python/typing 符合性套件中取得 99.3%",
    );
    await expect(page.locator(".hero__subheadline")).toContainText(
      "目前有一个文件未通过，修复正在进行中",
    );
    await expect(
      page.locator('.hero__subheadline a[href="/zh/docs/conformance/"]'),
    ).toHaveCount(1);
    await expect(
      page.locator('.hero__subheadline a[href*="github.com/python/typing"]'),
    ).toHaveCount(1);
    await expect(
      page.locator('.hero__subheadline a[href="/docs/benchmarks/"]'),
    ).toHaveCount(1);

    await expect(page.locator(".hero__proof .stat-card")).toHaveCount(2);
    await expect(page.locator(".hero__proof")).toContainText(
      "官方类型符合性得分",
    );
    await expect(page.locator(".hero__proof")).toContainText(
      /遗漏的必需错误 0 处，误报 1 处/,
    );
    await expect(page.locator(".hero__proof")).toContainText(
      "我们公开的冷启动基准测试中最快",
    );
  });

  test("keeps its comparative claims on the same gates as the English page", async ({
    page,
  }) => {
    // A gate that fires on one locale and not the other means one of the two
    // pages is asserting a comparative fact its data no longer supports.
    await page.goto("/");
    const englishAccents = await page
      .locator(".hero__headline-accent")
      .count();
    await page.goto("/zh/");
    expect(await page.locator(".hero__headline-accent").count()).toBe(
      englishAccents,
    );

    // Chinese readers search the English product nouns too; both must be present.
    const keywords = await page
      .locator('meta[name="keywords"]')
      .getAttribute("content");
    expect(keywords).toContain("python type checker");
    expect(keywords).toContain("python 类型检查器");
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
