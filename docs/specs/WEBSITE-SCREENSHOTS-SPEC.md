# Website: Automated CLI Screenshots {#WEBSITE-SCREENSHOTS}

## Purpose {#WEBSITE-SCREENSHOTS-PURPOSE}

The site embeds real `basilisk check` output as PNGs: the homepage before/after
demo (`cli-demo.png`, `cli-clean.png`) and one image per documented rule
(`e0001.png` … `e0025.png`, referenced from `website/src/docs/rules/*.md`).

A single command runs the real binary on each documented snippet and renders its
genuine coloured output inside a faithful macOS Terminal window — no manual
capture, no PII, with a guard that every snippet still triggers the diagnostic it
documents. The bytes shown are exactly what `basilisk check --color always`
prints; per project rule, these are never hand-typed code fences or synthetic
renders.

## Generator {#WEBSITE-SCREENSHOTS-GENERATE}

Run from `website/`:

```bash
npm run screenshots            # regenerate every image
node screenshots/generate.mjs e0001 e0012   # regenerate a subset by name
BASILISK_BIN=../target/release/basilisk npm run screenshots   # pin the binary
```

`website/screenshots/generate.mjs` ([WEBSITE-SCREENSHOTS-GENERATE]) for each shot:

1. Writes the snippet into a throwaway, neutrally-named temp dir
   (`basilisk-demo-*`) so diagnostic paths read `e0001.py:1:13` — relative and
   PII-free.
2. Runs `basilisk check --color always <file>` there. A non-zero exit is expected;
   stdout is read off the thrown error.
3. Asserts the documented diagnostic code is present
   ([WEBSITE-SCREENSHOTS-MANIFEST]). If absent the image is **not** written and
   generation fails loudly, so a checker change can never silently ship a
   misleading screenshot.
4. Renders the output in a Terminal window ([WEBSITE-SCREENSHOTS-CHROME]) via
   Playwright (Chromium, `deviceScaleFactor: 2`) and writes
   `website/src/assets/images/<name>.png`.

The binary defaults to `basilisk` on `PATH`; override with `BASILISK_BIN`. The
generator is **not** run in CI — PNGs are committed and regenerated locally when
CLI output changes; CI only verifies they render ([WEBSITE-SCREENSHOTS-VERIFY]),
per `[GITHUB-NO-ARTIFACTS]`.

## Manifest {#WEBSITE-SCREENSHOTS-MANIFEST}

`website/screenshots/shots.mjs` is the single source of truth for every CLI
screenshot. Each entry pairs the **exact** docs snippet with the code it must
produce (`expect`). Snippets isolate one rule — e.g. `e0001` keeps the `-> str`
return annotation so only `BSK-E0001` fires, not `BSK-E0002`; `e0011` documents
the explicit-`Any` check and asserts `BSK-W0014`; the home shots assert the
summary line (`Found 6 diagnostics`, `No issues found`). The assertion lives in
code and runs on every regeneration.

## Terminal chrome {#WEBSITE-SCREENSHOTS-CHROME}

### ANSI conversion {#WEBSITE-SCREENSHOTS-ANSI}

`website/screenshots/terminal.mjs` builds the window HTML; `ansi.mjs` converts the
binary's ANSI escapes to themed HTML. The binary emits a fixed SGR set — reset,
bold, and bold foreground red (errors), yellow (warnings), blue (gutters), cyan
(labels) — modelled exactly. The window is a 120-column macOS Terminal
("basilisk-demo — -zsh") on the default dark profile (`rgb(30, 30, 30)`).

## Render verification {#WEBSITE-SCREENSHOTS-VERIFY}

`website/tests/e2e/screenshots.spec.ts` runs under the same Playwright config as
[WEBSITE-E2E-SMOKE] (desktop + mobile, against the production `_site/` build),
importing the manifest so it cannot drift from the generated set. Asserts:

- The rule docs (`/docs/rules/missing-annotations/`, `/docs/rules/type-safety/`)
  embed **every** `e00*` screenshot, each decoding to non-zero pixels.
- The homepage demo embeds both `cli-demo.png` and `cli-clean.png`; `cli-demo`
  (visible) renders immediately, `cli-clean` after its tab panel is revealed (it
  is lazy-loaded, initially hidden).
- No `/assets/images/*.png` request returns a non-200 status.

A missing, zero-byte, or unreferenced screenshot fails CI. As with
[WEBSITE-E2E-NO-ARTIFACTS], CI emits only the stdout `list` reporter — no report,
trace, video or capture.
