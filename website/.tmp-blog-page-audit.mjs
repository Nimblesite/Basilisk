import { chromium } from "@playwright/test";

const target = "http://127.0.0.1:8099/blog/ai-agents-write-python-type-checking-guardrail/";
const viewports = [
  { name: "desktop", width: 1280, height: 720 },
  { name: "mobile", width: 375, height: 667 },
];

const browser = await chromium.launch();
const results = [];

for (const viewport of viewports) {
  const page = await browser.newPage({ viewport });
  const consoleErrors = [];
  const pageErrors = [];
  const failedRequests = [];
  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text());
  });
  page.on("pageerror", (error) => pageErrors.push(String(error)));
  page.on("requestfailed", (request) => {
    const url = new URL(request.url());
    if (url.hostname === "www.google-analytics.com" && url.pathname === "/g/collect") return;
    failedRequests.push(`${request.url()} — ${request.failure()?.errorText ?? "failed"}`);
  });

  const response = await page.goto(target, { waitUntil: "networkidle" });
  await page.locator(".blog-hero").evaluate((image) => image.decode());

  const pageMetrics = await page.evaluate(() => ({
    title: document.title,
    h1Count: document.querySelectorAll("h1").length,
    hasUpdatedDate: document.querySelector(".blog-post-meta")?.textContent?.includes("Updated") ?? false,
    bodyScrollWidth: document.documentElement.scrollWidth,
    viewportWidth: window.innerWidth,
    hero: (() => {
      const image = document.querySelector(".blog-hero");
      return image ? { naturalWidth: image.naturalWidth, naturalHeight: image.naturalHeight } : null;
    })(),
    jsonLdValid: [...document.querySelectorAll('script[type="application/ld+json"]')].every((node) => {
      try { JSON.parse(node.textContent ?? ""); return true; } catch { return false; }
    }),
    controls: [...document.querySelectorAll(".blog-post-tags a, .blog-post-footer .btn")].map((element) => {
      const rect = element.getBoundingClientRect();
      return { text: element.textContent?.trim(), width: rect.width, height: rect.height };
    }),
  }));

  let mobileMenu = null;
  if (viewport.name === "mobile") {
    const toggle = page.locator("#mobile-menu-toggle");
    const blogLink = page.locator('.nav-links a[href="/blog/"]');
    const hiddenBefore = await blogLink.isHidden();
    await toggle.click();
    const visibleAfter = await blogLink.isVisible();
    mobileMenu = { hiddenBefore, visibleAfter };
    await toggle.click();
  }

  const screenshot = `/tmp/basilisk-blog-audit-${viewport.name}.png`;
  await page.screenshot({ path: screenshot, fullPage: true });
  const failures = [];
  if (!response || response.status() !== 200) failures.push(`page status ${response?.status() ?? "none"}`);
  if (pageMetrics.h1Count !== 1) failures.push(`H1 count ${pageMetrics.h1Count}`);
  if (!pageMetrics.hasUpdatedDate) failures.push("updated date not visible");
  if (pageMetrics.bodyScrollWidth > pageMetrics.viewportWidth) failures.push(`horizontal overflow ${pageMetrics.bodyScrollWidth} > ${pageMetrics.viewportWidth}`);
  if (pageMetrics.hero?.naturalWidth !== 1200 || pageMetrics.hero?.naturalHeight !== 675) failures.push("hero dimensions mismatch");
  if (!pageMetrics.jsonLdValid) failures.push("invalid JSON-LD");
  if (pageMetrics.controls.some((control) => control.height < 48 || control.width < 48)) failures.push("post control below 48px target size");
  if (consoleErrors.length) failures.push("console errors");
  if (pageErrors.length) failures.push("page errors");
  if (failedRequests.length) failures.push("failed requests");
  if (mobileMenu && (!mobileMenu.hiddenBefore || !mobileMenu.visibleAfter)) failures.push("mobile menu did not toggle");

  results.push({ viewport, pageMetrics, mobileMenu, consoleErrors, pageErrors, failedRequests, screenshot, failures });
  await page.close();
}

await browser.close();
console.log(JSON.stringify(results, null, 2));
process.exitCode = results.some((result) => result.failures.length > 0) ? 1 : 0;
