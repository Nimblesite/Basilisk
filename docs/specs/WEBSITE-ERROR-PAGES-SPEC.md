# Website: Per-Diagnostic Error Pages {#WEBSITE-ERROR-PAGES}

> **SUPERSEDED — historical record.** The behaviour below no longer ships. Basilisk is unlisted and the site collapsed to one statement, with every retired URL redirecting to it ([WITHDRAWAL-UNLIST](DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-UNLIST)). This spec is kept as the record of what was built, not as a description of the site or a plan to rebuild it.

## Purpose {#WEBSITE-ERROR-PAGES-PURPOSE}

Every diagnostic Basilisk reports ends with a deep link, e.g.
`= see: https://www.basilisk-python.dev/errors/BSK-0001`. That URL is baked into
each rule (`docs_url` on the `ErrorCode` in
`crates/basilisk-checker/src/rules/*.rs`). This spec generates a landing page for
**every** diagnostic code at `/errors/BSK-XXXX/`, built from the checker source so
pages never drift from the diagnostics the binary emits.

## Data generation {#WEBSITE-ERROR-PAGES-DATA}

`scripts/gen_rules_reference.py --data` extracts one record per code from the
`//! BSK-XXXX: …` doc-comment header — and the prose/`​```python` examples beneath
it — on each rule module, and writes `website/src/_data/rules.json`:

```json
{ "code", "severity", "summary", "summaryHtml", "body": [{type:"text"|"code"}], "group", "docsUrl", "references": [{label, url}] }
```

`docsUrl` is read from the rule's own `docs_url` literal, so the page URL and CLI
link are identical. The same data drives the `/docs/rules/` reference table and
the headline counts (`_data/ruleStats.js`, `_data/ruleGroups.js`) — prose, table,
and pages share one source.

### Canonical references {#WEBSITE-ERROR-PAGES-REFERENCES}

Every record carries `references` — the canonical upstream documentation for
the diagnostic, in order: the rule's chapter of the maintained
[typing spec](https://typing.python.org/en/latest/spec/index.html) (chapter
titles and filenames mirror that index verbatim), then the PEP(s) that chapter
incorporates merged with every `PEP NNN` the rule's own doc comment cites
(linked to `https://peps.python.org/pep-NNNN/`), then any language-reference
link for rules governed by Python semantics rather than the typing spec (the
`names_*` unbound/undefined checks link the execution model's Naming and
binding section). The prefix→chapter and prefix→PEP tables live in
`scripts/gen_rules_reference.py` (`SPEC_CHAPTER_BY_PREFIX`, `PEPS_BY_PREFIX`);
PEP links are labelled `PEP NNN` only, so no title can drift from
peps.python.org. `inline_html` additionally links every `PEP NNN` mention in
rendered summary/body prose. Opt-in house rules (`BSK-` codes) map through
`REFERENCE_PREFIX_BY_BSK_CODE` to the chapter/PEPs documenting the mechanism
they police (annotation rules → Type annotations; BSK-0152 → PEP 561;
BSK-0011/0012 → PEP 621; BSK-0013 → uv's lockfile docs), plus any PEP their
own docs cite (BSK-0025 → PEP 698). The suppression rules (BSK-0060..0063)
police Basilisk's own directives and deliberately list nothing. `error.njk`
renders the list as a "Canonical documentation" section on every
`/errors/<code>/` page.

### Drift guard {#WEBSITE-ERROR-PAGES-DRIFT}

The CI website job regenerates the data and `diff`s it against the committed
`rules.json`, failing on any difference; the change-classifier treats edits under
`crates/basilisk-checker/src/rules/` as website changes, so adding/renaming a rule
re-runs the guard. A new rule cannot ship without its page.

## Pages {#WEBSITE-ERROR-PAGES-PAGES}

`website/src/errors/error.njk` paginates `rules` (size 1) to emit
`/errors/{{ code }}/` for every record. Each page shows the code, a severity
badge, the summary, the doc-comment body (text + code blocks), a worked
`basilisk check` screenshot when one exists, the canonical-documentation links
([WEBSITE-ERROR-PAGES-REFERENCES]), how-to-handle guidance, and the canonical
`docsUrl`. `website/src/errors/index.njk` is a grouped, browsable
directory of all codes. Pages deliberately omit `eleventyNavigation` so the 160
entries never flood the docs sidebar.

### Worked examples {#WEBSITE-ERROR-PAGES-EXAMPLES}

`_data/examples.js` maps a code to its demonstrating screenshot by reading the
manifest's `expect` field ([WEBSITE-SCREENSHOTS-MANIFEST]), so the mapping is
correct even where image stem and code differ (e.g. `e0011.png` demonstrates
`BSK-0014`). Adding a verified shot to `screenshots/shots.mjs` and regenerating
gives a code's page a real-output example.

## Verification {#WEBSITE-ERROR-PAGES-VERIFY}

`website/tests/e2e/errors.spec.ts` (same Playwright config as [WEBSITE-E2E-SMOKE])
asserts: every `rules.json` code has a built `/errors/<code>/index.html` (≥ 155 —
every CLI-linked code); a sampled page renders its code, title and severity badge;
the `/errors/` index links every code; and every worked-example screenshot decodes
on its page. With the drift guard, this closes the loop checker source → data →
page → render.
