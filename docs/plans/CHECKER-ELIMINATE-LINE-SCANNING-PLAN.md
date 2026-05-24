# Plan: Eliminate Line-Based String Scanning ("Regex-Equivalent") in Checker Rules

## Context

`CLAUDE.md` is explicit: **"Regex = ⛔️ ILLEGAL. Use the proper parsing mechanism — usually ruff"**.

Good news: there is **zero `regex` / `fancy_regex` crate usage** in `crates/basilisk-checker/`. No `Cargo.toml` depends on a regex engine.

Bad news: 13 rules implement the **moral equivalent** — they iterate `source.lines()` and `String::starts_with` / `find` / `contains` Python keywords to reconstruct structure that already exists in `basilisk_resolver::ResolvedModule`. This is exactly the anti-pattern that produced the BSK-E0149 showstopper false positive (issue [#43](https://github.com/Nimblesite/Basilisk/issues/43)): a line *inside a module docstring* whose trimmed prefix happens to start with `class ` was parsed as a real class definition, polluting the type-param table and triggering a hard error on innocent prose.

This plan eliminates line-based scanning across all 13 rules and re-grounds them on the AST.

---

## Scope: 13 rules, 31 `.lines()` call sites

Inventory produced by repo-wide grep audit (2026-05-23). Severity reflects false-positive blast radius — HIGH means the rule shares BSK-E0149's exact bug class (string-matching `class `/`def `/`type ` prefixes that fire inside docstrings).

| Rule | Files | Scans for | Severity | AST already has it? |
|------|-------|-----------|----------|---------------------|
| **E0149** | `e0149/violations.rs`, `e0149/mod.rs` | `class`/`def`/`async def`/`type ` prefixes; PEP 695 type params | **HIGH** *(filed: #43)* | Yes — `type_statements`, `classes`, `functions` |
| **E0130** | `e0130/check.rs`, `e0130/collect.rs`, `e0130/mod.rs`, `e0130/variance.rs`, `e0130/variance_check.rs`, `e0130/utils.rs` | `class`/`def ` prefixes; `Generic[...]` patterns; TypeVar scoping | **HIGH** | Yes — `classes`, `functions`, `typevar_calls` |
| **E0126** | `e0126_helpers.rs` | `def`/`class`/`@` prefixes; `return`/`assert_type`/`if`/`else`/`for ` | **HIGH** | Yes — `functions`, `classes` |
| **E0060** | `e0060.rs` | `class`/`def`/`@` prefixes + operator lines | MEDIUM | Yes — `classes`, `functions` |
| **E0070** | `e0070.rs` | Annotated assignments (`: ` and ` = `) | MEDIUM | Yes — `module_vars` |
| **E0108** | `e0108.rs` | `ClassName(...).__slots__` patterns | MEDIUM | Yes — `classes` |
| **E0124** | `e0124.rs` | `self.X = expr` attribute assignments | MEDIUM | Yes — `functions` |
| **E0128** | `e0128.rs`, `e0128_helpers.rs` | `TypeVar()` calls with `=` parse; class filtering | MEDIUM | Yes — `typevar_calls`, `classes` |
| **E0142** | `e0142/helpers.rs` | Comparison operators (`<`, `>`, `<=`, `>=`) | MEDIUM | Partial — needs new AST accessor |
| **E0127** | `e0127.rs` | Indentation / body boundary | LOW | Yes — `functions` |
| **E0129** | `e0129.rs` | Leading indentation (comments, blanks) | LOW | Yes — `functions` |
| **E0131** | `e0131/mod.rs` | Comment / empty line filtering | LOW | Yes — `functions` |
| **E0125** | `e0125.rs` | Comment / empty line detection | LOW | Safe — line offset only |

**Aggregate**: 13 rules, **3 HIGH** (latent showstoppers, same bug class as #43), **6 MEDIUM**, **4 LOW**. Total ~31 `.lines()` call sites.

**Also in scope (non-rule)**: `crates/basilisk-checker/src/rules/shared.rs` and `crates/basilisk-checker/src/suppression.rs` use `.lines()` — these are LOW (line geometry / `# type: ignore` detection) and stay.

---

## Principle

**Every rule consumes `ResolvedModule` AST nodes. No rule iterates `source.lines()` to reconstruct Python structure.**

`ResolvedModule` already exposes (see `crates/basilisk-resolver/src/lib.rs`):
- `classes: Vec<ClassDef>` with `name_span`, `bases`, methods, type params
- `functions: Vec<FunctionDef>` with `name_span`, params, return type, local vars, decorators
- `type_statements: Vec<TypeStmt>` (PEP 695 `type` aliases)
- `module_vars: Vec<ModuleVar>` (annotated module-level assignments)
- `typevar_calls: Vec<TypeVarCall>` (old-style `TypeVar(...)` calls)
- `calls: Vec<Call>` with callee + args + spans
- `imports: Vec<Import>`

If a rule needs something not on `ResolvedModule`, **add it to the resolver**, don't re-parse the text in the rule.

Permitted line-level access:
- `span_for_line` for diagnostic geometry only.
- `suppression.rs` `# type: ignore` parsing (lexically simple, no Python-keyword matching).
- `format!()` / display of source slices for error messages.

---

## Phase 1 — Stop the bleeding (HIGH severity, blocks #43 and prevents next showstopper)

### Step 1.1: E0149 — full AST rewrite *(closes #43)*

**Files**: `crates/basilisk-checker/src/rules/e0149/violations.rs`, `crates/basilisk-checker/src/rules/e0149/mod.rs`, `crates/basilisk-checker/src/rules/e0149/helpers.rs`

Delete `collect_pep695_type_params` (lines 19–38 of `violations.rs`) and the main line-iteration driver in `mod.rs:78-117`. Replace with:

- `module.type_statements` for PEP 695 `type Foo[T] = ...` aliases (already has name, params with bounds, RHS spans).
- `module.classes` + `module.functions` filtered to those with `type_params: Vec<TypeParam>` populated by the resolver.
- For each violation check (bound cross-reference, module-level use, decorator use, method shadowing, `type` stmt circular / in-function / old-TypeVar), walk the AST node — not the line.

If `ResolvedModule` doesn't already carry PEP 695 type params on `ClassDef` / `FunctionDef` / `TypeStmt`, **add them in the resolver** as part of this step. That's a one-time cost paid by every rule below.

**Verification**:
1. Minimal repro from #43 produces zero diagnostics.
2. Fixture with `[SPEC-ID]` cross-references in docstring produces zero diagnostics.
3. All existing `e0149_tests.rs` and `e0126_e0149_tests.rs` pass.
4. Conformance suite FP count for E0149 stays at 0.

### Step 1.2: E0130 — TypeVar scoping AST rewrite

**Files**: all 6 files under `crates/basilisk-checker/src/rules/e0130/`

Same pattern: `e0130/collect.rs` and `e0130/check.rs` both line-iterate looking for `class ` / `def ` prefixes and `Generic[...]` usage. Drive off `module.classes` (incl. bases for `Generic[T]`) and `module.functions`. `module.typevar_calls` is already structured.

**Verification**: existing E0130 tests pass; add a docstring-prose fixture that previously would have falsely fired.

### Step 1.3: E0126 — return statement / body-boundary AST rewrite

**File**: `crates/basilisk-checker/src/rules/e0126_helpers.rs`

Currently scans for `def `/`class `/`@` to find function body boundaries, and for `return ` / `assert_type` inside the function. AST already has `FunctionDef.body` spans; use them. `assert_type` calls are in `module.calls`.

**Verification**: existing E0126 tests pass; add a docstring fixture containing the strings `def foo():` and `return None` and assert no false positive.

---

## Phase 2 — MEDIUM severity (false positives possible but narrower)

Each of the following is the same shape: replace `.lines()` walks with iteration over the relevant `ResolvedModule` collection. One sub-step per rule.

- **2.1 E0060** — `module.classes`/`functions` for decorator/header iteration.
- **2.2 E0070** — `module.module_vars` for annotated module-level assignments.
- **2.3 E0108** — `module.classes` for `__slots__` introspection.
- **2.4 E0124** — `module.functions[*].local_vars` for `self.X = ...` patterns inside methods.
- **2.5 E0128** — `module.typevar_calls` for `TypeVar(...)` and class-context filtering via `module.classes`.
- **2.6 E0142** — add `module.comparisons: Vec<Comparison>` to the resolver (operator + operand spans), then iterate it. Don't pattern-match `<`/`>` from raw text.

Each sub-step ships with:
1. A docstring-prose fixture that exercises the bug class.
2. Updated rule that emits **identical diagnostics** on real code (regression-locked by existing tests).
3. Zero new conformance FPs.

---

## Phase 3 — LOW severity cleanup (defensible, but should still go)

- **3.1 E0127**, **3.2 E0129**, **3.3 E0131**: indent / comment / blank-line filtering. Replace with `FunctionDef.body_span` lookups. Lexically simple but inconsistent — kill it for uniformity.
- **3.4 E0125**: line-offset usage is safe; leave it but add a comment justifying why this one is OK (it doesn't look at line *contents*).
- **3.5 `shared.rs` / `suppression.rs`**: keep `.lines()` usage. Add a doc comment to `span_for_line` and the `# type: ignore` parser explaining why these are the *only* permitted exceptions, so future rules don't copy the pattern.

---

## Phase 4 — Enforcement (prevent regression)

### Step 4.1: Lint script

Add `scripts/check-no-line-scanning.sh` (or a make target) that runs:

```
! grep -rn '\.lines()' crates/basilisk-checker/src/rules/ \
    | grep -v 'span_for_line\|//\|tests/'
```

Wire into `make lint` and CI (`.github/workflows/ci.yml`). Any new `.lines()` call site in `rules/` fails the build.

### Step 4.2: Forbidden-pattern lint for keyword string matching

```
! grep -rn 'starts_with("class \|starts_with("def \|starts_with("async def \|starts_with("type \|starts_with("@\|starts_with("import \|starts_with("from ' crates/basilisk-checker/src/ \
    | grep -v 'tests/'
```

Same wiring. Currently 64 hits across the rule tree — Phase 1+2 should drive this to 0.

### Step 4.3: CLAUDE.md amendment

Add to the **Rules** section of `CLAUDE.md`:

> **Line-based string scanning is regex.** `source.lines()` + `line.starts_with("class "|"def "|"type "|"@"|"import "|"from ")` is the moral equivalent of regex and is equally **⛔️ ILLEGAL**. Drive every rule off `ResolvedModule` AST nodes. If the AST is missing what you need, extend the resolver — don't re-parse the text in the rule. Permitted exceptions: `span_for_line` (diagnostic geometry), `suppression.rs` `# type: ignore` parsing. Nothing else.

### Step 4.4: GitHub issues for each HIGH rule

File one issue per HIGH rule (E0149 = #43 already filed, plus E0130 and E0126) with label `showstopper`, linking back to this plan and #43. MEDIUM/LOW rules get one umbrella tracking issue.

---

## Verification (whole plan)

- `make ci` passes.
- Conformance suite FP count strictly decreases (ratchet).
- `make test` passes — no test mutations to make a rule pass; only AST migration.
- The lint scripts in Step 4.1 / 4.2 return zero hits inside `crates/basilisk-checker/src/rules/`.
- A fresh fixture file containing ONLY docstring prose with `class `/`def `/`type ` prefixes and `[SPEC-ID]` brackets produces **zero diagnostics** from the checker.

---

## Out of scope

- `lspkit-*` migration (tracked separately in `CLAUDE.md`).
- IDE extension code (line-based parsing in TS/Rust LSP server is mostly LSP-position arithmetic, not Python-keyword matching).
- `basilisk-test-macros` / profiler scenario tests (test infrastructure, not checker rules).

---

## Risk

The resolver may not carry all the structured info HIGH rules need today (e.g. PEP 695 type params on `ClassDef`). Phase 1.1 may grow the resolver. That's fine — it's a one-time cost that benefits every subsequent rule and aligns with the architecture. Better than 13 rules each re-parsing the same text wrong.
