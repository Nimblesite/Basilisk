# Mass Autofix & Gradual Adoption — Specification {#AUTOFIX}

## Problem {#AUTOFIX-PROBLEM}

When a user opens an existing Python module in Basilisk for the first time, the file is **red with errors**. Every missing type annotation, every implicit `Any`, every untyped parameter — they all fire as hard errors. This is correct behavior (strict-by-default is the point), but it makes Basilisk hostile to adoption on existing codebases. The user cannot work productively when the entire file is a wall of red.

We need two things:

1. **Mass Autofix** — apply every safe autofix in one action (single diagnostic, file, or entire module).
2. **Gradual Adoption Mode** — after autofixing everything possible, demote all remaining errors to warnings *per-file*, so the user sees yellow instead of red and can fix issues incrementally without being blocked.

## Mass Autofix {#AUTOFIX-MASS}

### Overview {#AUTOFIX-MASS-OVERVIEW}

Basilisk already produces diagnostics with structured fix metadata. Mass Autofix extends this so that **all applicable fixes can be applied in a single action** at three scopes:

| Scope | Trigger | What it does |
|---|---|---|
| **Single diagnostic** | Code action on a specific squiggle | Applies the fix for that one diagnostic |
| **File** | Command / code action at file level | Applies all fixable diagnostics in the current file |
| **Module / Workspace** | Command palette / CLI flag | Applies all fixable diagnostics across all files in scope |

### Fix Classification {#AUTOFIX-CLASSIFY}

Every autofix is classified into one of two safety tiers:

| Tier | Label | Meaning | Example |
|---|---|---|---|
| **Safe** | `SafeFix` | Guaranteed not to change runtime semantics. Can be applied without review. | Adding `: int` to a parameter where the type is unambiguously inferred from usage |
| **Unsafe** | `UnsafeFix` | Might change semantics or could be wrong. Requires review. | Inserting `-> None` on a function that might actually return something in an unreachable branch |

When the user triggers Mass Autofix, they choose one of:

- **Safe only** (default) — applies only `SafeFix` items.
- **All fixes** — applies both `SafeFix` and `UnsafeFix` items, with each unsafe fix marked in a review list.

### Fix Metadata {#AUTOFIX-METADATA}

Each diagnostic that supports autofix carries a `Fix` structure:

```rust
pub struct Fix {
    /// Human-readable description of what the fix does
    pub message: String,
    /// The text edits to apply
    pub edits: Vec<TextEdit>,
    /// Safety classification
    pub safety: FixSafety,
    /// Whether this fix can be combined with other fixes in the same file
    pub combinable: bool,
    /// Source of the fix (rule-based, heuristic, or AI-assisted)
    pub source: FixSource,
}

pub enum FixSafety {
    Safe,
    Unsafe,
}

pub enum FixSource {
    /// Deterministic fix derived from the rule definition
    RuleBased,
    /// Heuristic fix based on usage patterns and type inference
    Heuristic,
    /// AI-assisted fix (see Feature 3: AI Typing)
    AiAssisted,
}
```

### VS Code Integration {#AUTOFIX-MASS-VSCODE}

The extension exposes Mass Autofix through:

1. **Code Actions** — when the cursor is on a diagnostic, the lightbulb menu shows "Fix this", "Fix all in file (safe)", "Fix all in file (all)".
2. **Command Palette**:
   - `Basilisk: Fix All (Safe) in File`
   - `Basilisk: Fix All in File`
   - `Basilisk: Fix All (Safe) in Workspace`
   - `Basilisk: Fix All in Workspace`
3. **CLI** — `basilisk fix [--unsafe] [path]`

### Conflict Resolution {#AUTOFIX-CONFLICTS}

When multiple fixes target overlapping text ranges in the same file:

1. Fixes are sorted by start position (ascending).
2. If two fixes overlap, the **safer** fix wins. If same safety, the **earlier-registered** fix wins.
3. The losing fix is skipped and reported as "skipped due to conflict".
4. After applying all non-conflicting fixes, diagnostics are re-evaluated. Skipped fixes may become applicable on the next pass.

### Undo {#AUTOFIX-UNDO}

Mass Autofix is a single undo unit in VS Code. One `Ctrl+Z` reverts all changes from the batch.

---

## Gradual Adoption Mode {#AUTOFIX-ADOPTION}

### Overview {#AUTOFIX-ADOPTION-OVERVIEW}

After Mass Autofix has done everything it can, there will still be diagnostics that cannot be auto-fixed. In a strict-by-default checker, these are **errors** — red squiggles that block the user's flow.

Gradual Adoption Mode **demotes all remaining unfixable errors to warnings per-file**. The user sees yellow instead of red. They can work productively and fix warnings one by one at their own pace.

### How It Works {#AUTOFIX-ADOPTION-FLOW}

1. User triggers **"Basilisk: Adopt File"** (or "Adopt Workspace").
2. Basilisk runs Mass Autofix (safe only) on the target scope.
3. For each remaining error diagnostic in each file:
   - The diagnostic code (e.g. `BSK-E0001`) is recorded in a per-file override list.
   - The severity for that code **in that file only** is demoted from `Error` to `Warning`.
4. The override list is written to a `.basilisk/adoptions.toml` file in the project root.
5. From this point on, Basilisk uses `Warning` severity for those codes in those files.

### Adoption File Format {#AUTOFIX-ADOPTION-FILE}

```toml
# .basilisk/adoptions.toml
# Auto-generated by Basilisk Gradual Adoption Mode
# Remove entries as you fix the underlying issues

[overrides]

[overrides."src/utils.py"]
demoted = ["BSK-E0001", "BSK-E0003", "calls_argument_type"]

[overrides."src/models/user.py"]
demoted = ["BSK-E0001", "BSK-E0002"]
```

### Behavior Rules {#AUTOFIX-ADOPTION-RULES}

- **New code is still strict.** If you create a new file, all rules are errors. Adoption only applies to files that have been explicitly adopted.
- **New violations in adopted files are still errors.** If an adopted file has `BSK-E0001` demoted, and the user adds a *new* function with a missing type annotation, that new `BSK-E0001` is still a warning (the demotion is per-code-per-file, not per-instance). This is intentional — the user should not be blocked.
- **Fixing all instances of a demoted code auto-removes the override.** When Basilisk detects that a file has zero remaining instances of a demoted code, it removes that code from the adoption file. The file progressively "graduates" to full strictness.
- **Manual un-adoption.** The user can remove entries from `adoptions.toml` manually or via `Basilisk: Un-adopt File` to restore full strictness.

### VS Code Integration {#AUTOFIX-ADOPTION-VSCODE}

1. **Command Palette**:
   - `Basilisk: Adopt File` — autofix + demote remaining errors for current file
   - `Basilisk: Adopt Workspace` — autofix + demote remaining errors for all files
   - `Basilisk: Un-adopt File` — restore full strictness for current file
2. **Status Bar** — when a file has active adoptions, the status bar shows "Basilisk: Adopted (N rules demoted)".
3. **Gutter indicators** — adopted (demoted) warnings get a distinct icon to differentiate them from "natural" warnings.

---

## AI Typing Hooks {#AUTOFIX-AI}

AI Typing is an AI-assisted type inference feature that goes beyond what deterministic analysis can achieve. When Basilisk encounters a diagnostic it cannot autofix (typically missing type information), it feeds the **full analyzer context** — the AST, inferred types, call graph, usage patterns, and surrounding code — to an AI model. The model returns its best guess for the correct fix.

> For the full AI provider abstraction, request/response types, and implementation plan, see [LSP-AI-SPEC.md §LSPAI-FEATURE-MASSAUTOFIX](LSP-AI-SPEC.md#LSPAI-FEATURE-MASSAUTOFIX). This section documents only the integration point between Mass Autofix and the AI layer.

### Scope {#AUTOFIX-AI-SCOPE}

**AI Typing implementation is outside the scope of the Mass Autofix / Gradual Adoption work.** The current plan only requires:

1. The `FixSource::AiAssisted` variant in the fix metadata.
2. The `AiTypingProvider` trait definition.
3. A no-op default implementation that returns `None` for all requests.
4. The `AiTypingRequest` / `AiTypingResponse` structures.

This ensures the fix pipeline is AI-ready without blocking the core autofix and adoption features on AI integration work. When AI Typing is implemented later, it slots in without architectural changes.

---

## Summary {#AUTOFIX-SUMMARY}

| Feature | User sees | Scope | Safety |
|---|---|---|---|
| Mass Autofix (Safe) | Fixes applied, no review needed | Diagnostic / File / Workspace | Only deterministic, semantics-preserving fixes |
| Mass Autofix (All) | Fixes applied, review list for unsafe ones | Diagnostic / File / Workspace | Includes heuristic and potentially wrong fixes |
| Gradual Adoption | Errors become warnings, user unblocked | File / Workspace | No code changes, only severity overrides |
| AI Typing (future) | AI-suggested fixes with explanations | Single diagnostic | Always unsafe, always requires confirmation |
