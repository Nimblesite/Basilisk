# LSP Test Integration Spec

> Single source of truth for test discovery, execution, and result reporting across all editors.
>
> **uv Integration**: [LSP-UV-INTEGRATION-SPEC.md §LSPUV-DETECT](LSP-UV-INTEGRATION-SPEC.md#LSPUV-DETECT) — pytest resolution, environment management, dependency verification

---

## Architecture {#TESTINT-ARCH}

Test integration follows the same subprocess-delegation pattern as formatting (Ruff) and debugging (debugpy):

1. **Discovery** — Basilisk parses Python test files from AST (no import/execution needed) via `basilisk-parser`
2. **Execution** — Delegates to `pytest` subprocess (or `unittest` runner), using `uv run` when a uv project is detected
3. **Result streaming** — Streams pass/fail/skip/error results back to the editor's test UI

The Rust implementation lives in `crates/basilisk-lsp/src/test_discovery.rs`.

---

## Supported Frameworks {#TESTINT-FRAMEWORKS}

| Framework | Detection |
|---|---|
| **pytest** | `def test_*` functions, `@pytest.mark` decorators, `conftest.py` fixtures |
| **unittest** | Classes inheriting `unittest.TestCase`, `setUp`/`tearDown` methods |
| **auto** (default) | Detect framework from `pyproject.toml [tool.pytest]`, `pytest.ini`, or fall back to pytest |

---

## Test Item Data Model {#TESTINT-MODEL}

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

### Test Item Hierarchy {#TESTINT-HIERARCHY}

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

## Test Discovery {#TESTINT-DISCOVERY}

- Scan workspace for `test_*.py` and `*_test.py` files
- Parse with `basilisk-parser` to extract test items without importing
- Detect pytest fixtures, parametrize markers, and unittest `setUp`/`tearDown`
- Auto-refresh on file save (when `basilisk.testExplorer.autoDiscoverOnSave` is enabled)

---

## Test Execution {#TESTINT-EXEC}

- Execute via `pytest` subprocess with node ID targeting (e.g. `pytest tests/test_api.py::test_login`)
- Honour `pytest.ini`, `pyproject.toml [tool.pytest]`, conftest fixtures
- Support running: individual test, test class, test file, entire suite
- Parse output to extract pass/fail/skip/error status per test item

### uv-Aware Execution {#TESTINT-UV}

In uv projects, test execution uses `uv run` instead of invoking pytest directly. This guarantees the correct virtual environment is active without manual `VIRTUAL_ENV` setup.

#### Pytest Resolution {#TESTINT-UV-PYTEST}

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

#### Environment Variables {#TESTINT-UV-ENV}

When NOT using `uv run`, the LSP sets environment variables on the subprocess:

| Variable | Value | Purpose |
|----------|-------|---------|
| `VIRTUAL_ENV` | Detected venv path | Activate the correct environment |
| `PATH` | `{venv}/bin:$PATH` | Ensure venv binaries take precedence |
| `PYTHONDONTWRITEBYTECODE` | `1` | Avoid `.pyc` pollution during test runs |

---

## Configuration Settings {#TESTINT-CONFIG}

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

## Features {#TESTINT-FEATURES}

- **Auto-discovery**: Finds pytest and unittest tests from AST (no import needed)
- **Run/debug individual tests**: Click play on any test function or class
- **Run all**: Run entire test suite from test explorer root
- **Inline failure messages**: Show assertion errors and tracebacks inline
- **Go to test**: Click any test item to navigate to its source
- **Re-run failed**: Quick action to re-run only failed tests
- **Type-checked tests**: Basilisk diagnostics run on test files too
- **Coverage overlay**: Integrate with `pytest-cov` to show coverage gutters
- **uv-aware execution**: Automatically uses `uv run pytest` in uv projects for deterministic environments
- **Test dependency diagnostics**: Warns when pytest or test dependencies are missing from the lock file

---

## uv Integration {#TESTINT-UV-INT}

When a uv project is detected (see [LSP-UV-INTEGRATION-SPEC.md §LSPUV-DETECT](LSP-UV-INTEGRATION-SPEC.md#LSPUV-DETECT)), test integration gains several capabilities. All uv enhancements are additive — non-uv projects behave identically to before.

### Pytest via uv run {#TESTINT-UV-RUN}

In uv projects, `uv run pytest` replaces bare `pytest` invocation. This is the same subprocess-delegation pattern used for `basilisk.uv.sync` and other uv commands (see [LSP-UV-INTEGRATION-SPEC.md §LSPUV-CMDS](LSP-UV-INTEGRATION-SPEC.md#LSPUV-CMDS)).

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

### Test Dependency Verification {#TESTINT-UV-DEPS}

The `PackageRegistry` (built from `uv.lock`) enables test dependency diagnostics:

| Condition | Diagnostic | Severity | Code Action |
|-----------|-----------|----------|-------------|
| pytest not in `uv.lock` | `Test runner \"pytest\" is not installed. Run \"uv add --dev pytest\" to install.` | Warning | `basilisk.uv.addDev` with `pytest` |
| `pytest-cov` not in `uv.lock` (coverage requested) | `Coverage plugin \"pytest-cov\" is not installed.` | Info | `basilisk.uv.addDev` with `pytest-cov` |
| Test imports unresolved package | Standard BSK-E0010 with uv context (see [LSP-UV-INTEGRATION-SPEC.md §LSPUV-DIAG-E0010](LSP-UV-INTEGRATION-SPEC.md#LSPUV-DIAG-E0010)) | Error | `basilisk.uv.add` |

These diagnostics are only emitted in uv projects and respect the existing `basilisk.uv.enabled` setting.

### Coverage with uv run {#TESTINT-UV-COV}

When coverage is enabled, the LSP invokes:

```
uv run pytest --cov=<src_root> --cov-report=xml:<workspace>/.basilisk/coverage.xml <test_ids>
```

This ensures `pytest-cov` resolves from the uv-managed environment. The coverage XML path is deterministic, enabling the file watcher to detect changes and push `basilisk/coverageResult` notifications.

### Hot Reload Interaction {#TESTINT-UV-RELOAD}

When `uv.lock` changes (package added/removed), the existing hot reload pipeline ([LSP-UV-INTEGRATION-SPEC.md §LSPUV-LOCK-RELOAD](LSP-UV-INTEGRATION-SPEC.md#LSPUV-LOCK-RELOAD)) rebuilds the `PackageRegistry`. The test integration layer checks:

1. Was `pytest` added or removed? → Update test runner availability status
2. Were test dependencies (fixtures, plugins) added or removed? → Trigger re-discovery notification
3. Was `pytest-cov` added or removed? → Update coverage availability

---

## Editor-Specific Integration {#TESTINT-EDITORS}

Each editor implements test integration using its native test UI. Behavioral details live here; only editor-specific API wiring belongs in the editor specs.

### VS Code {#TESTINT-VSCODE}

- Implement `TestController` via VS Code's `vscode.tests` API
- Stream results back to Test Explorer as pass/fail/skip/error
- Debug integration via existing DAP proxy (see [LSP-DEBUG-INTEGRATION-SPEC.md §LSPDEBUG-ARCH](LSP-DEBUG-INTEGRATION-SPEC.md#LSPDEBUG-ARCH))

### Neovim {#TESTINT-NEOVIM}

- **Discovery**: Run `pytest --collect-only -q`, parse output into tree
- **Tree UI**: Dedicated side-panel buffer with `basilisk-tests` filetype
  - Hierarchical rendering: File > Class > Function
  - Status icons: pass/fail/running/unknown
  - Keymaps: `<CR>` run, `d` debug, `R` re-run failed, `q` close
- **Run**: Spawn pytest subprocess, parse output, update tree status
- **Debug**: Trigger nvim-dap with specific test as target
- **Inline failures**: `vim.diagnostic.set()` in `basilisk-test` namespace
- **Coverage**: Parse `coverage.xml`, display as extmark gutter highlights

### Zed {#TESTINT-ZED}

- TBD — Zed lacks a native test explorer panel API. Will evaluate when available.

---

## LSP Protocol {#TESTINT-PROTOCOL}

### Commands {#TESTINT-PROTOCOL-CMDS}

| Command | Description |
|---|---|
| `basilisk.runTests` | Run all tests in workspace |
| `basilisk.runTestFile` | Run tests in current file |
| `basilisk.debugTest` | Debug a specific test |

### Custom Notifications {#TESTINT-PROTOCOL-NOTIF}

| Direction | Method | Payload |
|---|---|---|
| Server → Client | `basilisk/testDiscoveryResult` | `{ items: TestItem[] }` |
| Server → Client | `basilisk/testRunResult` | `{ id: string, status: pass/fail/skip/error, message?: string }` |
| Client → Server | `basilisk/discoverTests` | `{ uri: string }` |
| Client → Server | `basilisk/runTest` | `{ id: string, debug: boolean }` |

### uv Command Interaction {#TESTINT-UV-CMDS}

Test-related code actions that invoke uv (e.g., "Add pytest" on missing test runner) reuse the existing `basilisk.uv.addDev` command. After the uv command completes, the post-command hook ([LSP-UV-INTEGRATION-SPEC.md §LSPUV-ACTIONS-EXEC](LSP-UV-INTEGRATION-SPEC.md#LSPUV-ACTIONS-EXEC)) triggers lock re-parse and registry rebuild, which in turn updates test runner availability.
