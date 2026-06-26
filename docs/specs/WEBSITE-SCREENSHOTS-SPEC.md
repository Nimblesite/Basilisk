# Website: Automated CLI Screenshots {#WEBSITE-SCREENSHOTS}

**Version**: 0.1.0
**Status**: Active
**License**: MIT

---

## Purpose {#WEBSITE-SCREENSHOTS-PURPOSE}

The marketing/docs site embeds real `basilisk check` output as PNGs: the homepage
before/after demo (`cli-demo.png`, `cli-clean.png`) and one image per documented
rule (`e0001.png` … `e0025.png`, referenced from `website/src/docs/rules/*.md`).

Historically these were produced by a manual macOS process — drive Terminal.app,
`screencapture` a specific window by id, then crop with ImageMagick. That process
is slow, non-reproducible, easy to get wrong (snippets that don't actually trigger
the rule they claim), and leaks environment details (home dir, username) if run
carelessly.

This spec defines a **fully automated, reproducible** replacement. A single
command runs the real binary on each documented snippet and renders its genuine,
coloured output inside a faithful macOS Terminal window — no manual capture, no
PII, and a built-in guard that every snippet still triggers the diagnostic it
documents.

The images remain **real binary output**, honouring the project rule that
marketing/doc screenshots are never hand-typed code fences or synthetic renders:
the bytes shown are exactly what `basilisk check --color always` prints.

## Generator {#WEBSITE-SCREENSHOTS-GENERATE}

Run from `website/`:

```bash
npm run screenshots            # regenerate every image
node screenshots/generate.mjs e0001 e0012   # regenerate a subset by name
BASILISK_BIN=../target/release/basilisk npm run screenshots   # pin the binary
```

`website/screenshots/generate.mjs` ([WEBSITE-SCREENSHOTS-GENERATE]) for each shot:

1. Writes the snippet into a throwaway, neutrally-named temp directory
   (`basilisk-demo-*`) so diagnostic paths read `e0001.py:1:13` — relative and
   PII-free, satisfying the "no PII / clean prompt" rule without a custom shell.
2. Runs `basilisk check --color always <file>` in that directory. A non-zero exit
   (any file with diagnostics) is expected; stdout is read off the thrown error.
3. Asserts the documented diagnostic code is present — see
   [WEBSITE-SCREENSHOTS-MANIFEST]. If it is absent the image is **not** written and
   generation fails loudly, so a checker behaviour change can never silently ship a
   misleading screenshot.
4. Renders the output in a Terminal window ([WEBSITE-SCREENSHOTS-CHROME]) via
   Playwright (Chromium, `deviceScaleFactor: 2` for crisp Retina output) and writes
   `website/src/assets/images/<name>.png`.

The binary defaults to `basilisk` on `PATH`; override with `BASILISK_BIN`. The
generator is **not** run in CI — the PNGs are committed, and regenerated locally
when the CLI output changes. CI only verifies they render
([WEBSITE-SCREENSHOTS-VERIFY]), keeping with `[GITHUB-NO-ARTIFACTS]`.

## Manifest {#WEBSITE-SCREENSHOTS-MANIFEST}

`website/screenshots/shots.mjs` is the single source of truth for every CLI
screenshot. Each entry pairs the **exact** snippet shown in the docs with the
diagnostic code that snippet must produce (`expect`). The snippets are crafted to
isolate one rule — e.g. `e0001` keeps the `-> str` return annotation so only
`BSK-E0001` fires, not `BSK-E0002`. `e0011` documents the explicit-`Any` check and
asserts the warning code the binary actually emits (`BSK-W0014`); the home shots
assert the summary line (`Found 6 diagnostics`, `No issues found`).

This is the automated form of the manual-process rule "many rule examples do NOT
trigger the rule they claim — always confirm the exact target code appears": the
assertion lives in code and runs on every regeneration.

## Terminal chrome {#WEBSITE-SCREENSHOTS-CHROME}

`website/screenshots/terminal.mjs` builds the window HTML and `ansi.mjs` converts
the binary's ANSI escapes to themed HTML. The binary emits a small, fixed SGR set —
reset, bold, and bold foreground red (errors), yellow (warnings), blue (gutters),
cyan (labels) — modelled exactly rather than as a general terminal. The window is a
120-column macOS Terminal ("basilisk-demo — -zsh") on the default dark profile
(`rgb(30, 30, 30)`), matching the original captures so regenerated images are
visually identical.

## Render verification {#WEBSITE-SCREENSHOTS-VERIFY}

`website/tests/e2e/screenshots.spec.ts` runs under the same Playwright config as
[WEBSITE-E2E-SMOKE] (desktop + mobile, against the production `_site/` build). It
imports the manifest so it can never drift from the generated set, and asserts:

- The rule docs (`/docs/rules/missing-annotations/`, `/docs/rules/type-safety/`)
  embed **every** `e00*` screenshot and each decodes to non-zero pixels.
- The homepage before/after demo embeds both `cli-demo.png` and `cli-clean.png`;
  `cli-demo` (visible) renders immediately, and `cli-clean` renders after its tab
  panel is revealed (it is lazy-loaded and initially hidden).
- No `/assets/images/*.png` request returns a non-200 status.

A missing, zero-byte, or unreferenced screenshot fails CI rather than shipping a
broken image. As with [WEBSITE-E2E-NO-ARTIFACTS], CI emits only the stdout `list`
reporter — no report, trace, video or capture is uploaded.
