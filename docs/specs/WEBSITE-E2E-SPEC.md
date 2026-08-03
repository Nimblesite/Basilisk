# Website: Navigation & End-to-End Smoke Tests {#WEBSITE-E2E}

## Purpose {#WEBSITE-E2E-PURPOSE}

Browser smoke tests for the Eleventy site (`website/`), run against the
**production build** of `_site/` on a desktop and a phone viewport, enforcing in
CI that a visitor can navigate the site.

## Smoke Coverage {#WEBSITE-E2E-SMOKE}

`website/tests/e2e/navigation.spec.ts` and `website/tests/e2e/homepage.spec.ts`, driven by
`website/playwright.config.ts` (two projects: `desktop` = Desktop Chrome,
`mobile` = iPhone SE 3rd generation emulated in Chromium, 375 × 667), served by
`website/tests/static-server.js`. Run with
`npm run test:e2e` (`test:e2e:ui` locally). Asserts per viewport:

- **Top navigation resolves** — the home page links to Docs, Rules, Blog,
  Discord and GitHub (matched by `href`, so the check holds even where the nav
  is collapsed behind the hamburger on a phone).
- **Docs landing page loads** — `/docs/` renders with the docs sidebar present.
- **Desktop sidebar** — the docs sidebar is permanently visible and navigates
  between sections without any toggle.
- **Mobile docs submenu** — see [WEBSITE-MOBILE-DOCS-NAV].
- **Mobile top nav** — the hamburger reveals the collapsed top nav.
- **Homepage positioning** — the title, H1 and opening answer identify Basilisk
  as a Python type checker and language server, with only measured, linked proof.
- **Headline claims carry their proof** — the hero's two comparative claims (sole
  perfect official conformance score, and lowest median cold full-file CLI time)
  each sit beside the link that grades them: the official `python/typing` results
  and the published benchmark. False positives are asserted at 0 — a ratchet per
  [CHKARCH-CONFORMANCE] — while the caught-error count is left open, since
  upstream adds test cases over time.
- **Homepage mobile usability** — no horizontal overflow and visible calls to
  action retain a minimum 48 px touch target on the iPhone SE viewport.

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
