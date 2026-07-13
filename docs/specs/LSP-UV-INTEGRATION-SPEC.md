# Basilisk uv integration {#LSPUV}

The uv integration detects projects from files, parses lock/project metadata in-process,
enriches import analysis, and delegates package mutations to the `uv` executable.

## Approach {#LSPUV-WHY}

Detection, lock parsing, registry lookup, and version selection do not spawn uv. Only explicit
package-management commands start a subprocess. The Rust implementation is split between
`basilisk-uv` and the LSP's uv handlers.

## Detection {#LSPUV-DETECTION}

### Signals {#LSPUV-DETECTION-SIGNALS}

The first workspace root matching any signal is treated as a uv project:

- `uv.lock` exists;
- `pyproject.toml` contains `[tool.uv]`;
- `.venv/pyvenv.cfg` contains `uv = true`; or
- `.python-version` exists and neither `poetry.lock` nor `Pipfile.lock` exists.

Detection is a filesystem/TOML check; it does not run `uv` or Python.

### Result {#LSPUV-DETECTION-RESULT}

`UvProjectInfo` contains the root and four booleans: `has_lockfile`, `has_tool_uv`,
`uv_managed_venv`, and `has_python_version`.

### Fallback {#LSPUV-DETECTION-FALLBACK}

When no signal matches, ordinary Basilisk project discovery continues. uv-specific registry
enrichment and commands are not required for type checking.

## Lock-file intelligence {#LSPUV-LOCK}

### Parsed data {#LSPUV-LOCK-EXTRACT}

`parse_lock_file` deserializes lock version, `requires-python`, packages, package versions,
sources, runtime dependencies, grouped dev dependencies, and per-dependency markers. Unknown
fields are retained as TOML values for forward-compatible parsing. Top-level
`resolution-markers` are not interpreted.

### Package registry {#LSPUV-LOCK-REGISTRY}

`PackageRegistry` is keyed by Python import name. Each `PackageInfo` stores distribution name,
version, import name, dependency kind (`Direct`, `Dev`, or `Transitive`), and whether its
source is editable. Direct dependencies come from `[project].dependencies`; dev names come
from lock-file dev groups; the remainder are transitive.

### Import-name mapping {#LSPUV-LOCK-IMPORT-MAPPING}

Distribution names use a curated mismatch table first (for example Pillow → `PIL`) and
otherwise lowercase and replace hyphens with underscores. The implementation does not scan
site-packages to discover import roots, so unmapped multi-root distributions may be missed.

### Hot reload {#LSPUV-LOCK-HOT-RELOAD}

Relevant file changes and successful uv commands rebuild the full registry and recheck the
workspace. There is no package-level diff path.

## Target Python version {#LSPUV-PYTHON-VERSION}

### Resolution order {#LSPUV-PYTHON-VERSION-RESOLUTION-ORDER}

An explicit Basilisk config value is applied by the consumer first. Otherwise
`basilisk-uv` checks `.python-version`, then the lower bound of
`[project].requires-python`, then the lock file's `requires-python`. If none exists, the
checker uses its centralized default. No interpreter subprocess is probed.

## Diagnostics {#LSPUV-DIAGNOSTICS}

### Unresolved imports {#LSPUV-DIAGNOSTICS-MODULE-NOT-FOUND}

When an import cannot be resolved, registry knowledge can distinguish a missing dependency
from a known installed distribution and offer `uv add` when appropriate. The checker rule
catalog owns the diagnostic code and severity.

### Missing stubs {#LSPUV-DIAGNOSTICS-MISSING-STUBS}

The stub-distribution index can offer `uv add --dev <stub-package>` for a known companion
package. If no published mapping exists, the LSP can offer the local-stub command instead.
Resolution and provenance are specified in
[CHECKER-STUB-RESOLUTION-SPEC.md](CHECKER-STUB-RESOLUTION-SPEC.md).

## uv workspaces {#LSPUV-WORKSPACE}

### Detection {#LSPUV-WORKSPACE-DETECTION}

`parse_uv_workspace` reads `[tool.uv.workspace]`, expands literal members and simple trailing
`/*` patterns, and returns sorted existing directories. It parses `exclude`, but currently
does not subtract excluded members.

### Model {#LSPUV-WORKSPACE-MODEL}

`UvWorkspace` contains only `members: Vec<PathBuf>` and `exclude: Vec<String>`. There is no
rich member object or independent multi-root registry model.

### Import resolution {#LSPUV-WORKSPACE-IMPORT-RESOLUTION}

Discovered member directories and their conventional source roots can be added to import
search. The helper that derives member folders is not currently wired into production LSP
multi-root routing; this section does not promise per-member server ownership.

## Code actions {#LSPUV-ACTIONS}

### Quick fixes {#LSPUV-ACTIONS-QUICK-FIXES}

uv-aware actions invoke the shared server commands for adding a dependency, adding a dev
stub package, or synchronizing the environment. Action availability is derived from current
diagnostics and registry data.

### Execution {#LSPUV-ACTIONS-EXECUTION}

Commands run with the workspace root as their working directory, capture stdout/stderr, and
time out after 30 seconds. Output is returned after process completion rather than streamed.
A successful command rebuilds project state before diagnostics are republished.

## Hover {#LSPUV-HOVER}

Import hover can add the resolved distribution name/version, direct/dev/transitive kind, and
editable status from `PackageRegistry`.

### Data flow {#LSPUV-HOVER-DATA-FLOW}

Lock/project files → `PackageRegistry` → import-name lookup → hover suffix. Missing
registry entries leave ordinary hover unchanged.

## Commands {#LSPUV-COMMANDS}

| LSP command | Subprocess |
|---|---|
| `basilisk.uv.sync` | `uv sync` |
| `basilisk.uv.add` | `uv add <package>` |
| `basilisk.uv.addDev` | `uv add --dev <package>` |
| `basilisk.uv.remove` | `uv remove <package>` |
| `basilisk.uv.lock` | `uv lock` |
| `basilisk.uv.createEnv` | `uv venv [--python <version>]` |

### Failure UX {#LSPUV-COMMAND-FAILURE-UX}

Spawn failures and stderr are classified as package-not-found, resolution-conflict, network,
uv-not-found, or generic. Clients receive a short remediation message while full captured
output remains available in the Basilisk output channel.

## Binary resolution status {#LSPUV-CONFIG-BINARY-RESOLUTION}

`basilisk-uv::find_uv_binary` implements a candidate-path helper, but LSP commands do not call
it: they currently spawn bare `uv` through the process `PATH`. Editor settings named
`enabled`, `executablePath`, and `autoSync` are declared but do not currently control this
server path. Wiring these settings is tracked by the conformance-audit plan.

## Watchers {#LSPUV-WATCHERS}

The server refresh path watches changes to `uv.lock`, `pyproject.toml`, and
`.python-version`. `.venv/pyvenv.cfg` participates in startup detection but is not watched.
