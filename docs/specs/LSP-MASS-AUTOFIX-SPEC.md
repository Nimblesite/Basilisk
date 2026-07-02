# Mass Autofix & Gradual Adoption — Specification {#AUTOFIX}

Two features for adopting strict-by-default checking on existing code:

1. **Mass Autofix** — apply every safe autofix in one action (single diagnostic, file, or module).
2. **Gradual Adoption Mode** — after autofixing, demote remaining errors to warnings *per-file* for incremental fixing without being blocked.

## Mass Autofix {#AUTOFIX-MASS}

### Scopes {#AUTOFIX-MASS-OVERVIEW}

Applies all applicable fixes in one action at three scopes:

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

The user chooses one of:

- **Safe only** (default) — applies only `SafeFix` items.
- **All fixes** — applies `SafeFix` and `UnsafeFix`, each unsafe fix marked in a review list.

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

Exposed through:

1. **Code Actions** — on a diagnostic, the lightbulb shows "Fix this", "Fix all in file (safe)", "Fix all in file (all)".
2. **Command Palette**:
   - `Basilisk: Fix All (Safe) in File`
   - `Basilisk: Fix All in File`
   - `Basilisk: Fix All (Safe) in Workspace`
   - `Basilisk: Fix All in Workspace`
3. **CLI** — `basilisk fix [--unsafe] [path]`

### Conflict Resolution {#AUTOFIX-CONFLICTS}

When multiple fixes produce overlapping text edits in the same file (`collect_non_overlapping_edits`, `crates/basilisk-lsp/src/code_actions/mass_fix.rs`):

1. Candidate edits are sorted by start position (line, then character), ascending.
2. Edits are accepted greedily in that order; an edit whose range overlaps the previously accepted edit is skipped. Resolution is purely positional — safety is never compared at this stage. With the shipped fix set, Safe and Unsafe fixes cannot produce overlapping edits: all fixes except BSK-W0050's annotation removal are zero-width inserts targeting mutually exclusive constructs.
3. Skipped edits are silently dropped from the batch — they are not applied and not itemised in the result.
4. Re-evaluation happens through the normal check loop rather than an internal second pass: in the editor, applying the `WorkspaceEdit` triggers a re-check that re-publishes remaining diagnostics (and their fixes); on the CLI, `basilisk fix` is idempotent and a re-run applies any fix that became applicable.

Safety scoping is the caller's concern, upstream of conflict resolution: the CLI filters to safe-only by default (`--unsafe` / `--rules` / `all` widen it), while the LSP fix-all commands currently operate on all fixable rules.

### Undo {#AUTOFIX-UNDO}

Mass Autofix is a single undo unit in VS Code — one `Ctrl+Z` reverts the whole batch.

---

## Gradual Adoption Mode {#AUTOFIX-ADOPTION}

After Mass Autofix runs, diagnostics that cannot be auto-fixed are demoted from error to warning **per-file** for incremental fixing.

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

- **New code is still strict.** Adoption applies only to explicitly adopted files; new files are all errors.
- **New violations in adopted files stay demoted.** Demotion is per-code-per-file, not per-instance: a new `BSK-E0001` in a file with `BSK-E0001` demoted is still a warning.
- **Fixing all instances auto-removes the override.** When a file has zero remaining instances of a demoted code, Basilisk removes that code from the adoption file — the file "graduates" to full strictness.
- **Manual un-adoption.** Remove entries from `adoptions.toml` manually or via `Basilisk: Un-adopt File`.

### VS Code Integration {#AUTOFIX-ADOPTION-VSCODE}

1. **Command Palette**:
   - `Basilisk: Adopt File` — autofix + demote remaining errors for current file
   - `Basilisk: Adopt Workspace` — autofix + demote remaining errors for all files
   - `Basilisk: Un-adopt File` — restore full strictness for current file
2. **Status Bar** — when a file has active adoptions, the status bar shows "Basilisk: Adopted (N rules demoted)".
3. **Gutter indicators** — adopted (demoted) warnings get a distinct icon to differentiate them from "natural" warnings.

---

## AI Typing Hooks {#AUTOFIX-AI}

For a diagnostic that cannot be deterministically autofixed (typically missing type information), AI Typing feeds analyzer context (AST, inferred types, call graph, usage patterns, surrounding code) to an AI model that returns a candidate fix. AI-assisted fixes are always `Unsafe` and require confirmation.

> The AI provider abstraction, request/response types, and plan live in [LSP-AI-SPEC.md §LSPAI-FEATURE-MASSAUTOFIX](LSP-AI-SPEC.md#LSPAI-FEATURE-MASSAUTOFIX). This section documents only the Mass Autofix ↔ AI integration point.

### Scope {#AUTOFIX-AI-SCOPE}

AI Typing implementation is out of scope here. This spec requires only the AI-ready seams in the fix pipeline:

1. The `FixSource::AiAssisted` variant in the fix metadata.
2. The `AiTypingProvider` trait definition.
3. A no-op default implementation returning `None` for all requests.
4. The `AiTypingRequest` / `AiTypingResponse` structures.
