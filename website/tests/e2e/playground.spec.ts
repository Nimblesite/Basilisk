import { test, expect } from "@playwright/test";

// Tests [WASM-PLAN-SITE]: production Monaco loads and invokes the real checker.
test.describe("WASM playground", () => {
  test("checks Python and links diagnostics to rule pages", async ({ page }) => {
    await page.goto("/playground/");
    await expect(page.locator(".monaco-editor")).toBeVisible({ timeout: 20_000 });
    await page.locator("#check-code").click();
    await expect(page.locator("#engine-status")).toContainText("Engine ready", { timeout: 30_000 });
    await expect(page.locator("#diagnostic-count")).not.toHaveText("—");
    await expect(page.locator("#diagnostics a[href^='/errors/']").first()).toBeVisible();
  });

  test("keeps editor and diagnostics visible on phones", async ({ page, isMobile }) => {
    test.skip(!isMobile, "phone-only responsive assertion");
    await page.goto("/playground/");
    await expect(page.locator("#editor")).toBeVisible();
    await expect(page.locator(".diagnostics-pane")).toBeVisible();
  });

  test("fills the viewport without document scrolling", async ({ page }) => {
    await page.goto("/playground/");
    const dimensions = await page.evaluate(() => {
      const shell = document.querySelector(".playground-shell")?.getBoundingClientRect();
      return {
        viewportHeight: window.innerHeight,
        documentHeight: document.documentElement.scrollHeight,
        shellLeft: shell?.left,
        shellRight: shell?.right,
        shellBottom: shell?.bottom,
        viewportWidth: window.innerWidth,
      };
    });
    expect(dimensions.documentHeight).toBe(dimensions.viewportHeight);
    expect(dimensions.shellLeft).toBe(0);
    expect(dimensions.shellRight).toBe(dimensions.viewportWidth);
    expect(dimensions.shellBottom).toBe(dimensions.viewportHeight);
  });
});
