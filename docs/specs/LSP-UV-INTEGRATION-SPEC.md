# Basilisk + uv Integration — Specification {#LSPUV}

> **Plan**: [LSP-PLAN.md](../plans/LSP-PLAN.md)
> **LSP Spec**: [LSP-ARCHITECTURE-SPEC.md](LSP-ARCHITECTURE-SPEC.md) — configuration, commands, binary resolution

---

## 1. Approach {#LSPUV-WHY}

Basilisk understands uv projects from local files only: `uv.lock` (TOML dependency graph — every package, version, platform marker — parsed in Rust with zero subprocess overhead), `.python-version`, `pyproject.toml [tool.uv]`, and uv workspace layouts. No external commands for detection or resolution.

---

## 2. Detection: Is This a uv Project? {#LSPUV-DETECTION}

Basilisk MUST auto-detect uv projects with zero configuration, using filesystem signals only (no subprocess calls).

### 2.1 Detection Signals {#LSPUV-DETECTION-SIGNALS}

A workspace is a uv project if **any** of these are true (checked in order):

| Signal | File | Confidence |
|--------|------|------------|
| Lock file exists | `uv.lock` in workspace root | Definitive |
| uv config section | `pyproject.toml` contains `[tool.uv]` | Definitive |
| uv-created venv | `.venv/pyvenv.cfg` contains `uv = true` | High |
| `.python-version` + no other manager | `.python-version` exists, no `poetry.lock` / `Pipfile.lock` | Medium |

### 2.2 Detection Result {#LSPUV-DETECTION-RESULT}

`basilisk_uv::detect_uv_project(workspace_roots: &[PathBuf]) -> Option<UvProjectInfo>` scans the roots in order and returns info for the first directory matching a [detection signal](#LSPUV-DETECTION-SIGNALS); `None` means "not a uv project".

```rust
pub struct UvProjectInfo {
    pub root: PathBuf,          // matched workspace root
    pub has_lockfile: bool,     // uv.lock exists at root
    pub has_tool_uv: bool,      // pyproject.toml contains [tool.uv]
    pub uv_managed_venv: bool,  // .venv/pyvenv.cfg contains `uv = true`
}
```

The result carries the raw boolean signals, not resolved paths — consumers derive paths from `root` (e.g. `root.join("uv.lock")` in the `build_uv_registry` functions in `crates/basilisk-lsp/src/server/init.rs` and `crates/basilisk-cli/src/main.rs`). Workspace members ([LSPUV-WORKSPACE-MODEL](#LSPUV-WORKSPACE-MODEL)), `.python-version` resolution ([LSPUV-PYTHON-VERSION](#LSPUV-PYTHON-VERSION)), and venv discovery ([LSPUV-DETECTION-FALLBACK](#LSPUV-DETECTION-FALLBACK)) are separate call paths, not fields on the detection result. There is no `EnvironmentManager` enum: `Option<UvProjectInfo>` is the whole environment-manager decision — `Some` enables the additive uv features (lock-file registry, uv-run test execution, uv status); `None` only skips them, and venv discovery via `find_venv_dir()` runs either way.

### 2.3 Fallback {#LSPUV-DETECTION-FALLBACK}

If uv detection fails or signals are ambiguous, fall back to existing `find_venv_dir()` logic. uv integration is additive — it MUST NOT break non-uv projects.

---

## 3. Lock File Intelligence {#LSPUV-LOCK}

`uv.lock` is TOML, complete, and parsed in Rust at zero cost.

### 3.1 What We Extract {#LSPUV-LOCK-EXTRACT}

| Field | Source in `uv.lock` | Use |
|-------|---------------------|-----|
| Package name | `[[package]].name` | Import resolution validation |
| Package version | `[[package]].version` | Hover info, diagnostics |
| Source (registry/git/path) | `[[package]].source` | Distinguish local vs remote |
| Dependencies | `[[package]].dependencies` | Direct vs transitive classification |
| Platform markers | `[[package]].resolution-markers` | Platform-aware resolution |
| Python requires | `requires-python` | Validate `python_version` config |

### 3.2 Package Registry {#LSPUV-LOCK-REGISTRY}

Parsed lock data is stored in a lookup structure:

```rust
pub struct PackageRegistry {
    /// All packages from uv.lock, keyed by normalized import name
    packages: HashMap<String, PackageInfo>,
    /// Direct dependencies (listed in pyproject.toml [project.dependencies])
    direct_deps: HashSet<String>,
    /// Dev dependencies ([dependency-groups] or [tool.uv.dev-dependencies])
    dev_deps: HashSet<String>,
    /// Python version constraint from lock file
    requires_python: Option<String>,
}

pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub is_direct: bool,
    pub has_stubs: bool,           // types-X or X-stubs in registry
    pub stub_package: Option<String>, // name of stub package if known
    pub source: PackageSource,
}

pub enum PackageSource {
    Registry { index: String },
    Git { url: String, rev: String },
    Path { path: PathBuf },
    Editable { path: PathBuf },
}
```

### 3.3 Import Name Mapping {#LSPUV-LOCK-IMPORT-MAPPING}

Package names don't always match import names (e.g. `Pillow` → `PIL`, `scikit-learn` → `sklearn`). The registry maps via, in order:

1. **Top-level module detection** — scan the package's `site-packages` directory for top-level `__init__.py` / `.pyi` files.
2. **Known mappings** — compiled table of common mismatches (Pillow/PIL, scikit-learn/sklearn, python-dateutil/dateutil, etc.).
3. **Normalized fallback** — lowercase, replace `-` with `_`.

### 3.4 Hot Reload {#LSPUV-LOCK-HOT-RELOAD}

When `uv.lock` changes (LSP file watcher or `workspace/didChangeWatchedFiles`): re-parse, diff against the current `PackageRegistry`, invalidate affected import resolutions for added/removed packages, publish updated diagnostics. Log e.g. `"uv.lock changed: +3 packages, -1 package, re-resolving 12 files"`. No LSP restart or user interaction.

---

## 4. Python Version Detection {#LSPUV-PYTHON-VERSION}

### 4.1 Resolution Order (uv-aware) {#LSPUV-PYTHON-VERSION-RESOLUTION-ORDER}

Highest wins:

1. Explicit `python-version` in project config (`[tool.basilisk] python-version` in `pyproject.toml`, or `pythonVersion`/`python-version` in `basilisk.json`) — always wins
2. `.python-version` file in the project root (uv standard; first non-empty, non-comment line)
3. `[project].requires-python` in `pyproject.toml` — lower bound of the first `>=`/`==`/`~=` clause
4. `uv.lock` top-level `requires-python` — same lower-bound extraction
5. Default: `3.12` (the checker's centralized `DEFAULT_TARGET_VERSION`, [`[CHKARCH-VERSION-TARGET]`](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-VERSION-TARGET))

Steps 2–4 are `basilisk_uv::python_version::resolve_target_python_version`; step 1 and the default belong to the consumers (`WorkspaceIndex::load_root_configs`, CLI `main.rs`, `CheckContext::from_config`). Resolution reads declared project metadata only — Basilisk deliberately never probes the venv interpreter (`python3 --version`): the resolved target stays deterministic across machines (a venv built with a different interpreter does not silently shift checker semantics) and version resolution adds no subprocess spawn. The `basilisk.python` VS Code setting is the interpreter *path* for the debugger/profiler and plays no role in version resolution.

### 4.2 Impact on Type Checking {#LSPUV-PYTHON-VERSION-IMPACT}

The detected version flows into the checker as `CheckContext.target_version` — see [`[CHKARCH-VERSION-TARGET]`](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-VERSION-TARGET) for the typed context, the centralized 3.12 default, and the wiring (`basilisk_uv::python_version::resolve_target_python_version` → `WorkspaceIndex::new` / CLI `main.rs`).

Implemented today:

- `sys.version_info` branch narrowing (`directives_version_platform` dead-branch analysis)
- PEP 695 syntax gating below 3.12 (`version_target_syntax`)

Planned (not yet version-gated):

- stdlib module availability (e.g., `tomllib` only in 3.11+)
- further syntax feature support (e.g., `match` in 3.10+)
- `typing_extensions` vs `typing` import suggestions

---

## 5. Enhanced Diagnostics {#LSPUV-DIAGNOSTICS}

### 5.1 Actionable "Module Not Found" (imports_unresolved) {#LSPUV-DIAGNOSTICS-MODULE-NOT-FOUND}

With uv integration, imports_unresolved becomes context-aware:

| Scenario | Diagnostic Message | Code Action |
|---|---|---|
| Package not installed, not in `pyproject.toml` | `Import "requests" could not be resolved. Package is not a dependency.` | `uv add requests` |
| Package in `pyproject.toml` but env not synced | `Import "requests" could not be resolved. Run "uv sync" to install dependencies.` | `uv sync` |
| Package installed but no stubs | `Import "requests" resolves but has no type stubs.` | `uv add --dev types-requests` |
| Stdlib module, wrong Python version | `Module "tomllib" requires Python >= 3.11 (project targets 3.10)` | — |
| Workspace member, not in deps | `Import "my_lib" resolves as workspace member "packages/my_lib"` | — (info, not error) |

### 5.2 Missing Stub Suggestions (BSK-E0152) {#LSPUV-DIAGNOSTICS-MISSING-STUBS}

Strict-by-default error when a package is installed but has no type information (opt down with `"BSK-E0152" = "warning"`).

**When typeshed publishes a stub** (`requests` → `types-requests`):

```
error[BSK-E0152]: Package `requests` is installed but has no type stubs available
  --> src/app.py:3:1
   |
 3 | import requests
   | ^^^^^^^^^^^^^^^^ types will be inferred as Unknown
   |
   = help: Type stubs available as `types-requests` — use quick fix to install
   = note: Packages without type stubs or a PEP 561 `py.typed` marker provide no type information — https://peps.python.org/pep-0561/
```

**When no published stub exists** (private/first-party package) — the help points at the local-stub route and the official guide:

```
error[BSK-E0152]: Package `acme_internal` is installed but has no type stubs available
  --> src/app.py:3:1
   |
 3 | import acme_internal
   | ^^^^^^^^^^^^^^^^^^^^^ types will be inferred as Unknown
   |
   = help: No published type stubs for `acme_internal` — create a local stub (`acme_internal.pyi` in a `stub-paths` directory) or upstream a PEP 561 `py.typed` marker. Guide: https://typing.python.org/en/latest/guides/writing_stubs.html
   = note: Packages without type stubs or a PEP 561 `py.typed` marker provide no type information — https://peps.python.org/pep-0561/
```

Per [STUBRES-CODEACTIONS](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CODEACTIONS) the help describes the fix and never embeds a shell command — the code action does the work. `help`/`note` lines are folded onto the LSP diagnostic message so editors (no `help`/`note` fields) still surface the guidance.

The typeshed stub suggestion (and its `basilisk.uv.addDev` quick fix) is emitted only when the bundled typeshed index (`basilisk_stubs::typeshed_stub_distribution` — a committed TSV regenerated from python/typeshed's `stubs/<DIST>` tree, `crates/basilisk-stubs/data/typeshed_stub_distributions.tsv`, compiled into a phf map by `build.rs`) maps the import root to a real published `types-<DIST>` distribution (e.g. `yaml` → `types-PyYAML`). Stub names are never guessed by string concatenation — neither `types-{name}` nor `{name}-stubs` — so the quick fix never offers a package that does not exist on PyPI. The "stub already installed" case needs no lockfile check: an installed stub-only package (a `{name}-stubs` directory in site-packages — the install form of both typeshed `types-*` distributions and third-party stub packages such as `pandas-stubs`) resolves first in the PEP 561 order ([STUBRES-PEP561](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PEP561) step 3), so the import no longer resolves to an untyped `.py` and BSK-E0152 does not fire at all.

The **create-local-stub** quick fix (`basilisk.stubs.createLocal`, [STUBRES-CREATE-LOCAL](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CREATE-LOCAL)) is offered for **every** BSK-E0152 — the only fix when no typeshed stub exists, a fallback when one does.

### 5.3 Dependency Hygiene Diagnostics {#LSPUV-DIAGNOSTICS-DEP-HYGIENE}

New optional diagnostics (disabled by default, enabled via config):

| Code | Severity | Description |
|------|----------|-------------|
| BSK-W0011 | Warning | Import of package not listed in `pyproject.toml` dependencies |
| BSK-W0012 | Info | Package in `pyproject.toml` but never imported in project source |
| BSK-W0013 | Warning | `uv.lock` is stale — `pyproject.toml` dependencies changed but lock not updated |

---

## 6. uv Workspace Support {#LSPUV-WORKSPACE}

uv workspaces define multi-package monorepos. Basilisk MUST understand them for correct import resolution.

### 6.1 Workspace Detection {#LSPUV-WORKSPACE-DETECTION}

Parse `pyproject.toml` for:

```toml
[tool.uv.workspace]
members = ["packages/*", "libs/*"]
```

### 6.2 Workspace Model {#LSPUV-WORKSPACE-MODEL}

```rust
pub struct UvWorkspaceInfo {
    pub root: PathBuf,
    pub members: Vec<WorkspaceMember>,
}

pub struct WorkspaceMember {
    pub name: String,
    pub path: PathBuf,
    pub pyproject: PathBuf,
    pub src_roots: Vec<PathBuf>,  // typically ["src"] or ["."]
}
```

### 6.3 Import Resolution for Workspaces {#LSPUV-WORKSPACE-IMPORT-RESOLUTION}

When resolving imports in a workspace:

1. Check if the import matches a workspace member name
2. If yes, resolve to that member's source root (editable install semantics)
3. Workspace members are always considered "typed" (no imports_unresolved for cross-member imports)
4. The shared `uv.lock` at workspace root governs all third-party resolution

### 6.4 LSP Multi-Root Mapping {#LSPUV-WORKSPACE-MULTI-ROOT}

Each workspace member is an LSP workspace folder. The server keeps one `PackageRegistry` per workspace root, shared by all members under it.

---

## 7. Code Actions {#LSPUV-ACTIONS}

### 7.1 uv-Powered Quick Fixes {#LSPUV-ACTIONS-QUICK-FIXES}

| Trigger | Code Action Title | Command |
|---------|-------------------|---------|
| imports_unresolved (unresolved, package available) | "Add dependency: `requests`" | `basilisk.uv.add` |
| BSK-E0152 (typeshed stub exists) | "Install type stubs for `requests` (uv add --dev)" | `basilisk.uv.addDev` |
| BSK-E0152 (any — esp. no typeshed stub) | "Create local type stub for `acme_internal`" | `basilisk.stubs.createLocal` |
| BSK-W0013 (stale lock) | "Sync environment" | `basilisk.uv.sync` |

`basilisk.stubs.createLocal` writes a permissive `.pyi` skeleton (no `uv` subprocess) — [STUBRES-CREATE-LOCAL](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CREATE-LOCAL).

The imports_unresolved `basilisk.uv.add` quick fix is offered **only** when the unresolved import's top-level name is a valid PyPI distribution name (PEP 508/503: ASCII alphanumerics plus `.`, `-`, `_`, starting and ending alphanumeric). Internal/vendored modules like `_pydevd_bundle` (leading `_`) are not installable — `uv` rejects them, so the fix is suppressed (issue #84).

### 7.2 Execution {#LSPUV-ACTIONS-EXECUTION}

uv-invoking code actions run as **LSP commands** via `workspace/executeCommand`: the LSP spawns `uv`, streams output via `window/logMessage`, and re-parses the lock file on completion.

Each command returns `{success, stdout, stderr}`. The client shows a success toast (e.g. "Added `requests`.") only when `success` is `true`. On failure the server surfaces an error toast, so the client shows nothing — never a success toast alongside the server's error toast for the same operation (issue #84).

```rust
// Subprocess execution — NOT inline. Runs uv in project root.
// Output streamed to client. Lock file re-parsed on success.
pub struct UvCommand {
    pub args: Vec<String>,
    pub cwd: PathBuf,
}
```

---

## 8. Hover Enrichment {#LSPUV-HOVER}

Hovering an import in a uv project appends package metadata to the popup:

```
requests (v2.31.0) — direct dependency
Source: PyPI registry
Stubs: types-requests (v2.31.0.20240311) installed
```

For workspace members:

```
my_lib — workspace member
Source: packages/my_lib (editable)
```

### 8.1 Data Flow {#LSPUV-HOVER-DATA-FLOW}

1. Hover handler resolves the import to a module path.
2. Match the module path against `PackageRegistry`.
3. If matched, append `PackageInfo` metadata to the hover markdown.
4. If no match (stdlib, local file), standard hover applies.

---

## 9. LSP Commands {#LSPUV-COMMANDS}

Commands registered via `workspace/executeCommand`:

| Command | Arguments | Description |
|---------|-----------|-------------|
| `basilisk.uv.sync` | `{}` | Run `uv sync` in project root |
| `basilisk.uv.add` | `{package: string}` | Run `uv add <package>` |
| `basilisk.uv.addDev` | `{package: string}` | Run `uv add --dev <package>` |
| `basilisk.uv.remove` | `{package: string}` | Run `uv remove <package>` |
| `basilisk.uv.lock` | `{}` | Run `uv lock` (resolve without installing) |
| `basilisk.uv.createEnv` | `{pythonVersion?: string}` | Run `uv venv` (optionally with `--python`) |

All commands:
- Execute in the workspace root directory
- Stream stdout/stderr to the client via `window/logMessage`
- Trigger `uv.lock` re-parse on successful completion
- Report failure via `window/showMessage` (error level), classified per
  [9.1](#LSPUV-COMMAND-FAILURE-UX)

### 9.1 Failure Classification & User-Facing Messaging {#LSPUV-COMMAND-FAILURE-UX}

A failed uv command MUST NOT surface raw resolver stderr as the toast (issue #94). `crates/basilisk-lsp/src/uv_failure.rs` classifies the (ANSI-stripped, whitespace-normalized) stderr; the toast carries a plain-language headline plus a remediation hint:

| Category | Detected from | Toast headline + action |
|---|---|---|
| `package_not_found` | `No solution found` + `was not found in the package registry` / `there are no versions of` | "Package `<pkg>` couldn't be found or has no compatible version. Check the package name for a typo, or confirm it exists on the configured index." |
| `resolution_conflict` | `No solution found` (without not-found markers) | "`<pkg>` conflicts with your existing dependencies. Relax a version pin, or retry with `--frozen` to skip locking." |
| `network_error` | connection refused / DNS / request-send errors | "Couldn't reach the package index. Check your network or index URL, then retry." |
| `uv_not_found` | spawn `ErrorKind::NotFound` | "`uv` isn't installed or isn't on PATH." + install link |
| `generic` | anything else | "`<label>` failed. See the Basilisk Output channel for the full uv output." |

Requirements:
- The **full** uv stderr always remains in the Output channel via
  `window/logMessage` — only the toast is classified.
- Structured logging on failure includes `command`, `package`, `exit_code`,
  `failure_category`, and `duration_ms`. Never log index credentials.
- Covered by the e2e tests in
  `crates/basilisk-lsp/tests/lsp/ws_test_execute_uv.rs` (package-not-found and
  generic cases).

---

## 10. Configuration {#LSPUV-CONFIG}

Settings added to the shared LSP configuration (extends [LSP-ARCHITECTURE-SPEC.md](LSP-ARCHITECTURE-SPEC.md)):

| Setting Key | Type | Default | Description |
|---|---|---|---|
| `basilisk.uv.enabled` | `boolean` | `true` | Enable uv integration (auto-detected) |
| `basilisk.uv.executablePath` | `string` | `""` (auto-detect) | Path to `uv` binary |
| `basilisk.uv.autoSync` | `boolean` | `false` | Auto-run `uv sync` when `pyproject.toml` changes |
| `basilisk.uv.stubSuggestions` | `boolean` | `true` | Suggest installing type stub packages |
| `basilisk.uv.dependencyDiagnostics` | `boolean` | `false` | Enable BSK-W0011/W0012/W0013 |

### 10.1 uv Binary Resolution {#LSPUV-CONFIG-BINARY-RESOLUTION}

| Priority | Source |
|----------|--------|
| 1 | `basilisk.uv.executablePath` setting |
| 2 | `UV_PATH` environment variable |
| 3 | `~/.cargo/bin/uv` |
| 4 | `~/.local/bin/uv` |
| 5 | OS PATH search |

The uv binary is needed only for **commands** (sync, add, remove). Lock-file parsing and environment detection are pure filesystem operations and work even without `uv` installed.

---

## 11. File Watchers {#LSPUV-WATCHERS}

The LSP registers additional file watchers for uv projects:

| Pattern | Event | Action |
|---------|-------|--------|
| `uv.lock` | Create / Change | Reload checker config, rebuild `PackageRegistry`, re-resolve imports, re-check |
| `.python-version` | Create / Change | Reload checker config (version-aware rules — [CHKARCH-VERSION-TARGET]), re-check |
| `pyproject.toml` | Change | Reload checker config, re-detect workspace members, rebuild registry, re-check |
| `basilisk.json` | Change | Reload checker config (severity overrides, `python-version`), re-check |
| `.venv/pyvenv.cfg` | Create / Delete | Re-detect environment manager |

On any of these, the LSP first re-reads each root's `BasiliskConfig` from disk (`WorkspaceIndex::reload_root_configs`) so a changed `python-version` or rule severity takes effect without an LSP restart, then re-checks every indexed file and republishes diagnostics.

---

## 12. Logging {#LSPUV-LOGGING}

uv integration logs (structured `tracing`):

| Level | Event |
|-------|-------|
| `info` | Project detected; `uv.lock` parsed (package count) |
| `debug` | Lock diff on reload; per-import resolution decisions |
| `warn` | Stale `uv.lock`; `uv` binary not found (code actions disabled) |
| `error` | `uv.lock` parse failure (invalid TOML) |

---

## 13. Non-Goals {#LSPUV-NON-GOALS}

Hard invariants for the implementation:

- **uv.lock writing** — Basilisk is read-only on lock files; mutations go through the `uv` CLI.
- **Network calls** — no PyPI queries or index lookups; every resolution input is local.
- **Replacing uv** — package management is delegated, never reimplemented.
- **Non-uv managers** — Poetry/Pipenv/PDM/pip have no dedicated path; they fall back to existing import resolution ([LSPUV-DETECTION-FALLBACK]).
