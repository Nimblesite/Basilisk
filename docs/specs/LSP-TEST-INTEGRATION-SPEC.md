# LSP Test Integration Spec {#LSPTEST}

> Single source of truth for test discovery, execution, and result reporting across all editors.
>
> **uv Integration**: [LSP-UV-INTEGRATION-SPEC.md](LSP-UV-INTEGRATION-SPEC.md) — pytest resolution, environment management, dependency verification

---

## Architecture {#LSPTEST-ARCHITECTURE}

Same subprocess-delegation pattern as debugging (debugpy) — note formatting is now **in-process** via the embedded Ruff crate, not a subprocess ([LSPFMT-ENGINE](LSP-FORMATTING-SPEC.md#LSPFMT-ENGINE)):

1. **Discovery** — parse test files from AST via `basilisk-parser` (no import/execution)
2. **Execution** — delegate to a `pytest` subprocess (or `unittest` runner), using `uv run` when a uv project is detected
3. **Result streaming** — stream pass/fail/skip/error results to the editor's test UI

Rust implementation: `crates/basilisk-lsp/src/test_discovery.rs`.

---

## Supported Frameworks {#LSPTEST-SUPPORTED-FRAMEWORKS}

| Framework | Detection |
|---|---|
| **pytest** | `def test_*` functions, `@pytest.mark` decorators, `conftest.py` fixtures |
| **unittest** | Classes inheriting `unittest.TestCase`, `setUp`/`tearDown` methods |
| **auto** (default) | Detect framework from `pyproject.toml [tool.pytest]`, `pytest.ini`, or fall back to pytest |

---

## Test Item Data Model {#LSPTEST-TEST-ITEM-DATA-MODEL}

```rust
pub struct TestItem {
    pub name: String,          // e.g. "test_login", "TestUserEndpoints::test_get_user"
    pub id: String,            // e.g. "tests/test_api.py::test_login"
    pub file: PathBuf,         // file where test is defined
    pub line: usize,           // 0-based line number
    pub kind: TestItemKind,    // File | Function | Class | Method
    pub children: Vec<TestItem>,
}
```

### Test Item Hierarchy {#LSPTEST-TEST-ITEM-DATA-MODEL-HIERARCHY}

```
tests/
    test_api.py
        test_login
        test_signup -- AssertionError: expected 200, got 401
        TestUserEndpoints
            test_get_user
            test_delete_user
            test_update_user
    test_models.py
        test_create_widget
        test_slow_query (skipped)
```

---

## Test Discovery {#LSPTEST-TEST-DISCOVERY}

- Scan workspace for `test_*.py` and `*_test.py` files
- Parse with `basilisk-parser` to extract test items without importing
- Detect pytest fixtures, parametrize markers, and unittest `setUp`/`tearDown`
- Auto-refresh on file save (when `basilisk.testExplorer.autoDiscoverOnSave` is enabled)

---

## Test Execution {#LSPTEST-TEST-EXECUTION}

- Execute via `pytest` subprocess with node ID targeting (e.g. `pytest tests/test_api.py::test_login`)
- Honour `pytest.ini`, `pyproject.toml [tool.pytest]`, conftest fixtures
- Support running: individual test, test class, test file, entire suite
- Parse output to extract pass/fail/skip/error status per test item

### uv-Aware Execution {#LSPTEST-TEST-EXECUTION-UV-AWARE}

In uv projects, test execution uses `uv run` rather than invoking pytest directly, guaranteeing the correct virtual environment without manual `VIRTUAL_ENV` setup.

#### Pytest Resolution Cascade {#LSPTEST-TEST-EXECUTION-UV-AWARE-PYTEST-RESOLUTION-CASCADE}

| Priority | Strategy | When |
|----------|----------|------|
| 1 | `basilisk.testExplorer.pytestPath` | User explicitly configured a path |
| 2 | `uv run pytest` | uv project detected and `uv` binary available |
| 3 | `{venv}/bin/pytest` | Virtual environment detected (non-uv) |
| 4 | `pytest` on PATH | Fallback — bare system pytest |

When `uv run` is used:
- uv handles venv activation, `VIRTUAL_ENV`, and `PATH` setup automatically
- The lock file's resolved dependency versions are guaranteed to be active
- No risk of running tests against a stale or wrong environment
- Coverage tools (`pytest-cov`) installed via `uv add --dev` are available without extra config

#### Environment Variables {#LSPTEST-TEST-EXECUTION-UV-AWARE-ENVIRONMENT-VARIABLES}

When NOT using `uv run`, the LSP sets environment variables on the subprocess:

| Variable | Value | Purpose |
|----------|-------|---------|
| `VIRTUAL_ENV` | Detected venv path | Activate the correct environment |
| `PATH` | `{venv}/bin:$PATH` | Ensure venv binaries take precedence |
| `PYTHONDONTWRITEBYTECODE` | `1` | Avoid `.pyc` pollution during test runs |

---

## Configuration Settings {#LSPTEST-CONFIGURATION-SETTINGS}

> These settings are shared across all editors. Each editor spec should reference this table rather than duplicating it.

| Setting | Type | Default | Description |
|---|---|---|---|
| `basilisk.testExplorer.enabled` | `boolean` | `true` | Enable test discovery and execution |
| `basilisk.testExplorer.framework` | `enum` | `"auto"` | `pytest` / `unittest` / `auto` |
| `basilisk.testExplorer.pytestPath` | `string` | `"pytest"` | Path to pytest executable (overrides uv resolution) |
| `basilisk.testExplorer.args` | `string[]` | `[]` | Additional test runner arguments |
| `basilisk.testExplorer.autoDiscoverOnSave` | `boolean` | `true` | Re-discover tests on file save |
| `basilisk.testExplorer.useUvRun` | `boolean` | `true` | Use `uv run pytest` in uv projects (auto-disabled if not a uv project) |

---

## uv Integration {#LSPTEST-UV-INTEGRATION}

When a uv project is detected (see [LSP-UV-INTEGRATION-SPEC.md §2](LSP-UV-INTEGRATION-SPEC.md)), test integration gains capabilities. All uv enhancements are additive — non-uv projects are unaffected.

### Pytest Resolution via `uv run` {#LSPTEST-UV-INTEGRATION-PYTEST-RESOLUTION}

In uv projects, `uv run pytest` replaces bare `pytest` — the same subprocess-delegation pattern as `basilisk.uv.sync` (see [LSP-UV-INTEGRATION-SPEC.md §9](LSP-UV-INTEGRATION-SPEC.md)).

```rust
pub fn build_test_command(
    test_config: &TestConfig,
    uv_info: Option<&UvProjectInfo>,
    uv_binary: Option<&Path>,
) -> Command {
    // Priority 1: Explicit pytestPath overrides everything
    if test_config.pytest_path != "pytest" {
        return Command::new(&test_config.pytest_path);
    }

    // Priority 2: uv run pytest (uv project + binary available + setting enabled)
    if let (Some(info), Some(uv)) = (uv_info, uv_binary) {
        if test_config.use_uv_run {
            let mut cmd = Command::new(uv);
            cmd.arg("run").arg("pytest");
            cmd.current_dir(&info.project_root);
            return cmd;
        }
    }

    // Priority 3: venv pytest / bare pytest
    Command::new(&test_config.pytest_path)
}
```

### Test Dependency Verification {#LSPTEST-UV-INTEGRATION-TEST-DEPENDENCY-VERIFICATION}

The `PackageRegistry` (built from `uv.lock`) enables test dependency diagnostics:

| Condition | Diagnostic | Severity | Code | Code Action |
|-----------|-----------|----------|------|-------------|
| pytest not in `uv.lock` | `Test runner \"pytest\" is not installed. Run \"uv add --dev pytest\" to install.` | Warning | `BSK-W0015` | `basilisk.uv.addDev` with `pytest` |
| `pytest-cov` not in `uv.lock` (coverage requested) | `Coverage plugin \"pytest-cov\" is not installed.` | Info | — | `basilisk.uv.addDev` with `pytest-cov` |
| Test imports unresolved package | Standard imports_unresolved with uv context (see [LSP-UV-INTEGRATION-SPEC.md §5](LSP-UV-INTEGRATION-SPEC.md)) | Error | — | `basilisk.uv.add` |

These diagnostics are only emitted in uv projects and respect the existing `basilisk.uv.enabled` setting.

### Coverage with `uv run` {#LSPTEST-UV-INTEGRATION-COVERAGE}

When coverage is enabled, the LSP invokes:

```
uv run pytest --cov=<src_root> --cov-report=xml:<workspace>/.basilisk/coverage.xml <test_ids>
```

`pytest-cov` resolves from the uv-managed environment. The deterministic coverage XML path lets the file watcher detect changes and push `basilisk/coverageResult` notifications.

### Hot Reload Interaction {#LSPTEST-UV-INTEGRATION-HOT-RELOAD}

When `uv.lock` changes, the hot reload pipeline ([LSP-UV-INTEGRATION-SPEC.md §3.4](LSP-UV-INTEGRATION-SPEC.md)) rebuilds the `PackageRegistry`; the test layer reacts:

1. `pytest` added/removed → update test runner availability status
2. test dependencies (fixtures, plugins) added/removed → trigger re-discovery notification
3. `pytest-cov` added/removed → update coverage availability

---

## Editor-Specific Integration {#LSPTEST-EDITOR-SPECIFIC-INTEGRATION}

Each editor wires its native test UI to this spec; only editor-specific API wiring belongs in the editor specs.

### VS Code {#LSPTEST-EDITOR-SPECIFIC-INTEGRATION-VSCODE}

- Implement `TestController` via VS Code's `vscode.tests` API
- Stream results back to Test Explorer as pass/fail/skip/error
- Debug integration via existing DAP proxy (see `LSP-DEBUG-INTEGRATION-SPEC.md`)

### Neovim {#LSPTEST-EDITOR-SPECIFIC-INTEGRATION-NEOVIM}

- **Discovery**: Run `pytest --collect-only -q`, parse output into tree
- **Tree UI**: Dedicated side-panel buffer with `basilisk-tests` filetype
  - Hierarchical rendering: File > Class > Function
  - Status icons: pass/fail/running/unknown
  - Keymaps: `<CR>` run, `d` debug, `R` re-run failed, `q` close
- **Run**: Spawn pytest subprocess, parse output, update tree status
- **Debug**: Trigger nvim-dap with specific test as target
- **Inline failures**: `vim.diagnostic.set()` in `basilisk-test` namespace
- **Coverage**: Parse `coverage.xml`, display as extmark gutter highlights

#### e2e harness result gate {#LSPTEST-EDITOR-SPECIFIC-INTEGRATION-NEOVIM-E2E-GATE}

The Neovim e2e suite runs via `PlenaryBustedDirectory tests/lsp` (one child nvim
per `*_spec.lua`, `sequential = true` so luacov stats merge without racing). The
verdict is determined by **parsing the run output**, not the nvim exit code,
which is unreliable both ways: the parent nvim can exit non-zero on teardown (a
lingering LSP child or async handle reaped late under `make ci`'s `-j3` load) even
when all tests passed, and a run that silently executed no tests exits zero.

`assert_plenary_pass` (`scripts/common.sh`, used by `scripts/test-nvim.sh`)
passes **iff** all four hold: every spec file started (one `Testing:` line each),
every spec file emitted a final `Success:` summary, zero tests failed, zero tests
errored, and no Lua traceback / nvim runtime error appeared. The exit code is
logged for diagnostics but is not authoritative.

---

## LSP Protocol {#LSPTEST-LSP-PROTOCOL}

### Commands {#LSPTEST-LSP-PROTOCOL-COMMANDS}

| Command | Description |
|---|---|
| `basilisk.runTests` | Run all tests in workspace |
| `basilisk.runTestFile` | Run tests in current file |
| `basilisk.debugTest` | Debug a specific test |

### Custom Notifications (planned) {#LSPTEST-LSP-PROTOCOL-CUSTOM-NOTIFICATIONS}

| Direction | Method | Payload |
|---|---|---|
| Server → Client | `basilisk/testDiscoveryResult` | `{ items: TestItem[] }` |
| Server → Client | `basilisk/testRunResult` | `{ id: string, status: pass/fail/skip/error, message?: string }` |
| Client → Server | `basilisk/discoverTests` | `{ uri: string }` |
| Client → Server | `basilisk/runTest` | `{ id: string, debug: boolean }` |

### Interaction with uv Commands {#LSPTEST-LSP-PROTOCOL-UV-INTERACTION}

Test code actions invoking uv (e.g. "Add pytest" on a missing test runner) reuse `basilisk.uv.addDev`. Its post-command hook ([LSP-UV-INTEGRATION-SPEC.md §7.2](LSP-UV-INTEGRATION-SPEC.md)) re-parses the lock and rebuilds the registry, updating test runner availability.
