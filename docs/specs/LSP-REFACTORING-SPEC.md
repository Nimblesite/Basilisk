# LSP Refactoring Spec {#REFACTOR}

**Status**: Draft
**Depends on**: [LSP-ARCHITECTURE-SPEC.md §LSPARCH-FEATURES](LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES), [LSP-ANALYSIS-MODES-SPEC.md §ANALYSIS-CROSSLSP](LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CROSSLSP)

## Overview {#REFACTOR-OVERVIEW}

Deterministic, type-aware refactoring. Every refactoring is a structured transformation over the resolved AST and checker type information — no regex, no string hacking.

## AST mandate {#REFACTOR-AST}

**Normative, and it overrides every algorithm in this document.** A refactoring
operates on parsed AST nodes and resolved symbols. It may **never** locate a
definition, a statement boundary, an import, a scope, or a reference by matching
characters in the source file. Banned without exception, in every code path:

- Finding a definition by a line's leading characters (`strip_prefix("def ")`,
  `starts_with("class ")`).
- Finding the import block by scanning for `import ` / `from `.
- Hand-lexing identifiers or maintaining a Python keyword table in Rust.
- Determining scope or nesting by measuring indentation in bytes.
- Locating an attribute reference by searching for `self.` + the attribute name.

Each of these was implemented here and each has been **deleted**; see
[REFACTOR-STATUS](#REFACTOR-STATUS). They were not merely imprecise — every one
of them matched inside comments, docstrings, and string literals, missed
constructs spanning multiple lines, and changed behaviour under reformatting
alone. A refactoring that edits code it misidentified corrupts the user's file,
which makes this stricter than the checker's equivalent rule, not looser:
the checker's failure mode is a wrong diagnostic, a refactoring's is data loss.

The parser has already produced every node these mechanisms tried to recover.
Use it: `ruff_python_parser` for structure, `ResolvedModule` for symbols and
imports, `BindingTable` for what a name refers to
([RESOLV-CANONICAL](CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL)).

Searching for the **user's own symbol** by name is not covered by this ban — a
rename must find occurrences of the identifier the user selected. What is banned
is hardcoding *Python's* vocabulary into Rust to infer structure.

Permitted: line and column **geometry** for computing an edit range once the
target node is known.

## Design Principles {#REFACTOR-PRINCIPLES}

Invariants every refactoring must satisfy:

1. **Deterministic** — a single, predictable result. Optional AI work, if pursued, remains outside this contract in the [AI plan](../plans/LSP-AI-PLAN.md).
2. **Type-aware** — uses resolved types, import graph, call sites.
3. **Cross-file** — operates across the workspace via the import graph.
4. **Atomic undo** — returns a single `WorkspaceEdit` (one undo step).
5. **Safe by default** — refactorings that could change runtime behavior are marked and require confirmation.
6. **Structural** — satisfies [REFACTOR-AST](#REFACTOR-AST). A refactoring that cannot identify its target structurally offers no code action at all; it never guesses.

## Implementation status {#REFACTOR-STATUS}

This document specifies the target. As of 2026-08-06 the following actions are
**not shipped**: their implementations violated [REFACTOR-AST](#REFACTOR-AST)
and were deleted rather than patched. Rebuild order is
[ASTREBUILD-PHASE-LSP](../plans/CHECKER-AST-RECONSTRUCTION-PLAN.md#ASTREBUILD-PHASE-LSP).

| Section | Status |
|---|---|
| [REFACTOR-RENAME](#REFACTOR-RENAME) | Ships. Identifier, keyword-argument, and docstring occurrences only — the `self.attr` / `cls.attr` sweep in [REFACTOR-RENAME-SCOPE](#REFACTOR-RENAME-SCOPE) is deleted and unbuilt. |
| [REFACTOR-RENAMEMOD](#REFACTOR-RENAMEMOD) | Not shipped. Path→module mapping survives; import rewriting is inert. |
| [REFACTOR-EXTRACT-FUNC](#REFACTOR-EXTRACT-FUNC) | Not shipped. |
| [REFACTOR-EXTRACT-VAR](#REFACTOR-EXTRACT-VAR) | Extract **variable** ships. Extract **constant** is deleted and unbuilt. |
| [REFACTOR-MOVE](#REFACTOR-MOVE) | Not shipped. The `MOVE_SYMBOL` command is still advertised with no code action producing it, which violates [LSPARCH-CMDREG](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CMDREG); it must be rebuilt or withdrawn. |
| [REFACTOR-CONVERT](#REFACTOR-CONVERT) | Union/Optional syntax conversion and NamedTuple conversion are deleted and unbuilt. |
| [REFACTOR-INLINE-VAR](#REFACTOR-INLINE-VAR), [REFACTOR-INLINE-FUNC](#REFACTOR-INLINE-FUNC), [REFACTOR-SIGNATURE](#REFACTOR-SIGNATURE), [REFACTOR-ABSTRACT](#REFACTOR-ABSTRACT) | Never implemented. |

Unshipped behaviour is not advertised in the README, the website, or the
user-facing docs until it works.

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

Renames identifiers across files using the import graph. Enhancements needed:

### Scope-Aware Rename {#REFACTOR-RENAME-SCOPE}

**Known gap.** The shipped implementation finds occurrences by whole-word text
search over the source, with a mask excluding string literals and comments. It
does not hardcode Python vocabulary — it searches for the *user's own* selected
identifier — so it is not a [REFACTOR-AST](#REFACTOR-AST) violation of the kind
that was deleted. It is still wrong: it cannot distinguish two unrelated
bindings that share a name, so renaming a local `x` renames every other `x` in
the file. It must be replaced with occurrences taken from resolved AST
references, which is what makes the scope rules below expressible at all:

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

When renaming a parameter, offer (opt-in) to update docstring references (`:param old_name:` → `:param new_name:`).

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

### Algorithm {#REFACTOR-EXTRACT-VAR-ALGO}

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

### Algorithm {#REFACTOR-INLINE-VAR-ALGO}

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

### Algorithm {#REFACTOR-INLINE-FUNC-ALGO}

1. **Resolve** the function definition.
2. **Validate**: function body is a single `return` expression, or a short block that can be inlined.
3. **Substitute** parameters with arguments. Handle keyword arguments, defaults, `*args`, `**kwargs`.
4. **Replace** the call expression with the substituted body.
5. **Remove** the function definition if it has no other callers (offer as option).

Initial implementation supports only single-expression bodies (`def f(x): return x + 1` → inline `f(y)` to `y + 1`).

---

## Move Symbol {#REFACTOR-MOVE}

**Code action kind**: `refactor.move`

**Trigger**: Cursor on a top-level function, class, or constant definition.

### Algorithm {#REFACTOR-MOVE-ALGO}

1. **Identify the symbol** and all its dependencies (imports it uses, other symbols it references).
2. **User selects destination** module (via editor file picker or quick-pick).
3. **Move the definition** to the destination file:
   - Add required imports to the destination.
   - Remove the definition from the source.
   - Add a re-export in the source (`from new_module import symbol`) if the source's `__all__` included it, or if other modules import it from the source.
4. **Update all importers** — rewrite `from old_module import symbol` to `from new_module import symbol` across the workspace.
5. **Clean up** — remove now-unused imports from both source and destination.

### Move to New File {#REFACTOR-MOVE-NEW}

As above, but the destination is a new file named after the symbol (`my_func` → `my_func.py` in the same directory).

---

## Change Signature {#REFACTOR-SIGNATURE}

**Code action kind**: `refactor.rewrite.signature`

**Trigger**: Cursor on a function definition.

### Operations {#REFACTOR-SIGNATURE-OPS}

- **Add parameter**: insert a new parameter with a default value. All existing callers remain valid.
- **Remove parameter**: remove a parameter. Update all callers to remove the corresponding argument (reject if any caller passes a value that differs from the default).
- **Reorder parameters**: change the order. Update all callers using positional arguments.
- **Rename parameter**: rename a parameter and update all keyword-argument callers.

Each operation produces a `WorkspaceEdit` covering the definition and all call sites found via the import graph and reference search.

---

## Convert Constructs {#REFACTOR-CONVERT}

**Code action kind**: `refactor.rewrite.convert`

Context-sensitive conversions offered when the cursor is on an applicable construct. Each must preserve runtime semantics:

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

---

## Implement Abstract Methods {#REFACTOR-ABSTRACT}

**Code action kind**: `refactor.rewrite.implement`

**Trigger**: Cursor on a class that inherits from an abstract base class with unimplemented abstract methods.

### Algorithm {#REFACTOR-ABSTRACT-ALGO}

1. **Resolve base classes** — find all `@abstractmethod` methods in the MRO.
2. **Filter** — exclude methods already implemented in the class.
3. **Generate stubs** for each missing method:
   - Copy the signature (name, parameters, type annotations).
   - Body: `raise NotImplementedError` (or `...` if the user prefers, configurable).
   - Preserve `@staticmethod`/`@classmethod` decorators.
4. **Insert** after the last existing method in the class (or after `__init__` if present).

---

## Cross-Cutting Concerns {#REFACTOR-CROSS}

Constraints applying to every refactoring.

### Formatter Conflict {#REFACTOR-FORMATTER}

After generating a `WorkspaceEdit`, run ruff on the affected ranges so generated code matches project formatting and does not trigger a format-on-save diff.

### Undo Granularity {#REFACTOR-UNDO}

Every refactoring returns exactly one `WorkspaceEdit` (one undo step). Never split into multiple sequential edits.

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

Implementation order. Restoring deleted behaviour outranks new behaviour, and
within that, user-visible breakage outranks silence — an action that produces a
*wrong* edit is worse than one that is absent.

1. **Auto-import insertion point** — currently hardcoded to line 0, so every
   auto-import lands above the module docstring. Wrong output, shipped today.
2. **Rename Symbol** — replace text occurrences with resolved AST references
   ([REFACTOR-RENAME-SCOPE](#REFACTOR-RENAME-SCOPE)), then re-add the
   `self.attr` / `cls.attr` sweep, then validation.
3. **Rename Module** (`workspace/willRenameFiles`) — import rewriting from
   `ImportInfo` spans.
4. **Move Symbol** — or withdraw the orphaned `MOVE_SYMBOL` command.
5. **Extract Function**
6. **Extract Constant** and **add `__all__`**
7. **Convert Between Constructs** (each conversion independent)
8. **Implement Abstract Methods**
9. **Inline Variable**
10. **Change Signature**
11. **Inline Function**

Every item above ships only with tests that fail against a text-matching
implementation: the same construct inside a docstring, inside a string literal,
split across lines, and reformatted.
