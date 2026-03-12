# Mass Autofix & Gradual Adoption — Implementation Plan

## Prerequisites

- Basilisk diagnostic pipeline is functional and producing `BSK-E####` codes.
- LSP server is serving diagnostics to the VS Code extension.
- VS Code extension can display diagnostics and handle code actions.

---

## Phase 1: Fix Metadata Infrastructure

**Goal:** Every diagnostic can optionally carry fix metadata. No fixes are implemented yet — just the data structures and pipeline.

### Tasks

1. **Define core fix types** in `basilisk-core` (or shared types crate):
   - `Fix` struct (message, edits, safety, combinable, source)
   - `FixSafety` enum (`Safe`, `Unsafe`)
   - `FixSource` enum (`RuleBased`, `Heuristic`, `AiAssisted`)
   - `TextEdit` struct (range, new_text)

2. **Extend `Diagnostic` to carry `Vec<Fix>`** — each diagnostic can have zero or more proposed fixes.

3. **Map fix metadata to LSP `CodeAction`** in the LSP server:
   - Each `Fix` becomes a `CodeAction` with `kind: quickfix`.
   - `SafeFix` items get `isPreferred: true`.
   - `AiAssisted` items get a distinct title prefix: `"(AI) "`.

4. **Wire up empty fix vectors** — all existing diagnostic producers return `fixes: vec![]` for now.

### Deliverables
- Compiles. All existing tests pass. No behavioral changes.
- Fix types are usable by diagnostic rules.

---

## Phase 2: Implement Safe Autofixes for Core Rules

**Goal:** The most common "missing annotation" diagnostics get deterministic safe fixes.

### Priority Fixes (Safe)

| Rule | Fix | Safety |
|---|---|---|
| `BSK-E0001` Missing parameter type | Infer from usage if unambiguous, insert annotation | Safe (if unambiguous) |
| `BSK-E0002` Missing return type | Infer from return statements, insert `-> T` | Safe (if unambiguous) |
| `BSK-E0003` Missing variable type | Infer from assignment RHS, insert `: T =` | Safe (if unambiguous) |
| `BSK-E0005` Explicit `Any` required | Insert `Any` with import | Safe |

### Heuristic Fixes (Unsafe)

| Rule | Fix | Safety |
|---|---|---|
| `BSK-E0001` Missing parameter type (ambiguous) | Best-guess from partial usage | Unsafe |
| `BSK-E0002` Missing return type (ambiguous) | Best-guess from partial returns | Unsafe |

### Tasks

1. **Create a `FixProvider` trait**:
   ```rust
   pub trait FixProvider {
       fn provide_fixes(&self, diagnostic: &Diagnostic, context: &FixContext) -> Vec<Fix>;
   }
   ```

2. **Implement `FixProvider` for each rule** listed above. Each provider:
   - Receives the diagnostic and a `FixContext` (AST, type info, scope).
   - Returns zero or more `Fix` items with correct safety classification.

3. **Register providers in the diagnostic pipeline** so fixes are attached before diagnostics reach the LSP layer.

4. **Test each fix** with `.py` fixture files:
   - Input file with known issues.
   - Expected output file after fix is applied.
   - Assert fix safety classification is correct.

### Deliverables
- Code actions appear in VS Code for fixable diagnostics.
- Single-diagnostic fixes work end-to-end.

---

## Phase 3: Mass Autofix Engine

**Goal:** Apply all fixes in a file or workspace in one action.

### Tasks

1. **Build `MassFixEngine`**:
   ```rust
   pub struct MassFixEngine {
       safety_filter: FixSafety, // Safe or Unsafe (meaning: include unsafe)
   }

   impl MassFixEngine {
       pub fn collect_fixes(&self, diagnostics: &[Diagnostic]) -> Vec<ApplicableFix>;
       pub fn resolve_conflicts(&self, fixes: Vec<ApplicableFix>) -> Vec<ApplicableFix>;
       pub fn apply(&self, fixes: Vec<ApplicableFix>, source: &str) -> FixResult;
   }
   ```

2. **Conflict resolution**:
   - Sort fixes by start position.
   - Detect overlapping ranges.
   - Keep safer / earlier-registered fix, skip the other.
   - Report skipped fixes in `FixResult`.

3. **LSP integration** — register workspace-level code actions:
   - `source.fixAll.basilisk.safe` — all safe fixes in file.
   - `source.fixAll.basilisk.all` — all fixes in file.

4. **VS Code commands**:
   - `basilisk.fixAllSafe` — triggers `source.fixAll.basilisk.safe`.
   - `basilisk.fixAll` — triggers `source.fixAll.basilisk.all`.
   - `basilisk.fixAllWorkspaceSafe` — iterates all open/known files.
   - `basilisk.fixAllWorkspace` — iterates all open/known files.

5. **CLI integration**:
   - `basilisk fix <path>` — safe fixes only.
   - `basilisk fix --unsafe <path>` — all fixes.
   - Output summary: `Fixed 42 diagnostics in 12 files (3 skipped due to conflicts)`.

6. **Undo integration** — all edits from a mass fix are grouped in a single `WorkspaceEdit` so VS Code treats them as one undo unit.

### Deliverables
- Mass Autofix works from command palette, code actions, and CLI.
- Conflict resolution is tested.

---

## Phase 4: Gradual Adoption Mode

**Goal:** After autofix, demote remaining errors to warnings per-file.

### Tasks

1. **Define `adoptions.toml` schema**:
   ```toml
   [overrides."path/to/file.py"]
   demoted = ["BSK-E0001", "BSK-E0003"]
   ```

2. **Create `AdoptionStore`**:
   ```rust
   pub struct AdoptionStore {
       overrides: HashMap<PathBuf, HashSet<DiagnosticCode>>,
   }

   impl AdoptionStore {
       pub fn load(project_root: &Path) -> Result<Self, AdoptionError>;
       pub fn save(&self, project_root: &Path) -> Result<(), AdoptionError>;
       pub fn is_demoted(&self, file: &Path, code: &DiagnosticCode) -> bool;
       pub fn adopt_file(&mut self, file: &Path, codes: Vec<DiagnosticCode>);
       pub fn unadopt_file(&mut self, file: &Path);
       pub fn auto_graduate(&mut self, file: &Path, remaining_codes: &HashSet<DiagnosticCode>);
   }
   ```

3. **Integrate `AdoptionStore` into the diagnostic pipeline**:
   - After diagnostics are produced, before they reach the LSP:
   - Check each diagnostic against the adoption store.
   - If demoted, change severity from `Error` to `Warning`.
   - Tag the diagnostic with `"adopted": true` in its data payload.

4. **Implement the "Adopt" action**:
   - Run Mass Autofix (safe) on the target.
   - Collect all remaining error codes per file.
   - Write them to `adoptions.toml`.
   - Re-publish diagnostics with demoted severities.

5. **Auto-graduation**:
   - On every diagnostic pass, check if any demoted code has zero remaining instances in a file.
   - If so, remove that code from the file's adoption list.
   - If the file has no remaining demotions, remove the file entry entirely.

6. **VS Code commands**:
   - `basilisk.adoptFile` — adopt current file.
   - `basilisk.adoptWorkspace` — adopt all files.
   - `basilisk.unadoptFile` — restore full strictness for current file.

7. **Status bar integration**:
   - When the active file has adoptions, show `"Basilisk: Adopted (N rules demoted)"`.
   - Clicking it opens the command palette filtered to adoption commands.

8. **CLI integration**:
   - `basilisk adopt <path>` — autofix + adopt.
   - `basilisk unadopt <path>` — restore strictness.
   - `basilisk adopt --status` — show adoption summary.

### Deliverables
- Adopt workflow works end-to-end: autofix -> demote -> warnings.
- Auto-graduation removes adoptions as issues are fixed.
- `adoptions.toml` is committed to the repo so the team shares adoption state.

---

## Phase 5: AI Typing Hooks (Stubs Only)

**Goal:** The fix infrastructure supports AI-generated fixes. No AI provider is implemented — only the interface and a no-op default.

> **Note:** Full AI Typing implementation is outside the scope of this plan. This phase only creates the integration points so that AI can be plugged in later without architectural changes.

### Tasks

1. **Define `AiTypingProvider` trait**:
   ```rust
   pub trait AiTypingProvider: Send + Sync {
       fn suggest_fix(&self, request: AiTypingRequest) -> Result<Option<AiTypingResponse>, AiTypingError>;
       fn is_available(&self) -> bool;
   }
   ```

2. **Define request/response types**:
   - `AiTypingRequest` — diagnostic, AST context, inferred types, call sites, available types, source context.
   - `AiTypingResponse` — fix, confidence score, reasoning string.

3. **Implement `NoOpAiTypingProvider`**:
   ```rust
   pub struct NoOpAiTypingProvider;

   impl AiTypingProvider for NoOpAiTypingProvider {
       fn suggest_fix(&self, _request: AiTypingRequest) -> Result<Option<AiTypingResponse>, AiTypingError> {
           Ok(None)
       }
       fn is_available(&self) -> bool {
           false
       }
   }
   ```

4. **Wire `AiTypingProvider` into the fix pipeline**:
   - The `FixProvider` pipeline checks `ai_provider.is_available()`.
   - If available, it calls `suggest_fix()` for diagnostics with no rule-based fix.
   - AI-suggested fixes get `source: AiAssisted` and `safety: Unsafe`.

5. **VS Code extension configuration** (stubs):
   - `basilisk.aiTyping.enabled` setting (default: `false`).
   - `basilisk.aiTyping.provider` setting (default: `"none"`).
   - These settings are read but do nothing until a real provider is implemented.

### Deliverables
- `AiTypingProvider` trait exists and is wired in.
- No-op implementation compiles and is the default.
- A future implementor can add a real provider by implementing the trait and registering it.

---

## Phase Summary

| Phase | What | Depends On |
|---|---|---|
| 1. Fix Metadata | Data structures, LSP mapping | Existing diagnostic pipeline |
| 2. Safe Autofixes | Individual fix providers for core rules | Phase 1 |
| 3. Mass Autofix Engine | Batch apply, conflict resolution, CLI/extension commands | Phase 2 |
| 4. Gradual Adoption | `adoptions.toml`, severity demotion, auto-graduation | Phase 3 |
| 5. AI Typing Hooks | Trait, no-op impl, pipeline wiring | Phase 1 |

Phases 2 and 5 can run in parallel (both depend only on Phase 1). Phase 3 depends on Phase 2. Phase 4 depends on Phase 3.

```
Phase 1 ──┬── Phase 2 ── Phase 3 ── Phase 4
           └── Phase 5 (stubs only)
```
