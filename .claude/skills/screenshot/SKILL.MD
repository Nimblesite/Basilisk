---
name: screenshot
description: Regenerate real product screenshots — either CLI screenshots (actual `basilisk check` output rendered in a Terminal window) or VS Code editor screenshots (the extension showing diagnostics, hover, quick-fix, activity panel). Use when the user asks to regenerate, update, or add marketing/docs screenshots. Requires a target argument, `cli` or `vsix`.
argument-hint: "cli|vsix [cli-shot-names...]"
arguments: target
allowed-tools: Bash(npm run *), Bash(node screenshots/*), Bash(cargo build *)
---

# Regenerate product screenshots

Screenshots must be **real output of the actual binary/extension**, never hand-typed code fences or synthetic renders (those drift and mislead). Both suites write to the canonical location `website/src/assets/images/` and are committed; CI only verifies they render (`website/tests/e2e/screenshots.spec.ts`), never captures, per `[GITHUB-NO-ARTIFACTS]`.

## Step 1 — Resolve the target

The target is `$target` (`$ARGUMENTS`).

- `cli` → CLI screenshots. Go to **Step 2a**.
- `vsix` (or `vscode`) → VS Code editor screenshots. Go to **Step 2b**.
- **Empty or anything else** → STOP and ask the user which target they want: `cli` or `vsix`. Do not guess — the two paths capture different images with different tooling.

## Step 2a — CLI screenshots

Real `basilisk check --color always` output rendered in a faithful macOS Terminal window via Playwright. See `[WEBSITE-SCREENSHOTS]` (`docs/specs/WEBSITE-SCREENSHOTS-SPEC.md`).

Images: rule shots `e0001.png`…`e0025.png` (named after the code), plus the homepage pair `cli-demo.png` (errors) + `cli-clean.png` (pass). Snippet→expected-code pairings live in `website/screenshots/shots.mjs`; `generate.mjs` **asserts the documented diagnostic actually fires**, so a checker change can't silently ship a misleading image.

From `website/`:

```bash
npm run screenshots                          # regenerate every image
node screenshots/generate.mjs e0001 e0012    # only the named shots
BASILISK_BIN=../target/release/basilisk npm run screenshots  # pin the binary
```

If the user passed extra args after `cli` (e.g. `e0001 e0012`), pass them through to `node screenshots/generate.mjs <names>` to regenerate only those. Otherwise regenerate all.

To **add or change** a shot, edit `website/screenshots/shots.mjs` (snippet + expected code) and rerun — never craft images by hand. After regenerating, run `npm run build` and confirm the images copied to `website/_site/assets/images/`.

## Step 2b — VS Code editor screenshots

Headed VS Code capture of the running extension (diagnostics, hover, quick-fix, activity panel) via a dependency-free CDP sidecar (`vscode-extension/screenshot-watcher.mjs`). See `[VSIX-EDITOR-SCREENSHOTS]` (`docs/specs/VSIX-EDITOR-SCREENSHOTS-SPEC.md`).

Images: `vscode-*.png` in `website/src/assets/images/`.

The suite is a no-op without the `BASILISK_SCREENSHOTS=1` flag (which `npm run screenshots:editor` sets), so normal `npm test` never opens these windows.

First build the binaries the extension stages, then run the suite. From `vscode-extension/`:

```bash
cargo build -p basilisk-cli -p basilisk-profiler-helper
npm run screenshots:editor
```

`screenshots:editor` stages the freshly built binary into the dev extension, copies `shipwright.json`, launches the **headed** "Editor screenshots" suite, and the sidecar captures each window to `website/src/assets/images/vscode-*.png`.

To **add** an editor screenshot, add a `test(...)` that makes the feature visible and calls `takeWindowScreenshot(...)`, then rerun.

> ⚠️ Never kill a VS Code process (per CLAUDE.md) — it disrupts active debugging/test sessions. Let the headed suite open and close its own windows.

## Step 3 — Report

Report exactly which images were regenerated and their paths. If a run asserted a diagnostic and it failed to fire, surface that failure — do not commit a misleading image.
