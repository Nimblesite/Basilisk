# LSP Implementation Plan

> **Spec**: `docs/specs/LSP-SPEC.md` — read before touching any code.

---

## Status

Phases 0–6 are COMPLETE. Remaining work is cross-module infrastructure, advanced refactoring, and closing every Pylance parity gap.

---

## Phase 7 — Cross-Module Foundation (BLOCKING for everything below)

> **The big unlock.** Without a workspace module resolver, none of the cross-file
> features are possible. This phase builds the infrastructure.

| Task | Description | Difficulty | Status |
|------|-------------|------------|--------|
| 7.1 | Workspace module resolver — scan workspace, resolve `import X` to file paths | Hard | DONE — `import_resolver.rs` resolves absolute/relative imports to `.py`/`.pyi` files via workspace roots, extraPaths, venv site-packages; wired into `server.rs` `initialized()` after workspace scan; 17 tests |
| 7.2 | Multi-file `ResolvedModule` graph — resolve across files, cache per-file | Hard | TODO — `ResolvedModule` is per-file only, no cross-file symbol sharing |
| 7.4 | Salsa integration — memoized incremental computation (like rust-analyzer) | Hard | TODO — no `salsa` dependency, full re-parse on every change |
| 7.5 | Stub file (`.pyi`) support — resolve type info from `.pyi` alongside `.py` | Medium | PARTIAL — dedup logic prefers `.pyi` over `.py`, both collected; `import_resolver.rs` sets `StubPyi` resolution when `.pyi` found; no cross-file type extraction from stubs yet |
| 7.6 | Third-party type stubs — typeshed bundling, `py.typed` marker detection (PEP 561) | Medium | MINIMAL — `basilisk-stubs` crate is skeleton with basic `lookup_builtin()` only; no typeshed bundle, no `py.typed` detection |
| 7.7 | Config file reading — `pyrightconfig.json`, `pyproject.toml`, `basilisk.json` | Medium | DONE — reads `pythonVersion`, `pythonPlatform`, `include`, `exclude`, `extraPaths`, `typeCheckingMode`, `venvPath`, `venv`; `extraPaths` and `venv` now fed to `ImportSearchPaths` in import resolver |

## Phase 8 — Cross-Module Features (requires Phase 7)

| Task | Description | Difficulty | Status |
|------|-------------|------------|--------|
| 8.1 | Cross-file Go to Definition | Medium | TODO |
| 8.2 | Cross-file Find All References | Medium | TODO |
| 8.3 | Cross-file Rename | Hard | TODO |
| 8.4 | Auto-import suggestions — suggest imports from workspace index | Hard | TODO |
| 8.5 | Module-level auto-import index with depth control | Hard | TODO |
| 8.6 | Multi-root workspace support | Medium | TODO |

## Phase 9 — Advanced Refactoring (requires Phase 7+8)

| Task | Description | Difficulty | Status |
|------|-------------|------------|--------|
| 9.1 | Full type inference (generics, unions, narrowing, type guards) | Very Hard | TODO |
| 9.2 | Extract variable (code action) | Medium | TODO |
| 9.3 | Extract method (code action) | Hard | TODO |
| 9.4 | Implement abstract methods (code action) | Medium | TODO |
| 9.5 | Override stub completions | Medium | TODO |
| 9.6 | Move symbol to existing/new file (code action) | Hard | TODO |

---

## Detailed TODO — Pylance Parity

> Every feature Pylance advertises. Every gap must be closed.
> Reference: [Pylance marketplace](https://marketplace.visualstudio.com/items?itemName=ms-python.vscode-pylance), [Pyright docs](https://microsoft.github.io/pyright/#/)

### Type Checking & Diagnostics

- [ ] **Configurable strictness modes** — off / basic / standard / strict (Pylance has 4 tiers)
- [ ] **Per-rule diagnostic severity overrides** — `diagnosticSeverityOverrides` map (each of 70+ rules individually overridable to none/information/warning/error)
- [ ] **Diagnostic scope setting** — workspace vs open files only (`diagnosticMode`)
- [ ] **Reachability analysis** — detect and grey out unreachable code
- [ ] **Pattern match exhaustiveness checking** — `reportMatchNotExhaustive`
- [ ] **Deprecated symbol detection** — `reportDeprecated` (PEP 702)
- [ ] **Unused import/variable/function/class detection** — `reportUnusedImport`, `reportUnusedVariable`, `reportUnusedFunction`, `reportUnusedClass`
- [ ] **Override method compatibility checking** — `reportIncompatibleMethodOverride`, `reportIncompatibleVariableOverride`
- [ ] **Missing super().__init__() call** — `reportMissingSuperCall`
- [ ] **Uninitialized instance variable** — `reportUninitializedInstanceVariable`
- [ ] **Import cycle detection** — `reportImportCycles`
- [ ] **Constant redefinition detection** — `reportConstantRedefinition`
- [ ] **Unnecessary isinstance/cast/comparison** — `reportUnnecessaryIsInstance`, `reportUnnecessaryCast`, `reportUnnecessaryComparison`, `reportUnnecessaryContains`
- [ ] **Self/cls parameter naming validation** — `reportSelfClsParameterName`
- [ ] **Implicit string concatenation warning** — `reportImplicitStringConcatenation`
- [ ] **Abstract class usage validation** — `reportAbstractUsage`
- [ ] **Overload consistency checking** — `reportInconsistentOverload`, `reportOverlappingOverload`
- [ ] **TypedDict required/not-required access** — typed dict access validation
- [ ] **Unhashable type detection** — `reportUnhashable`
- [ ] **Unused call result / coroutine detection** — `reportUnusedCallResult`, `reportUnusedCoroutine`
- [ ] **Unnecessary type: ignore detection** — `reportUnnecessaryTypeIgnoreComment`
- [ ] **Possibly-unbound variable detection** — `reportPossiblyUnboundVariable`
- [ ] **Invalid string escape sequence** — `reportInvalidStringEscapeSequence`
- [ ] **Implicit override warning** — `reportImplicitOverride`
- [ ] **Private usage detection** — `reportPrivateUsage`, `reportPrivateImportUsage`
- [ ] **Wildcard import from library** — `reportWildcardImportFromLibrary`
- [ ] **Untyped decorators/base classes/named tuples** — `reportUntypedFunctionDecorator`, `reportUntypedClassDecorator`, `reportUntypedBaseClass`, `reportUntypedNamedTuple`

### Type Inference Engine

- [ ] **Conditional flow-based type narrowing** — isinstance, is None, truthiness, literal equality, membership tests, bool(), aliased conditionals
- [ ] **Strict list/dict/set inference** — `strictListInference`, `strictDictionaryInference`, `strictSetInference` settings
- [ ] **"Unknown" type vs explicit `Any`** — distinguish unknown from intentional Any
- [ ] **Constraint solver with complexity scoring** — pick simplest satisfying type for ambiguous TypeVars
- [ ] **Conditional types for value-constrained TypeVars**
- [ ] **Analysis of unannotated functions** — `analyzeUnannotatedFunctions` setting
- [ ] **Assignment-based type narrowing** — narrow regardless of annotations
- [ ] **Decorator effect evaluation** — honour class/function decorators for type changes
- [ ] **Literal math** — operations on literal values preserve literal types
- [ ] **Union-based type operations** — preserve more type info than join-based

### IntelliSense / Completions

- [ ] **Auto-import in completion items** — automatically add import statement on accept
- [ ] **Spelling correction suggestions with auto-import** — suggest corrected symbols
- [ ] **Direct dependency filtering** — filter auto-imports to only direct project dependencies
- [ ] **Third-party package indexing** — `indexing`, `packageIndexDepths` settings
- [ ] **Function parentheses auto-completion** — `completeFunctionParens`
- [ ] **Override stub completions** — complete method stubs from abstract base classes
- [ ] **Pytest fixture parameter completions**

### Code Actions & Refactoring

- [ ] **Extract variable** (code action)
- [ ] **Extract method** (code action)
- [ ] **Move symbol to existing file** (code action)
- [ ] **Move symbol to new file** (code action)
- [ ] **Implement all abstract methods** (code action)
- [ ] **Convert relative to absolute imports** (code action)
- [ ] **Convert absolute to relative imports** (code action)
- [ ] **Rename shadowing modules** (code action)
- [ ] **Add type annotations — whole file** (source action)
- [ ] **Remove all unused imports — file-level** (source action)
- [ ] **Convert lambda to named function** (code action)
- [ ] **Convert to f-strings** (code action)
- [ ] **Import path updates on file move/rename** — auto-update imports when files are reorganised
- [ ] **Fix all diagnostics command** — apply all safe fixes in one action
- [ ] **Add pytest fixture type annotations** (code action)

### Inlay Hints

- [ ] **Pytest fixture parameter type hints** — `inlayHints.pytestParameters`

### Navigation

- [ ] **Go to Implementation** — `textDocument/implementation` (distinct from definition)
- [ ] **Pytest fixture go-to-definition** — navigate from fixture parameter to fixture function
- [ ] **Cross-file Go to Definition** (requires Phase 7)
- [ ] **Cross-file Find All References** (requires Phase 7)
- [ ] **Cross-file Rename** (requires Phase 7)

### Hover

- [ ] **Multi-format docstring rendering** — Google, Sphinx, NumPy docstring styles
- [ ] **Docstring template generation** — generate reST docstring skeleton

### Workspace / Configuration

- [ ] **Multiple execution environments** — different Python versions per subtree
- [ ] **Virtual environment auto-detection**
- [ ] **Configuration `extends`** — base configuration inheritance
- [ ] **Extra module search paths** — `extraPaths`
- [ ] **Custom typeshed path** — `typeshedPath`
- [ ] **Custom stub path** — `stubPath`
- [ ] **Namespace package support**
- [ ] **Persistent index caching** — cache workspace index to disk
- [ ] **Multi-root workspace support** (requires Phase 8)
- [ ] **Module-level auto-import index with depth control** (requires Phase 8)
- [ ] **Workspace-wide diagnostics** — diagnose all files, not just open ones (partially done via whole-module analysis)

### Stubs & Typeshed

- [ ] **Bundle current typeshed stdlib stubs** — ship with the binary
- [ ] **py.typed package support** (PEP 561)
- [ ] **Automatic type stub generation** — CLI `--createstub`
- [ ] **Type completeness verification** — CLI `--verifytypes`
- [ ] **Library code analysis fallback** — `useLibraryCodeForTypes`
- [ ] **Stub file (.pyi) resolution** — resolve `.pyi` alongside `.py`

### CLI Features (Pyright parity)

- [ ] **Watch mode** — `--watch` with incremental updates
- [ ] **JSON output** — `--outputjson` for CI integration
- [ ] **Multi-threaded checking** — `--threads`
- [ ] **Performance statistics** — `--stats`
- [ ] **Import dependency emission** — `--dependencies`
- [ ] **Skip unannotated functions** — `--skipunannotated`
- [ ] **Platform/version targeting** — `--pythonplatform`, `--pythonversion`
- [ ] **Structured exit codes** — 0-4 for different failure modes

### Miscellaneous

- [ ] **Auto string splitting on Enter** — multi-line string continuation
- [ ] **Graceful syntax error recovery** — analysis continues on partial/broken code
- [ ] **Jupyter notebook cell awareness** — cross-cell type checking

### PEP Conformance (Pylance supports 29+ PEPs)

> We're at ~83% PEP conformance. Pylance supports: 484, 487, 526, 544, 561, 563, 570, 585, 586, 589, 591, 593, 604, 612, 613, 635, 646, 647, 655, 673, 675, 681, 692, 695, 696, 698, 702, 705, 728, 742.

- [ ] Audit each PEP against our implementation — identify specific gaps
- [ ] Target 100% conformance on the typing conformance test suite

---

## Rules

- Build must stay GREEN at all times
- No `.unwrap()` in server code
- No `println!` in production code (LSP stdout is sacred)
- `cargo clippy` must pass after every task
- E2E tests for every feature — no unit test theatre
- Do NOT delete failing tests — add more
