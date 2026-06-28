import { test, expect } from "@playwright/test";

// Implements [WEBSITE-E2E-SMOKE]: navigation smoke tests on desktop + phone.
// Tests [WEBSITE-E2E] / [WEBSITE-E2E-PURPOSE]: this is the end-to-end smoke suite
// that exercises the production `_site/` build so navigation regressions fail CI.
// See docs/specs/WEBSITE-E2E-SPEC.md.

// Top-level nav links exist and resolve on every viewport (the markup is in the
// DOM on mobile too — it is just collapsed behind the hamburger).
const TOP_NAV = [
  { name: "Docs", href: "/docs/" },
  { name: "Rules", href: "/docs/rules/" },
  { name: "Blog", href: "/blog/" },
  { name: "GitHub", href: "https://github.com/Nimblesite/Basilisk" },
];

test.describe("top navigation", () => {
  // Viewport-agnostic: on a phone the nav is collapsed (display:none), so we
  // match the anchors by href rather than by visible role.
  test("home page links to Docs, Rules, Blog and GitHub", async ({ page }) => {
    await page.goto("/");
    for (const { name, href } of TOP_NAV) {
      const link = page.locator(`.nav-links a[href="${href}"]`);
      await expect(link).toHaveCount(1);
      await expect(link).toHaveText(new RegExp(name));
    }
  });

  test("Docs nav link loads the docs landing page", async ({ page }) => {
    await page.goto("/docs/");
    await expect(page.locator("#docs-sidebar")).toBeAttached();
    await expect(page).toHaveTitle(/.+/);
  });
});

test.describe("docs sidebar (desktop)", () => {
  test.skip(
    ({ isMobile }) => !!isMobile,
    "desktop layout keeps the sidebar permanently visible",
  );

  test("sidebar is visible with no toggle and navigates between sections", async ({
    page,
  }) => {
    await page.goto("/docs/installation/");
    const sidebar = page.locator("#docs-sidebar");
    await expect(sidebar).toBeVisible();

    await sidebar.getByRole("link", { name: "Quick Start" }).click();
    await expect(page).toHaveURL(/\/docs\/quick-start\/$/);
  });
});

test.describe("docs sidebar (mobile)", () => {
  test.skip(
    ({ isMobile }) => !isMobile,
    "this guards the phone-only collapsed-sidebar behaviour",
  );

  // Guards issue #186 — [WEBSITE-MOBILE-DOCS-NAV]. On a phone the docs section
  // submenu must be reachable: it starts collapsed, and the hamburger reveals it.
  test("docs section submenu is reachable via the hamburger", async ({ page }) => {
    await page.goto("/docs/installation/");

    const submenuLink = page
      .locator("#docs-sidebar")
      .getByRole("link", { name: "Quick Start" });

    // Collapsed by default so the doc body is readable without a wall of links.
    await expect(submenuLink).toBeHidden();

    // The hamburger must reveal the docs submenu (not just the top nav).
    await page.locator("#mobile-menu-toggle").click();
    await expect(submenuLink).toBeVisible();

    // …and it actually navigates to the chosen section.
    await submenuLink.scrollIntoViewIfNeeded();
    await submenuLink.click();
    await expect(page).toHaveURL(/\/docs\/quick-start\/$/);
  });

  test("hamburger also opens the top nav", async ({ page }) => {
    await page.goto("/docs/installation/");
    const docsLink = page
      .locator(".nav-links")
      .getByRole("link", { name: "Blog", exact: true });

    await expect(docsLink).toBeHidden();
    await page.locator("#mobile-menu-toggle").click();
    await expect(docsLink).toBeVisible();
  });
});
