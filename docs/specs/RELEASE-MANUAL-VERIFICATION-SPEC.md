<a id="RELEASE-VERIFICATION"></a>

# Release manual verification

> **SUPERSEDED for the product surface — one clause survives.** Basilisk is
> unlisted. There is exactly one release left ([WITHDRAWAL-UNLIST](DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-UNLIST)),
> and it ships an inert CLI and a notice-only extension: there is no diagnostic,
> debugger, profiler or editor surface left to walk through, so every checklist
> below describing one is a record of what used to be verified, not work to do.
> What still applies is the shape: **verify the published artifact, not the
> tree.** For the final release that means [`delist/01-verify-final-release.sh`](../../delist/README.md)
> — every channel must be serving the final version before anything is unlisted.

Every release gets a manual pass **before** the tag is pushed and a second pass
once the Marketplace VSIX is publicly available. Automated gates prove the
tree; these passes prove the packaged product and the version users install.

[RELEASE-LAW](#RELEASE-LAW) · [RELEASE-CI-PREP](#RELEASE-CI-PREP) ·
[RELEASE-PROVENANCE](#RELEASE-PROVENANCE) ·
[RELEASE-RESPONSIVENESS](#RELEASE-RESPONSIVENESS) ·
[RELEASE-SURFACE](#RELEASE-SURFACE) · [RELEASE-PRE](#RELEASE-PRE) ·
[RELEASE-POST](#RELEASE-POST) · [RELEASE-TRIAGE](#RELEASE-TRIAGE)

<a id="RELEASE-LAW"></a>

## The law

> **The release person MUST manually test BEFORE the release AND AFTER the
> release.**

- **BEFORE** — install the local release candidate with
  `make _reinstall_vsix TARGET=darwin-arm64` (or `make _reinstall_vsix` for the host). It uses
  the same `_release_vsix` packaging path as the release workflow.
- **AFTER** — wait for the new version to appear in the VS Code Marketplace,
  remove local builds, and install that VSIX through the Marketplace UI. Run
  the full surface on at least three large, materially different real-world
  codebases, then smoke-test the other published distributions.

A release is complete only when that Marketplace-installed version passes
provenance, responsiveness, and the full manual surface on all three codebases.

<a id="RELEASE-CI-PREP"></a>

## Automated gate first — `/ci-prep`

`/ci-prep` runs first. It derives its checklist from
`.github/workflows/ci.yml` and loops until one clean run passes formatting,
linting, builds, tests and coverage, editor packages, and mutation checks. It
proves the tree is sound; the manual passes cover interactive behavior and
installed artifacts.

Green `/ci-prep` is the **entry condition, not the finish line**. Do not begin a
manual pass on a tree that is not already green.

```
/ci-prep  →  RELEASE-PRE  →  push tag  →  Marketplace live  →  RELEASE-POST
automated    local VSIX       release.yml                  Marketplace VSIX
```

<a id="RELEASE-PROVENANCE"></a>

## Artifact-provenance gate

Run this **first**, before any feature testing. If it fails, stop — nothing
below is meaningful.

<a id="RELEASE-PROVENANCE-TAG"></a>

### The tag contains every claimed fix

Create the release tag locally before this gate, but do not push it yet. For
every claimed issue or PR, prove that the tag contains its fix:

```bash
git tag --contains <fix-commit>          # MUST list the release tag
git rev-list -n 1 <tag>                  # the exact SHA being shipped
git log --oneline <previous-tag>..<tag>  # everything actually in the release
```

Write the notes from this range, never from `main`.

<a id="RELEASE-PROVENANCE-BINARY"></a>

### The binary's metadata matches the intended commit

Check the installed binary, not a local build:

```bash
<installed-binary> --version          # "basilisk X.Y.Z" + "Ruff formatter: N.N.N"
<installed-binary> --version --json   # version, gitSha, gitDirty, buildTime, target, toolchain
```

BEFORE, confirm `gitSha` points at the local tag and the formatter version is
expected. AFTER, also require the Marketplace version to match the tag,
`gitDirty` to be `false`, and `buildTime` to follow the tag push. Any mismatch
stops the release. On macOS, the helper binary must report the same version.

Generate the release-note component block from the tested binary:

```bash
scripts/gen_release_notes.py <binary> <tag> shipwright.json
```

<a id="RELEASE-PROVENANCE-ARTIFACT"></a>

### You are testing the real installed artifact

**Never** test `target/release/basilisk`. Always test the binary VS Code
actually launches:

```bash
# Binary VS Code launches; basilisk-profiler-helper is beside it on macOS:
~/.vscode/extensions/nimblesite.basilisk-<version>-<platform>/bin/<platform>/basilisk
code --list-extensions --show-versions | grep -i basilisk
ls -d ~/.vscode/extensions/nimblesite.basilisk-*
```

Confirm exactly one build is installed and `basilisk.executablePath` /
`basilisk.binaries.*` are unset.

<a id="RELEASE-RESPONSIVENESS"></a>

## Known-hang / responsiveness smoke test

Run the shipped binary against pathological input. Every one must **terminate**.

```bash
scratch_dir="$(mktemp -d)"
printf 'class C(C[int], C[bool]):\n    pass\n' > "$scratch_dir/self_base.py"
printf 'class A(B):\n    pass\nclass B(A):\n    pass\n' > "$scratch_dir/cycle.py"

time <installed-binary> check "$scratch_dir"  # Must finish within 30 seconds.
time <installed-binary> check <large-repo>     # Repeat for every release test repo.
```

Then, with the extension running on a real project:

1. Open a pathological file; diagnostics must appear and the **Modules** panel
   must still react when a file is added.
2. After analysis settles, CPU must idle. Disable diagnostics: published
   diagnostics must clear and CPU must drop; re-enable them and confirm return.
3. `Basilisk: Restart Language Server` must recover the session.

<a id="RELEASE-SURFACE"></a>

## Manual test surface

Test every applicable area in both passes. Each checkbox is a representative
journey with an observable result, not a requirement to try every flag or menu.
In the AFTER pass, complete the surface separately on each of the three large
release-test codebases.

<a id="RELEASE-SURFACE-CLI"></a>

### CLI

- [ ] `check` and `analyze` on representative passing and failing projects:
      usable text/JSON, correct exit codes, color, and cache/no-cache behavior
- [ ] `format` / `format --check` and `fix` (safe, unsafe, and rule-scoped) make
      the expected changes
- [ ] The adoption lifecycle works: adopt, status, and unadopt
- [ ] LSP over stdio/WebSocket and MCP over stdio start and respond
- [ ] Typeshed and stub workflows complete; version (text/JSON) and help output
      are accurate

<a id="RELEASE-SURFACE-LSP"></a>

### LSP features

Exercise these in representative real files:

- [ ] Authoring feedback is correct: diagnostics update and clear; hover,
      signature help, completion/auto-import, quick fixes, inlay hints, and
      semantic tokens respond
- [ ] Cross-file navigation and refactoring work: definitions, symbols,
      references/highlights, rename (including file-rename import updates), and
      call/type hierarchies
- [ ] Formatting and structural features work: full/range formatting,
      folding/selection ranges, code lens, and color handling
- [ ] Each `basilisk.analysisMode` setting analyzes the intended scope

<a id="RELEASE-SURFACE-VSCODE"></a>

### VS Code extension

- [ ] Modules, Python Processes, and Basilisk info panels populate and refresh;
      sample a sort/filter and contextual action in each panel
- [ ] Status menu, output, server restart, diagnostics toggle, and configuration
      editor work
- [ ] Safe/all file/workspace fixes, import organization, and adoption commands
      produce the expected changes
- [ ] uv environment/dependency commands and Test Explorer discovery, run,
      debug, and coverage work
- [ ] The Getting Started walkthrough completes, and the palette exposes only
      LSP-implemented commands

<a id="RELEASE-SURFACE-DEBUG"></a>

### Debugger / DAP

- [ ] A `basilisk-debug` launch hits a breakpoint; stepping, resume, variables,
      watches, and console output work
- [ ] Bundled `debugpy` is present, and memory inspection works while paused

<a id="RELEASE-SURFACE-PROFILER"></a>

### Profiler and memory

- [ ] CPU profiling completes from a current file, debug session, and Python
      Processes row; snapshots and results render
- [ ] Memory tracking starts, snapshots/compares, forces GC, and shows references
      from the advertised entry points
- [ ] Inline heat-map decorations appear and clear

<a id="RELEASE-SURFACE-TYPESHED"></a>

### Typeshed

- [ ] A fresh unpinned workspace shows the advisory; download writes a pin and
      clears it
- [ ] Commit/package verification and configuration-editor downloads work
- [ ] Checking never downloads ([STUBRES-TYPESHED-DOWNLOAD]); verify offline

<a id="RELEASE-SURFACE-EDITORS"></a>

### Other editors

- [ ] Neovim resolves and attaches the binary; diagnostics, hover, and definition
      work ([NEOVIM-SPEC.md](NEOVIM-SPEC.md))
- [ ] Zed installs the development extension, starts the server, and renders
      diagnostics ([ZED-SPEC.md](ZED-SPEC.md))

<a id="RELEASE-SURFACE-CHANNELS"></a>

### Distribution channels

Every `release.yml` publish job must be green and its output smoke-tested after
publishing on a compatible host:

- [ ] GitHub Release binaries and checksums for all five platforms
- [ ] VS Code Marketplace and Open VSX packages
- [ ] Homebrew, Scoop, and PyPI packages
- [ ] Neovim and Zed extensions
- [ ] GitHub Pages, including `/errors/BSK-XXXX`

<a id="RELEASE-PRE"></a>

## Before publishing

1. [ ] `/ci-prep` green — one complete clean run, zero failures, start to
   finish ([RELEASE-CI-PREP](#RELEASE-CI-PREP)). Nothing below starts until it is.
2. [ ] `make conformance` — a live run against a fresh `python/typing@main`
   clone. Record what it reports and compare it to the previous release's
   record; an unexplained change is a regression to investigate. A drop
   explained by a deliberate deletion is expected and is noted in the release
   record. **The figure is never published or quoted**
   ([CHKARCH-CONFORMANCE](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFORMANCE)).
3. [ ] `make _bench` — indicative only, gates nothing; record the numbers.
4. [ ] `python3 scripts/verify_release_attribution.py --policy-only` passes and
   licence manifests are current (`npm run licenses:check` in
   `vscode-extension/`).
6. [ ] Draft notes from `git log <prev-tag>..HEAD`, create the tag locally, and
   run [RELEASE-PROVENANCE-TAG](#RELEASE-PROVENANCE-TAG) without pushing it.
7. [ ] `make _reinstall_vsix TARGET=darwin-arm64` (or `make _reinstall_vsix`) — installs the
   candidate built through the release packaging path.
8. [ ] [RELEASE-PROVENANCE-BINARY](#RELEASE-PROVENANCE-BINARY) and
   [RELEASE-PROVENANCE-ARTIFACT](#RELEASE-PROVENANCE-ARTIFACT) against the
   installed binary.
8. [ ] [RELEASE-RESPONSIVENESS](#RELEASE-RESPONSIVENESS).
9. [ ] Walk the whole of [RELEASE-SURFACE](#RELEASE-SURFACE).
10. [ ] Only then push the tag.

<a id="RELEASE-POST"></a>

## After publishing

1. [ ] Every `release.yml` job succeeded and the new Marketplace version is
   publicly installable — no skipped or merely queued publish.
2. [ ] `code --uninstall-extension Nimblesite.basilisk`, delete every
   `~/.vscode/extensions/nimblesite.basilisk-*` directory, restart VS Code.
3. [ ] Install **from the Marketplace UI** (not a local VSIX), on a machine
   that has never built this repo if one is available.
4. [ ] Re-run [RELEASE-PROVENANCE](#RELEASE-PROVENANCE) — the Marketplace
   binary's `gitSha` must match the tag, `gitDirty` must be `false`, and the
   `Ruff formatter:` line must match the tree.
5. [ ] Re-run [RELEASE-RESPONSIVENESS](#RELEASE-RESPONSIVENESS) against the
   Marketplace binary and every large release-test codebase.
6. [ ] Complete [RELEASE-SURFACE](#RELEASE-SURFACE) on at least three large,
   materially different real-world codebases using the Marketplace build.
7. [ ] Install and smoke-test each remaining channel in
   [RELEASE-SURFACE-CHANNELS](#RELEASE-SURFACE-CHANNELS).
8. [ ] Keep the release incomplete while anything is red; contain a
   user-impacting failure and unpublish, yank, or patch as its severity requires.

<a id="RELEASE-TRIAGE"></a>

## If it regresses in the field

1. **Find the process and prove its version.**
   ```bash
   ps aux | grep '[b]asilisk'
   ps -o comm= -p <pid>                 # full path of the running binary
   <that path> --version --json         # version + gitSha + gitDirty + buildTime
   ```
   A `gitSha` that is not the current tag means a stale artifact, not a new bug.

2. **Capture a ten-second process sample** (macOS) and attach it to the bug:
   ```bash
   sample <pid> 10 -f /tmp/basilisk-sample.txt
   ```

3. **Read the extension log.**
   ```
   ~/Library/Application Support/Code/logs/<session>/window<N>/exthost/Nimblesite.basilisk/Basilisk.log
   ~/Library/Application Support/Code/logs/<session>/window<N>/exthost/Nimblesite.basilisk/basilisk-debug-trace.log
   ```
   Use the newest `<session>`. Start with `Basilisk: Show Output`; raise
   `basilisk.trace.server` only if the normal log is not enough.

4. **Confirm which extension build is running.**
   ```bash
   code --list-extensions --show-versions | grep -i basilisk
   ls -d ~/.vscode/extensions/nimblesite.basilisk-*
   ```
   More than one directory means VS Code may be launching a build you are not
   looking at.

5. **Reduce and land the fix as a test first** — see the
   [fix-bug skill](../../.claude/skills/fix-bug/SKILL.md). A hang needs a
   deadline-bounded regression test.
