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
      "Basilisk — Python Type Checker & Language Server",
    );
    await expect(page.locator("h1")).toHaveCount(1);
    await expect(page.locator("h1")).toHaveText(
      "An open-source Python type checker and language server, built in Rust.",
    );
    await expect(page.locator(".hero__subheadline")).toContainText(
      "withdrawn our former conformance claim",
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

  test("puts the integrity correction beside the product introduction", async ({ page }) => {
    await expect(page.locator(".hero__subheadline")).toContainText(
      "withdrawn our former conformance claim and published benchmark figures",
    );
    await expect(page.locator(".hero__subheadline")).toContainText(
      "removed from the official python/typing results at our request",
    );
    await expect(page.locator(".hero__subheadline")).toContainText(
      "current conformance percentage is temporarily unknown",
    );
    await expect(page.locator(".hero__subheadline")).toContainText(
      "robustness and mutation testing",
    );
  });

  test("shows both withdrawn result notices and their detail links", async ({ page }) => {
    await expect(page.locator(".hero__proof .stat-card")).toHaveCount(2);
    await expect(page.locator(".hero__proof")).toContainText(
      "Current typing conformance",
    );
    await expect(page.locator(".hero__proof")).toContainText(
      "Published benchmark figures",
    );
    await expect(page.locator(".hero__proof")).toContainText(
      "removed from the official results table",
    );
    await expect(page.locator(".hero__proof-cta")).toContainText(
      "Both sets of figures are withdrawn",
    );
    await expect(
      page.locator('.hero__proof a[href*="github.com/python/typing"]'),
    ).toHaveCount(1);
    await expect(
      page.locator('.hero__proof-cta a[href="/docs/benchmarks/"]'),
    ).toBeVisible();

    const body = await page.locator("body").innerText();
    expect(body).not.toContain("scores 100%");
    expect(body).not.toContain("fastest we’ve benchmarked");
    expect(body).not.toContain("Strict by default");
    expect(body).not.toContain("Every diagnostic");
    expect(body).not.toContain("One binary");
  });

  test("publishes matching social and software metadata", async ({ page }) => {
    await expect(page.locator('meta[property="og:title"]')).toHaveAttribute(
      "content",
      "Basilisk — Python Type Checker & Language Server",
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
// pitch: same sections and the same integrity disclosures. These tests fail if
// one locale quietly retains a claim that the other has withdrawn.
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

  test("renders the same correction and withdrawn-result notices", async ({ page }) => {
    await page.goto("/zh/");

    await expect(page.locator("h1")).toHaveCount(1);
    await expect(page.locator("h1")).toContainText(
      "用 Rust 构建的开源 Python 类型检查器与语言服务器。",
    );

    await expect(page.locator(".hero__subheadline")).toContainText(
      "已撤回此前的符合性声明和公开的基准测试数据",
    );
    await expect(page.locator(".hero__subheadline")).toContainText(
      "当前符合性百分比暂时未知",
    );

    await expect(page.locator(".hero__proof .stat-card")).toHaveCount(2);
    await expect(page.locator(".hero__proof")).toContainText(
      "当前类型符合性",
    );
    await expect(page.locator(".hero__proof")).toContainText(
      "公开的基准测试数据",
    );
  });

  test("keeps its disclosure structure aligned with the English page", async ({
    page,
  }) => {
    await page.goto("/");
    const englishCards = await page.locator(".hero__proof .stat-card").count();
    await page.goto("/zh/");
    expect(await page.locator(".hero__proof .stat-card").count()).toBe(englishCards);

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
