# Basilisk checker architecture {#CHKARCH}

## No "strict mode" — two commands, one config {#CHKARCH-CONFIGURATION-ONLY}

Basilisk has **no modes** (no `--strict`, no `off`/`basic`/`standard`/`strict` dial). One rule universe is partitioned exactly once, by provenance tag, into two commands ([CHKARCH-COMMANDS](#CHKARCH-COMMANDS)):

1. **`basilisk check` — the typing spec, always.** Every `pep`-tagged rule runs on every check, config or no config. Configuration can grade a PEP rule (`error`/`warning`/`info`) but can **never disable one**. A bare tree — exactly what the conformance scorer runs, no Basilisk config of any format, no "conformance mode" — is therefore every PEP rule at `error` ([CHKARCH-CONFORMANCE-MODE](#CHKARCH-CONFORMANCE-MODE)).

2. **`basilisk analyze` — the opt-in layer, tabula rasa.** Every rule *not* tagged `pep` — require-annotation (`BSK-0001`/`BSK-0002`/`BSK-0004`), require-`@override` (`BSK-0025`), redundant-annotation (`BSK-0050`), explicit-`Any` nudge (`BSK-0014`), suppression audit, uv dependency hygiene, stub suggestions — runs only when configuration resolves it to a non-disabled severity. No entry, no check. An empty or missing `[tool.basilisk]` table means `analyze` reports nothing.

Configuration **grades**; commands **select**. The config file never chooses commands, and there are no presets, mutation intents, or rule-family booleans. Strict-by-default is delivered by the LSP's one-time two-line seed — `"basilisk" = "error"` — never by hidden defaults ([LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING)).

No PEP rule may be disabled, deleted, or unregistered to move the conformance number ([CHKARCH-CONFORMANCE-MODE](#CHKARCH-CONFORMANCE-MODE)).

### The partition {#CHKARCH-COMMANDS}

A rule is check-scope **iff** it carries the `pep` provenance tag; everything else is analyze-scope. Every rule belongs to exactly one command, and the registry's canonical tags are the single source of that partition. The LSP publishes the union of both scopes by default; an IDE-level client option — never project config — can restrict it to check ([LSPARCH-DIAGNOSTIC-SCOPE](LSP-ARCHITECTURE-SPEC.md#LSPARCH-DIAGNOSTIC-SCOPE)).

---

## Dependency Strategy {#CHKARCH-DEPS}

Depend on established open-source tools rather than reimplementing them.

### Direct Dependencies {#CHKARCH-DEPS-DIRECT}

| Dependency | Purpose | License | Rationale |
|---|---|---|---|
| **`ruff_python_formatter`** | Code formatting | MIT | Embedded in-process — the formatter is Ruff's, no `ruff` CLI. Pinned to the same rev as the parser ([LSPFMT-ENGINE](LSP-FORMATTING-SPEC.md#LSPFMT-ENGINE)). |
| **`ruff_python_parser`** | Python AST parsing | MIT | Battle-tested Rust crate. Powers Ruff. Our parser. |
| **`python/typeshed` stdlib** | Standard-library `.pyi` and stub-distribution data | Apache-2.0; parts MIT | Implements "Typeshed stubs for the standard library" from pinned typing step 3 ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)). Step 3 uses one custom tree, exact-SHA archive, or bundled stdlib ZIP; sources never mix ([STUBRES-TYPESHED](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED)). |
| **Salsa** | Incremental computation framework | Apache-2.0/MIT | Powers rust-analyzer. Proven at scale. |
| **`lsp-server`** / **`tower-lsp`** | LSP implementation | MIT | Standard Rust LSP crates. |

### Interoperability {#CHKARCH-DEPS-INTEROP}

| Tool | Interop Strategy |
|---|---|
| **Ruff** | Basilisk **embeds** the `ruff_python_formatter` crate in-process for formatting and reimplements import hygiene natively — the `ruff` CLI is never spawned ([LSPFMT-DECISION](LSP-FORMATTING-SPEC.md#LSPFMT-DECISION)). Configuration unified in `pyproject.toml` (`[tool.ruff.format]`). |
| **typeshed** | Step 3 selects a custom `typeshed-path`, an exact-SHA archive (explicit pin or current `main`), or the bundled stdlib ZIP. Basilisk adopts the pinned typing step-3 canonical-path SHOULD; step-3 sources never mix ([STUBRES-PEP561](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PEP561), [`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)). |
| **PEP 561** | Full support for `py.typed` packages, inline type annotations, and stub-only packages. |

---

## Core Type System {#CHKARCH-TYPESYS}

How Basilisk decides what to report (configuration, not modes), the PEPs it covers, and how it infers, narrows, and reasons about reachability.

### Strictness Model {#CHKARCH-STRICTNESS}

Behaviour is per-rule configuration over the two-command partition ([CHKARCH-COMMANDS](#CHKARCH-COMMANDS)): `check` is pure PEP conformance, always; `analyze` is the explicit opt-in layer. The subsections define severity values, suppression/override directives, and their precedence.

#### No Modes — Configuration Decides Everything {#CHKARCH-STRICTNESS-ONLY}

The require-annotation house rules (`BSK-0001`/`BSK-0002`) are analyze-scope ([CHKARCH-COMMANDS](#CHKARCH-COMMANDS)): `basilisk check` never fires them, and `basilisk analyze` fires them only when configuration resolves them to a non-disabled severity. With no config these snippets pass everywhere:

```python
# Passes check always; fires BSK-0001 under analyze once configured
def greet(name):
    return f"Hello, {name}"

# Passes check always; fires BSK-0002 under analyze once configured
# (the returned call is not inferable — a literal/f-string return would
# infer the type and stay silent, [TYPEINF-EXCEEDS-REQUIRED])
def greet(name: str):
    return build_greeting(name)

# OK under any configuration
def greet(name: str) -> str:
    return f"Hello, {name}"
```

Even when enabled, the require-annotation rules fire only where the type cannot
be inferred — see
[TYPEINF-EXCEEDS-REQUIRED](CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-EXCEEDS-REQUIRED).

#### `Any` Is Explicit, Never Implicit {#CHKARCH-STRICTNESS-ANY}

```python
from typing import Any

# ERROR: Implicit Any -- untyped import [imports_unresolved]
from untyped_lib import do_stuff

# OK: Explicit Any with reason
result: Any = do_stuff()  # basilisk: allow[imports_unresolved] -- untyped dependency, tracking in #1234

# ERROR (when the explicit-Any house rule is enabled): Bare Any without justification
def process(data: Any) -> Any:  # BSK-0014: Explicit Any requires reason comment
    pass
```

The "untyped import" case above is an **implicit `Any`** produced by the static
resolution model: an import Basilisk cannot resolve by inspecting files on disk —
including computed/dynamic imports and modules only a runtime `sys.meta_path`
hook could supply — lands in a terminal unresolved state and is surfaced by
`imports_unresolved` rather than silently accepted. The interpreter is never
executed to follow an import. See the normative
[§STUBRES-STATIC-MODEL](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-STATIC-MODEL).

#### Diagnostic Severity Values {#CHKARCH-STRICTNESS-SEVERITY}

Every rule has four configurable severity values:

| Severity | Behavior | Blocks CI | LSP Indicator |
|---|---|---|---|
| `error` | Full diagnostic with fix suggestions | Yes | Red squiggly |
| `warning` | Diagnostic shown but does not block | No | Yellow squiggly |
| `info` | Informational hint only | No | Blue hint |
| `disabled` | Rule emits no diagnostic | No | Nothing |

Severity resolves through [CHKARCH-CONFIG-MODEL](#CHKARCH-CONFIG-MODEL): the
nearest deciding table wins, a rule entry beats tag entries, and the strictest
matching tag entry wins. PEP rules bottom out at `error` and can never be
disabled; analyze rules bottom out at disabled — no entry, no check. There are
no default, inherited, or "native" severity values, and rule codes carry no
severity class ([CHKARCH-DIAG-CODES](#CHKARCH-DIAG-CODES)). Inline directives
can still override any running rule per line, block, or file
([CHKARCH-STRICTNESS-SUPPRESSION](#CHKARCH-STRICTNESS-SUPPRESSION)).

A rule that resolves to disabled must emit nothing; the current
implementation still executes the shared rule registry and filters output
afterwards, so disabled guarantees no diagnostic, not zero execution cost.
Skipping execution entirely is an optimisation tracked by
[CONFIGEDITOR-PLAN-DOMAIN](../plans/LSP-CONFIGURATION-EDITOR-PLAN.md#CONFIGEDITOR-PLAN-DOMAIN),
not part of severity correctness.

#### Inline Suppression and Severity Override {#CHKARCH-STRICTNESS-SUPPRESSION}

Basilisk supports standard `# type: ignore` (mypy/Pyright compatible) plus its own comment directives.

**Per-line: standard compatibility**
```python
from fastmcp import FastMCP  # type: ignore
```

**Per-line: Basilisk-specific with error code**
```python
from fastmcp import FastMCP  # type: ignore[imports_unresolved]
```

**Per-line: severity override (demote or promote)**
```python
from fastmcp import FastMCP  # type: warning[imports_unresolved]
from fastmcp import FastMCP  # type: info[imports_unresolved]
from fastmcp import FastMCP  # type: disabled[imports_unresolved]
```

**Per-line: override all rules on this line**
```python
data = unsafe_cast(value)  # type: warning
data = unsafe_cast(value)  # type: disabled
```

**Per-block: override severity for a range of lines**
```python
# type: disabled[imports_unresolved]
from fastmcp import FastMCP
from result import Result, Ok, Err
from errors import AutomatorError, ErrorCode
from models import Platform, Credentials
# type: end-disabled[imports_unresolved]
```

Block directives work with all severity values: `# type: warning[CODE]` / `# type: end-warning[CODE]`, `# type: info[CODE]` / `# type: end-info[CODE]`, `# type: disabled[CODE]` / `# type: end-disabled[CODE]`. Omitting the code applies to all rules.

**Per-file: file-level directive at the top of the file**
```python
# basilisk: relaxed
# All errors become warnings in this file
```

```python
# basilisk: file-disabled[imports_unresolved]
# Disable E0010 for the entire file
```

```python
# basilisk: file-warning[imports_unresolved, returns_compatibility]
# Demote E0010 and E0011 to warnings for the entire file
```

**Per-folder configuration** in `pyproject.toml` — the nearest deciding table
wins per rule ([CHKARCH-CONFIG-MODEL](#CHKARCH-CONFIG-MODEL)):
```toml
# pyproject.toml at the project root
[tool.basilisk.rule-tags]
"basilisk" = "error"            # every house rule on — strict by default

[tool.basilisk.rules]
"BSK-0050" = "warning"         # ...except this one, graded down
```
```toml
# legacy/pyproject.toml — decides, per rule, for everything under legacy/
[tool.basilisk.rules]
"BSK-0001" = "disabled"        # house rules may be disabled
"imports_unresolved" = "warning" # PEP rules may be graded — never disabled
```

Third-party import noise is handled the same way: grade `imports_unresolved`
in the folder that contains the affected code, or use the inline directives
above at the import site. There are no module-pattern or glob-path override
tables.

#### Suppression Precedence {#CHKARCH-STRICTNESS-PRECEDENCE}

When multiple overrides apply, the most specific wins:

1. **Per-line comment** (highest priority)
2. **Per-block comment**
3. **Per-file directive**
4. **Nearest deciding folder config** — rule entry over tag entry, strictest
   matching tag ([CHKARCH-CONFIG-MODEL](#CHKARCH-CONFIG-MODEL))
5. **Scope default** (lowest): `pep` rules run at `error`; everything else
   does not run ([CHKARCH-COMMANDS](#CHKARCH-COMMANDS))

#### Compatibility {#CHKARCH-STRICTNESS-COMPAT}

Recognized comment formats:

| Comment | Behavior |
|---|---|
| `# type: ignore` | Suppress all diagnostics on this line (PEP 484 / mypy / Pyright compatible) |
| `# type: ignore[imports_unresolved]` | Suppress specific code (Basilisk extension, mypy-compatible syntax) |
| `# type: warning` | Demote all diagnostics to warnings (Basilisk-specific) |
| `# type: warning[imports_unresolved]` | Demote specific code to warning (Basilisk-specific) |
| `# type: info` | Demote all diagnostics to info (Basilisk-specific) |
| `# type: info[imports_unresolved]` | Demote specific code to info (Basilisk-specific) |
| `# type: disabled` | Disable all diagnostics on this line (Basilisk-specific) |
| `# type: disabled[imports_unresolved]` | Disable specific code on this line (Basilisk-specific) |
| `# basilisk: relaxed` | Per-file: all errors become warnings |
| `# basilisk: file-disabled[CODE]` | Per-file: disable specific rules |
| `# basilisk: file-warning[CODE]` | Per-file: demote specific rules to warnings |

The `# type:` prefix keeps compatibility with tools that recognize `# type: ignore`; others treat `# type: warning` as unknown and ignore it.

#### Suppression Directives as Opt-In Diagnostics {#CHKARCH-STRICTNESS-SUPPRESSION-DIAGNOSTICS}

Suppression auditing is a Basilisk-specific rule family and analyze-scope
([CHKARCH-COMMANDS](#CHKARCH-COMMANDS)): `basilisk check` never emits it, so a
bare clone stays clean, and the standard seed's `"basilisk" = "error"` tag
entry turns it on ([LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING)).
Once a rule resolves to a non-disabled severity, every parsed source directive
produces at most one audit diagnostic at the directive's comment span:

| Rule | Classification |
|---|---|
| `BSK-0060` | A valid code-specific directive actively suppresses a diagnostic or changes its severity |
| `BSK-0061` | An active blanket directive applies without a Basilisk rule selector |
| `BSK-0062` | A syntactically valid directive matches nothing or changes no effective severity |
| `BSK-0063` | The directive is malformed, names an unknown rule, conflicts with another directive, or has an unmatched block boundary |

Classification precedence is malformed → unused → active blanket → active
specific, so a directive never produces duplicate audit noise. The audit data
records its kind, scope, selected codes, and matched-diagnostic count for LSP
navigation. These diagnostics are appended **after** ordinary inline suppression
and are not passed through that same directive set; an ignore cannot hide the
audit diagnostic describing itself. Configuration can still set each audit rule
to `error`, `warning`, `info`, or `disabled` normally.

All four rules carry the tags `basilisk` and `suppressions`. Their workspace
configuration/editor behavior is specified by
[CONFIGEDITOR-SUPPRESSIONS](LSP-CONFIGURATION-EDITOR-SPEC.md#CONFIGEDITOR-SUPPRESSIONS).

### Python Typing PEP Coverage {#CHKARCH-PEPS}

Basilisk's **target** is 100% conformance with the Python typing specification. We measure against the latest **`python/typing@main`**, recording the exact graded commit by hash in `conformance_report.json` (currently [`<!--g:short-->60df123<!--/g:short-->`](https://github.com/python/typing/tree/60df123ccfe9ae0472b1409ef4a00d51ffc5d972/conformance)). Today the official scorer, run unmodified in CI on the binary in its default configuration (the PEP conformance set; see [CHKARCH-CONFORMANCE-MODE](#CHKARCH-CONFORMANCE-MODE)), reports **<!--g:pass-->141<!--/g:pass--> of <!--g:total-->141<!--/g:total--> files passing (<!--g:score-->100.0%<!--/g:score-->)**, with **<!--g:fp-->0<!--/g:fp--> false positives** and **<!--g:missed-->0<!--/g:missed--> missed required errors** (<!--g:caught-->970<!--/g:caught--> caught). We run that suite in CI on every change; the gate ratchets the pass-percentage **up** and the false-positive ceiling **down** — closed only by fixing the checker, never by disabling a rule.

#### Foundation PEPs {#CHKARCH-PEPS-FOUNDATION}

| PEP | Title | Status |
|---|---|---|
| 484 | Type Hints | Required |
| 526 | Variable Annotations | Required |
| 544 | Protocols (Structural Subtyping) | Required |
| 585 | Generics in Standard Collections | Required |
| 604 | Union `X \| Y` Syntax | Required |

#### Advanced PEPs {#CHKARCH-PEPS-ADVANCED}

| PEP | Title | Status |
|---|---|---|
| 586 | Literal Types | Required |
| 589 | TypedDict | Required |
| 591 | Final Qualifier | Required |
| 612 | ParamSpec | Required |
| 613 | TypeAlias | Required |
| 634 | Structural Pattern Matching | Required |
| 646 | Variadic Generics (TypeVarTuple) | Required |
| 647 | TypeGuard | Required |
| 673 | Self Type | Required |
| 675 | LiteralString | Required |
| 681 | Data Class Transforms | Required |
| 692 | TypedDict for **kwargs | Required |
| 695 | Type Parameter Syntax (`def f[T]()`) | Required |
| 696 | TypeVar Defaults | Required |
| 698 | Override Decorator | Required |
| 702 | Deprecated Decorator | Required |
| 742 | TypeIs (Exhaustive Narrowing) | Required |

### Type Inference Engine {#CHKARCH-INFERENCE}

When the require-annotation rules (`BSK-0001`/`BSK-0002`/`BSK-0004`) have config entries, explicit annotations are required on public APIs **only where the type cannot be inferred** ([TYPEINF-EXCEEDS-REQUIRED](CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-EXCEEDS-REQUIRED)); local variable types are always inferred:

```python
def process(items: list[str]) -> int:
    count = 0              # inferred: int
    filtered = [x for x in items if x.startswith("a")]  # inferred: list[str]
    count = len(filtered)  # OK: int = int
    return count
```

- **Public APIs** (module-level functions/variables, class methods): explicit annotations required when the require-annotation rules are enabled in configuration
- **Local variables**: inferred from assignments, comprehensions, control flow
- **Cross-module**: does NOT cross boundaries for public symbols; imports from typed modules resolve to declared types, from untyped modules produce `imports_unresolved`

### Type Narrowing and Flow Analysis {#CHKARCH-NARROWING}

Full support for:
- `isinstance()` / `issubclass()` guards with bidirectional narrowing
- Truthiness narrowing (`if x:` narrows `Optional[T]` to `T`)
- Pattern matching exhaustiveness (PEP 634)
- Sentinel / `None` narrowing
- Custom type guards (`TypeGuard`, `TypeIs` per PEP 742)
- Negative narrowing in `else` branches
- Assignment-based narrowing

### Reachability Analysis {#CHKARCH-REACHABILITY}

- Dead code detection after narrowing
- Unreachable branch elimination
- `NoReturn` propagation from `sys.exit()`, `raise`, and custom `NoReturn` functions
- `assert_never()` for exhaustiveness checking
- Platform-aware reachability (an unknown target preserves every platform branch)

### Target Version and Platform {#CHKARCH-VERSION-TARGET}

A rule consults the project's Python version only when the maintained typing
specification, an accepted PEP, or Python language semantics makes its result
version-dependent. A version-independent rule must not branch on a Python
release. Basilisk has no canonical Python version.

- `BasiliskConfig.python_version` / `python_platform` (from `pyproject.toml`
  `[tool.basilisk]` `python-version`/`python-platform`) parse into a typed
  `CheckContext { target_version: (major, minor), target_platform }`
  (`crates/basilisk-checker/src/context.rs`).
- When the checker config does not pin a version, the CLI and LSP detect it
  from project files per
  [`[LSPUV-PYTHON-VERSION-RESOLUTION-ORDER]`](LSP-UV-INTEGRATION-SPEC.md):
  `.python-version` → `[project].requires-python` lower bound → `uv.lock`
  `requires-python` lower bound (`basilisk_uv::python_version::resolve_target_python_version`).
- When `python-platform` is absent, the CLI and LSP probe only an EXPLICITLY
  selected interpreter (`python-interpreter` / `BASILISK_PYTHON`) for
  `sys.platform`; with no explicit interpreter the host constant is the
  evidence (an auto-discovered interpreter can only report the host value —
  see [`[LSPUV-PYTHON-VERSION-RESOLUTION-ORDER]`](LSP-UV-INTEGRATION-SPEC.md)).
  Either way the concrete evidence threads through Typeshed guard selection
  and checker rules. An explicit `python-platform = "All"` keeps
  cross-platform intersection semantics. A failed explicit-interpreter probe
  leaves the platform unknown; the checker never substitutes the host for an
  interpreter the user explicitly chose.
- `rules::run_all(module, ctx)` threads the context into every
  `Rule::check(module, ctx, diagnostics)`. Feature-version boundaries come from
  their governing PEP or Python language rule, never from a book-wide or
  product-wide support target.

#### Version/Platform Narrowing {#CHKARCH-VERSION-NARROWING}

The pinned stub syntax text specifically says the `type` keyword is "only
accepted by the Python parser in Python 3.12 and later"
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- `directives_version_platform` evaluates `sys.version_info` / `sys.platform` guards against
  `ctx.target_version`, so dead-branch analysis follows the project's real
  target.
- `version_target_syntax` rejects PEP 695 syntax (`type X = …`, `class C[T]`, `def f[T]`)
  when `ctx.target_version < (3, 12)` — the target interpreter cannot even
  parse it.

Tests: `crates/basilisk-checker/tests/checker/version_target_tests.rs`.

---

## Mojo-inspired safety analysis {#CHKARCH-MOJO-SAFETY}

Status: planned, opt-in, and not wired into the checker pipeline. The
`basilisk-mojo` crate is scaffolding; shipping PEP rules must not reuse these
anchors or diagnostic descriptions.

### Ownership tracking {#CHKARCH-MOJO-OWNERSHIP}

The target is explicit `Annotated[T, Borrowed|InOut|Owned]` analysis for
mutation-of-borrowed and use-after-transfer diagnostics.

### Parameter immutability {#CHKARCH-MOJO-IMMUTABLE}

The target is an opt-in rule that treats mutable parameters as read-only unless
marked `InOut`; it does not change Python runtime semantics.

### Structural discipline {#CHKARCH-MOJO-STRUCTURAL}

The target is opt-in checks for dynamic attributes and related typed-class
structure. Existing PEP/dataclass rules remain separate.

### Explicit coercion {#CHKARCH-MOJO-COERCION}

The target is opt-in diagnostics for selected implicit conversions. It must not
contradict the typing-spec numeric tower used by default PEP rules.

### Compatibility contract {#CHKARCH-MOJO-COMPAT}

All metadata uses standard Python typing constructs, all rules are off by
default, and the implementation plan is
[CHECKER-ADVANCED-FEATURES-PLAN.md](../plans/CHECKER-ADVANCED-FEATURES-PLAN.md).

## Diagnostic Rules {#CHKARCH-DIAG}

### Design Philosophy {#CHKARCH-DIAG-PHILOSOPHY}

Every diagnostic must be:
1. **Precise** -- exact location (file, line, column, span)
2. **Clear** -- explains what is wrong and why
3. **Actionable** -- suggests at least one fix
4. **Stable** -- error codes are never renumbered or reused

### Error Code System {#CHKARCH-DIAG-CODES}

Format: `BSK-nnnn` for Basilisk-original rules; conformance-named PEP rules keep their `python/typing` snake_case names. A code carries **no severity class** — severity resolves through [CHKARCH-CONFIG-MODEL](#CHKARCH-CONFIG-MODEL): PEP rules always run in `check` and bottom out at `error`; analyze rules run only when configuration decides them. Inline directives can still override per line, block, or file — see [CHKARCH-STRICTNESS-SEVERITY](#CHKARCH-STRICTNESS-SEVERITY) and [CHKARCH-STRICTNESS-SUPPRESSION](#CHKARCH-STRICTNESS-SUPPRESSION).

### Generated rule index {#CHKARCH-DIAG-REFERENCE}

The checker rule source is authoritative. Run
`python3 scripts/gen_rules_reference.py --data` to generate
`website/src/_data/rules.json`, which drives the public
[rule reference](https://www.basilisk-python.dev/docs/rules/) and per-code
error pages. Do not maintain a second code/description table here.

Compatibility anchors used by existing implementation and test comments:

- Live rule modules and conformance categories {#CHKARCH-DIAG-CATEGORIES}
- Historical test shard: quality {#CHKARCH-DIAG-QUALITY}
- Historical test shard: unused {#CHKARCH-DIAG-UNUSED}
- Missing-annotation rules {#CHKARCH-DIAG-MISSING}
- Core type-safety rules {#CHKARCH-DIAG-TYPESAFETY}
- Historical group: ownership {#CHKARCH-DIAG-OWNERSHIP}
- Historical group: immutability {#CHKARCH-DIAG-IMMUTABILITY}
- Historical group: structural typing {#CHKARCH-DIAG-STRUCTURAL}
- Historical group: coercion {#CHKARCH-DIAG-COERCION}
- Historical group: optional/special types {#CHKARCH-DIAG-OPTIONAL}

These group names are navigation anchors, not current rule classifications.
Tags from [CHKTAG](CHECKER-RULE-TAGGING-SPEC.md#CHKTAG) are the only
classification authority.

#### Constructor-to-callable conversion {#CHKARCH-DIAG-CTOR-CALLABLE}

`constructors_callable` implements the typing-spec rule
["Converting a constructor to callable"](https://typing.readthedocs.io/en/latest/spec/constructors.html#converting-a-constructor-to-callable).
When a class object flows through an identity-over-callable function
(`def f(cb: Callable[P, R]) -> Callable[P, R]`), the returned value gains the
class's constructor-to-callable signature, and calls to a variable bound that way
are validated against it.

Synthesized signature, in priority order:

1. The metaclass `__call__` (when declared) — e.g. `__call__(*args, **kwargs)` accepts any call.
2. `__new__` when its return type is neither `Self` nor the class itself (e.g. `-> int`, `-> Proxy`, `-> Any`); `__init__` is then ignored.
3. Otherwise `__init__` (or `__new__` when no `__init__`); a class with neither synthesizes a zero-argument callable returning the instance.

Fires when a call supplies too few/many positional arguments, names a non-parameter keyword, or binds a function-scoped `TypeVar` inconsistently (`list[T]` filled by both `list[int]` and `list[str]`). Conservative: starred positional args and `**kwargs` unpacking suppress arity checks. Implemented in `crates/basilisk-checker/src/rules/e0153.rs`; tests in `crates/basilisk-checker/tests/e0153_tests.rs`.

#### Strict local-stub member access {#CHKARCH-DIAG-STUB-MEMBER}

`imports_module_attribute` makes a **user/local stub authoritative**: when `import X` resolves
to a `.pyi` under a configured `stub-paths` directory (including the
auto-discovered `.basilisk/stubs/` the "Create local type stub" quick fix writes —
see [STUBRES-CREATE-LOCAL](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CREATE-LOCAL)),
accessing `X.attr` where the stub declares neither `attr` nor a module-level
`def __getattr__` is a hard error.

The `def __getattr__(name: str) -> Any: ...` in the create-local skeleton is the
**explicit opt-out**: keep it and every attribute is permitted (module stays `Any`);
remove it and declare specific symbols to opt into checked member access.

Scope (Phase 1): only plain, single-segment `import X` backed by a user stub. The
member API is captured during import resolution
(`crates/basilisk-lsp/src/import_resolver.rs`, CLI and LSP paths) and carried on
`ResolvedModule.imported_modules`. That map is populated *only* for user stubs, so
the rule is a no-op for code without local stubs (conformance suite, first-party
code) — false-positive surface is zero by construction. Third-party typeshed /
`py.typed` packages, instance/class attribute access, and dotted/aliased imports
are deferred. Implemented in `crates/basilisk-checker/src/rules/e0154/`; tests in
`crates/basilisk-checker/src/rules/e0154/tests.rs`.

#### Missing imported name {#CHKARCH-DIAG-IMPORT-MEMBER}

`imports_missing_name` closes the symbol-level half of import checking (GitHub #55):
`from M import name` where the module path `M` resolves to a **workspace `.py`
source** but `name` is neither a module-level binding in `M` nor an existing
submodule of the package is an error — at runtime it raises `ImportError`.
This complements [CHKARCH-DIAG-TYPESAFETY] (`imports_unresolved`, module-path
level) and [CHKARCH-DIAG-STUB-MEMBER] (`imports_module_attribute`, stub-backed
attribute access).

The rule re-reads and parses the target module at check time (read through
`basilisk_common::fs::read_tracked` so the CLI result cache records the content
edge; the parse is memoized per `(path, content-hash)` process-wide) and
collects **every** module-level binding form: `def`/`class`, all assignment
forms, `import`/`from` bindings (re-exports), `for`/`with`/`match`/`except`
targets, walrus expressions, and `type` alias statements — recursing through
control flow but not into function/class bodies.

Conservative by construction — silence over guessing:

- a module-level `__getattr__` (PEP 562) permits any name;
- a target containing `from x import *` has an unknowable member set and is skipped;
- an unreadable or unparseable target is skipped (its own check reports the syntax error);
- `from pkg import mod` is satisfied by an existing `pkg/mod.py`, `pkg/mod.pyi`, or `pkg/mod/`;
- imports resolving into `site-packages` are out of scope (PEP 561 trust
  boundary — untyped third-party sources belong to `missing_type_stubs`);
- in cross-module mode a name found in live-buffer `imported_symbols` is
  trusted over the on-disk view.

Implemented in `crates/basilisk-checker/src/rules/imports_missing_name.rs`;
tests in `crates/basilisk-checker/tests/imports_missing_name_tests.rs`.

#### TypedDict `extra_items` / `closed` (PEP 728) {#CHKARCH-DIAG-TYPEDDICT-EXTRA-ITEMS}

`typeddicts_extra_items` implements [PEP 728](https://peps.python.org/pep-0728/) — the
`extra_items=` and `closed=` class keywords on `TypedDict`. `extra_items=T` defines an
infinite set of non-required (read-only when `T` is `ReadOnly[...]`) extra items of value
type `T`; `closed=True` forbids extra items. The rule validates four families, operating
directly on the module AST (independent of resolver state):

1. **Class-definition legality.** `closed=` must be a literal `True`/`False`;
   `extra_items=` may not wrap `Required[...]`/`NotRequired[...]`; a subclass may
   not set `closed=False` when a superclass is `closed=True` or sets
   `extra_items`; a subclass may not set `closed=True` when a superclass has a
   *non-read-only* `extra_items`; and a subclass may not redeclare `extra_items`
   unless the nearest superclass that declares it does so as `ReadOnly[...]` (a
   plain TypedDict carries the implicit read-only `extra_items=ReadOnly[object]`,
   so overriding it is always permitted).
2. **Dict-literal construction.** When a dict literal is assigned to a TypedDict
   with `extra_items=T`, every key outside the declared schema must carry a value
   type assignable to `T`.
3. **TypedDict-to-TypedDict assignability.** When both sides resolve to
   TypedDicts, each source field outside the target schema, and the source's
   effective `extra_items` pseudo-item, must satisfy the target's `extra_items`
   (covariant when the target is read-only, consistent — and non-required — when
   it is not). A plain TypedDict contributes the implicit `ReadOnly[object]`.
4. **Constructor calls.** Calling the class object with a keyword outside the
   declared schema is rejected unless the TypedDict declares a non-read-only
   `extra_items=T` whose type the argument matches.

Implemented in `crates/basilisk-checker/src/rules/e0156/`; conformance fixture is
`conformance/tests/typeddicts_extra_items.py`.

#### `ReadOnly` `TypedDict` inheritance {#CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE}

Implements the typing-spec
[read-only `TypedDict` items](https://typing.readthedocs.io/en/latest/spec/typeddict.html#read-only-items)
rules (PEP 705). The foundation is **transitive `TypedDict` recognition**:
`ClassInfo::is_typed_dict` was `true` only for classes naming `TypedDict`
*directly*, so a subclass (`class Album(NamedDict): ...`) was invisible to every
`TypedDict` rule. Shared helpers in
`crates/basilisk-resolver/src/scope/typeddict_meta.rs`
(`is_transitive_typeddict`, `has_extra_items_transitive`,
`transitive_typeddict_names`, `strip_typeddict_qualifiers`) and the field-merge in
`crates/basilisk-resolver/src/visitor/typeddict_schema.rs` (`effective_fields`)
compute each `TypedDict`'s full schema (own + inherited fields, most-derived
declaration winning, carrying `ReadOnly` qualifier and required-ness).

The qualifier rules are enforced across four codes:

- **`typeddicts_inheritance`** (`crates/basilisk-checker/src/rules/e0038.rs`) — redeclaration
  legality. A writable item may not be redeclared `ReadOnly`; a required item
  may not be redeclared not-required; a writable item's value type is invariant
  while a `ReadOnly` item's may be narrowed to a subtype (a different container
  head is a legal narrowing, the same *invariant* container — `list`/`dict`/`set`
  — with different arguments is not). Multiple inheritance with two bases
  declaring a field with conflicting core type, required-ness, or read-only-ness
  is rejected. The decision functions (`parse_field_qualifiers`,
  `redeclaration_violation`, `value_type_incompatible`, `type_head`,
  `is_invariant_container`, `bases_conflict`) are pure and mutation-tested
  (`crates/basilisk-checker/tests/mutation_kill_tests.rs`, every viable mutant
  killed).
- **`typeddicts_readonly`** — writes to an *inherited* `ReadOnly` field that the subclass
  did not redeclare as writable.
- **`typeddicts_operations`** — wrong value type / missing required key against the merged
  schema, including plain reassignment of an already-typed variable.
- **`assignment_compatibility`** — skips dict-literal assignments to transitive `TypedDict`
  subclasses (field-level checking belongs to E0093).

Conformance: flips `typeddicts_readonly_inheritance.py`. Benchmark fixture:
`benchmarks/fixtures/typeddict_readonly_inheritance.py`.

## Architecture {#CHKARCH-ARCH}

### High-Level Pipeline {#CHKARCH-ARCH-PIPELINE}

```
Source Files (.py)
       |
       v
+------------------+     +-----------+
| basilisk-parser  | <-- | ruff_python_parser (MIT crate, evaluate)
+------------------+
       |  AST
       v
+------------------+
| basilisk-resolver|  Name resolution, scope analysis, imports
+------------------+
       |  Symbol Table + Resolved AST
       v
+------------------+
| basilisk-checker |  Type checking, inference, narrowing, PEP conformance
+------------------+
       |  Typed AST + Diagnostics
       v
+------------------+     +------------------+
| basilisk-lsp     |     | basilisk-cli     |
| (IDE server)     |     | (CI/terminal)    |
+------------------+     +------------------+
       |                         |
       v                         v
  VS Code / Cursor /      Terminal / CI
  Windsurf / Zed /
  Neovim

  (planned: basilisk-mojo — Mojo-inspired ownership / immutability / coercion
   analysis — is not yet wired into the pipeline.)
```

All stages are backed by:
```
+------------------+
| basilisk-db      |  Salsa incremental computation database
+------------------+
```

### Parse Nesting-Depth Guard {#CHKARCH-ARCH-PARSEDEPTH}

`ruff_python_parser` is recursive-descent, and the resolver and checker walk the
AST recursively (as does its `Drop`). All three overflow the thread stack on
pathologically nested input: a bracket expression past ~4 000 levels aborts with
`SIGABRT`, which produced an LSP crash-restart loop. To stay crash-safe — and match
CPython, which rejects this at the *tokenizer* — `parse_source` (the single entry
point into `ruff_python_parser`) runs a nesting-depth guard **before** parsing:

- Measures depth with ruff's **linear lexer** (`lex` + `next_token`), a flat byte
  scan that never recurses and short-circuits at the first violating token.
- Rejects **bracket nesting** (`(`, `[`, `{`, cumulative) deeper than **200**,
  matching CPython's `MAXLEVEL`; message is CPython's verbatim `too many nested parentheses`.
- Rejects **indentation** deeper than **99 levels**, matching CPython's `MAXINDENT`;
  message is verbatim `too many levels of indentation`.
- Rejects **operator chains** longer than **50 000** depth-building tokens in one
  uninterrupted expression context (per bracket level; reset at `,` `;` `=` and
  logical newlines); message `expression too deeply nested`. A flat token stream
  can still build an arbitrarily deep AST — `total = 1 + 1 + …` in generated code
  nests one `BinOp` per term with zero bracket nesting, and the recursive visitors
  abort even the 64 MiB analysis stacks of [LSPARCH-ARCH-STACK] at ~150 000 levels
  (GitHub #278, the LSP crash-restart loop). Counted tokens are the ones that
  deepen the tree (binary/unary operators, `.`, ternary `if`/`else`, `lambda`);
  flat-by-construction operators (`and`/`or` → one `BoolOp` list, chained
  comparisons → one `Compare` list, `,` → one tuple/call node) are deliberately
  exempt so giant flat generated literals stay analysable.

All limits sit well below their overflow floors and above any real source
(~15 brackets / ~10 indents / chains measured safe past 100 000 on the analysis
stacks), so the guard is crash-proof without false positives.
Rejection surfaces as `ParseError::Syntax` (`BSK-PARSE` in the LSP).

Implemented in `crates/basilisk-parser/src/depth.rs` and `…/src/lib.rs`
(`parse_source`); boundary tests in `crates/basilisk-parser/tests/parse_tests.rs`,
the propagation test in `crates/basilisk-checker/tests/checker_tests.rs`, and the
real-binary crash-safety tests in
`crates/basilisk-cli/tests/e2e_deep_expressions.rs`.

### Rust Crate Structure {#CHKARCH-ARCH-CRATES}

```
basilisk/
  crates/
    basilisk-parser/       # Python AST parsing (wraps or extends ruff_python_parser)
    basilisk-resolver/     # Name resolution, scope analysis, import resolution
    basilisk-checker/      # Core type checking engine
    basilisk-mojo/         # Mojo-inspired safety analysis passes
    basilisk-lsp/          # Language Server Protocol implementation
    basilisk-cli/          # Command-line interface
    basilisk-stubs/        # Stub generation, loading, registry client
    basilisk-db/           # Salsa incremental computation database
    basilisk-safety/       # Python package: Borrowed, Owned, InOut annotations
  editors/
    vscode/                # VS Code extension (VSIX)
    neovim/                # Neovim configuration / plugin
    helix/                 # Helix language config
```

### Crate Dependencies {#CHKARCH-ARCH-DEPGRAPH}

```
basilisk-db (foundation)
  <- basilisk-parser
       <- basilisk-resolver
            <- basilisk-checker
                 <- basilisk-mojo
                      <- basilisk-lsp (leaf: IDE)
                      <- basilisk-cli (leaf: terminal)

basilisk-stubs (standalone, used by basilisk-resolver)
```

### Build System {#CHKARCH-ARCH-BUILD}

- **Cargo workspace** with all crates
- Cross-compilation targets: `x86_64-linux`, `aarch64-linux`, `x86_64-darwin`, `aarch64-darwin`, `x86_64-windows`
- CI: `cargo clippy`, `cargo test`, conformance suite, benchmarks, fuzzing (nightly)
- Release: pre-compiled binaries for all platforms (no build dependencies for users)

#### Shared build-info emitter {#CHKARCH-ARCH-BUILD-VERSIONINFO}

Every binary crate exposing a Shipwright `--version` payload (`basilisk-cli`,
`basilisk-profiler-helper`) stamps the same `SHIPWRIGHT_*` env vars (git SHA,
`SHIPWRIGHT_GIT_DIRTY`, build time, target, toolchain) at compile time. The logic
lives once in `basilisk-buildinfo` (`emit_version_env`), so each `build.rs` is a
one-line delegation. `SHIPWRIGHT_BUILD_TIME` uses the same RFC 3339 formatter the
profiler uses for sample timestamps —
`basilisk_common::datetime::rfc3339_from_secs` — so the Howard Hinnant
`civil_from_days` algorithm exists in exactly one place.

---

## Incremental Computation {#CHKARCH-INCREMENTAL}

### Salsa Architecture {#CHKARCH-INCREMENTAL-SALSA}

Basilisk uses the [Salsa](https://crates.io/crates/salsa) incremental computation
framework (the same system powering rust-analyzer) for **in-session** incremental
checking.

**There is nothing here to configure, and that is deliberate.** This layer has
no `[tool.basilisk]` key and no switch: there is no alternative code path to
select, so a toggle could only ever be a no-op. It is not the same thing as the
opt-in persistent result cache, which *is* configuration
([CHKCACHE-CONFIG](CHECKER-CACHE-SPEC.md#CHKCACHE-CONFIG)); the two layers are
contrasted in
[CHKCACHE-CONFIG-SALSA](CHECKER-CACHE-SPEC.md#CHKCACHE-CONFIG-SALSA). Because a
silent always-on layer is indistinguishable from a missing one, the
configuration editor reports it read-only — engine, always-on state, and the
live memoized-file count — beside the keys that *are* editable
([LSPCFGED-CACHE](LSP-CONFIGURATION-EDITOR-SPEC.md#LSPCFGED-CACHE)).

- **Input queries**: a file's source text — `SourceFile::text`
  (`crates/basilisk-db/src/db.rs`) — and the effective configuration —
  `ConfigInput::value`, a `ConfigValue(BasiliskConfig)`
  (`crates/basilisk-checker/src/incremental.rs`). The database
  (`BasiliskDatabase`) and the shared `Db` trait live in `basilisk-db`, the
  dependency-graph foundation, so the derived queries are defined in the crates
  that own the work. `ConfigInput` lives in `basilisk-checker` (beside its only
  consumer) so the salsa `Update` wrapper never reaches the salsa-free
  `basilisk-config` leaf crate. The **resolution environment** is likewise a
  tracked input — `SearchPathsInput::value`, an `ImportSearchPaths` (workspace
  roots, `extraPaths`, stub dirs, venv site-packages, and the `uv.lock`-derived
  `PackageRegistry`); `ImportSearchPaths` lives in `basilisk-checker` and derives
  `salsa::Update` directly (its `Arc<PackageRegistry>` compares by value via
  `PartialEq`, so no salsa dependency reaches `basilisk-uv`).
- **Derived queries**: the per-file diagnostics
  (`crates/basilisk-checker/src/incremental.rs`). `checked_file` runs `parse →
  resolve → check_with_config`, keyed on `(file, config)` — the **pure**,
  import-free pipeline. `checked_file_resolved` additionally runs
  `resolve_module_imports` between resolve and check, keyed on `(file, config,
  search_paths)` — the **full** pipeline the batch CLI runs. Two further
  queries carry the cross-module view: `module_exports(file)` derives a
  workspace file's exported symbols from its tracked text (its `PartialEq`
  value enables **backdating** — a body-only edit re-derives an equal export
  set and every importer's memo stays valid), and `cross_resolved_module` /
  `checked_file_cross` layer `imported_symbols` population
  (`crates/basilisk-checker/src/exports.rs`) over `resolved_module`, resolving
  workspace-tracked imports through `module_exports` and external `.pyi` /
  PEP 561 `py.typed` sources from disk. Granularity is **module-level**: each
  pipeline is fused into one tracked query per file. Editing one file — or the
  configuration, or the search paths — re-executes only the affected queries;
  unrelated files are served from their memos.

The value type is the owned `CachedDiagnostic` (it satisfies salsa's `Update`
bound), so the engine adds **no** salsa dependency to `basilisk-resolver` or
`basilisk-stubs`.

**Equivalence guarantee.** Each query is a pure memoization wrapper over the
[check pipeline](#CHKARCH-ARCH-PIPELINE). For any file that parses and resolves,
`file_diagnostics(db, file, config)` equals `check_with_config(&resolved, cfg)`
byte-for-byte (and, with the default config, `check(&resolved)`);
`file_diagnostics_resolved(db, file, config, search_paths, workspace)` equals
`{ resolve_module_imports; check_with_config }` byte-for-byte — i.e. the batch
CLI's `process_file` core — **when `workspace` (the `WorkspaceFiles` registry) is
empty or every tracked file's `SourceFile` matches disk.** With a non-empty
registry the user-stub re-capture intentionally reads a tracked `.pyi`'s
in-memory text instead of disk, so it *diverges* from `process_file` for an
edited-but-unsaved stub (correct editor behaviour, but no longer byte-identical
to disk). Both equalities are asserted directly with an empty registry
(`crates/basilisk-checker/tests/incremental_tests.rs`
`checked_file_is_equivalent_to_direct_check` +
`checked_file_honours_explicit_rule_severity`;
`incremental_resolved_tests.rs`
`resolved_query_equivalent_to_direct_import_pipeline` +
`resolved_query_applies_import_resolution`), so salsa memoization can never
corrupt a result.

**Cross-file invalidation + filesystem-impurity boundary.** `resolved_module`
takes a `WorkspaceFiles` input (a path → `SourceFile` map) and, after
resolution, records a **content edge on exactly the imports whose output
depends on content**: workspace-tracked **user-stub `.pyi`** imports, whose
member API is re-derived from the tracked text
(`recapture_user_stub_from_source`) — so editing the stub's content updates
the importer's `imports_module_attribute` diagnostics, an edge that changes
*output*, not just triggers a re-run
(`editing_a_user_stub_updates_the_importer_diagnostics` at the checker level;
`editing_open_stub_refreshes_importer_via_salsa` proves it end-to-end through
the LSP with the disk left stale). A non-stub import records **no** text edge
— the importer's resolved module is identical for any content of the imported
file (`editing_a_non_stub_imported_file_does_not_reparse_the_importer`), and a
coarse text edge would re-parse every importer on any dependency keystroke.
Sibling `.py` **type/symbol** sharing instead flows through
`cross_resolved_module`: workspace imports depend on the imported file's
`module_exports`, so an export edit updates the importers' diagnostics from
tracked (possibly unsaved) content while a body-only edit backdates and
re-checks nothing
(`body_edit_backdates_exports_and_export_edit_propagates`,
`crates/basilisk-checker/tests/incremental_cross_tests.rs`). What remains
**untracked** (mirroring
[CHKCACHE-LIMITS](CHECKER-CACHE-SPEC.md#CHKCACHE-LIMITS)):
`resolve_module_imports`' existence probes and the content of files *outside*
the workspace (third-party packages, venv site-packages, external stubs) —
those invalidate only on a re-set `SearchPathsInput` / `WorkspaceFiles`.

**Input writes compare-before-set.** Salsa 0.27 treats *every* input `set` as
a new revision — a same-value write still re-executes dependents (pinned by
`crates/basilisk-checker/tests/salsa_set_semantics.rs`). The LSP engine
therefore compares each input (source text, config, search paths) against the
stored value and writes only on a real change; without the guard, syncing
inputs on every analysis would silently discard the database's memos and turn
every workspace sweep into a full recompute.

**LSP adoption.** The engine is the LSP's analysis path once the workspace scan
has built the search paths. `basilisk-lsp`'s `SalsaAnalysisEngine`
(`salsa_engine.rs`) holds a persistent [`BasiliskDatabase`] plus the input
handles (one `SourceFile` per file, one `ConfigInput` per root, one
`SearchPathsInput`, one `WorkspaceFiles` registry), sets them to the current
values on each analysis, and reads the resolved-module query (for navigation —
hover / references / go-to-definition) and its diagnostics projection; in
`crossModule` mode these are the cross-module variants (`cross_resolved_module`
/ `file_diagnostics_cross`), elsewhere the plain CLI-parity pair.
`WorkspaceIndex::analyse_and_resolve` routes the `didOpen`/`didChange` path
through it, and `WorkspaceIndex::reresolve_imports_and_recheck` — the
post-scan re-check, the config/`uv.lock` refresh, and the dependent refresh
when an edited file's exports change — is a **salsa sweep**: it primes the
engine with every indexed file's current text (open buffers included) and
re-analyses each file through the memoized queries, so only files whose
dependencies actually changed recompute. The pre-scan import-free path is
unchanged. `ResolvedModule` (and its transitively-contained types) derive
`PartialEq` so `Arc<ResolvedModule>` satisfies salsa's `Update` bound via the
fallback, keeping `basilisk-resolver` salsa-free.

**What the engine replaced, and what remains outside it.** The former LSP-side
cross-module machinery is gone: `cross_module.rs` (two-pass
`populate_cross_module_symbols`) and `resolve_workspace_imports` were retired
in favour of the queries above, and `import_graph.rs` is reduced to the
navigation handlers' reverse lookups ([ANALYSIS-GRAPH]) — invalidation no
longer walks the graph. The startup scan itself analyses through the engine:
search paths are built first, the engine is primed with every collected
file's text, and each file runs the memoized queries exactly once
([ANALYSIS-STARTUP-WHOLE]) — there is no separate pre-salsa analysis pass.
Still outside salsa: the `FileEntry` index itself (the LSP-side store —
`FileEntry.resolved` shares the salsa memo's `Arc<ResolvedModule>`, no
duplication) and the no-search-paths degrade path (`recheck_all_files`, plus
the per-file import-free fallback used before configuration is known).
Engine `SourceFile`/registry bookkeeping is dropped on file deletion
(`SalsaAnalysisEngine::remove`), though salsa 0.27 cannot reclaim an input's
internal memo, so a deleted file's memo lingers until the database is dropped;
the database's memory footprint scales with the workspace (every analysed
file's inputs and memos stay resident for the session — the standard
incremental-engine trade).

**Scope — the CLI/conformance path is deliberately unchanged.** The batch CLI
(`process_file`) still runs the direct pipeline, so this work **cannot affect the
conformance score**. Routing the CLI (and the LSP's bulk scan) through the engine
is future work — the CLI is the conformance path (must prove byte-for-byte parity
first) and, being one-shot, reuses no memos. The engine is a public API
(`basilisk_checker::{BasiliskDatabase, SourceFile, ConfigInput, ConfigValue,
SearchPathsInput, WorkspaceFiles, ModuleExports, checked_file,
file_diagnostics, resolved_module, module_exports, cross_resolved_module,
checked_file_resolved, checked_file_cross, file_diagnostics_resolved,
file_diagnostics_cross}`).

Incremental behaviour is proven by `crates/basilisk-db/tests/db_tests.rs`
(memoization, invalidation, cross-file isolation) and the checker tests above,
plus `crates/basilisk-checker/tests/incremental_cross_tests.rs` (cross-module
population semantics, PEP 561 gating, backdating, in-memory export
propagation).

### Cancellation {#CHKARCH-INCREMENTAL-CANCEL}

When a new keystroke arrives while a check is in progress, the in-flight
computation must be abandoned rather than run to completion and waste work — this
is what keeps an editor responsive under fast typing. Salsa provides this: a write
raises the revision's cancellation flag, and the next query checkpoint unwinds
with the `Cancelled` sentinel. Verified deterministically by
`crates/basilisk-db/tests/db_tests.rs::cancellation_unwinds_in_flight_work`.

### Persistent Cache {#CHKARCH-INCREMENTAL-CACHE}

Cross-session persistence is the **content-addressed result cache**
([CHKCACHE](CHECKER-CACHE-SPEC.md), `crates/basilisk-db/src/cache.rs`), not salsa:
a fresh process loads cached diagnostics and recomputes only files whose recorded
read-set changed on disk, eliminating cold-start cost. The two layers are
complementary — salsa makes an *editing session* incremental; the result cache
makes *repeat invocations* incremental — and a hit in either is sound by
construction (salsa via tracked dependencies, the result cache by re-verifying
every recorded file).

## Language server and editors {#CHKARCH-LSP}

The shared protocol, commands, configuration, binary resolution, and editor
boundaries live in [LSPARCH](LSP-ARCHITECTURE-SPEC.md#LSPARCH). Editor-specific
contracts live in [VSIX](VSIX-SPEC.md#VSIX), [NVIM](NEOVIM-SPEC.md#NVIM), and
[ZED](ZED-SPEC.md#ZED). This checker spec does not duplicate those inventories.

Compatibility anchors: supported methods {#CHKARCH-LSP-METHODS}; custom commands
{#CHKARCH-LSP-COMMANDS}; editor integrations {#CHKARCH-EDITORS}.

## Command-line interface {#CHKARCH-CLI}

### Commands {#CHKARCH-CLI-COMMANDS}

- `basilisk check [paths]` / `basilisk analyze [paths]` — the two scopes of the
  partition ([CHKARCH-COMMANDS](#CHKARCH-COMMANDS)), identical pipeline
- `basilisk format [paths] [--check]` — the embedded Ruff formatter
  ([LSPFMT-CLIENTS](LSP-FORMATTING-SPEC.md#LSPFMT-CLIENTS))
- `basilisk fix [paths] [--unsafe] [--rules ...]`
- `basilisk adopt|unadopt [paths]`
- `basilisk lsp [--transport stdio|ws]`
- `basilisk stubs generate|status` (plus the Pyright-compat `createstub` spelling)

### Output {#CHKARCH-CLI-OUTPUT}

`check` and `analyze` support human-readable text and structured JSON. Other
formats are not part of the current contract.

#### Scope notice {#CHKARCH-CLI-SCOPE-NOTICE}

`check` drops every analyze-scope diagnostic at the edge
([CHKARCH-COMMANDS](#CHKARCH-COMMANDS)), so on a project that grades non-`pep`
rules it can report "All checked. No issues found." while none of those rules
were ever evaluated. A silent clean run is indistinguishable from a clean
project — the same class of failure as a skipped CI gate reporting success.

A `check` run in text format therefore closes with one line naming how many
rules the configuration selected that this command never runs, and pointing at
`basilisk analyze`. The notice is a fact about the project, not boilerplate: a
tree whose configuration selects no analyze-scope rule prints nothing extra,
and `analyze` — which just ran them — never prints it. Exit codes
([CHKARCH-CLI-EXITCODES](#CHKARCH-CLI-EXITCODES)) and the JSON contract are
unchanged; machine consumers see no new field.

#### Unanalysable files {#CHKARCH-CLI-OUTPUT-FAILURES}

A file the run could not analyse at all — a syntax error, an unreadable path —
appears in the JSON array alongside the diagnostics, with `code` set to `null`
because no rule produced it and a `severity` of `"error"`. It anchors at line 1,
column 1: the failure is about the file as a whole, and the parser's own message
carries whatever position it knows.

Omitting it rendered `[]` — byte-for-byte the answer a clean file gets — for a
file that was never checked, so every consumer reading the report rather than
the exit code was told a file with a syntax error had no problems. The exit code
([CHKARCH-CLI-EXITCODES](#CHKARCH-CLI-EXITCODES)) still distinguishes the two,
but a report that contradicts it is worse than one that says nothing.

Consumers must therefore treat `code` as nullable, and must not require it to
recognise an entry.

### Exit codes {#CHKARCH-CLI-EXITCODES}

| Code | Meaning |
|---|---|
| 0 | Completed without error diagnostics |
| 1 | Error diagnostics were found |
| 2 | Invalid configuration (e.g. a `pep` rule resolved to `disabled`) |
| 3 | Internal failure |

### CI use {#CHKARCH-CLI-CI}

CI invokes the same `check` path as local use. Machine consumers should select
JSON and branch on the documented exit code; no CI-only analysis mode exists.

## Stub System {#CHKARCH-STUBS}

### Auto-stub generation {#CHKARCH-STUBS-AUTOGEN}

`basilisk stubs generate` supports runtime, AST, and hybrid discovery. Runtime mode imports
the target package; AST mode does not execute it; hybrid mode combines both. Generated files
are written under `.basilisk/stubs/`, at the head of the stub search path. The command and
provenance contracts live in [CHECKER-STUB-RESOLUTION-SPEC.md](CHECKER-STUB-RESOLUTION-SPEC.md).

### Stub quality tiers {#CHKARCH-STUBS-TIERS}

`StubTier` distinguishes trusted hand-written stubs, reviewed/generated stubs, and
best-effort inference. The generated model and diagnostic behavior are canonical in the
stub-resolution spec and `basilisk-stubs`.

### typeshed compatibility {#CHKARCH-STUBS-TYPESHED}

Pinned typing step 3 says a configured custom typeshed is the "canonical source"
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
Accordingly, `typeshed-path` is the sole step-3 tree when set. Otherwise Basilisk
resolves the pinned `typeshed-commit` (unset = the bundled commit) from the
local store or the bundled stdlib ZIP — always offline, never a download.
Step-3 sources never mix and a missing pin never substitutes another source.
`typeshed-store-path` only relocates the local store, while `stub-paths`
remains the separate step-1 override
([STUBRES-TYPESHED](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED)).

---

## Plugin host (planned) {#CHKARCH-PLUGINS}

No plugin crate ships today. A WASM host remains future work rather than a placeholder runtime.

### Sandbox target {#CHKARCH-PLUGINS-ARCH}

The planned host runs WASM without ambient filesystem or network access.

### Extension target {#CHKARCH-PLUGINS-EXTENSIONS}

The first planned extension point is third-party diagnostics over explicit AST/type inputs.

### Distribution target {#CHKARCH-PLUGINS-DIST}

Configuration, package format, and compatibility policy are unresolved and remain tracked in
the advanced-features plan.

---

## Configuration {#CHKARCH-CONFIG}

### Configuration Model {#CHKARCH-CONFIG-MODEL}

The design source is [`models/configuration.td`](../../models/configuration.td).
A configuration is two flat maps and nothing else:

- `[tool.basilisk.rules]` — explicit per-rule entries:
  `"<code>" = "error" | "warning" | "info" | "disabled"`.
- `[tool.basilisk.rule-tags]` — explicit group entries:
  `"<tag>" = "<severity>"` — one written line that grades every rule carrying
  the tag (e.g. `"basilisk" = "error"` turns every house rule on). A tag entry
  is config in the file, never an implicit switch.

**Resolution** — per rule, per checked file, one walk, first decision wins:

1. Walk from the file's folder to the root. The **nearest** `[tool.basilisk]`
   table that decides the rule wins outright.
2. Within a table, a per-rule entry beats tag entries; among matching tag
   entries the **strictest** severity wins
   (`error` > `warning` > `info` > `disabled`).
3. No table decides the rule: `pep`-tagged rules run at `error`
   ([CHKARCH-COMMANDS](#CHKARCH-COMMANDS)); every other rule is disabled.

That is the whole model. There are no default severities beyond the check
scope's `error`, no inherited state, no glob path patterns, no per-file or
per-module exceptions, no precedence scores, and no merge intents. Scoping a
rule differently for part of the tree means putting a config file in that
folder. A missing table and an empty table behave identically — PEP rules at
`error`, nothing else runs; the only thing that distinguishes them is the
LSP's one-time seed
([LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING)).

`disabled` never applies to a `pep`-tagged rule: any configuration that
resolves a PEP rule to `disabled` — by rule entry or tag entry — is invalid
and fails config loading. Line-level `# type: ignore` and `exclude` remain the
escape hatches ([CHKARCH-STRICTNESS-SUPPRESSION](#CHKARCH-STRICTNESS-SUPPRESSION),
[CHKARCH-CONFIG-EXCLUDE](#CHKARCH-CONFIG-EXCLUDE)).

Rule codes carry no severity class (`BSK-nnnn`, or a conformance snake_case
name — [CHKARCH-DIAG-CODES](#CHKARCH-DIAG-CODES)); only entries carry severity.

### Configuration File {#CHKARCH-CONFIG-FILE}

`pyproject.toml` under `[tool.basilisk]` is the **single** Basilisk-native
configuration source and the seeding target for new projects
([LSPARCH-CONFIG-SEEDING](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING)).
There is no other Basilisk config format: a legacy root-level `basilisk.json`
is **never read or written**
([CONFIGEDITOR-SOURCES](LSP-CONFIGURATION-EDITOR-SPEC.md#CONFIGEDITOR-SOURCES)).
For drop-in migration, the **analysis-environment tier only** (import
resolution, stub search, typeshed activation — never rule severities) also
accepts pyright-compatible spellings: a root `pyrightconfig.json`, a
`[tool.pyright]` fallback table, and camelCase key aliases (`pythonVersion`,
`extraPaths`, `typeshedPath`). Note that pyright's stub-directory key is the
**singular** `stubPath` holding one path string, not a plural array; both it
and Basilisk's own `stub-paths`/`stubPaths` list are accepted, with the list
winning when both appear. Priority is specified in
[ANALYSIS-CONFIG-PRI](LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CONFIG-PRI).
How the file is found and how multiple ancestor tables combine is specified in
[Configuration Discovery](#CHKARCH-CONFIG-DISCOVERY).

Project TOML example:

```toml
[tool.basilisk]
python-platform = "All"          # Explicit cross-platform analysis
stub-paths = ["stubs/"]          # resolution step 1: prepend extra .pyi stub dirs
# An unset pin IS the bundled commit; `basilisk typeshed download` updates it:
# typeshed-commit = "<full commit SHA>"  # optional explicit immutable source
# typeshed-store-path = ".cache/typeshed"  # optional: relocate the local store
# typeshed-path = "typeshed-x"   # resolution step 3: your sole custom stdlib tree
include = ["src/", "tests/"]
exclude = ["**/migrations/**"]
cache = true                     # reuse check results between runs ([CHKCACHE-CONFIG])
# cache-dir = "build/bsk-cache"  # optional: relocate the persistent result cache

[tool.basilisk.rules]
"imports_unresolved" = "warning"    # a PEP rule graded down — never disabled
"BSK-0050" = "error"               # one house rule promoted above its tag entry

[tool.basilisk.rule-tags]
"basilisk" = "error"                # every house rule on — strict by default
```

Scoping a rule differently for part of the tree means placing another
`pyproject.toml` with a `[tool.basilisk]` table in that folder; the nearest
deciding table wins per rule ([CHKARCH-CONFIG-MODEL](#CHKARCH-CONFIG-MODEL)).

Every key here is read by a real consumer. A setting no code reads does not
belong in this file: it reads as a knob the author turned, while the checker
never sees it (the retired `auto-stub-*` keys are pinned gone by test). The
converse also holds and is worth stating, because caching is where readers
most often assume a missing key: the **in-session Salsa memo layer**
([CHKARCH-INCREMENTAL-SALSA](#CHKARCH-INCREMENTAL-SALSA)) is always on and has
**no key at all**, while `cache`/`cache-dir` govern only the *persistent,
cross-session* result cache
([CHKCACHE-CONFIG-SALSA](CHECKER-CACHE-SPEC.md#CHKCACHE-CONFIG-SALSA)). The
configuration editor states both layers side by side for exactly this reason
([LSPCFGED-CACHE](LSP-CONFIGURATION-EDITOR-SPEC.md#LSPCFGED-CACHE)).

### Configuration Discovery {#CHKARCH-CONFIG-DISCOVERY}

Rule configuration is resolved **per checked file** through one shared routine
(`basilisk_config::load_basilisk_config`), used identically by `basilisk check`,
`basilisk fix`, `basilisk adopt`, and the LSP (GitHub #311). The result is
independent of argument order, path spelling, and cwd — for the same file in
the same project, every surface resolves the identical **rule** configuration.
(Which keys are per-file versus per-project is scoped under **Two tiers**
below.)

**Walk.** Starting from the file's own directory, every ancestor directory up
to the filesystem root is visited. Each directory contributes at most its
`pyproject.toml` `[tool.basilisk]` table. A `pyproject.toml` **without**
`[tool.basilisk]` contributes nothing and does not stop the walk (Ruff's
`[tool.ruff]` semantics).

**Rule resolution.** Rules are never merged: the nearest table that decides a
rule — per-rule entry first, then tag entries — wins outright
([CHKARCH-CONFIG-MODEL](#CHKARCH-CONFIG-MODEL)).

**Scalar merge.** Non-rule fields merge additively, nearest directory winning
per key: `stub-paths` appends (deduplicated); remaining scalar/list fields
keep the ancestor's value unless the child explicitly sets one; the nearest
config's directory becomes the merged config's `project_root`, anchoring
`include`/`exclude` globs.

**Two tiers.** The per-file ancestor walk above governs the **rule tier**:
rule severities (`rules`, `rule-tags`) and `python-version`/`python-platform`
as consumed by version-gated rules. `include`/`exclude` live in the same
`[tool.basilisk]` tables but are **discovery-time** keys: each invocation
resolves them once — from the first checked path's ancestor chain on the
CLI, per workspace root in the LSP — because they decide *which files are
collected* before any per-file rule resolution exists
([CHKARCH-CONFIG-EXCLUDE](#CHKARCH-CONFIG-EXCLUDE)). The
**analysis-environment tier** — `extra-paths`, `stub-paths` as import search
roots, every `typeshed-*` key, the `cache`/`cache-dir` keys
([CHKCACHE-CONFIG](CHECKER-CACHE-SPEC.md#CHKCACHE-CONFIG): one project, one
cache, so the ROOT config decides), and `python-version` as the stub-resolution
target — is instead resolved **once per project root** by the workspace
loader (which also honors the pyright-compatible sources,
[CHKARCH-CONFIG-FILE](#CHKARCH-CONFIG-FILE)) and applied uniformly to the
whole workspace by `basilisk check`, the MCP server, and LSP initialization
([ANALYSIS-CONFIG-PRI](LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CONFIG-PRI)).
A nested `[tool.basilisk]` table therefore cannot re-point the import
environment for a subtree — only the root decides it. One deliberate
exception: the LSP configuration editor and its config-file watcher read the
typeshed keys back through the ancestor-walk loader, so editor changes
round-trip through the same file the editor writes
([CONFIGEDITOR-SOURCES](LSP-CONFIGURATION-EDITOR-SPEC.md#CONFIGEDITOR-SOURCES)).

**Surfaces.**

- **CLI `check`/`fix`**: each collected file is checked with the config
  discovered from its own directory (memoized per directory —
  `resolve_dir_configs`). The first path argument only anchors project-level
  concerns (include expansion, version detection, cache location); the check
  cache fingerprints every directory's config so a child config edit
  invalidates cached results.
- **CLI `adopt`/`unadopt`**: the config root anchors at the nearest ancestor
  directory holding a config table (`basilisk_config::discover_config_dir`),
  so `adopt` writes exactly where `check` discovers.
- **LSP**: per-file config is the owning workspace root's **in-memory** config
  merged with the ancestor chain discovered strictly *below* that root
  (`WorkspaceIndex::config_for_file` / `load_basilisk_config_below`, memoized
  per directory; the shared refresh tail the server runs on every
  configuration change invalidates the memo). The root's own config file is
  never re-read from disk here: the in-memory root config already reflects it
  — or, authoritatively, an applied editor-UI change or an open, unsaved
  config buffer ([LSPARCH-CONFIG](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG)).
  A workspace folder opened *inside* a project discovers the project's config
  the same way (its root config is loaded through the full ancestor walk at
  index build/reload).

### Include Semantics {#CHKARCH-CONFIG-INCLUDE}

`include` lists the roots scanned when no CLI paths are given. Explicit CLI paths
override it; `exclude` applies within the include roots. When `include` is absent
or empty, the current directory is scanned. Entries resolve relative to the config
file's directory (issue #37).

The **LSP** honors the same `include` roots on both paths, so the editor analyses
exactly the files `basilisk check` would. The bulk scan walks only the include
roots (`WorkspaceIndex::scan_dirs_for`); the per-file/open path suppresses
diagnostics for files outside them (`WorkspaceIndex::is_outside_include_roots`, in
`analyse_and_resolve` and `recheck_all_files`) — so a generated-tree file shows no
diagnostics even when opened, like an `exclude`d file.

### Exclude Semantics {#CHKARCH-CONFIG-EXCLUDE}

`exclude` uses **gitignore-style globs**, matched against the path relative to
the workspace root:

- a bare name with no `/` matches that segment at **any** depth — `build` excludes
  every `build` dir, `*.pb.py` every generated file;
- `**` matches zero or more directory segments (`**/bundled/**`); `*` / `?` match
  within a single segment only;
- an anchored pattern (containing `/`) matches the full path or any ancestor
  directory, so `vendor/**` or `src/generated` also excludes everything beneath it.

When `exclude` is **unset**, a default set of vendored/cache directories is
excluded (`basilisk_config::DEFAULT_EXCLUDES`: `node_modules`, `site-packages`,
`.venv`, `__pycache__`, `build`, `dist`, the extension's `bundled` /
`_vendored` trees, and friends). Setting `exclude` **replaces** those defaults
entirely — re-add any default entries explicitly if they are still needed.
Two **structural** skips sit outside `exclude` entirely and no configuration can
switch them off, because both surfaces must agree on them file-for-file:

- hidden directories (`.`-prefixed); and
- **virtualenv roots**, identified by [PEP 405](https://peps.python.org/pep-0405/#specification)'s
  `pyvenv.cfg` marker (`basilisk_config::is_virtualenv_dir`) rather than by
  directory name, so `env/` and `.direnv/python-3.13/` are pruned exactly like
  `.venv/`. A venv holds installed third-party packages — never the user's code.
  Without this, a project that sets any custom `exclude` loses the
  `venv`/`site-packages` default entries and `basilisk fix` **rewrites installed
  packages** (GitHub #341). The skip prunes *traversal into* a venv; an explicit
  CLI path pointing inside one is still checked, matching the walk's depth-0 root
  exemption.

The single canonical matcher
`basilisk_config::path_matches_pattern` is shared by every entry point so they
exclude identically:

- LSP **workspace scan** (`workspace_scan::is_excluded`),
- CLI **walk** for `check`/`fix`/`adopt` (`is_excluded_path`), and
- LSP **incremental per-file path** (`WorkspaceIndex::is_path_excluded`, in
  `analyse_and_resolve`) — a vendored file *opened* or *edited* is parsed for
  navigation but publishes **no** diagnostics, matching the bulk scan.

### Migration from existing tools (planned) {#CHKARCH-CONFIG-MIGRATION}

No `basilisk migrate` command ships. A future importer must report every mapped and unmapped
setting and must not claim semantic equivalence between another checker's modes and Basilisk
rule configuration.

---

## Diagnostics Experience {#CHKARCH-DIAGEXP}

### Quality Standard {#CHKARCH-DIAGEXP-QUALITY}

Diagnostics follow the rustc format:

```
error[BSK-0001]: Missing parameter type annotation
  --> src/utils.py:14:5
   |
14 | def process(data):
   |             ^^^^ parameter `data` has no type annotation
   |
   = help: Add a type annotation: `data: <type>`
   = note: this project's configuration enables BSK-0001, which requires explicit parameter types
   = see: https://www.basilisk-python.dev/errors/BSK-0001
```

### Quick Fixes {#CHKARCH-DIAGEXP-QUICKFIXES}

Every error has at least one associated code action:

| Error | Quick Fix |
|---|---|
| BSK-0001 (missing param type) | Insert `: Any` (the rule only fires where no type is inferable, [TYPEINF-EXCEEDS-REQUIRED](CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-EXCEEDS-REQUIRED)) |
| BSK-0002 (missing return type) | Insert `-> Any` (same rationale) |
| enums_behaviors (mutation of immutable param) | Add `InOut` annotation |
| dataclasses_order (implicit coercion) | Wrap in explicit conversion |

---

## Performance Engineering {#CHKARCH-PERF}

### Parallelism {#CHKARCH-PERF-PARALLEL}

Analysis is single-threaded, by design and in fact. `check` / `analyze` / `fix` /
`adopt` all run on one dedicated large-stack thread (`run_with_analysis_stack`,
[LSPARCH-ARCH-STACK]) because the AST walk recurses deeply enough to overflow the
default main-thread stack. No Basilisk crate calls Rayon — `rayon` reaches the
lockfile only as a transitive dependency of `salsa` and `ruff_db`.

Concurrency lives in the LSP server instead, on Tokio: request multiplexing, plus
`spawn_blocking` for the genuinely blocking work (typeshed download, debug-adapter
accept loop, process enumeration). What keeps an edit sub-10ms is Salsa's
incremental invalidation ([CHKARCH-INCREMENTAL]), not thread count.

File-level parallelism stays a future option. It is not implemented, and no
benchmark number in this repository depends on it — so a change that adds it must
still clear the benchmark ratchet on its own merits
([CHKARCH-TESTING-BENCH-RATCHET]).

### Memory {#CHKARCH-PERF-MEMORY}

- Arena allocation for AST nodes
- Interned strings for identifiers and paths
- Memory-mapped file I/O

### Benchmarks {#CHKARCH-PERF-BENCHMARKS}

The suite that exists today is `benchmarks/` — single-construct typing-spec
stress fixtures timed cold across Basilisk, Pyright, mypy, ty, Pyrefly, and
zuban by `benchmarks/run.sh`. Each run does a full `cargo clean` + fresh
`--release` build of basilisk, pulls the LATEST official release of every
competitor, times all fixtures, and writes the measured numbers to the
per-machine status CSV **immediately and unconditionally** — the write is never
gated. A **separate** read-only regression gate then compares those numbers
against the committed baseline and fails CI on a slip beyond a small noise
tolerance. Full mechanism: [CHKARCH-TESTING-BENCH-RATCHET].

**Planned, not yet built:** a real-world-codebase suite — **PyTorch** (~600K
LOC), **Django** (~250K LOC), **FastAPI** (~30K LOC), **Python standard
library** (~500K LOC) — with the same comparison baselines. This paragraph is
a design target, not a claim of existing measurement.

---

## Testing Strategy {#CHKARCH-TESTING}

| Layer | Method | Purpose |
|---|---|---|
| Unit tests | `cargo test` per crate | Crate-level correctness |
| Integration tests | Multi-file scenarios | Cross-module type checking |
| Conformance tests | Python typing test suite | PEP compliance (target: 100%) |
| Golden file tests | Expected diagnostic output | Diagnostic regression |
| Fuzzing | `cargo-fuzz` | Crash resistance, soundness |
| Property tests | `proptest` crate | Type system invariants |
| Benchmarks | `make bench` (hyperfine, `benchmarks/run.sh`) vs Pyright/mypy/ty/Pyrefly/Zuban | Performance tracking (results written to `benchmarks/status/<machine>.csv` immediately, every run) + zero-tolerance regression gate that fails if basilisk gets slower than the **committed** baseline on any fixture ([CHKARCH-TESTING-BENCH-RATCHET]) |

### PEP Conformance Scoring {#CHKARCH-CONFORMANCE}

The conformance score is produced by **RUNNING the real `python/typing`
conformance harness** — the suite's own `conformance/src/main.py` driving its
built-in `BasiliskTypeChecker` — against the compiled binary on **every run**,
never a Basilisk reimplementation. It is the exact tooling the reference checkers
(pyright, mypy, pyrefly, ty, zuban, pycroscope) are graded with. **A build in which
that official check did not run against a freshly cloned suite is a BUILD FAILURE.**

**The mechanism — every CI run, in order, no step skippable or the build dies:**

1. **Freshly download** the tests **and** the harness/calculator from
   `python/typing@main`'s **latest** commit — `git clone --depth 1
   https://github.com/python/typing`. No cache, no committed fixtures, no vendored
   calculator. (So the moment upstream merges a new rule/fixture, the very next run
   grades against it — and if we regress, CI tanks.)
2. **Freshly build a CLEAN release** `basilisk` binary from THIS checkout's source
   — `cargo build --release`, un-instrumented, byte-for-byte what ships. Never the
   PyPI wheel (a prior version), never an instrumented build.
3. **Run the suite's OWN `conformance/src/main.py --only-run basilisk`** against
   that binary (pointed at it via `BASILISK_BIN`), and **fail HARD on ANY false
   positive or ANY missed required error** — the gate demands 100 % pass / 0 FP
   (`coverage-thresholds.json`). One stray diagnostic tanks the build.
4. **Regenerate `conformance/conformance_status.csv`** (and the website report)
   from the harness's OWN `results/basilisk/*.toml` — the committed scoreboard is
   always a product of the live run, never hand-authored.

> ⛔️ **DISABLING, DELETING, OR UNREGISTERING ANY CONFORMANCE RULE IS FORBIDDEN.**
> The binary is scored in its **full default configuration with EVERY core
> PEP/conformance rule enabled** — no Basilisk config (any format; the legacy
> `basilisk.json` is no longer read), no per-rule override, no "spec-conformance mode",
> no skipped fixtures, no deleting rule source (`src/rules/*.rs`), no removing rules
> from `all_rules()`. The binary is scored over a **fresh `python/typing` clone**
> whose tree holds no Basilisk config of any format, so nothing of ours can silence a rule;
> deleting the rules themselves is the **same crime by another route** and equally
> forbidden — as is hand-editing `conformance/conformance_status.csv` or loosening
> the `coverage-thresholds.json` gate (`threshold` / `max_false_positives`). A
> strict default firing on valid code is a **real conformance gap to FIX in the
> checker**, never to hide. Gaming the number is a punishable offence.

- **Runner — the real harness, nothing else**:
  [`conformance/run_conformance.py`](../../conformance/run_conformance.py) is the
  ONE conformance path. Every run it clones the
  **latest [`python/typing@main`](https://github.com/python/typing/tree/main/conformance)**
  FRESH (we shoot for the current spec suite, not a frozen commit) and runs the
  suite's **OWN unmodified `conformance/src/main.py --only-run basilisk`** against
  the real compiled binary (via `BASILISK_BIN`). The suite already ships the
  official Basilisk adapter — `BasiliskTypeChecker` in
  [`conformance/src/type_checker.py`](https://github.com/python/typing/blob/main/conformance/src/type_checker.py) —
  so **nothing of ours is injected, vendored, adapted, or reimplemented**. The
  harness writes `results/basilisk/*.toml`; every `Pass`/`Fail` verdict and every
  `errors_diff` is the harness's OWN, produced by the same code that grades pyright,
  mypy, pyrefly, ty, zuban and pycroscope. From those real results the runner only
  *reports*: it writes `conformance/conformance_status.csv` and **records the exact
  graded commit hash** in
  [`website/src/_data/conformance_report.json`](../../website/src/_data/conformance_report.json),
  so every published number is pinned *by hash* on the website. There is **NO
  vendored calculator and NO cached-fixtures fallback** — a build in which the real
  harness could not be cloned and run is a **BUILD FAILURE**, by design. (The only
  auxiliary number not in the toml, `caught` = required errors matched, is taken
  from upstream's own `get_expected_errors` imported live from the fresh clone — the
  official function on the official tests, never a copy.)
- **Pass rule** (upstream's, verbatim): a file passes iff the upstream
  `errors_diff` is empty — every `# E` line gets an error, every `# E[tag]`
  group is satisfied, and **no error lands on a line the suite does not mark**.
  `conformance_automated = "Fail" if errors_diff.strip() else "Pass"`.
- **Nothing excluded.** The harness counts **every** diagnostic — errors **and**
  warnings (strictest grading, as pyright is graded); no looser mode, no opt-out.
  The binary runs with **every core PEP/conformance rule enabled** in its default
  configuration over a fresh `python/typing` clone whose tree holds no
  Basilisk config of any format, so nothing of ours can silence a conformance rule. Opt-in
  Basilisk-specific rules remain off by the same ordinary default
  ([CHKARCH-CONFORMANCE-MODE](#CHKARCH-CONFORMANCE-MODE)).
- **Gate**: `make test` (via [`scripts/test-rust.sh`](../../scripts/test-rust.sh))
  builds the `basilisk` binary, then runs
  `python3 conformance/run_conformance.py --gate` on it — which runs the REAL
  harness and delegates the 100 %-pass / 0-false-positive check to
  [`conformance/assert_wheel_conformance.py`](../../conformance/assert_wheel_conformance.py)
  over the harness's OWN `results/basilisk/*.toml`. There is **no Rust conformance
  test** and **no in-repo scorer**: the score is the real suite's own verdict on the
  compiled binary. The pass-percentage floor and false-positive ceiling live in
  `coverage-thresholds.json` (`conformance.threshold`,
  `conformance.max_false_positives`); the former ratchets **up**, the latter
  **down**. Per-file results are written to `conformance/conformance_status.csv`.
- **Current score** — measured against `python/typing@main` at the exact graded
  commit recorded in `conformance_report.json`, currently
  [`<!--g:short-->60df123<!--/g:short-->`](https://github.com/python/typing/tree/60df123ccfe9ae0472b1409ef4a00d51ffc5d972/conformance):
  **<!--g:pass-->141<!--/g:pass--> / <!--g:total-->141<!--/g:total--> = <!--g:score-->100.0%<!--/g:score-->**, **<!--g:fp-->0<!--/g:fp--> false positives**, **<!--g:missed-->0<!--/g:missed--> missed required errors**, with
  **<!--g:caught-->970<!--/g:caught-->** required errors caught. The binary runs in its default configuration — the
  PEP conformance set — over a fresh `python/typing` clone whose tree holds no
  Basilisk config of any format, so nothing can silence a rule; Basilisk's opt-in house-style rules never run during scoring,
  so they can neither pad nor sink the number. The gate
  ratchets the pass-percentage **up** and the false-positive ceiling **down**
  (`coverage-thresholds.json` → `conformance.threshold` /
  `conformance.max_false_positives`), driven only by genuinely fixing the checker,
  **never** by disabling a rule. (History: a **baseline reset on 2026-06-26**
  corrected a gamed *fake 100%* that had disabled six house-style rules before
  scoring; conformance has been measured honestly in the default config ever since.)
  Target: **100%**.

#### No "spec-conformance mode" — the scorer runs the genuine default config {#CHKARCH-CONFORMANCE-MODE}

There is **no** conformance mode, and there never will be. The scorer runs the binary
in exactly the configuration a user gets out of the box — the **default config, which
is the pure PEP conformance set** ([CHKARCH-CONFIGURATION-ONLY](#CHKARCH-CONFIGURATION-ONLY))
— with no Basilisk config of any format (the legacy `basilisk.json` is no longer
read), no per-rule override, and no special scoring path. Basilisk's
opinionated *house-style* rules (require-annotations `BSK-0001`/`BSK-0002`/`BSK-0004`,
require-`@override` `BSK-0025`, redundant-annotation `BSK-0050`, the explicit-`Any`
nudge `BSK-0014`) are **opt-in and off by default**, so they never run during scoring
and can neither pad nor sink the number. The figure is the genuine out-of-the-box
conformance result — currently <!--g:score-->100.0%<!--/g:score-->. Any shortfall would be a real
checker bug to fix (a missing spec feature, or a false positive from an over-strict
*conformance* rule), never something to paper over by silencing a rule.

⛔️ **Disabling, deleting, or unregistering a conformance (PEP) rule to move the number
is forbidden** — as is hand-editing `conformance_status.csv` or loosening the
`coverage-thresholds.json` gate (`threshold` / `max_false_positives`) to match a faked
run. This has been attempted twice, back when the house rules still ran by default and
counted toward the score. First, a revision wrote a `basilisk.json` that turned six
rules off before scoring and reported a **fake 100%**; that was removed, and the
scorer now runs the binary over a **fresh `python/typing` clone** whose tree contains
no Basilisk config of any format (the legacy `basilisk.json` is no longer read at
all), so no config can silence a rule. Second — when config-disabling
was blocked — a revision tried to
*delete the offending rule source files outright* and unregister them from
`all_rules()`, then re-report a **fake 100%**: the same lie by another route. **Deleting
a rule to dodge the config guard is the identical offence.**

The path to 100% is to make the checker **correct**, never to silence a rule at score
time: implement the spec features it still misses, and teach its conformance rules to
stop firing on spec-valid code (recognising inferred return types, honouring `# E`-free
lines) so the false positives fall on their own merits. Anyone may relax rules *in their
own project* via config; the **conformance scorer never does**.

### Mutation Testing Ratchet {#CHKARCH-TESTING-MUTATION-RATCHET}

Mutation testing proves the test suite actually asserts behaviour. Scope only ever **grows** toward all Rust code:

- **Scope is test-driven.** `#[mutation_safe(rule = "<rule-slug>", fns = "fn_a|fn_b")]`
  attributes on e2e tests drive the `cargo mutants` examine regex
  (`scripts/mutation_examine_re.py`). `<rule-slug>` is the rule's path stem under
  `crates/basilisk-checker/src/rules/` (file like `aliases_implicit` or directory
  like `assignment_compatibility`); omitting `fns` scopes the whole file. Adding
  these tests is the only way to widen scope.
- **Baseline is ratcheted.** `mutation_testing/mutation_scores.json` is the committed
  baseline; `mutation_testing/mutants_report.py::regression_messages` fails the build
  when `kill_rate` drops below the baseline or the absolute floor, when `detected`
  (`caught` + `timeout`) drops **while the viable pool did not grow**, or when
  `timeout` rises. (`unviable` mutants don't compile and are excluded.) Absolute
  `missed` is deliberately *not* a signal: widening scope mutates more code, so a
  larger raw `missed` against a smaller-pool baseline is expected — `kill_rate` is
  the size-independent guard. Both `make mutation-test` and the CI shard merge
  enforce the same function.
- **A timeout may never rise.** A `timeout` is credited as a kill (the PIT/Stryker
  convention: a terminating suite made non-terminating *has* been detected). That
  credit is only honest while timeouts come from hung code rather than slowness —
  and the mutants that time out are structurally the likely *survivors*, since a
  killed mutant exits at the first failing test binary while an uncaught one runs
  the whole suite. So a rise in `timeout` is itself a build failure: it means
  mutants were credited as killed without being evaluated. Fix the budget or the
  suite's speed ([`.cargo/mutants.toml`](../../.cargo/mutants.toml)); never absorb it.
- **Direction.** End state is the full workspace under mutation
  (`make mutation-test ALL=1`); until then each checker-logic PR leaves the viable
  pool the same size or larger.

### Benchmark Non-Regression {#CHKARCH-TESTING-BENCH-RATCHET}

Performance and conformance ratchet **together** — neither traded for the other.
`make bench` (`benchmarks/run.sh`) runs the fixture suite and enforces the
performance gate. Two responsibilities are deliberately **DECOUPLED**, so one can
never suppress the other (`benchmarks/summarize.py`):

1. **WRITE — unconditional and immediate.** Every measured number is written
   straight to the per-machine status CSV `benchmarks/status/<machine>.csv` the
   instant it exists: `summarize.py` runs in `incremental` mode after **each**
   fixture (rewriting the CSV from all results so far) and again in `final` mode
   at the end. There is **no gate on the write, no branch, no "left unchanged"
   path** — the file ALWAYS reflects exactly what this build just measured. A run
   that measured a number but did not record it is a lie about the build's
   performance, and the whole point of the suite is to KNOW the moment a number
   slips. So the write happens regardless of what the gate later decides
   (atomic tmp + `os.replace`, so a kill mid-write never tears the file).

2. **GATE — read-only, CI pass/fail, separate judgment.** In `final` mode, AFTER
   the numbers are on disk, the run's basilisk times are compared against the
   **COMMITTED** baseline — the status CSV read from git at `BENCH_BASELINE_REF`
   (default `HEAD`) via `git show`, **never the working copy the run just
   overwrote**, so a slower run can never launder its regression into the
   baseline. Any backwards step on any fixture exits 3 →
   CI FAILURE. The gate only READS; it never edits the file. The committed
   baseline advances only when a run is committed, so it still ratchets toward
   faster — while the live file never hides a slip.

- **Fresh binary, every run.** `run.sh` ALWAYS does a full `cargo clean` + a
  from-scratch `cargo build --release --bin basilisk` before timing a single
  fixture. A number is only honest if it came from a from-scratch optimized build
  of the exact tree under test — never a stale or incrementally-linked binary. The
  `# generated` timestamp and the basilisk version recorded in the CSV header are
  captured after this build, so the header proves the numbers came from it.
- **Latest competitors, every run.** Before discovery/timing, `run.sh` upgrades
  each officially-recognized checker (pyright, mypy, ty, pyrefly, zuban — only
  those tracked by the `python/typing` conformance suite; never unofficial tools)
  to its newest official release via `pip install --upgrade` (best-effort per
  tool, loud warning on failure). Competitor columns therefore always reflect
  current upstream, never a pinned build. The pull runs outside all timing.
- **Zero-tolerance ratchet.** The committed tolerance is zero
  (`BENCH_TOLERANCE_PCT=0`), so every fixture must be monotonically
  non-increasing. It lives in the tracked script, not an env var; the gate itself
  cannot be disabled or widened at runtime (`BENCH_NO_GATE` /
  `BENCH_REGRESS_PCT` / `BENCH_TOLERANCE_PCT` overrides are rejected).
- Run it whenever checker hot paths change (resolver visitors, rule `check` loops,
  conformance-driven additions). Conformance logic that blows the gate must be
  optimised or restructured. A machine without a baseline establishes one only
  after a successful run is committed.

> **Planned — bench in the pipeline (CI).** Today `make bench` is run locally and
> its results are committed. The intention is to eventually run the benchmark gate
> in CI on a fixed runner class, on the same write-always / gate-separately
> discipline described here, so a performance regression fails the pipeline the way
> the conformance and coverage gates already do. Until that lands, the discipline
> is enforced by running `make bench` locally and committing the updated status CSV.

### CI Artifact Storage Policy {#GITHUB-NO-ARTIFACTS}

Basilisk is **public**: GitHub-hosted runner compute (all `ubuntu-24.04`) is free and unlimited; GitHub bills for **stored Actions artifacts** (GB-days). Therefore:

- **CI stores no artifacts.** No `actions/upload-artifact` for coverage HTML,
  mutation reports, logs, screenshots. Gates enforce in-job (coverage, mutation
  merge, benchmarks) and reports reproduce locally (`make test`, `make
  mutation-test`). Codecov consumes `lcov.info` directly without GitHub storage.
- **The only permanent store is the GitHub Release.** Release assets (binaries,
  per-platform VSIX) are free and unlimited; never retained Actions artifacts.
- **Transient cross-job hand-offs are the sole exception** (matrix jobs on separate
  runners can't share a filesystem: mutation shards → merge/score; release build
  matrix → publish). Each such upload **must** set `retention-days: 1`; the 90-day
  default is never acceptable.
- **Existing artifacts are purged, not left to expire** when this policy is
  tightened (`gh api repos/<owner>/<repo>/actions/artifacts` → `DELETE …/artifacts/{id}`).

Implemented by `.github/workflows/ci.yml` and `.github/workflows/release.yml` (every
`upload-artifact` carries `retention-days: 1` and a `[GITHUB-NO-ARTIFACTS]`
reference). The Actions **cache** (`Swatinem/rust-cache`, `actions/cache`) is
separate, free, and unaffected.

---

## Migration tooling target {#CHKARCH-MIGRATION}

### mypy import target {#CHKARCH-MIGRATION-MYPY}

Map supported mypy settings and emit an explicit unmapped-options report.

### Pyright import target {#CHKARCH-MIGRATION-PYRIGHT}

Map supported Pyright settings and emit an explicit unmapped-options report.

### Gradual-adoption mapping {#CHKARCH-MIGRATION-GRADUAL}

The CLI and LSP record current error debt as ordinary warning-severity rule
entries in the config file of the folder holding the debt — plain entries in
the one configuration model, with no exact-file overrides, ownership markers,
or sidecar state ([CHKARCH-CONFIG-MODEL](#CHKARCH-CONFIG-MODEL)). Re-running
adoption recomputes the debt and rewrites those entries, so rules that no
longer fire revert to the ancestor severity by deleting the folder entry.
There is no hidden compatibility mode
([AUTOFIX-ADOPTION](LSP-MASS-AUTOFIX-SPEC.md#AUTOFIX-ADOPTION)).
