# Spec ↔ Code Conformance Audit — Findings & Remediation {#CONFAUDIT}

Conformance deviations found during the repo-wide spec-ID cross-referencing pass
(every implemented spec section read against its implementing code). Each finding
cites the spec ID so `grep` walks spec → code → this tracker. Grouped by **category**,
because the fix differs:

- **`CONFAUDIT-STALE-SPEC`** — code is correct and intentional; the **spec text is
  stale**. Fix = correct the spec (docs honesty). Some fixed in this change; the rest
  are follow-up.
- **`CONFAUDIT-ROADMAP`** — spec describes a feature **not yet built**, reads as if
  complete, and must carry an honest status marker ([Documentation Honesty](../../CLAUDE.md)).
  Banners added in this change.
- **`CONFAUDIT-CODE-BUG`** — **spec is the intended behaviour and the code deviates**.
  Fix = change code under the TDD bug process (failing test first). Authoritative
  backlog below; none silently "fixed" by weakening a test.

Cross-references to implementing code were added at every site below, with an inline
`NOTE (conformance)` where the code diverges — so no `// Implements [...]` comment makes
a false claim.

---

## `CONFAUDIT-STALE-SPEC` — spec text behind reality {#CONFAUDIT-STALE-SPEC}

| Spec ID | Deviation | Code (authoritative) | Status |
|---|---|---|---|
| `LSPARCH-CMDS` | Command table uses slash form `basilisk/startDebugSession`, `basilisk/profiler/start`, `basilisk/memory/refs`… | Advertised commands are dot form `basilisk.startDebugSession`, `basilisk.profiler.start`, `basilisk.memory.diff`/`.references` (`basilisk-common/src/lib.rs`) | **Fixed in this change** |
| `NVIM-USER-COMMANDS` | Same slash-vs-dot drift in the keymap/command table | Lua calls the dot-form advertised commands | **Fixed in this change** |
| `ZED-STATUS-TESTING` | Claims "33 unit tests" | `basilisk-zed/src/logic_tests.rs` has 97 `#[test]` | **Fixed in this change** (made non-numeric) |
| `ZED-EXTTOML`, `ZED-LIBRS`, `ZED-CARGOTOML`, `ZED-DAP` | Manifest/code examples predate `zed_extension_api` 0.7.0 and the 13 shipped slash commands | `basilisk-zed/{extension.toml,src/lib.rs,Cargo.toml}` | Follow-up (illustrative drift) |
| `VSIX-PYTHON-DEBUGGER-DAP-LAUNCH-CONFIGURATIONS` | Shows debugger `type: "basilisk"` + `env`/`typeChecking` launch fields | Real type is `basilisk-debug`; launch schema differs | Follow-up |
| `VSIX-OUTPUT-CHANNELS` | Says file log at `/tmp/basilisk-debug-trace.log` | Code uses `context.logUri` (secure per-extension dir) — intentional | Follow-up (spec should adopt secure path) |
| `VSIX-STATUS-BAR` | Example uses `$(warning)` for an error count | Code uses `$(error)` for errors, `$(warning)` for warnings (correct) | Follow-up |
| `ANALYSIS-INDEX-STRUCT`, `LSPUV-DETECTION-RESULT`, `LSPUV-WORKSPACE-MODEL`, `LSPUV-LOCK-REGISTRY` | Spec Rust struct shapes are simplified illustrations | Real structs differ (often more correct, e.g. `version: i32`) | Follow-up (refresh examples) |
| `WEBSITE-MOBILE-DOCS-NAV` | Names a non-existent `mobile-menu.js` | Toggle ships from `eleventy-plugin-techdoc`; reveal rule in `styles.css` | Follow-up |
| `STUBRES-PROVENANCE-HOVER` | Tier-2 label wording differs between hover label and diag table | `basilisk-stubs/src/types.rs` `hover_label` | Follow-up (wording) |

---

## `CONFAUDIT-ROADMAP` — spec overstates current reality {#CONFAUDIT-ROADMAP}

| Spec ID | Reality | Status |
|---|---|---|
| `LSPAI` (whole spec) | Only the `AiTypingProvider` trait + `NoOpAiTypingProvider` default + request/response/error types exist (`ai_typing.rs`, ~250 LOC). All providers, features, config, protocol commands, truncation, and tests are unbuilt. | **Status banner added** |
| `COMPILER` (whole spec) | `basilisk-compiler` is a parse→resolve→check→**tree-walking interpreter** (~1.8k LOC). No HIR, LLVM/Cranelift, native codegen, AOT/JIT, interop, runtime, stdlib, or `run`/`build` CLI. The four "new" crates do not exist. | **Status banner added**; false "Cranelift JIT" claim in `compiler/src/lib.rs` corrected |
| `COMPILER-LAYOUT-ISINSTANCE` | `codegen.rs` `isinstance` is a stub returning `true` unconditionally | Roadmap (interpreter limitation) |
| `EXTACT-MODULES-DIAGNOSTICS` | `ModuleNode.diagnostics` drill-down not implemented; the `nE nW` tally has no child rows | Roadmap |
| `AUTOFIX-AI` / `AUTOFIX-AI-SCOPE` | AI-enhanced autofix not built (deterministic mass-fix is) | Roadmap |
| `AUTOFIX-ADOPTION-VSCODE` | No status-bar "Adopted (N)" / gutter icons; auto-graduation never invoked in production | Roadmap |
| `PROFILE-CONFIG-CODES` (`BSK-PROF-GIL`) | GIL-contention diagnostic documented but never emitted | Roadmap |
| `PROFILE-PROCESSES-NOTIFY` | `processesChanged` push reserved/optional; v1 resolves inline | Conformant-by-design |
| `LSPTEST-UV-INTEGRATION-HOT-RELOAD` | uv.lock change does not call back into the test-discovery layer | Roadmap |
| `STUBRES-AUTOGEN` (`--all`) | `basilisk stubs generate --all` prints "not yet implemented" | Roadmap |
| `CHKARCH-CLI-COMMANDS` (`stats`,`migrate`,`init`,`--watch`), `CHKARCH-CLI-OUTPUT` (`sarif`,`junit`), `CHKARCH-CONFIG-MIGRATION` | Commands/formats documented but not implemented | Roadmap |

---

## `CONFAUDIT-CODE-BUG` — code deviates from intended spec (backlog) {#CONFAUDIT-CODE-BUG}

Each needs a failing test then a fix (TDD), per [CLAUDE.md](../../CLAUDE.md). Not
fixed in this change (kept a pure cross-reference/docs pass); filed here so each
deviation is tracked, not hidden.

| Spec ID | Bug | Location |
|---|---|---|
| `LSPUV-WORKSPACE-DETECTION` | `exclude` is parsed but never subtracted from `members`; a test even asserts the excluded member is returned | `basilisk-uv/src/workspace.rs` `resolve_member_patterns` |
| `LSPUV-CONFIG-BINARY-RESOLUTION` | 5-priority `find_uv_binary` cascade has **no caller**; `run_uv` spawns bare `uv`, so `executablePath`/`UV_PATH` are ignored | `basilisk-uv/src/binary.rs` vs `uv_commands.rs` |
| `LSPUV-DETECTION-SIGNALS` | 4th detection signal (`.python-version` w/o poetry/Pipfile lock) not checked | `basilisk-uv/src/detect.rs` |
| `LSPUV-LOCK-IMPORT-MAPPING` | site-packages top-level-module scan (mechanism 1) not implemented | `basilisk-uv/src/import_map.rs` |
| `LSPUV-DIAGNOSTICS-MISSING-STUBS` | `{name}-stubs` stub form not matched (only `types-{name}`) | `basilisk-uv/src/registry.rs` `find_stub_package` |
| `LSPUV-PYTHON-VERSION-RESOLUTION-ORDER` | venv `python3 --version` probe (step 5) missing | `basilisk-uv/src/python_version.rs` |
| `ANALYSIS-INCR-DEBOUNCE` | spec mandates 150 ms; code uses 200 ms | `basilisk-lsp/src/server/mod.rs` `FILE_WATCHER_DEBOUNCE_MS` |
| `LSPTEST-...-ENVIRONMENT-VARIABLES` | `PYTHONDONTWRITEBYTECODE=1` never set | `basilisk-lsp/src/test_discovery/runner.rs` `set_venv_env` |
| `LSPTEST-UV-INTEGRATION-COVERAGE` | emits bare `--cov` instead of `--cov=<src_root>` | `runner.rs` |
| `LSPTEST-...-PYTEST-RESOLUTION-CASCADE` | `pytestPath` priority does not pre-empt `uv run`; cascade order differs | `runner.rs` `run_tests` |
| `LSPTEST-UV-INTEGRATION-TEST-DEPENDENCY-VERIFICATION` | reuses `BSK-W0014` (the explicit-`Any` nudge code) for "pytest not installed" | `test_handlers.rs` `make_pytest_not_found_diagnostic` |
| `REFACTOR-SIGNATURE-OPS` | reorder params sorts alphabetically and edits no call sites; remove param has no "differs-from-default" guard and matches calls by substring | `code_actions/refactor/change_signature.rs` |
| `REFACTOR-MOVE-ALGO` | importers not updated, unused imports not pruned, all imports copied, `__all__` re-export added unconditionally | `code_actions/refactor/move_symbol.rs` |
| `REFACTOR-EXTRACT-VAR-ALGO` | occurrence matching is substring, not AST-structural | `code_actions/refactor/extract.rs` `find_all_occurrences` |
| `REFACTOR-EXTRACT-FUNC-EDGE` | only `yield`/`break`/`continue` handled; `return`, nonlocal/global, loop-scoping rules missing/over-rejecting | `code_actions/refactor/extract_function.rs` |
| `REFACTOR-FORMATTER` | strips whitespace only; never runs ruff | `code_actions/refactor/helpers.rs` `format_inserted_text` |
| `REFACTOR-ABSTRACT-ALGO` | only direct same-module bases; no MRO; body hardcoded (ignores `REFACTOR-CONFIG`) | `code_actions/refactor/abstract_methods.rs` |
| `REFACTOR-RENAME-VALIDATE` | shadowing/builtin-conflict rejections computed then discarded | `references.rs` |
| `AUTOFIX-CONFLICTS` | overlap keeps earlier-by-position fix; never compares `FixSafety` (spec rule 2) | `code_actions/mass_fix.rs` `collect_non_overlapping_edits` |
| `AUTOFIX-ADOPTION-RULES` | `auto_graduate` only called from tests; never runs in production | `basilisk-config/src/adoption.rs` |
| `AUTOFIX-ADOPTION-FLOW` | Adopt does not run safe autofix first; demotions only applied on the adopt-command path, not normal publish | `basilisk-lsp/src/server/adoption.rs`, `workspace_analysis.rs` |
| `TYPEINF-EXCEEDS-NOUNKNOWN` | spec claims no `Unknown` ever produced, but `InferredType::Unknown` is produced for calls/lambdas and treated as bidirectionally compatible | `basilisk-checker/src/{types.rs,inference.rs}` |
| `TYPEINF-SUBTYPING-NOMINAL` | `complex` collapsed into `Float` at parse (loses `float <: complex`); `bytearray <: bytes` not modeled | `basilisk-checker/src/types_parsing.rs` |
| `TYPEINF-VARS-AUGMENTED`, `TYPEINF-NARROWING-ASSIGN`, `TYPEINF-NARROWING-DICTKEY` | not implemented (augmented-assign re-typing; assignment narrowing extraction; `"k" in typed_dict` narrowing) | resolver/checker |
| `TYPEINF-SUBTYPING-IMPL` / `TYPEINF-IMPL` | spec describes an `is_subtype_of(ctx)` engine + named components that don't exist; real path is `InferredType::is_assignable_to` (name-comparison fallback) | `basilisk-checker/src/types.rs` |
| `CHKARCH-CLI-EXITCODES` | exit code `2` (configuration error) never produced; malformed config silently falls back to defaults | `basilisk-cli/src/main.rs`, `basilisk-config/src/lib.rs` |
| `LSPDEBUG-ERRORS` | all `start_session` failures map to `-32002` (spec reserves it for "no Python interpreter") | `basilisk-lsp/src/server/commands.rs` |

---

## `CONFAUDIT-NOTEST` — implemented but no spec-ID-linked test {#CONFAUDIT-NOTEST}

Tracked for added coverage (mutation/coverage ratchets only rise):
`ANALYSIS-CROSSLSP-RENAME` (cross-file), `ANALYSIS-CAPS`, `NVIM-DEFAULT-KEYMAPS-STANDARD-LSP`,
LSP adoption command handlers + `apply_adoptions`, `REFACTOR-FORMATTER`, `REFACTOR-INLINE-VAR-SAFETY`,
`PROFILE-MEMORY-VIS-REFGRAPH`, `PROFILE-PERMISSIONS-WINDOWS` (cfg-gated).
