# LSP Refactoring Spec

**Status**: Draft
**Depends on**: [LSP-ARCHITECTURE-SPEC.md](LSP-ARCHITECTURE-SPEC.md), [LSP-ANALYSIS-MODES-SPEC.md](LSP-ANALYSIS-MODES-SPEC.md)

## Overview {#REFACTOR-OVERVIEW}

Deterministic, type-aware refactoring tools that bring Basilisk to feature parity with Pylance and beyond. Every refactoring is a structured code transformation — no regex, no string hacking. All operations use the resolved AST and type information from the checker.

## Design Principles {#REFACTOR-PRINCIPLES}

1. **Deterministic first** — every refactoring produces a single, predictable result. AI-assisted variants live in [LSP-AI-SPEC.md](LSP-AI-SPEC.md), never here.
2. **Type-aware** — refactorings use full type information (resolved types, import graph, call sites) to produce correct transformations.
3. **Cross-file** — all refactorings that affect imports or references operate across the workspace via the import graph.
4. **Atomic undo** — each refactoring returns a single `WorkspaceEdit` so the user can undo in one step.
5. **Safe by default** — refactorings that could change runtime behavior are clearly marked and require confirmation.

## Code Action Kinds {#REFACTOR-KINDS}

All refactorings are exposed as LSP code actions with hierarchical kinds:

| Kind | Description |
|---|---|
| `refactor.extract.function` | Extract selection to function/method |
| `refactor.extract.variable` | Extract expression to variable |
| `refactor.inline.variable` | Inline a variable (replace all references with its value) |
| `refactor.inline.function` | Inline a function (replace call with body) |
| `refactor.move` | Move symbol to another module |
| `refactor.rewrite.signature` | Change function signature |
| `refactor.rewrite.convert` | Convert between equivalent constructs |

Additionally, `textDocument/rename` and `textDocument/prepareRename` handle symbol renaming, and `workspace/willRenameFiles` handles module/file renaming.

---

## Rename Symbol {#REFACTOR-RENAME}

**LSP methods**: `textDocument/rename`, `textDocument/prepareRename`

Current implementation renames identifiers across files using the import graph. Enhancements needed:

### Scope-Aware Rename {#REFACTOR-RENAME-SCOPE}

The current rename uses whole-word text matching. It must become scope-aware:

- **Local variables**: rename only within the enclosing function/block scope.
- **Parameters**: rename the parameter and all references within the function body. If the function has callers using keyword arguments, rename those too.
- **Class attributes**: rename `self.x` across all methods in the class and all external `obj.x` references.
- **Module-level symbols**: rename across all importing modules (current behavior, keep).
- **Type aliases / `TypeVar` names**: rename the alias and all usages.

### Rename Validation {#REFACTOR-RENAME-VALIDATE}

Before applying a rename, validate:

- New name is a valid Python identifier.
- New name does not shadow an existing binding in the same scope.
- New name does not conflict with builtins unless the user confirms.
- If renaming a public symbol (`__all__`), update `__all__` entries.

### Docstring Updates {#REFACTOR-RENAME-DOCS}

When renaming a parameter, offer to update references in docstrings (`:param old_name:` → `:param new_name:`). This is opt-in, not automatic.

---

## Rename Module {#REFACTOR-RENAMEMOD}

**LSP method**: `workspace/willRenameFiles`

When a user renames a `.py` file in their editor:

1. Compute all modules that import the old module path.
2. Rewrite `import old_module` → `import new_module` and all `from old_module import ...` forms.
3. Update relative imports within the renamed module if the directory changed.
4. Update `__init__.py` re-exports if the module was listed there.
5. Return a single `WorkspaceEdit` applied before the file system rename completes.

---

## Extract Function {#REFACTOR-EXTRACT-FUNC}

**Code action kind**: `refactor.extract.function`

**Trigger**: User selects one or more complete statements.

### Algorithm {#REFACTOR-EXTRACT-FUNC-ALGO}

1. **Parse selection** — expand to enclosing complete statements. Reject if the selection splits a statement.
2. **Analyze data flow**:
   - **Reads**: variables read inside the selection that are defined outside it → become parameters.
   - **Writes**: variables written inside the selection that are read after it → become return values.
   - **Local-only**: variables both defined and last-used inside the selection → stay local.
3. **Determine context**:
   - If inside a class method and selection references `self`/`cls`, extract as a method on the same class.
   - Otherwise, extract as a module-level function.
   - If inside an `async` function and the selection contains `await`, the extracted function must be `async`.
4. **Generate function**:
   - Name: prompt the user (via `textDocument/rename`-style placeholder).
   - Parameters: ordered by first occurrence in the selection. Annotate types if the originals had annotations.
   - Return: if one write, `return var`. If multiple writes, `return (var1, var2)` with tuple unpacking at the call site.
   - Decorators: none. The user adds them manually.
5. **Replace selection** with a call to the new function, with appropriate unpacking.
6. **Place function** immediately before the enclosing function/class (module-level) or after the last method in the class (class method).

### Edge Cases {#REFACTOR-EXTRACT-FUNC-EDGE}

- Selection contains `yield` → reject (cannot extract generator mid-stream).
- Selection contains `return` → reject unless it is the only return path (extract the entire branch).
- Selection references nonlocal/global → include the declaration in the extracted function.
- Selection contains `break`/`continue` inside a loop that is partially outside the selection → reject.

---

## Extract Variable {#REFACTOR-EXTRACT-VAR}

**Code action kind**: `refactor.extract.variable`

**Trigger**: User selects an expression (not a statement).

### Algorithm

1. **Validate selection** — must be a single, complete expression node in the AST.
2. **Find all identical occurrences** in the enclosing scope (same AST structure, not string matching).
3. **Choose insertion point** — the statement immediately before the first occurrence.
4. **Generate assignment**: `<name> = <expression>`. Prompt user for name.
5. **Replace all occurrences** (or only the selected one — offer both options).
6. **Type annotation**: omit unless the expression's inferred type is ambiguous (e.g., `[]` → `list[Any]`).

---

## Inline Variable {#REFACTOR-INLINE-VAR}

**Code action kind**: `refactor.inline.variable`

**Trigger**: Cursor on a variable assignment.

### Algorithm

1. **Find the definition** — must be a simple `name = expr` assignment with exactly one definition point.
2. **Find all references** in the enclosing scope.
3. **Validate**: if the variable is reassigned later, reject (not a simple inline).
4. **Replace** each reference with the expression. Parenthesize if the expression has lower precedence than the surrounding context.
5. **Delete** the original assignment.

### Safety {#REFACTOR-INLINE-VAR-SAFETY}

- If the expression has side effects (function call, I/O), warn the user — inlining may change evaluation count.
- If the variable is used in a loop, inlining a function call would change from evaluate-once to evaluate-per-iteration.

---

## Inline Function {#REFACTOR-INLINE-FUNC}

**Code action kind**: `refactor.inline.function`

**Trigger**: Cursor on a function call where the function is defined in the same workspace.

### Algorithm

1. **Resolve** the function definition.
2. **Validate**: function body is a single `return` expression, or a short block that can be inlined.
3. **Substitute** parameters with arguments. Handle keyword arguments, defaults, `*args`, `**kwargs`.
4. **Replace** the call expression with the substituted body.
5. **Remove** the function definition if it has no other callers (offer as option).

This is a complex refactoring. Initial implementation should support only single-expression bodies (i.e., `def f(x): return x + 1` → inline `f(y)` to `y + 1`).

---

## Move Symbol {#REFACTOR-MOVE}

**Code action kind**: `refactor.move`

**Trigger**: Cursor on a top-level function, class, or constant definition.

### Algorithm

1. **Identify the symbol** and all its dependencies (imports it uses, other symbols it references).
2. **User selects destination** module (via editor file picker or quick-pick).
3. **Move the definition** to the destination file:
   - Add required imports to the destination.
   - Remove the definition from the source.
   - Add a re-export in the source (`from new_module import symbol`) if the source's `__all__` included it, or if other modules import it from the source.
4. **Update all importers** — rewrite `from old_module import symbol` to `from new_module import symbol` across the workspace.
5. **Clean up** — remove now-unused imports from both source and destination.

### Move to New File {#REFACTOR-MOVE-NEW}

Same as above, but the destination is a new file named after the symbol (e.g., `my_func` → `my_func.py` in the same directory).

---

## Change Signature {#REFACTOR-SIGNATURE}

**Code action kind**: `refactor.rewrite.signature`

**Trigger**: Cursor on a function definition.

### Operations

- **Add parameter**: insert a new parameter with a default value. All existing callers remain valid.
- **Remove parameter**: remove a parameter. Update all callers to remove the corresponding argument (reject if any caller passes a value that differs from the default).
- **Reorder parameters**: change the order. Update all callers using positional arguments.
- **Rename parameter**: rename a parameter and update all keyword-argument callers.

Each operation produces a `WorkspaceEdit` covering the definition and all call sites found via the import graph and reference search.

---

## Convert Constructs {#REFACTOR-CONVERT}

**Code action kind**: `refactor.rewrite.convert`

Context-sensitive conversions offered when the cursor is on an applicable construct:

| Conversion | Trigger | Action |
|---|---|---|
| f-string ↔ `.format()` ↔ `%` | Cursor on a string expression | Convert between string formatting styles |
| `dict()` ↔ `{}` | Cursor on `dict()` call or dict literal | Convert between constructor and literal |
| `list()` ↔ `[]` | Cursor on `list()` call or list literal | Convert between constructor and literal |
| Ternary ↔ `if/else` block | Cursor on a conditional expression or simple if/else | Convert between inline and block form |
| `Union[X, Y]` ↔ `X \| Y` | Cursor on a `Union` or `\|` type | Convert between old and new union syntax (PEP 604) |
| `Optional[X]` ↔ `X \| None` | Cursor on `Optional` type | Convert to PEP 604 syntax |
| `TypedDict` ↔ `dataclass` | Cursor on a class definition | Convert between equivalent data structures |
| Named tuple class ↔ functional | Cursor on a `NamedTuple` | Convert between class syntax and functional form |

These are offered as code actions only when applicable and safe. Each conversion must preserve runtime semantics.

---

## Implement Abstract Methods {#REFACTOR-ABSTRACT}

**Code action kind**: `refactor.rewrite.implement`

**Trigger**: Cursor on a class that inherits from an abstract base class with unimplemented abstract methods.

### Algorithm

1. **Resolve base classes** — find all `@abstractmethod` methods in the MRO.
2. **Filter** — exclude methods already implemented in the class.
3. **Generate stubs** for each missing method:
   - Copy the signature (name, parameters, type annotations).
   - Body: `raise NotImplementedError` (or `...` if the user prefers, configurable).
   - Preserve `@staticmethod`/`@classmethod` decorators.
4. **Insert** after the last existing method in the class (or after `__init__` if present).

---

## Cross-Cutting Concerns {#REFACTOR-CROSS}

### Formatter Conflict {#REFACTOR-FORMATTER}

All generated code must match the project's formatting settings. After generating a `WorkspaceEdit`, run the formatter (ruff) on the affected ranges to normalize style. This prevents the refactoring from triggering a format-on-save diff.

### Undo Granularity {#REFACTOR-UNDO}

Every refactoring returns exactly one `WorkspaceEdit`. The editor treats this as one undo step. Never split a refactoring into multiple sequential edits.

### Telemetry {#REFACTOR-TELEMETRY}

Each refactoring records:
- Which refactoring was invoked.
- Whether it succeeded or was rejected (and why).
- Number of files and edits in the resulting `WorkspaceEdit`.

No code content is ever recorded.

### Configuration {#REFACTOR-CONFIG}

```toml
[tool.basilisk.refactoring]
# Whether "Extract Variable" replaces all occurrences or just the selected one
extract_variable_replace_all = true

# Whether "Inline Variable" warns about side effects
inline_warn_side_effects = true

# Body style for generated abstract method stubs: "raise" or "ellipsis"
abstract_method_body = "raise"
```

---

## Priority Order {#REFACTOR-PRIORITY}

For reaching feature parity with Pylance, implement in this order:

1. **Rename Symbol** enhancements (scope-aware, validation) — highest impact, builds on existing code.
2. **Rename Module** (`workspace/willRenameFiles`) — critical for file reorganization.
3. **Extract Function** — second most requested refactoring universally.
4. **Extract Variable** — frequently used, simpler than extract function.
5. **Move Symbol** — enables large-scale refactoring.
6. **Implement Abstract Methods** — high quality-of-life, moderate complexity.
7. **Convert Between Constructs** — incremental, each conversion is independent.
8. **Inline Variable** — useful but less frequently needed.
9. **Change Signature** — complex, high value for large codebases.
10. **Inline Function** — least common, most complex.
