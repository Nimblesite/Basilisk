# Website: Navigation & End-to-End Smoke Tests {#WEBSITE-E2E}

## Purpose {#WEBSITE-E2E-PURPOSE}

Browser smoke tests for the Eleventy site (`website/`), run against the
**production build** of `_site/` on a desktop and a phone viewport, enforcing in
CI that a visitor can navigate the site.

## Smoke Coverage {#WEBSITE-E2E-SMOKE}

`website/tests/e2e/navigation.spec.ts`, driven by
`website/playwright.config.ts` (two projects: `desktop` = Desktop Chrome,
`mobile` = Pixel 5), served by `website/tests/static-server.js`. Run with
`npm run test:e2e` (`test:e2e:ui` locally). Asserts per viewport:

- **Top navigation resolves** — the home page links to Docs, Rules, Blog,
  Discord and GitHub (matched by `href`, so the check holds even where the nav
  is collapsed behind the hamburger on a phone).
- **Docs landing page loads** — `/docs/` renders with the docs sidebar present.
- **Desktop sidebar** — the docs sidebar is permanently visible and navigates
  between sections without any toggle.
- **Mobile docs submenu** — see [WEBSITE-MOBILE-DOCS-NAV].
- **Mobile top nav** — the hamburger reveals the collapsed top nav.

### CI constraint {#WEBSITE-E2E-NO-ARTIFACTS}

Per `[GITHUB-NO-ARTIFACTS]`, CI emits only the stdout `list` reporter — no
Playwright HTML report, trace, video or screenshot. Those (HTML report + on-retry
trace) are local-only and git-ignored (`website/.gitignore`). The website CI job
(`.github/workflows/ci.yml`) installs only Chromium, since both presets run on it.

## Mobile Docs Submenu Reachability {#WEBSITE-MOBILE-DOCS-NAV}

On phones (`max-width: 768px`) the docs section sidebar collapses. It **must**
remain reachable: the hamburger toggle (`mobile-menu.js`, which adds `.open` to
`.sidebar`) reveals it via `.sidebar.open { display: block; }` in
`website/src/assets/css/styles.css`, mirroring the `.nav-links.open` rule for the
top nav. Without that reveal rule the toggle has no effect and the per-section
submenu is unreachable on a phone (regression issue #186). Guard test
`"docs section submenu is reachable via the hamburger"` in
`website/tests/e2e/navigation.spec.ts` asserts the submenu is hidden by default,
becomes visible after the hamburger is tapped, and navigates to the section.
