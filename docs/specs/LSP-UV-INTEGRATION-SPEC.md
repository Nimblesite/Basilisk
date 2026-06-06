# Basilisk + uv Integration — Specification {#LSPUV}

> **Goal**: Make Basilisk the first type checker that deeply understands uv projects — zero-config, instant, always in sync.
>
> **Plan**: [LSP-UV-INTEGRATION-PLAN.md](../plans/LSP-UV-INTEGRATION-PLAN.md)
>
> **LSP Spec**: [LSP-ARCHITECTURE-SPEC.md](LSP-ARCHITECTURE-SPEC.md) — configuration, commands, binary resolution

---

## 1. Why This Matters {#LSPUV-WHY}

uv is the fastest-growing Python environment manager. Written in Rust (like Basilisk), it manages interpreters, virtual environments, dependencies, and lockfiles. Today's LSPs treat environment detection as an afterthought — probe for `.venv`, hope for the best, restart when things change.

Basilisk can do better. uv's `uv.lock` is a **TOML file containing the complete dependency graph** — every package, every version, every platform marker. We can parse it directly in Rust with zero subprocess overhead. Combined with `.python-version`, `pyproject.toml [tool.uv]`, and uv workspace layouts, Basilisk can achieve **perfect environment understanding without running a single external command**.

### What This Unlocks {#LSPUV-WHY-UNLOCKS}

| Capability | Without uv | With uv integration |
|---|---|---|
| Know what's installed | Scan `site-packages` dirs | Parse `uv.lock` — instant, complete |
| Python version | Probe interpreter binary | Read `.python-version` — no subprocess |
| Missing package diagnostics | "Module not found" | "Module `requests` not found — run `uv add requests`" |
| Missing stubs | Silent `Unknown` types | Code action: "Install type stubs: `uv add --dev types-requests`" |
| Dep changes | Restart LSP | Watch `uv.lock` — hot reload, zero restart |
| Monorepo support | Flat workspace roots | Parse `[tool.uv.workspace]` — correct resolution per member |
| Unused deps | Not possible | Cross-reference imports against `pyproject.toml` |
| Hover context | Type signature only | Type + package version + direct/transitive + stub status |

---

## 2. Detection: Is This a uv Project? {#LSPUV-DETECTION}

Basilisk MUST auto-detect uv projects with **zero configuration**. Detection uses filesystem signals only — no subprocess calls.

### 2.1 Detection Signals {#LSPUV-DETECTION-SIGNALS}

A workspace is a uv project if **any** of these are true (checked in order):

| Signal | File | Confidence |
|--------|------|------------|
| Lock file exists | `uv.lock` in workspace root | Definitive |
| uv config section | `pyproject.toml` contains `[tool.uv]` | Definitive |
| uv-created venv | `.venv/pyvenv.cfg` contains `uv = true` | High |
| `.python-version` + no other manager | `.python-version` exists, no `poetry.lock` / `Pipfile.lock` | Medium |

### 2.2 Detection Result {#LSPUV-DETECTION-RESULT}

```rust
pub enum EnvironmentManager {
    Uv(UvProjectInfo),
    TraditionalVenv,
    NoEnvironment,
}

pub struct UvProjectInfo {
    pub project_root: PathBuf,
    pub lock_file: Option<PathBuf>,          // uv.lock
    pub pyproject: PathBuf,                  // pyproject.toml
    pub python_version_file: Option<PathBuf>, // .python-version
    pub venv_dir: Option<PathBuf>,           // .venv
    pub workspace: Option<UvWorkspaceInfo>,  // if [tool.uv.workspace] present
}
```

### 2.3 Fallback {#LSPUV-DETECTION-FALLBACK}

If uv detection fails or signals are ambiguous, fall back to existing `find_venv_dir()` logic. uv integration is additive — it MUST NOT break non-uv projects.

---

## 3. Lock File Intelligence {#LSPUV-LOCK}

The `uv.lock` file is the crown jewel. It's TOML, it's complete, and we can parse it in Rust with zero cost.

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

Parsed lock data is stored in a fast lookup structure:

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

Python package names don't always match import names (e.g., `Pillow` is imported as `PIL`, `scikit-learn` as `sklearn`). The registry maintains a mapping using:

1. **Top-level module detection** — scan the package's installed directory in `site-packages` for top-level `__init__.py` or `.pyi` files
2. **Known mappings** — compiled table of common mismatches (Pillow/PIL, scikit-learn/sklearn, python-dateutil/dateutil, etc.)
3. **Normalized fallback** — lowercase, replace `-` with `_`

### 3.4 Hot Reload {#LSPUV-LOCK-HOT-RELOAD}

When `uv.lock` changes (detected via LSP file watcher or `workspace/didChangeWatchedFiles`):

1. Re-parse the lock file
2. Diff against current `PackageRegistry`
3. For added/removed packages: invalidate affected import resolutions
4. Publish updated diagnostics for affected files
5. Log: `"uv.lock changed: +3 packages, -1 package, re-resolving 12 files"`

No LSP restart required. No user interaction needed.

---

## 4. Python Version Detection {#LSPUV-PYTHON-VERSION}

### 4.1 Resolution Order (uv-aware) {#LSPUV-PYTHON-VERSION-RESOLUTION-ORDER}

Extended version of current Python version detection:

1. `basilisk.python` setting (explicit user override — always wins)
2. `.python-version` file in workspace root (uv standard)
3. `[project].requires-python` in `pyproject.toml` (lower bound)
4. `uv.lock` top-level `requires-python` field
5. Probe `python3 --version` in the detected venv
6. Default: `3.12`

### 4.2 Impact on Type Checking {#LSPUV-PYTHON-VERSION-IMPACT}

The detected Python version controls:

- stdlib module availability (e.g., `tomllib` only in 3.11+)
- Syntax feature support (e.g., `match` in 3.10+, `type` statement in 3.12+)
- `sys.version_info` branch narrowing
- `typing_extensions` vs `typing` import suggestions

---

## 5. Enhanced Diagnostics {#LSPUV-DIAGNOSTICS}

### 5.1 Actionable "Module Not Found" (BSK-E0010) {#LSPUV-DIAGNOSTICS-MODULE-NOT-FOUND}

Current behavior: `"Unresolved import 'requests'"` — useless.

With uv integration, BSK-E0010 becomes context-aware:

| Scenario | Diagnostic Message | Code Action |
|---|---|---|
| Package not installed, not in `pyproject.toml` | `Import "requests" could not be resolved. Package is not a dependency.` | `uv add requests` |
| Package in `pyproject.toml` but env not synced | `Import "requests" could not be resolved. Run "uv sync" to install dependencies.` | `uv sync` |
| Package installed but no stubs | `Import "requests" resolves but has no type stubs.` | `uv add --dev types-requests` |
| Stdlib module, wrong Python version | `Module "tomllib" requires Python >= 3.11 (project targets 3.10)` | — |
| Workspace member, not in deps | `Import "my_lib" resolves as workspace member "packages/my_lib"` | — (info, not error) |

### 5.2 Missing Stub Suggestions (BSK-E0152) {#LSPUV-DIAGNOSTICS-MISSING-STUBS}

Strict-by-default error when a package is installed but has no type information
(opt down with `"BSK-E0152" = "warning"` to import untyped libraries at your own risk).

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

**When no published stub exists** (private/first-party package) — the help points
at the local-stub route and links the official guide instead of a dead end:

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

Per [STUBRES-CODEACTIONS](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CODEACTIONS) the
help describes the fix; it never embeds a shell command — the code action does
the work. The `help`/`note` lines are folded onto the LSP diagnostic message so
editors (which have no `help`/`note` fields) still surface the guidance.

The typeshed stub suggestion (and its `basilisk.uv.addDev` quick fix) is only
emitted when:
- The package IS in `uv.lock` (confirmed installed)
- A matching stub package exists (`types-{name}` or `{name}-stubs`)
- The stub package is NOT already in `uv.lock`

The **create-local-stub** quick fix (`basilisk.stubs.createLocal`, see
[STUBRES-CREATE-LOCAL](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CREATE-LOCAL)) is
offered for **every** BSK-E0152 — it is the only fix when no typeshed stub
exists, and a fallback when one does.

### 5.3 Dependency Hygiene Diagnostics {#LSPUV-DIAGNOSTICS-DEP-HYGIENE}

New optional diagnostics (disabled by default, enabled via config):

| Code | Severity | Description |
|------|----------|-------------|
| BSK-W0011 | Warning | Import of package not listed in `pyproject.toml` dependencies |
| BSK-W0012 | Info | Package in `pyproject.toml` but never imported in project source |
| BSK-W0013 | Warning | `uv.lock` is stale — `pyproject.toml` dependencies changed but lock not updated |

---

## 6. uv Workspace Support {#LSPUV-WORKSPACE}

uv workspaces (inspired by Cargo workspaces) define multi-package monorepos. Basilisk MUST understand them for correct import resolution.

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
3. Workspace members are always considered "typed" (no BSK-E0010 for cross-member imports)
4. The shared `uv.lock` at workspace root governs all third-party resolution

### 6.4 LSP Multi-Root Mapping {#LSPUV-WORKSPACE-MULTI-ROOT}

Each workspace member becomes an LSP workspace folder. The LSP server maintains one `PackageRegistry` per workspace root (shared by all members under that root).

---

## 7. Code Actions {#LSPUV-ACTIONS}

### 7.1 uv-Powered Quick Fixes {#LSPUV-ACTIONS-QUICK-FIXES}

| Trigger | Code Action Title | Command |
|---------|-------------------|---------|
| BSK-E0010 (unresolved, package available) | "Add dependency: `requests`" | `basilisk.uv.add` |
| BSK-E0152 (typeshed stub exists) | "Install type stubs for `requests` (uv add --dev)" | `basilisk.uv.addDev` |
| BSK-E0152 (any — esp. no typeshed stub) | "Create local type stub for `acme_internal`" | `basilisk.stubs.createLocal` |
| BSK-W0013 (stale lock) | "Sync environment" | `basilisk.uv.sync` |

The `basilisk.stubs.createLocal` action writes a permissive `.pyi` skeleton (no
`uv` subprocess) — see [STUBRES-CREATE-LOCAL](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CREATE-LOCAL).

### 7.2 Execution {#LSPUV-ACTIONS-EXECUTION}

Code actions that invoke uv run as **LSP commands** via `workspace/executeCommand`. The LSP spawns `uv` as a subprocess, streams output to the client via `window/logMessage`, and triggers a lock file re-parse on completion.

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

When hovering over an import statement in a uv project, the hover popup includes package metadata:

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

1. Hover handler resolves the import to a module path
2. Module path is matched against `PackageRegistry`
3. If matched, `PackageInfo` metadata is appended to the hover markdown
4. If no match (stdlib, local file), standard hover behavior applies

---

## 9. LSP Commands {#LSPUV-COMMANDS}

New commands registered via `workspace/executeCommand`:

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
- Report failure via `window/showMessage` (error level)

---

## 10. Configuration {#LSPUV-CONFIG}

New settings added to the shared LSP configuration (extends [LSP-ARCHITECTURE-SPEC.md](LSP-ARCHITECTURE-SPEC.md)):

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

uv binary is only needed for **commands** (sync, add, remove). Lock file parsing and environment detection are pure filesystem operations — they work even if `uv` is not installed.

---

## 11. File Watchers {#LSPUV-WATCHERS}

The LSP registers additional file watchers for uv projects:

| Pattern | Event | Action |
|---------|-------|--------|
| `uv.lock` | Create / Change | Re-parse lock, rebuild `PackageRegistry`, re-resolve imports |
| `.python-version` | Create / Change | Update Python version, re-check stdlib availability |
| `pyproject.toml` | Change | Re-detect workspace members, check lock staleness |
| `.venv/pyvenv.cfg` | Create / Delete | Re-detect environment manager |

---

## 12. Logging {#LSPUV-LOGGING}

All uv integration activity is logged at appropriate levels:

| Level | Examples |
|-------|---------|
| `info` | `"Detected uv project at /home/user/myapp"`, `"Parsed uv.lock: 47 packages"` |
| `debug` | `"uv.lock changed: +requests@2.31.0, -urllib3@1.26.0"`, `"Resolved 'requests' → site-packages via uv.lock"` |
| `warn` | `"uv.lock appears stale (pyproject.toml modified after lock)"`, `"uv binary not found — code actions disabled"` |
| `error` | `"Failed to parse uv.lock: invalid TOML at line 42"` |

---

## 13. Non-Goals {#LSPUV-NON-GOALS}

These are explicitly **out of scope**:

- **Replacing uv** — Basilisk delegates package management to uv, never reimplements it
- **Network calls** — No PyPI queries, no package index lookups. Everything comes from local files
- **Poetry/Pipenv/PDM integration** — Different spec. uv first because it's the best match (Rust, fast, TOML lockfile)
- **uv.lock writing** — Basilisk is read-only on lock files. Mutations go through `uv` CLI
- **pip fallback** — If it's not a uv project, existing import resolution handles it. No pip subprocess calls

---

## 14. Interaction with Other Specs {#LSPUV-INTERACTIONS}

| Spec | Interaction |
|------|-------------|
| [LSP-ANALYSIS-MODES-SPEC.md](LSP-ANALYSIS-MODES-SPEC.md) | `PackageRegistry` feeds into import resolution for all analysis modes |
| [CHECKER-CROSS-MODULE-PLAN.md](../plans/CHECKER-CROSS-MODULE-PLAN.md) | Phase 1 (stub infrastructure) gains lock-file-aware stub detection |
| [LSP-ARCHITECTURE-SPEC.md](LSP-ARCHITECTURE-SPEC.md) | New commands, settings, and file watchers added |
| [LSP-MASS-AUTOFIX-SPEC.md](LSP-MASS-AUTOFIX-SPEC.md) | Mass autofix can batch `uv add --dev` for all missing stubs |
| [LSP-TEST-INTEGRATION-SPEC.md](LSP-TEST-INTEGRATION-SPEC.md) | `uv run pytest` for deterministic test execution, `PackageRegistry` verifies pytest/pytest-cov availability, code actions to install missing test dependencies |
