# Website: Per-Diagnostic Error Pages {#WEBSITE-ERROR-PAGES}

**Version**: 0.1.0
**Status**: Active
**License**: MIT

---

## Purpose {#WEBSITE-ERROR-PAGES-PURPOSE}

Every diagnostic Basilisk reports ends with a deep link:

```
   = see: https://www.basilisk-python.dev/errors/BSK-E0001
```

That URL is baked into each rule (`docs_url` on the `ErrorCode` in
`crates/basilisk-checker/src/rules/*.rs`). Before this spec **none of those pages
existed** — every "learn more" link the CLI printed (155 codes) was a 404.

This spec defines a generated landing page for **every** diagnostic code at
`/errors/BSK-XXXX/`, built from the checker source so the pages can never drift
from the diagnostics the binary emits. It turns the tool's own output into a
navigable, explained reference.

## Data generation {#WEBSITE-ERROR-PAGES-DATA}

`scripts/gen_rules_reference.py --data` extracts one record per code from the
`//! BSK-XXXX: …` doc-comment header — and the prose/`​```python` examples beneath
it — on each rule module, and writes `website/src/_data/rules.json`:

```json
{ "code", "severity", "summary", "summaryHtml", "body": [{type:"text"|"code"}], "group", "docsUrl" }
```

`docsUrl` is read from the rule's own `docs_url` literal, so the page URL and the
CLI link are guaranteed identical. The same data drives the complete reference
table on `/docs/rules/` and the headline counts (`_data/ruleStats.js`,
`_data/ruleGroups.js`), so the prose, the table, and the pages share one source.

### Drift guard {#WEBSITE-ERROR-PAGES-DRIFT}

The CI website job regenerates the data and `diff`s it against the committed
`rules.json`, failing if they differ; the change-classifier treats edits under
`crates/basilisk-checker/src/rules/` as website changes so adding or renaming a
rule re-runs the guard. A new rule therefore cannot ship without its page.

## Pages {#WEBSITE-ERROR-PAGES-PAGES}

`website/src/errors/error.njk` paginates `rules` (size 1) to emit
`/errors/{{ code }}/` for every record. Each page shows the code, a severity
badge, the summary, the doc-comment body (text + code blocks), a worked
`basilisk check` screenshot when one exists, how-to-handle guidance, and the
canonical `docsUrl`. `website/src/errors/index.njk` is a grouped, browsable
directory of all codes. Pages deliberately omit `eleventyNavigation` so the 160
entries never flood the docs sidebar.

### Worked examples {#WEBSITE-ERROR-PAGES-EXAMPLES}

`_data/examples.js` maps a code to the screenshot that demonstrates it by reading
the screenshot manifest's `expect` field ([WEBSITE-SCREENSHOTS-MANIFEST]) — so the
mapping is correct even where the image stem and code differ (e.g. `e0011.png`
demonstrates `BSK-W0014`). Adding a verified shot to `screenshots/shots.mjs` and
regenerating is all it takes for a code's page to gain a real-output example.

## Verification {#WEBSITE-ERROR-PAGES-VERIFY}

`website/tests/e2e/errors.spec.ts` (same Playwright config as [WEBSITE-E2E-SMOKE])
asserts: every code in `rules.json` has a built `/errors/<code>/index.html`
(≥ 155 — every CLI-linked code); a sampled page renders its code, title and
severity badge; the `/errors/` index links every code; and every worked-example
screenshot decodes on its page. With the drift guard above, this closes the loop
from checker source → data → page → render.
