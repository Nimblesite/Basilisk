# LSP Refactoring Plan

**Spec**: [LSP-REFACTORING-SPEC.md](../specs/LSP-REFACTORING-SPEC.md)
**Goal**: Feature parity with Pylance refactoring, then surpass it.

## Current State

**What we have**:
- `textDocument/rename` and `textDocument/prepareRename` — cross-file via import graph, but text-match based (not scope-aware).
- `source.organizeImports` — delegates to ruff.
- Quick-fix code actions for diagnostics (add annotations, remove redundant annotations, suppress).
- Mass autofix engine with conflict resolution.
- `CodeActionKind::REFACTOR` registered in capabilities but no refactor actions implemented.

**What Pylance has that we don't**:
- Scope-aware rename with validation.
- `workspace/willRenameFiles` (module rename with import updates).
- Extract Method / Extract Variable.
- Move Symbol to another file / new file.
- Implement All Abstract Methods.

**What we'll add beyond Pylance**:
- Inline Variable / Inline Function (Pylance doesn't have these).
- Change Signature (only Rope has this in the Python ecosystem).
- Construct conversions (f-string ↔ format, Union ↔ `|`, etc.).

---

## Phase 1: Scope-Aware Rename (Enhance Existing)

**Why first**: Rename is the most-used refactoring. Our current text-match approach can produce incorrect renames when the same name appears in different scopes. This is a correctness fix, not just a feature.

### Tasks

1. **Build a scope tree from the resolved AST** — each function, class, comprehension, and module creates a scope. Each scope maps names to their definition spans.
2. **Replace text-match rename with scope-walk** — when renaming at a position, find the definition in the innermost scope, then find all references to that specific binding (not just the same text).
3. **Keyword argument tracking** — when renaming a parameter, find all callers (via the import graph for public functions) and rename keyword arguments.
4. **`self.attr` / `cls.attr` rename** — when renaming a class attribute, walk all methods for `self.attr` and all external `obj.attr` references where `obj`'s type resolves to the class.
5. **Validation** — reject renames that would shadow existing bindings, conflict with builtins, or produce invalid identifiers.
6. **`__all__` updates** — if the renamed symbol is in `__all__`, update the string entry.
7. **Docstring parameter references** — offer to update `:param name:` in docstrings (opt-in code action).
8. **Tests** — scope-aware rename across nested functions, classes, comprehensions, keyword arguments, `self.attr`, shadowing rejection.

---

## Phase 2: Rename Module (`workspace/willRenameFiles`)

**Why second**: File reorganization is a daily operation. Without this, renaming a file breaks all imports.

### Tasks

1. **Register `workspace/willRenameFiles` capability** in `init.rs`.
2. **Handler implementation** — receive the old and new file URIs, compute the old and new module paths.
3. **Rewrite imports across workspace** — `import old` → `import new`, `from old import x` → `from new import x`, relative imports.
4. **Update `__init__.py` re-exports** — if the module was re-exported from a package `__init__.py`, update that too.
5. **Handle directory renames** — renaming a package directory should update all imports for all modules inside it.
6. **Tests** — single file rename, directory rename, relative imports, `__init__.py` re-exports.

---

## Phase 3: Extract Function / Method

**Why third**: Second most requested refactoring universally. This is the biggest single feature in this plan.

### Tasks

1. **Selection-to-statements resolver** — given a text range, snap to the enclosing complete statement(s) in the AST. Reject partial selections.
2. **Data flow analysis on selection**:
   - Compute reads (variables used inside, defined outside) → parameters.
   - Compute writes (variables modified inside, read after selection) → return values.
   - Compute locals (defined and last-used inside) → stay in extracted function.
3. **Context detection**:
   - Inside a class method → extract as method (include `self`/`cls`).
   - Contains `await` → extracted function is `async`.
   - Contains `yield` → reject.
   - Contains `break`/`continue` for a loop not fully inside selection → reject.
4. **Code generation**:
   - Build the `def` node with parameters (typed if originals were typed), body, and return statement.
   - Build the call expression with argument passing and return value unpacking.
5. **Placement** — insert the new function before the enclosing function (module level) or as the next method in the class.
6. **Format** — run ruff on the inserted code to normalize style.
7. **Tests** — simple extraction, multiple return values, async, class methods, rejection cases (yield, partial loop).

---

## Phase 4: Extract Variable

**Why fourth**: Complements extract function — simpler and very frequently used.

### Tasks

1. **Expression selection resolver** — given a text range, find the exact AST expression node. Reject if the selection doesn't map to a single expression.
2. **Find identical expressions** in the enclosing scope (structural AST comparison, not string).
3. **Insertion point** — the statement immediately before the first occurrence.
4. **Code generation** — `name = expr` assignment, replace occurrences with `name`.
5. **User choice** — offer "replace all occurrences" and "replace only selected" as separate code actions.
6. **Tests** — simple expression, multiple occurrences, nested scopes, precedence edge cases.

---

## Phase 5: Move Symbol

**Why fifth**: Enables large-scale codebase reorganization. Depends on solid cross-file rename infrastructure from Phases 1-2.

### Tasks

1. **Symbol dependency analysis** — for a top-level definition, compute all imports and references it needs.
2. **Destination selection** — integrate with editor file picker via `window/showDocument` or command arguments.
3. **Move to existing file**:
   - Copy definition + required imports to destination.
   - Remove definition from source.
   - Add re-export in source if it was in `__all__` or has external importers.
   - Update all importers to use the new path.
   - Clean up unused imports in both files.
4. **Move to new file** — create the file, name it after the symbol, then run the same logic.
5. **Tests** — move function, move class, move with dependencies, re-export preservation, importer updates.

---

## Phase 6: Implement Abstract Methods

**Why sixth**: High quality-of-life, moderate complexity. Users expect this from modern IDEs.

### Tasks

1. **Detect incomplete implementation** — when a class inherits from an ABC, check which `@abstractmethod` methods are missing.
2. **Generate method stubs** — copy signatures from the abstract methods, body is `raise NotImplementedError` (configurable to `...`).
3. **Preserve decorators** — `@staticmethod`, `@classmethod`, `@property`.
4. **Offer as code action** — `refactor.rewrite.implement`, titled "Implement all abstract methods".
5. **Offer as diagnostic fix** — if the checker already emits an error for missing abstract methods, wire this as the quick-fix.
6. **Tests** — single abstract method, multiple, multiple base classes (MRO), already partially implemented, classmethod/staticmethod/property.

---

## Phase 7: Construct Conversions

**Why seventh**: Each conversion is small and independent. Can be implemented incrementally.

### Tasks (each is a self-contained sub-task)

1. **`Union[X, Y]` ↔ `X | Y`** — convert old-style `Union` to PEP 604 syntax and vice versa.
2. **`Optional[X]` ↔ `X | None`** — same as above for `Optional`.
3. **f-string ↔ `.format()`** — convert between `f"{x}"` and `"{}".format(x)`.
4. **`dict()` ↔ `{}`** — convert `dict(a=1)` to `{"a": 1}` and vice versa.
5. **`list()` ↔ `[]`** — convert `list()` constructor to literal.
6. **Ternary ↔ if/else** — convert `x if cond else y` to block form and vice versa.
7. **NamedTuple class ↔ functional** — convert between `class Foo(NamedTuple)` and `Foo = namedtuple("Foo", ...)`.
8. **Tests** for each conversion — both directions, edge cases.

---

## Phase 8: Inline Variable

### Tasks

1. **Definition analysis** — find the single assignment point for the variable.
2. **Reference collection** — find all reads of the variable in the scope.
3. **Validation** — reject if the variable is reassigned, if inlining changes evaluation count (side effects), or if the variable is used in a loop with a side-effectful initializer.
4. **Substitution** — replace each reference with the expression, adding parentheses where needed for precedence.
5. **Cleanup** — remove the original assignment.
6. **Tests** — simple inline, precedence wrapping, rejection cases (reassignment, side effects, loop).

---

## Phase 9: Change Signature

### Tasks

1. **Signature editing UI** — present the current signature and let the user add/remove/reorder/rename parameters (via a custom command that returns the new signature).
2. **Call site analysis** — find all callers via import graph and reference search.
3. **Add parameter** — insert with default value, callers unchanged.
4. **Remove parameter** — remove from signature, remove corresponding argument from all callers (reject if callers pass non-default values).
5. **Reorder parameters** — update all positional callers. Keyword callers are unaffected.
6. **Rename parameter** — update all keyword argument callers.
7. **Tests** — each operation, combinations, rejection cases.

---

## Phase 10: Inline Function

### Tasks

1. **Resolve callee** — find the function definition for the call under cursor.
2. **Single-expression bodies only** (initial scope) — `def f(x): return expr` → replace `f(arg)` with `expr[x→arg]`.
3. **Argument substitution** — handle positional, keyword, default, `*args`, `**kwargs`.
4. **Precedence** — parenthesize the inlined expression if needed.
5. **Multi-statement bodies** (future) — inline as a block with variable renaming to avoid conflicts.
6. **Offer to remove definition** if no other callers exist.
7. **Tests** — simple inline, keyword args, defaults, precedence, rejection (complex bodies).

---

## Infrastructure Work (Cross-Phase)

These are needed by multiple phases and should be built as prerequisites emerge:

| Component | Needed By | Description |
|---|---|---|
| **Scope tree** | Phase 1, 3, 4, 5, 8 | Hierarchical scope map from resolved AST — maps each name to its definition and all references within the scope. |
| **Data flow analysis** | Phase 3, 5, 8 | Reads/writes/locals analysis for a selection of statements. |
| **Structural AST comparison** | Phase 4 | Compare two AST subtrees for structural equality (ignoring whitespace/comments). |
| **Workspace edit builder** | All phases | Helper to construct multi-file `WorkspaceEdit` with conflict detection and formatting pass. |
| **Code generation utilities** | Phase 3, 6, 7 | Generate Python code from AST nodes (function defs, assignments, imports) with proper formatting. |

---

## Competitive Comparison

| Feature | Basilisk (after plan) | Pylance | Pyright | Jedi-LSP | pylsp + Rope |
|---|---|---|---|---|---|
| Rename Symbol (scope-aware) | Phase 1 | Yes | Yes | Yes | Yes |
| Rename Module/File | Phase 2 | Yes | No | No | Yes |
| Extract Function/Method | Phase 3 | Yes | No | Yes | Yes |
| Extract Variable | Phase 4 | Yes | No | Yes | Yes |
| Move Symbol | Phase 5 | Yes | No | No | Yes |
| Implement Abstract Methods | Phase 6 | Yes | No | No | No |
| Construct Conversions | Phase 7 | Partial | No | No | No |
| Inline Variable | Phase 8 | No | No | No | Yes |
| Change Signature | Phase 9 | No | No | No | Partial |
| Inline Function | Phase 10 | No | No | No | Yes |

After Phase 6, we match Pylance. After Phase 10, we surpass every Python LSP.

---

## TODO

- [x] **Phase 1**: Build scope tree from resolved AST — `scope_tree.rs` (pre-existing)
- [x] **Phase 1**: Replace text-match rename with scope-walk rename — `references.rs` + `scope_tree.rs` (pre-existing)
- [x] **Phase 1**: Keyword argument rename for parameters — `references.rs::find_keyword_arg_sites()`
- [x] **Phase 1**: `self.attr` / `cls.attr` rename — `references.rs::find_self_attr_references()`
- [x] **Phase 1**: Rename validation (shadowing, builtins, invalid identifiers) — `scope_tree::validate_rename()` (pre-existing)
- [x] **Phase 1**: `__all__` entry updates on rename — `references.rs::find_dunder_all_entries()`
- [x] **Phase 1**: Docstring parameter reference updates — `references.rs::find_docstring_param_references()`
- [x] **Phase 1**: Scope-aware rename tests — (pre-existing in scope_tree.rs)
- [x] **Phase 2**: Module rename handler (rewrite imports across workspace) — `handlers/file_operations.rs`
- [x] **Phase 2**: Import rewriting (import/from patterns) — `file_operations.rs`
- [x] **Phase 2**: Module path computation from file paths — `file_operations.rs`
- [ ] **Phase 2**: Register `workspace/willRenameFiles` capability (blocked on tower-lsp 0.21+)
- [ ] **Phase 2**: Directory/package rename support
- [ ] **Phase 2**: Module rename e2e tests (blocked on capability registration)
- [x] **Phase 3**: Selection-to-statements resolver — `extract_function.rs`
- [x] **Phase 3**: Data flow analysis (reads, writes, locals) — `extract_function.rs::analyze_data_flow()`
- [x] **Phase 3**: Context detection (method vs function, async, yield/break rejection) — `extract_function.rs::detect_enclosing_context()`
- [x] **Phase 3**: Extract function code generation (def + call site) — `extract_function.rs`
- [x] **Phase 3**: Extract function placement logic — `extract_function.rs::find_insertion_point()`
- [x] **Phase 3**: Post-extraction formatting pass — PEP 8 blank-line separation + trailing whitespace cleanup
- [x] **Phase 3**: Extract function tests — `refactor/mod.rs`
- [x] **Phase 4**: Extract variable code generation and replacement — `code_actions/refactor/extract.rs`
- [x] **Phase 4**: Extract constant (module-level SCREAMING_SNAKE) — `code_actions/refactor/extract.rs`
- [x] **Phase 4**: Extract variable/constant tests — 9 tests in `refactor/mod.rs`
- [x] **Phase 4**: Extract variable — replace all identical occurrences — `extract.rs::find_all_occurrences()`
- [x] **Phase 5**: Move symbol to new file — `code_actions/refactor/move_symbol.rs`
- [x] **Phase 5**: Symbol body extraction and import collection — `move_symbol.rs`
- [x] **Phase 5**: `CamelCase` to `snake_case` file naming — `move_symbol.rs::to_snake_case()`
- [ ] **Phase 5**: Move to existing file (with re-export and importer updates)
- [x] **Phase 5**: Move symbol tests — 14 tests in `move_symbol.rs`
- [x] **Phase 6**: Abstract method detection via base class lookup — `code_actions/refactor/abstract_methods.rs`
- [x] **Phase 6**: Method stub generation (with self parameter) — `code_actions/refactor/abstract_methods.rs`
- [x] **Phase 6**: Wire as code action (`refactor.rewrite.implement`) — `code_actions/mod.rs`
- [x] **Phase 6**: Implement abstract methods tests
- [x] **Phase 7**: `Union[X, Y]` ↔ `X | Y` conversion — `code_actions/refactor/type_syntax.rs`
- [x] **Phase 7**: `Optional[X]` ↔ `X | None` conversion — `code_actions/refactor/type_syntax.rs`
- [x] **Phase 7**: f-string ↔ `.format()` conversion — `refactor/fstring.rs`
- [x] **Phase 7**: `dict()` ↔ `{}` conversion — `refactor/literals.rs`
- [x] **Phase 7**: `list()` ↔ `[]` conversion — `refactor/literals.rs`
- [x] **Phase 7**: Ternary ↔ if/else conversion — `refactor/ternary.rs`
- [x] **Phase 7**: NamedTuple class ↔ functional conversion — `refactor/namedtuple.rs`
- [x] **Phase 7**: Construct conversion tests — `refactor/mod.rs` (18 tests)
- [x] **Phase 8**: Inline variable (definition analysis, substitution, cleanup) — `refactor/inline.rs`
- [x] **Phase 8**: Inline variable validation (reassignment, side effects) — `refactor/inline.rs`
- [x] **Phase 8**: Inline variable tests — `refactor/mod.rs`
- [x] **Phase 9**: Change signature — remove parameter with call site updates — `refactor/change_signature.rs`
- [x] **Phase 9**: Change signature — add parameter with default value — `change_signature.rs::add_parameter()`
- [x] **Phase 9**: Change signature — reorder/sort parameters alphabetically — `change_signature.rs::reorder_parameters()`
- [x] **Phase 9**: Change signature tests
- [x] **Phase 10**: Inline function (single-expression bodies) — `refactor/inline_function.rs`
- [x] **Phase 10**: Argument substitution and precedence handling — `refactor/inline_function.rs`
- [x] **Phase 10**: Inline function tests
- [x] **Infrastructure**: Scope tree module — `scope_tree.rs` (pre-existing, 599 lines)
- [x] **Infrastructure**: Data flow analysis module — `extract_function.rs::analyze_data_flow()`
- [x] **Infrastructure**: Structural text comparison — `extract.rs::find_all_occurrences()`
- [ ] **Infrastructure**: Workspace edit builder with formatting
- [x] **Infrastructure**: Code generation utilities — `code_actions/refactor/helpers.rs`

### E2E Test Coverage

- [x] **E2E**: 19 refactoring e2e tests — `tests/lsp/lsp_e2e_refactoring.rs`
- [x] **E2E**: Extract variable — offered + edit correctness
- [x] **E2E**: Extract constant — offered
- [x] **E2E**: Extract function — offered + yield rejection
- [x] **E2E**: Union/Optional conversion — offered
- [x] **E2E**: f-string conversion — offered
- [x] **E2E**: dict/list conversion — offered
- [x] **E2E**: Ternary conversion — offered
- [x] **E2E**: Inline variable — offered + edit correctness
- [x] **E2E**: Inline function — offered
- [x] **E2E**: Move symbol — offered + negative case
- [x] **E2E**: NamedTuple conversion — offered
- [x] **E2E**: Scope-aware rename — scoped edits verified
- [x] **E2E**: Negative cases — empty selection, assignment (no false positives)

### Editor Integration

- [x] **VS Code**: All refactoring actions available via lightbulb menu (automatic via LSP)
- [x] **Zed**: All refactoring actions available via lightbulb menu (automatic via LSP, WASM extension)
- [x] **Neovim**: Refactoring commands registered — `BasiliskExtractVariable`, `BasiliskExtractConstant`, `BasiliskConvertUnion`, `BasiliskImplementMethods`
- [x] **Neovim**: Keybindings in `ftplugin/python.lua` — `<leader>bev`, `<leader>bec`, `<leader>bcu`, `<leader>bim`, `<leader>bfa`
- [x] **Neovim**: Automated test suite — `tests/basilisk/commands_spec.lua` (30+ command registration tests)
- [x] **CLI**: `basilisk fix` with `--rules` flag — `ALL_FIXABLE_RULES`, `SAFE_FIXABLE_RULES`, specific comma-separated list
