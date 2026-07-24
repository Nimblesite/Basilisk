# VS Code Editor Screenshots {#VSIX-EDITOR-SCREENSHOTS}

Automated pipeline that drives the extension in a headed VS Code instance and captures the real window (diagnostics, hover, Quick Fix, activity panel) for the website. Editor-side companion to the CLI pipeline ([WEBSITE-SCREENSHOTS]): both produce committed PNGs from the real product, verified (not captured) in CI, per [GITHUB-NO-ARTIFACTS].

## Capture pipeline {#VSIX-EDITOR-SCREENSHOTS-PIPELINE}

Prerequisite — build the binaries the dev extension resolves:

```bash
cargo build -p basilisk-cli -p basilisk-profiler-helper
```

Then, from `vscode-extension/`:

```bash
npm run screenshots:editor
```

`scripts/capture-screenshots.mjs` orchestrates one run:

1. Stages the built binaries into `vscode-extension/bin/<platform>/` via `stage-runtime.mjs` (the same path the packaged VSIX and the extension's shipwright resolver use) and mirrors the repo-root `shipwright.json` into the extension (both gitignored dev artifacts).
2. Compiles the extension and launches the **"Editor screenshots"** suite (`src/test/suite/screenshots-capture.test.ts`) headed, with `BASILISK_SCREENSHOTS=1`. The harness (`.vscode-test.mjs`) adds `--remote-debugging-port` so the window is reachable over CDP.
3. Runs the sidecar `scripts/screenshot-watcher.mjs`, which speaks Chrome DevTools Protocol over Node's **built-in WebSocket** (no Playwright, no browser download), forces a uniform Retina viewport (1440×900 @2×), and captures the workbench page on demand.

Each test drives a feature until visible, strips transient chrome (`notifications.clearAll`, close the Chat auxiliary bar), then calls `takeWindowScreenshot(name)` (`screenshot.ts`), which hands the sidecar a `.signal` file and waits for the PNG. Output lands in `website/src/assets/images/vscode-*.png`.

The suite is a **no-op unless `BASILISK_SCREENSHOTS=1`** (it `skip()`s in suiteSetup), so a normal `npm test` never opens these windows or writes into the repo.

## Captured set {#VSIX-EDITOR-SCREENSHOTS-SET}

| Image | Feature | Embedded on |
|---|---|---|
| `vscode-diagnostics.png` | Inline squiggles + Problems panel | `/docs/install-vscode/` |
| `vscode-hover.png` | Hover type popup | `/docs/quick-start/` |
| `vscode-quickfix.png` | Quick Fix / code-action menu | `/docs/refactoring/` |
| `vscode-module-explorer.png` | Basilisk activity panel | `/docs/` |
| `vscode-configuration-editor.png` | Tag-first rule severities | `/docs/configuration/` |

Add a capture by adding a `test(...)` that makes the feature visible and calls `takeWindowScreenshot('vscode-<name>.png')`.

### Configuration-editor capture {#VSIX-EDITOR-SCREENSHOTS-CONFIGURATION}

The headed suite now includes **configuration editor tag-first rules**. It
constructs the real `ConfigurationEditorController`, opens it for the real test
workspace, waits until the shared store receives a snapshot from the bundled
LSP, and requests `vscode-configuration-editor.png`. It never substitutes a
static HTML mock or a copied rule list.

The real-LSP capture is committed at 2880×1800 and has been visually verified:
the tag facets, rule inventory, per-rule severity controls, active configuration
source, and synchronization status are all visible. The capture evidence is
complete: the configuration guide embeds it and the website E2E suite requires
the image to decode to non-zero pixels. That gate is tracked by
[CONFIGEDITOR-PLAN-VSIX](../plans/LSP-CONFIGURATION-EDITOR-PLAN.md#CONFIGEDITOR-PLAN-VSIX).

## Verification {#VSIX-EDITOR-SCREENSHOTS-VERIFY}

The committed PNGs are regenerated locally (capture needs a headed VS Code and the built binary — not in CI). CI only verifies they render: `website/tests/e2e/screenshots.spec.ts` visits each embedding page and asserts the `vscode-*.png` is present and decodes to non-zero pixels, so a missing or zero-byte capture fails the build. No screenshot is ever produced or uploaded by CI ([GITHUB-NO-ARTIFACTS]); the gitignored full-screen capture in `screenshot.ts` (`captureScreenshot`) remains for local debugging only.
