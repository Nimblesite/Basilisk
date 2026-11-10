# Release manual verification {#RELEASE-VERIFICATION}

Every release is manually driven **before** publishing **and again after
publishing**, against the artifact users actually install. Automated gates
(`/ci-prep`, `make ci`, `make test`, `make conformance`, `make bench`) are
necessary and not sufficient: they run against the tree, not against the thing
on the Marketplace.

[RELEASE-LAW](#RELEASE-LAW) · [RELEASE-CI-PREP](#RELEASE-CI-PREP) ·
[RELEASE-PROVENANCE](#RELEASE-PROVENANCE) ·
[RELEASE-RESPONSIVENESS](#RELEASE-RESPONSIVENESS) ·
[RELEASE-SURFACE](#RELEASE-SURFACE) · [RELEASE-PRE](#RELEASE-PRE) ·
[RELEASE-POST](#RELEASE-POST) · [RELEASE-TRIAGE](#RELEASE-TRIAGE)

## The law {#RELEASE-LAW}

> **The release person MUST manually test BEFORE the release AND AFTER the
> release.**

- **BEFORE** — against the real per-platform VSIX built by the shared
  `_release_vsix` recipe, installed into VS Code:
  `make reinstall-vsix-macos` (or `make reinstall-vsix` for the host default).
  That recipe is the exact artifact the `release.yml` `vsix` job publishes.
- **AFTER** — uninstall everything local, install **from the Marketplace /
  Open VSX / brew / scoop / PyPI**, and run the same checklist against the
  downloaded artifact.

A release is not done when the tag is pushed. It is done when the AFTER pass
is green.

## Automated gate first — `/ci-prep` {#RELEASE-CI-PREP}

`/ci-prep` is the **automated** half, and it runs before anything else in this
document. It reads `.github/workflows/ci.yml` fresh, builds a checklist from the
actual CI steps, and loops until there is a complete clean run with zero
failures: fmt · clippy · build · Rust tests + coverage
(`coverage-thresholds.json`) · VSIX · neovim · zed · mutation.

The division of labour:

- **`/ci-prep` proves the tree is sound.** It never launches the editor, clicks
  a code action, or runs the binary a user installs.
- **The manual passes below prove the product works.** They cover exactly what
  `/ci-prep` cannot reach: interactive surfaces, and the published artifact.

Green `/ci-prep` is the **entry condition, not the finish line**. Do not begin a
manual pass on a tree that is not already green.

```
/ci-prep      →   RELEASE-PRE     →   push tag      →   RELEASE-POST
automated         manual, against     release.yml       manual, against
                  the local VSIX                        the published artifact
```

## Artifact-provenance gate {#RELEASE-PROVENANCE}

Run this **first**, before any feature testing. If it fails, stop — nothing
below is meaningful.

### The tag contains every claimed fix {#RELEASE-PROVENANCE-TAG}

For every issue/PR the release notes claim, resolve its commit and prove the
tag contains it:

```bash
git tag --contains <fix-commit>          # MUST list the release tag
git rev-list -n 1 <tag>                  # the exact SHA being shipped
git log --oneline <previous-tag>..<tag>  # everything actually in the release
```

Write the notes **from** `git log <prev>..<tag>`. Never from `main`.

### The binary's metadata matches the intended commit {#RELEASE-PROVENANCE-BINARY}

The binary self-reports its provenance (stamped by `scripts/stamp-version.sh`
in the `release.yml` `build` job):

```bash
<installed-binary> --version          # "basilisk X.Y.Z" + "Ruff formatter: N.N.N"
<installed-binary> --version --json   # version, gitSha, gitDirty, buildTime, target, toolchain
```

Assert all of:

- `version` equals the tag.
- `gitSha` is a prefix of `git rev-list -n 1 <tag>`.
- `gitDirty` is `false`. A dirty release build is a failed release.
- `buildTime` is after the tag was pushed.
- `Ruff formatter:` matches the tree's pinned ruff rev (`Cargo.toml`) — a
  stale formatter version is a stale binary.
- `basilisk-profiler-helper --version` reports the same version (macOS VSIX).

`scripts/gen_release_notes.py <binary> <tag> shipwright.json` generates the
component block straight from the binary and `shipwright.json`, so the notes
cannot claim different bytes from the build ([LSPFMT-RELEASE-NOTES]; drift test
`crates/basilisk-cli/tests/e2e_release_notes_block.rs`).

### You are testing the REAL installed artifact {#RELEASE-PROVENANCE-ARTIFACT}

**Never** test `target/release/basilisk`. Always test the binary VS Code
actually launches:

```bash
# the binary VS Code launches (+ basilisk-profiler-helper beside it):
~/.vscode/extensions/nimblesite.basilisk-<version>-<platform>/bin/<platform>/basilisk
code --list-extensions --show-versions | grep -i basilisk   # exactly ONE build installed
ls -d ~/.vscode/extensions/nimblesite.basilisk-*            # remove stale copies first
```

Also confirm `basilisk.executablePath` / `basilisk.binaries.*` are **unset** in
your settings, or you are testing someone else's binary.

## Known-hang / responsiveness smoke test {#RELEASE-RESPONSIVENESS}

Run the shipped binary against pathological input. Every one must **terminate**.

```bash
# self-referential and cyclic bases — regression tests live in
# crates/basilisk-resolver/tests/resolver/test_recursive_bases.rs
printf 'class C(C[int], C[bool]):\n    pass\n'          > /tmp/bsk/self_base.py
printf 'class A(B):\n    pass\nclass B(A):\n    pass\n' > /tmp/bsk/cycle.py

time timeout 30 <installed-binary> check /tmp/bsk/     # MUST exit well inside 30s
time timeout 60 <installed-binary> check <large-repo>  # sanity: no runaway
```

Then, with the extension running on a real project:

1. Open the pathological file. Confirm diagnostics appear and the **Modules**
   panel keeps updating (add a file → counts change). A frozen panel on stale
   counts is the hang signature.
2. Watch CPU (`top -o cpu` / Activity Monitor) — no `basilisk` process may sit
   at ~100% of a core once analysis settles.
3. Set `basilisk.enabled` to `false` (the **Diagnostics** row in the Basilisk
   panel, `basilisk.info.runAction`). Published diagnostics must clear **and
   CPU must drop**. Re-enable — diagnostics must come back.
4. `Basilisk: Restart Language Server` recovers a wedged server.

## Manual test surface {#RELEASE-SURFACE}

Every area below gets hands-on testing in both the BEFORE and AFTER passes.

### CLI {#RELEASE-SURFACE-CLI}

Command surface from `crates/basilisk-cli/src/main.rs`:

- [ ] `basilisk check <paths>` — text output, correct exit code
- [ ] `basilisk check --output json` — parses; `--color always|never|auto`
- [ ] `basilisk check --cache --cache-stats` and `--no-cache` / `--cache-dir`
- [ ] `basilisk analyze <paths>` — opt-in non-`pep` rules only
- [ ] `basilisk format <paths>` and `basilisk format --check`
- [ ] `basilisk fix` — plus `--unsafe` and `--rules BSK-0001,…`
- [ ] `basilisk adopt` / `basilisk adopt --status` / `basilisk unadopt`
- [ ] `basilisk lsp --transport stdio` and `--transport ws --port <n>`
- [ ] `basilisk mcp --workspace <dir>` — stdio tools respond
- [ ] `basilisk typeshed download` (and `--commit` / `--package`)
- [ ] `basilisk stubs generate` / `basilisk stubs status` / `basilisk --createstub`
- [ ] `basilisk --version`, `--version --json`, `--help`

### LSP features {#RELEASE-SURFACE-LSP}

From `crates/basilisk-lsp/src/server/handlers/`. Exercise each in a real file:

- [ ] **`features.rs`** — hover · signature help · inlay hints
  (`basilisk.inlayHints.parameterNames` / `.variableTypes`) · semantic tokens ·
  code actions / quick fixes · completion + completion resolve (incl.
  auto-import) · formatting · range formatting · folding ranges · selection
  ranges · code lens · document color + color presentation
- [ ] **`navigation.rs`** — go-to-definition · go-to-declaration ·
  go-to-type-definition · document symbols · workspace symbols · find
  references · document highlight · prepare rename + rename · call hierarchy
  (incoming + outgoing) · type hierarchy (supertypes + subtypes)
- [ ] **`file_operations.rs`** — rename a file in the explorer; imports update
- [ ] Diagnostics publish on open/edit/save and clear when fixed
- [ ] `basilisk.analysisMode` switches (open-file / module / cross-module)

### VS Code extension {#RELEASE-SURFACE-VSCODE}

Views and commands from `vscode-extension/package.json`:

- [ ] **Modules** panel (`basilisk.moduleExplorer`) — refresh, sort, filter,
      tree/flat toggle, Copy Import Path, Copy Qualified Name
- [ ] **Python Processes** panel (`basilisk.pythonProcesses`) — refresh, sort,
      group, filter, Copy PID, Reveal Script in Editor
- [ ] **Basilisk** info panel (`basilisk.info`) — server state, version,
      resolved python/uv/binary rows, Diagnostics toggle
- [ ] Status bar item · `Basilisk: Status Menu` · `Basilisk: Show Output` ·
      `Basilisk: Restart Language Server`
- [ ] `Basilisk: Open Configuration Editor` — preview/apply a change
- [ ] Mass autofix: Fix All (Safe) in File / All in File / in Workspace /
      All in Workspace; Organize Imports
- [ ] Adoption: Adopt File / Adopt Workspace / Un-adopt File
- [ ] uv commands: Sync, Add, Add Dev, Remove, Lock, Create Virtual Environment
- [ ] Test Explorer (`basilisk.testExplorer.*`) — discover, run, debug, coverage
- [ ] `Basilisk: Getting Started` walkthrough — all three steps
- [ ] The palette advertises no command the LSP does not implement
      (`crates/basilisk-lsp/src/server/commands.rs`)

### Debugger / DAP {#RELEASE-SURFACE-DEBUG}

- [ ] `basilisk-debug` launch config starts, hits a breakpoint, steps, resumes
- [ ] Variables + watch evaluation (`dap-evaluate.ts`), debug console output
- [ ] Bundled `debugpy` is present under the installed extension's `bundled/`
- [ ] Memory inspection during a paused debug session

### Profiler and memory {#RELEASE-SURFACE-PROFILER}

- [ ] Start / Stop / Snapshot profiling; Show Profile Results (flame graph)
- [ ] Profile Debug Session; Run & Profile CPU (Current File)
- [ ] Profile CPU / Track Memory from a row in **Python Processes**
- [ ] Memory: Start, Snapshot, Stop, Compare Snapshots, Force GC,
      Show Reference Graph; Run & Track Memory (Current File)
- [ ] Inline heat map decorations appear and clear

### Typeshed {#RELEASE-SURFACE-TYPESHED}

- [ ] Fresh workspace with no pin → the unpinned-source advisory fires
- [ ] `basilisk typeshed download` writes the pin; advisory clears
- [ ] `--commit <sha>` and `--package name@sha256:<hex>` verify and materialise
- [ ] Configuration-editor typeshed Download buttons work
- [ ] Checking **never** downloads ([STUBRES-TYPESHED-DOWNLOAD]) — re-run
      offline and confirm

### Other editors {#RELEASE-SURFACE-EDITORS}

- [ ] **Neovim** (`basilisk.nvim`) — plugin resolves a binary and attaches;
      diagnostics, hover, go-to-definition ([NEOVIM-SPEC.md](NEOVIM-SPEC.md))
- [ ] **Zed** (`basilisk-zed`) — dev extension installs, server starts,
      diagnostics render ([ZED-SPEC.md](ZED-SPEC.md))

### Distribution channels {#RELEASE-SURFACE-CHANNELS}

Every publishing job in `.github/workflows/release.yml` must be green **and**
its output installed and smoke-tested in the AFTER pass:

- [ ] GitHub Release binaries + checksums (all five platforms)
- [ ] VS Code Marketplace VSIX · Open VSX VSIX
- [ ] Homebrew formula · Scoop manifest · PyPI wheels
- [ ] Neovim plugin · Zed extension
- [ ] GitHub Pages deploy (website + `/errors/BSK-XXXX` pages resolve)

## Before publishing {#RELEASE-PRE}

1. [ ] `/ci-prep` green — one complete clean run, zero failures, start to
   finish ([RELEASE-CI-PREP](#RELEASE-CI-PREP)). Nothing below starts until it is.
2. [ ] `make conformance` — 100% / 0 false positives against a fresh
   `python/typing@main` clone.
3. [ ] `make bench` — no fixture slower than the committed baseline.
4. [ ] `python3 scripts/verify_release_attribution.py --policy-only` passes and
   licence manifests are current (`npm run licenses:check` in
   `vscode-extension/`).
5. [ ] Draft release notes **from `git log <prev-tag>..HEAD`**, then run
   [RELEASE-PROVENANCE-TAG](#RELEASE-PROVENANCE-TAG) against the commit you are
   about to tag.
6. [ ] `make reinstall-vsix-macos` (or `make reinstall-vsix`) — installs the
   exact release VSIX.
7. [ ] [RELEASE-PROVENANCE-BINARY](#RELEASE-PROVENANCE-BINARY) and
   [RELEASE-PROVENANCE-ARTIFACT](#RELEASE-PROVENANCE-ARTIFACT) against the
   installed binary.
8. [ ] [RELEASE-RESPONSIVENESS](#RELEASE-RESPONSIVENESS).
9. [ ] Walk the whole of [RELEASE-SURFACE](#RELEASE-SURFACE).
10. [ ] Only then push the tag.

## After publishing {#RELEASE-POST}

1. [ ] Every `release.yml` job succeeded — no skipped publish.
2. [ ] `code --uninstall-extension Nimblesite.basilisk`, delete every
   `~/.vscode/extensions/nimblesite.basilisk-*` directory, restart VS Code.
3. [ ] Install **from the Marketplace UI** (not a local VSIX), on a machine
   that has never built this repo if one is available.
4. [ ] Re-run [RELEASE-PROVENANCE](#RELEASE-PROVENANCE) — the Marketplace
   binary's `gitSha` must match the tag, `gitDirty` must be `false`, and the
   `Ruff formatter:` line must match the tree.
5. [ ] Re-run [RELEASE-RESPONSIVENESS](#RELEASE-RESPONSIVENESS) against the
   Marketplace binary.
6. [ ] Re-run [RELEASE-SURFACE](#RELEASE-SURFACE) against the Marketplace build.
7. [ ] Install and smoke-test each remaining channel in
   [RELEASE-SURFACE-CHANNELS](#RELEASE-SURFACE-CHANNELS).
8. [ ] Anything red → unpublish/yank or ship a patch immediately. Do not leave
   a known-bad artifact live.

## If it regresses in the field {#RELEASE-TRIAGE}

1. **Find the process and prove its version.**
   ```bash
   ps aux | grep '[b]asilisk'
   ps -o comm= -p <pid>                 # full path of the running binary
   <that path> --version --json         # version + gitSha + gitDirty + buildTime
   ```
   A `gitSha` that is not the current tag means a stale artifact, not a new bug.

2. **Sample a spinning process** (macOS):
   ```bash
   sample <pid> 10 -f /tmp/basilisk-sample.txt
   ```
   `100.0%` is one saturated core, not the whole machine, and a high thread
   count is just the Tokio pool. Read the per-thread leaves: one worker deep in
   self-recursion while the rest are parked in `__psynch_cvwait` / `kevent` is
   the runaway-recursion signature. A wide, varying stack means a hot loop
   instead; flat RSS rules out a leak.

3. **Read the extension log.**
   ```
   ~/Library/Application Support/Code/logs/<session>/window<N>/exthost/Nimblesite.basilisk/Basilisk.log
   ~/Library/Application Support/Code/logs/<session>/window<N>/exthost/Nimblesite.basilisk/basilisk-debug-trace.log
   ```
   Newest `<session>` directory wins. In-editor: `Basilisk: Show Output`, and
   raise `basilisk.trace.server` for protocol traffic.

4. **Confirm which extension build is running.**
   ```bash
   code --list-extensions --show-versions | grep -i basilisk
   ls -d ~/.vscode/extensions/nimblesite.basilisk-*
   ```
   More than one directory means VS Code may be launching a build you are not
   looking at.

5. **Reduce and land the fix as a test first** — see the
   [fix-bug skill](../../.claude/skills/fix-bug/SKILL.md). A hang gets a
   deadline-bounded regression test, as in
   `crates/basilisk-resolver/tests/resolver/test_recursive_bases.rs`.
