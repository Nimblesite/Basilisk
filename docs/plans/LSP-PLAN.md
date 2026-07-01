# LSP Implementation Plan {#LSPPLAN}

> **Spec**: [LSP-ARCHITECTURE-SPEC.md §LSPARCH-ARCH](../specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH) — read before touching any code.

---

## Status {#LSPPLAN-STATUS}

Phases 0–6 COMPLETE. Phase 7 (cross-module foundation) MOSTLY COMPLETE — stub infrastructure, import graph, cross-file symbols operational. Phase 7.5 (PEP conformance push) ACTIVE.

Score: the official `python/typing` scorer (unmodified, pinned commit 268d0c4e) reports **68/146 files passing (46.6%, errors+warnings strictest)**, binary run with EVERY rule enabled — no config, no `basilisk.json`, no "spec-conformance mode" (no such mode exists — see CHKARCH-CONFORMANCE-MODE). 0 missed required errors; the 265 false positives all come from strict-by-default house-style rules (require-annotation E0001/E0002/E0004, missing-@override E0025, explicit-Any W0014, redundant-annotation W0050) firing on spec-valid code where the spec treats unannotated as inferred. The only legitimate path to 100% is fixing the checker so its strict defaults stop firing — never disabling a rule.

History: last honest score was 59/146 = 40.4% (285 FPs) at PR #183; PRs #184/#185/#191 inflated it to a fake 100% via a `basilisk.json` that disabled those 6 house rules at score time. That disabling is REMOVED and FORBIDDEN; genuine progress over that span was 40.4% → 46.6%.

---

## Phase 7 — Cross-Module Foundation (MOSTLY COMPLETE) {#LSPPLAN-CROSS-MODULE-FOUNDATION}

> Module resolver, import graph, cross-file symbol sharing operational. Remaining: re-exports, rename, auto-import, multi-root.

| Task | Description | Difficulty | Status |
|------|-------------|------------|--------|
| 7.1 | Workspace module resolver — scan workspace, resolve `import X` to file paths | Hard | DONE — `import_resolver.rs` resolves imports, `workspace.rs` scans files |
| 7.2 | Multi-file `ResolvedModule` graph — cross-file symbol sharing | Hard | DONE — `imported_symbols: HashMap<String, ExternalSymbol>` on `ResolvedModule`, populated by `cross_module.rs` |
| 7.3 | Import graph — topological ordering, cycle detection, incremental invalidation | Medium | DONE — `import_graph.rs` with forward+reverse edges, Kahn's algorithm, DFS cycle detection |
| 7.4 | Salsa integration — memoized incremental computation (like rust-analyzer) | Hard | PARTIAL — engine landed ([CHKARCH-INCREMENTAL-SALSA](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA)): `salsa` 0.27 dependency, `basilisk-db` (`SourceFile` input + `BasiliskDatabase`), and the `checked_file` tracked query in `basilisk-checker` memoizing `parse → resolve → check`. `BasiliskConfig` is now a tracked salsa input (`ConfigInput`/`ConfigValue`), so the query honours configuration via `check_with_config`. Tests prove memoization, source- and config-edit invalidation, cross-file isolation, cancellation, and byte-for-byte equivalence to the direct pipeline. NOT yet wired into the CLI/LSP published-diagnostics paths — covers the configurable, import-free pipeline only, so it cannot affect the conformance score. Remaining: promote import search paths to a salsa input, fold in `resolve_module_imports`, then switch the LSP (and CLI) onto `file_diagnostics`. The `DashMap` + `Arc` path remains canonical until then |
| 7.5 | Stub file (`.pyi`) support — resolve type info from `.pyi` alongside `.py` | Medium | DONE — full `.pyi` parser in `pyi_parser.rs`, PEP 561 resolution order implemented |
| 7.6 | Third-party type stubs — typeshed bundling, `py.typed` marker detection (PEP 561) | Medium | DONE — `phf` stdlib module set, `py.typed` detection, stub package discovery |
| 7.7 | Config file reading — `pyproject.toml`, `basilisk.json` | Medium | DONE — `basilisk-config` crate with per-module/per-path overrides |

### Task 7.4 — Salsa incremental engine: status at a glance {#LSPPLAN-SALSA-STATUS}

> Spec: [CHKARCH-INCREMENTAL-SALSA](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA). The engine is built and proven correct, but is **not yet on any user-visible path** — nothing in the CLI or LSP calls it, so it cannot affect output or the conformance score until wired in.

**Done:**
- [x] `salsa` 0.27 dependency; `basilisk-db` is a real database, not the old stub.
- [x] `SourceFile` input query (`path` + `text`) and `BasiliskDatabase` (`crates/basilisk-db/src/db.rs`); `Debug` redacted so source text never reaches logs.
- [x] `checked_file` `#[salsa::tracked]` query memoizing `parse → resolve → check_with_config`, keyed on the `(file, config)` pair and returning owned `CachedDiagnostic`s (`crates/basilisk-checker/src/incremental.rs`); plus `file_diagnostics` convenience.
- [x] **`BasiliskConfig` is a salsa input** — `ConfigInput`/`ConfigValue` wrap it in `basilisk-checker` (salsa-free `basilisk-config` gains only `PartialEq`/`Eq`); the `salsa::Update` resolves through `PartialEq`, mirroring `CachedDiagnostic`. Editing the config invalidates exactly the affected files.
- [x] Tests: memoization, source- AND config-edit invalidation, nested-override-map invalidation, cross-file isolation, cancellation, no-source-leak, config-awareness (`strict_annotations` → BSK-E0001 differential), and byte-for-byte equivalence to the direct pipeline (`basilisk-db/tests/db_tests.rs`, `basilisk-checker/tests/incremental_tests.rs`).
- [x] **Import engine hoisted into `basilisk-checker`** (`crates/basilisk-checker/src/imports/`) — the filesystem-pure resolver (`resolve_module`, `resolve_module_imports`, `ImportSearchPaths`, …) moved down from `basilisk-lsp` (which now re-exports it, and builds an `ImportSearchPaths` via `search_paths_from_config`). Pure refactor: zero behaviour change, all tests relocated and green. This dissolves the checker→lsp layering blocker so import resolution can be folded into the tracked query next.

**Remaining (blocks adoption):**
- [ ] Give the relocated `ImportSearchPaths` salsa identity (`PartialEq`/`Eq` + a stable identity for its `Arc<PackageRegistry>`) and make it a salsa input, then fold `resolve_module_imports` into the tracked query (today it runs between resolve and check, reads the filesystem, and so changes `imports_unresolved` + cascade suppression). Document its filesystem-impurity boundary like [CHKCACHE-LIMITS].
- [ ] Wire the LSP diagnostics path onto `file_diagnostics`; then the CLI batch path.
- [ ] Cross-session salsa persistence (today cross-session is the separate content-addressed result cache, [CHKCACHE](../specs/CHECKER-CACHE-SPEC.md)).
- [ ] Finer-than-module granularity (per-function) — optional, not yet implemented.

## Phase 7.5 — PEP Conformance Push (ACTIVE — 46.6% → 100%) {#LSPPLAN-PEP-CONFORMANCE-PUSH}

> BLOCKING for Phase 9. Adds the type-system capabilities needed to stop false positives and catch the errors conformance expects.

### Tier 1 — Medium complexity, highest ROI {#LSPPLAN-PEP-CONFORMANCE-PUSH-TIER-MEDIUM-ROI}

| Task | Conformance files it flips | Complexity | Status |
|------|---------------------------|------------|--------|
| NamedTuple constructor arg count + type validation | namedtuples_define_class.py | Medium | IN PROGRESS |
| TypedDict `extra_items` kwarg in resolver | typeddicts_extra_items.py | Medium | TODO |
| Class inheritance in TypeVar constraints | generics_basic.py | Medium | TODO |
| Protocol structural subtyping (attrs satisfy properties) | protocols_definition.py | High | TODO |

### Tier 2 — High complexity, massive impact {#LSPPLAN-PEP-CONFORMANCE-PUSH-TIER-HIGH-IMPACT}

| Task | Conformance files it flips | Complexity | Status |
|------|---------------------------|------------|--------|
| TypeVarTuple semantics | 6-8 generics files | Very High | TODO |
| ParamSpec semantics | 2-3 generics files | Very High | TODO |
| Variance (covariant/contravariant) | protocols_generic.py + others | High | TODO |
| Dead branch elimination (`sys.version_info`) | directives_version_platform.py | High | TODO |

### Completed this sprint {#LSPPLAN-PEP-CONFORMANCE-PUSH-COMPLETED-SPRINT}
- [x] E0130: Module-level type alias TypeVar, Protocol[T] binding, multi-line sigs
- [x] E0111: Skip dataclass/TypedDict synthesized constructors
- [x] E0092: TypeVarTuple via Expr::Starred in name collection
- [x] E0111: NamedTuple constructor arg count validation
- [x] FP reduction: 435 → 294 unexpected diagnostics

## Phase 8 — Cross-Module Features (requires Phase 7) {#LSPPLAN-CROSS-MODULE-FEATURES}

| Task | Description | Difficulty | Status |
|------|-------------|------------|--------|
| 8.1 | Cross-file Go to Definition | Medium | DONE |
| 8.2 | Cross-file Find All References | Medium | DONE |
| 8.3 | Handle re-exports in Go to Definition | Medium | TODO |
| 8.4 | Cross-file Rename — multi-file `WorkspaceEdit` | Hard | TODO |
| 8.5 | Auto-import suggestions — suggest imports from workspace index | Hard | TODO |
| 8.6 | Module-level auto-import index with depth control | Hard | TODO |
| 8.7 | Multi-root workspace support | Medium | TODO |

## Phase 9 — Advanced Type Inference (requires Phase 7.5) {#LSPPLAN-ADVANCED-TYPE-INFERENCE}

> Full type inference engine — the core of Pyright/Pylance parity.

| Task | Description | Difficulty | Status |
|------|-------------|------------|--------|
| 9.1 | TypeVarTuple/ParamSpec full semantics | Very Hard | TODO |
| 9.2 | Variance tracking (covariant/contravariant/invariant) | Hard | TODO |
| 9.3 | Protocol structural subtyping | Hard | TODO |
| 9.4 | Conditional flow-based type narrowing (isinstance, is None, truthiness) | Very Hard | TODO |
| 9.5 | Class hierarchy awareness in type inference | Medium | TODO |
| 9.6 | Dead branch elimination (sys.version_info, sys.platform) | Hard | TODO |
| 9.7 | Extract variable / Extract method (code actions) | Medium | TODO |
| 9.8 | Implement abstract methods (code action) | Medium | TODO |
| 9.9 | Override stub completions | Medium | TODO |
| 9.10 | Move symbol to existing/new file (code action) | Hard | TODO |

---

## Rules {#LSPPLAN-RULES}

- Build must stay GREEN at all times
- No `.unwrap()` in server code
- No `println!` in production code (LSP stdout is sacred)
- `cargo clippy` must pass after every task
- E2E tests for every feature — no unit test theatre
- Do NOT delete failing tests — add more

---

## Detailed TODO — Pylance Parity {#LSPPLAN-PYLANCE-PARITY}

> Reference: [Pylance marketplace](https://marketplace.visualstudio.com/items?itemName=ms-python.vscode-pylance), [Pyright docs](https://microsoft.github.io/pyright/#/)
> Pylance PEP support: 484, 487, 526, 544, 561, 563, 570, 585, 586, 589, 591, 593, 604, 612, 613, 635, 646, 647, 655, 673, 675, 681, 692, 695, 696, 698, 702, 705, 728, 742.
> See [CHECKER-PEP-CONFORMANCE-PLAN.md](CHECKER-PEP-CONFORMANCE-PLAN.md) for the detailed conformance push plan.

- [ ] Configurable strictness modes — off / basic / standard / strict (Pylance has 4 tiers)
- [ ] Per-rule diagnostic severity overrides — `diagnosticSeverityOverrides` map (each of 70+ rules individually overridable to none/information/warning/error)
- [ ] Diagnostic scope setting — workspace vs open files only (`diagnosticMode`)
- [ ] Reachability analysis — detect and grey out unreachable code
- [ ] Pattern match exhaustiveness checking — `reportMatchNotExhaustive`
- [ ] Deprecated symbol detection — `reportDeprecated` (PEP 702)
- [ ] Unused import/variable/function/class detection — `reportUnusedImport`, `reportUnusedVariable`, `reportUnusedFunction`, `reportUnusedClass`
- [ ] Override method compatibility checking — `reportIncompatibleMethodOverride`, `reportIncompatibleVariableOverride`
- [ ] Missing super().__init__() call — `reportMissingSuperCall`
- [ ] Uninitialized instance variable — `reportUninitializedInstanceVariable`
- [ ] Import cycle detection — `reportImportCycles`
- [ ] Constant redefinition detection — `reportConstantRedefinition`
- [ ] Unnecessary isinstance/cast/comparison — `reportUnnecessaryIsInstance`, `reportUnnecessaryCast`, `reportUnnecessaryComparison`, `reportUnnecessaryContains`
- [ ] Self/cls parameter naming validation — `reportSelfClsParameterName`
- [ ] Implicit string concatenation warning — `reportImplicitStringConcatenation`
- [ ] Abstract class usage validation — `reportAbstractUsage`
- [ ] Overload consistency checking — `reportInconsistentOverload`, `reportOverlappingOverload`
- [ ] TypedDict required/not-required access — typed dict access validation
- [ ] Unhashable type detection — `reportUnhashable`
- [ ] Unused call result / coroutine detection — `reportUnusedCallResult`, `reportUnusedCoroutine`
- [ ] Unnecessary type: ignore detection — `reportUnnecessaryTypeIgnoreComment`
- [ ] Possibly-unbound variable detection — `reportPossiblyUnboundVariable`
- [ ] Invalid string escape sequence — `reportInvalidStringEscapeSequence`
- [ ] Implicit override warning — `reportImplicitOverride`
- [ ] Private usage detection — `reportPrivateUsage`, `reportPrivateImportUsage`
- [ ] Wildcard import from library — `reportWildcardImportFromLibrary`
- [ ] Untyped decorators/base classes/named tuples — `reportUntypedFunctionDecorator`, `reportUntypedClassDecorator`, `reportUntypedBaseClass`, `reportUntypedNamedTuple`
- [ ] TypeVarTuple semantics — unpack, concat, specialization (PEP 646)
- [ ] ParamSpec semantics — components, specialization (PEP 612)
- [ ] Variance tracking — covariant, contravariant, invariant type parameters
- [ ] Protocol structural subtyping — attribute satisfaction, method compatibility
- [ ] Class hierarchy awareness — subclass resolution for TypeVar constraints
- [ ] Dead branch elimination — `sys.version_info`, `sys.platform` guards
- [ ] Conditional flow-based type narrowing — isinstance, is None, truthiness, literal equality, membership tests, bool(), aliased conditionals
- [ ] Strict list/dict/set inference — `strictListInference`, `strictDictionaryInference`, `strictSetInference` settings
- [ ] "Unknown" type vs explicit `Any` — distinguish unknown from intentional Any
- [ ] Constraint solver with complexity scoring — pick simplest satisfying type for ambiguous TypeVars
- [ ] Conditional types for value-constrained TypeVars
- [ ] Analysis of unannotated functions — `analyzeUnannotatedFunctions` setting
- [ ] Assignment-based type narrowing — narrow regardless of annotations
- [ ] Decorator effect evaluation — honour class/function decorators for type changes
- [ ] Literal math — operations on literal values preserve literal types
- [ ] Union-based type operations — preserve more type info than join-based
- [ ] Auto-import in completion items — automatically add import statement on accept
- [ ] Spelling correction suggestions with auto-import — suggest corrected symbols
- [ ] Direct dependency filtering — filter auto-imports to only direct project dependencies
- [ ] Third-party package indexing — `indexing`, `packageIndexDepths` settings
- [ ] Function parentheses auto-completion — `completeFunctionParens`
- [ ] Override stub completions — complete method stubs from abstract base classes
- [ ] Pytest fixture parameter completions
- [ ] Extract variable (code action)
- [ ] Extract method (code action)
- [ ] Move symbol to existing file (code action)
- [ ] Move symbol to new file (code action)
- [ ] Implement all abstract methods (code action)
- [ ] Convert relative to absolute imports (code action)
- [ ] Convert absolute to relative imports (code action)
- [ ] Rename shadowing modules (code action)
- [ ] Add type annotations — whole file (source action)
- [ ] Remove all unused imports — file-level (source action)
- [ ] Convert lambda to named function (code action)
- [ ] Convert to f-strings (code action)
- [ ] Import path updates on file move/rename — auto-update imports when files are reorganised
- [ ] Fix all diagnostics command — apply all safe fixes in one action
- [ ] Add pytest fixture type annotations (code action)
- [ ] Pytest fixture parameter type hints — `inlayHints.pytestParameters`
- [ ] Cross-file Rename (Phase 8)
- [ ] Go to Implementation — `textDocument/implementation` (distinct from definition)
- [ ] Pytest fixture go-to-definition — navigate from fixture parameter to fixture function
- [ ] Multi-format docstring rendering — Google, Sphinx, NumPy docstring styles
- [ ] Docstring template generation — generate reST docstring skeleton
- [ ] Multiple execution environments — different Python versions per subtree
- [ ] Virtual environment auto-detection
- [ ] Configuration `extends` — base configuration inheritance
- [ ] Extra module search paths — `extraPaths`
- [ ] Custom typeshed path — `typeshedPath`
- [ ] Custom stub path — `stubPath`
- [ ] Namespace package support
- [ ] Persistent index caching — cache workspace index to disk
- [ ] Multi-root workspace support (requires Phase 8)
- [ ] Module-level auto-import index with depth control (requires Phase 8)
- [ ] Workspace-wide diagnostics — diagnose all files, not just open ones (partially done via whole-module analysis)
- [ ] Automatic type stub generation — CLI `--createstub`
- [ ] Type completeness verification — CLI `--verifytypes`
- [ ] Library code analysis fallback — `useLibraryCodeForTypes`
- [ ] Watch mode — `--watch` with incremental updates
- [ ] JSON output — `--outputjson` for CI integration
- [ ] Multi-threaded checking — `--threads`
- [ ] Performance statistics — `--stats`
- [ ] Import dependency emission — `--dependencies`
- [ ] Skip unannotated functions — `--skipunannotated`
- [ ] Platform/version targeting — `--pythonplatform`, `--pythonversion`
- [ ] Structured exit codes — 0-4 for different failure modes
- [ ] Auto string splitting on Enter — multi-line string continuation
- [ ] Graceful syntax error recovery — analysis continues on partial/broken code
- [ ] Jupyter notebook cell awareness — cross-cell type checking

---

## Remaining Items (from completed plans) {#LSPPLAN-REMAINING-ITEMS}

> Migrated from deleted plans: LSP-PROFILING-PLAN, EXTENSION-ACTIVITY-PANEL-PLAN, NEOVIM-PLAN, ZED-PLAN, LSP-UV-INTEGRATION-PLAN.

### Zed Extension {#LSPPLAN-REMAINING-ITEMS-ZED-EXTENSION}

- [ ] Verify: highlighting, outline panel, bracket matching, auto-indent (manual — requires Zed with extension installed)
- [ ] Test: breakpoints, stepping, variables, debug console, attach mode (manual — no Zed test framework)
- [ ] Publish to Zed extension registry (PR to `zed-industries/extensions`)
- [ ] When Zed adds panel API: implement native activity panels using same LSP commands

### Neovim Extension {#LSPPLAN-REMAINING-ITEMS-NEOVIM-EXTENSION}

- [ ] Verify all 21 core LSP features work (requires running basilisk binary against a real Python project)
- [ ] DapTcpProxy integration tests with live TCP
- [ ] Submit `lsp/basilisk.lua` PR to nvim-lspconfig

### uv Integration {#LSPPLAN-REMAINING-ITEMS-UV-INTEGRATION}

- [ ] imports_unresolved: attach `code_action_data` to diagnostic for quick-fix wiring
- [ ] BSK-W0012: unused dependency (in deps but never imported — whole-module only)
- [ ] BSK-W0013: stale lock (`pyproject.toml` mtime > `uv.lock` mtime)
- [ ] Graceful degradation: hide uv commands/actions when `uv` binary not found
- [ ] Multi-root LSP mapping for workspace members (not implemented in LSP protocol sense)
