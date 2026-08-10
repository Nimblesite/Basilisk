# Website: withdrawal-contract end-to-end tests {#WEBSITE-E2E}

## Purpose {#WEBSITE-E2E-PURPOSE}

Browser tests for the Eleventy site (`website/`), run against the **production build** of `_site/` on a desktop and a phone viewport. The site publishes one thing — the withdrawal statement ([WITHDRAWAL-COPY-FULL](DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-COPY-FULL)) — so these tests enforce that contract rather than navigation: the published words are the approved words, no retired URL 404s, and nothing forbidden survives anywhere in the build.

## Coverage {#WEBSITE-E2E-WITHDRAWAL}

`website/tests/e2e/withdrawal.spec.ts`, driven by `website/playwright.config.ts` (two projects: `desktop` = Desktop Chrome, `mobile` = iPhone SE 3rd generation emulated in Chromium, 375 × 667), served by `website/tests/static-server.js`. Run with `npm run test:e2e` (`test:e2e:ui` locally).

- **The statement is the approved copy** — the home page renders every paragraph of `withdrawal.full`, in order, from `website/src/_data/withdrawal.json`. That file is generated from the messaging spec by `scripts/gen_withdrawal_copy.py`, so a test failure means the page drifted from the spec, and a `--check` failure means the data did.
- **The four load-bearing facts appear** — incorrect results, removal from the `python/typing` results, the damage not being scoped to a known set of rules, and a wrong tool being worse than useless. Asserted on visible text, so deleting a paragraph fails even if the copy file still contains it.
- **Every retired URL redirects to the statement** — each entry in `website/src/_data/retiredUrls.json` has a built page, and that page is a redirect stub, not a second copy of the message ([WITHDRAWAL-UNLIST](DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-UNLIST)). A representative URL per family (`/docs/`, `/docs/rules/`, `/errors/BSK-XXXX/`, `/blog/`, `/playground/`, `/zh/docs/…`) is driven in a real browser and asserted to land on `/`; the served bytes are asserted to carry `noindex`, the canonical link to `/`, and the meta refresh. GitHub Pages has no redirect table, so the redirect is a meta refresh — which is why the test follows it rather than trusting a status code. `/errors/` matters most: shipped binaries print those links, and a 404 there strands a user with a diagnostic and no explanation.
- **Only the statement is indexable** — every built page except `/` carries `noindex`, and the sitemap lists `/` alone. 296 redirect stubs must not be offered to search engines as pages.
- **Nothing forbidden survives** — the whole build is scanned for anything [WITHDRAWAL-PROHIBITED](DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-PROHIBITED) bars: a percentage figure, install instructions for any channel, a marketplace or PyPI link, a competitor name, a benchmark claim, a `BSK-` rule code. This is the test that catches a page nobody remembered to delete.
- **The apology is linked, never quoted** — the statement links it; no page reproduces its wording. The redirect stubs carry no copy at all, so they carry no link either.

### CI constraint {#WEBSITE-E2E-NO-ARTIFACTS}

Per `[GITHUB-NO-ARTIFACTS]`, CI emits only the stdout `list` reporter — no Playwright HTML report, trace, video or screenshot. Those (HTML report + on-retry trace) are local-only and git-ignored (`website/.gitignore`). The website CI job (`.github/workflows/ci.yml`) installs only Chromium, since both presets run on it.
