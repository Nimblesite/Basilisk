# Website: Navigation & End-to-End Smoke Tests {#WEBSITE-E2E}

**Version**: 0.1.0
**Status**: Active
**License**: MIT

---

## Purpose {#WEBSITE-E2E-PURPOSE}

The marketing/docs site (`website/`) is a statically generated Eleventy build.
Before this spec, CI built the site but never exercised it, so navigation
regressions shipped silently. This spec defines browser smoke tests that run the
**production build** of `_site/` on both a desktop and a real phone viewport, so
the core "can a visitor actually get around the site" guarantees are enforced in
CI.

## Smoke Coverage {#WEBSITE-E2E-SMOKE}

Implemented by `website/tests/e2e/navigation.spec.ts`, driven by
`website/playwright.config.ts` (two projects: `desktop` = Desktop Chrome,
`mobile` = Pixel 5) and served by the dependency-free static server
`website/tests/static-server.js`. Run with `npm run test:e2e`
(`test:e2e:ui` locally).

The suite asserts, on each relevant viewport:

- **Top navigation resolves** — the home page links to Docs, Rules, Blog and
  GitHub (matched by `href`, so the check holds even where the nav is collapsed
  behind the hamburger on a phone).
- **Docs landing page loads** — `/docs/` renders with the docs sidebar present.
- **Desktop sidebar** — the docs sidebar is permanently visible and navigates
  between sections without any toggle.
- **Mobile docs submenu** — see [WEBSITE-MOBILE-DOCS-NAV].
- **Mobile top nav** — the hamburger reveals the collapsed top nav.

### CI constraint {#WEBSITE-E2E-NO-ARTIFACTS}

Per `[GITHUB-NO-ARTIFACTS]`, the CI run emits only the stdout `list` reporter.
No Playwright HTML report, trace, video or screenshot is produced or uploaded —
those (HTML report + on-retry trace) are reserved for local runs and are
git-ignored (`website/.gitignore`). The website CI job
(`.github/workflows/ci.yml`) installs only the Chromium browser, since both
presets run on Chromium.

## Mobile Docs Submenu Reachability {#WEBSITE-MOBILE-DOCS-NAV}

On phones (`max-width: 768px`) the docs section sidebar collapses so the article
body is readable. It **must** remain reachable: the hamburger toggle
(`mobile-menu.js`, which adds `.open` to `.sidebar`) reveals it via the CSS rule
`.sidebar.open { display: block; }` in `website/src/assets/css/styles.css`,
mirroring the existing `.nav-links.open` rule for the top nav.

Without that reveal rule the JS toggle has no visual effect and the per-section
submenu (Installation, Quick Start, Configuration, Diagnostics, Reference, …) is
unreachable on a phone — the regression tracked as issue #186. The guard test is
`"docs section submenu is reachable via the hamburger"` in
`website/tests/e2e/navigation.spec.ts`: it asserts the submenu is hidden by
default, becomes visible after the hamburger is tapped, and navigates to the
chosen section.
