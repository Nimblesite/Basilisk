import { defineConfig, devices } from "@playwright/test";

// Implements [WEBSITE-E2E-SMOKE]: navigation smoke tests on a desktop and a
// real phone viewport, run against the production build of `_site/`.
// See docs/specs/WEBSITE-E2E-SPEC.md.
//
// CI constraint [GITHUB-NO-ARTIFACTS]: no HTML report / trace / video / screenshot
// is ever uploaded. In CI we emit only the stdout `list` reporter; the HTML
// report and on-retry traces are reserved for local runs and stay on disk.
const isCI = !!process.env.CI;
const PORT = Number(process.env.PORT ?? 8099);
const baseURL = `http://127.0.0.1:${PORT}`;

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: true,
  forbidOnly: isCI,
  retries: isCI ? 1 : 0,
  reporter: isCI ? [["list"]] : [["list"], ["html", { open: "never" }]],
  use: {
    baseURL,
    trace: isCI ? "off" : "on-first-retry",
    screenshot: "off",
    video: "off",
  },
  projects: [
    { name: "desktop", use: { ...devices["Desktop Chrome"] } },
    { name: "mobile", use: { ...devices["Pixel 5"] } },
  ],
  webServer: {
    command: "node tests/static-server.js",
    url: baseURL,
    reuseExistingServer: !isCI,
    timeout: 30_000,
  },
});
