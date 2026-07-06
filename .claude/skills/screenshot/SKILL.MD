## Generating CLI Screenshots (real `basilisk check` output)

Marketing/doc screenshots of CLI output must be **real output of the actual binary**, never hand-typed code fences or synthetic renders (those drift and are usually inaccurate). Canonical location: `website/src/assets/images/` (referenced as `/assets/images/<name>.png`). Rule screenshots are named after the code (`e0001.png` … `e0025.png`); the homepage demo pair is `cli-demo.png` (errors) + `cli-clean.png` (pass).

These are generated **automatically** — no manual Terminal.app / `screencapture` / ImageMagick. From `website/`:

```bash
npm run screenshots                       # regenerate every image
node screenshots/generate.mjs e0001 e0012 # a subset by name
BASILISK_BIN=../target/release/basilisk npm run screenshots  # pin the binary
```

`screenshots/generate.mjs` runs the real `basilisk check --color always` on each snippet in `screenshots/shots.mjs` (in a throwaway, neutrally-named temp dir so paths read `e0001.py:1:13` with no PII), **asserts the documented diagnostic actually fires** (the snippet→code pairing lives in the manifest, so a checker change can't silently ship a misleading image), then renders the genuine colored output in a faithful macOS Terminal window via Playwright. See `[WEBSITE-SCREENSHOTS]` (`docs/specs/WEBSITE-SCREENSHOTS-SPEC.md`).

To add or change a screenshot, edit `screenshots/shots.mjs` (snippet + expected code) and rerun — never craft images by hand. Committed PNGs are regenerated locally when CLI output changes; CI only verifies they render (`website/tests/e2e/screenshots.spec.ts`, `[WEBSITE-SCREENSHOTS-VERIFY]`), never captures, per `[GITHUB-NO-ARTIFACTS]`. After regenerating, rebuild (`npm run build`) and confirm the images copy to `_site/assets/images/`. VSIX integration tests capture their own editor screenshots to the gitignored `vscode-extension/.screenshots/` (never committed, never a CI artifact).



-------




## VS Code editor screenshots (`vscode-*.png`)

Real screenshots of the extension running in VS Code (diagnostics, hover, quick-fix, activity panel) are captured automatically — see `[VSIX-EDITOR-SCREENSHOTS]` (`docs/specs/VSIX-EDITOR-SCREENSHOTS-SPEC.md`). From `vscode-extension/`, after `cargo build -p basilisk-cli -p basilisk-profiler-helper`:

```bash
npm run screenshots:editor
```

This stages the binary into the dev extension, copies `shipwright.json` (gitignored dev artifacts), launches the **headed** "Editor screenshots" suite with `BASILISK_SCREENSHOTS=1`, and a dependency-free CDP sidecar (`screenshot-watcher.mjs`, Node's built-in WebSocket — no Playwright) captures the window to `website/src/assets/images/vscode-*.png`. The suite is a no-op without the env flag, so normal `npm test` never opens these windows. To add one, add a `test(...)` that makes the feature visible and calls `takeWindowScreenshot(...)`. As with the CLI shots, PNGs are committed and regenerated locally; CI only verifies they render (`website/tests/e2e/screenshots.spec.ts`), per `[GITHUB-NO-ARTIFACTS]`.
