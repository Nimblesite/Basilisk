# AI-Powered LSP Features — Implementation Plan

## Status

**Prerequisites:**
- Diagnostic pipeline: EXISTS (BSK-E#### codes, spans, messages)
- Code actions: EXISTS (E0001-E0003, W0050 quick fixes, suppress, imports)
- Mass Autofix spec: EXISTS (but Phase 5 AI Typing Hooks not yet implemented)
- AI provider code: NOTHING exists

**This plan expands:** `MASS-AUTOFIX-PLAN.md` Phase 5 (AI Typing Hooks — stubs only). That phase was designed as a stub. This plan upgrades the stubs to a full provider abstraction. Mass Autofix Phases 1-4 are **completely independent of this plan** — they are deterministic features that work without AI. AI is an optional enhancement layer that plugs in after deterministic fixes for diagnostics that can't be fixed otherwise.

**Dependency:** Mass Autofix Phase 1 (Fix Metadata Infrastructure) must land first. AI fixes use the same `Fix` struct.

---

## Phase 1: Provider Abstraction & NoOp Default

**Goal:** The `AiProvider` trait exists, is wired into the LSP, and does nothing by default. Zero overhead when disabled.

### Tasks

1. **Create `basilisk-ai` crate** in the workspace:
   - `Cargo.toml` — minimal dependencies (serde, serde_json for request/response serialization). No AI SDK dependencies.
   - `src/lib.rs` — re-exports.
   - `src/provider.rs` — `AiProvider` trait (all methods: `suggest_fix`, `suggest_import`, `suggest_rename`, `suggest_refactoring`, `explain_diagnostic`, `generate_docstring`, `generate_stub`, `enhance_completions`), `AiProviderCapabilities`, `AiProviderError`.
   - `src/types/mod.rs` — shared types (`InferredTypeInfo`, `CallSiteContext`, `AvailableType`, `TypeConfidence`, `SymbolKind`, `NamingConventions`, `CasingStyle`).
   - `src/types/fix.rs` — `AiFixRequest`, `AiFixResponse`, `AlternativeFix`.
   - `src/types/import.rs` — `AiImportRequest`, `AiImportResponse`, `ImportSuggestion`, `ImportUsageContext`.
   - `src/types/rename.rs` — `AiRenameRequest`, `AiRenameResponse`, `RenameSuggestion`.
   - `src/types/refactor.rs` — `AiRefactorRequest`, `AiRefactorResponse`, `RefactorKind`, `RefactorSuggestion`.
   - `src/types/explain.rs` — `AiExplainRequest`, `AiExplainResponse`.
   - `src/types/docstring.rs` — `AiDocstringRequest`, `AiDocstringResponse`, `DocstringStyle`.
   - `src/types/stub.rs` — `AiStubRequest`, `AiStubResponse`, `StubUsagePattern`.
   - `src/types/completion.rs` — `AiCompletionRequest`, `AiCompletionResponse`, `CompletionSummary`, `AiCompletionItem`.
   - `src/types/search.rs` — `AiSemanticSearchRequest`, `AiSemanticSearchResponse`, `SymbolIndexEntry`, `SemanticSearchResult`.
   - `src/types/dead_code.rs` — `AiDeadCodeRequest`, `AiDeadCodeResponse`, `ConfigSnippet`.
   - `src/types/modernize.rs` — `AiModernizeRequest`, `AiModernizeResponse`, `ModernizeSuggestion`.
   - `src/types/next_edit.rs` — `AiNextEditRequest`, `AiNextEditResponse`, `NextEditPrediction`, `RelatedSymbol`.
   - `src/noop.rs` — `NoOpProvider` implementation (returns `None` / `is_available() = false` for everything).

2. **Wire `AiProvider` into the LSP server**:
   - `LspServer` gains a field: `ai_provider: Arc<dyn AiProvider>`.
   - Default: `Arc::new(NoOpProvider)`.
   - Code action handler checks `ai_provider.is_available()` before offering AI actions.

3. **Configuration reading**:
   - Read `[tool.basilisk.ai]` from `pyproject.toml`.
   - Read editor settings under `basilisk.ai.*`.
   - Read env vars: `BASILISK_AI_ENABLED`, `BASILISK_AI_PROVIDER`.
   - Provider selection logic: config → env var override → default (NoOp).

4. **Context builder**:
   - `src/context.rs` — `fn build_fix_request(diagnostic: &Diagnostic, resolved: &ResolvedModule, source: &str) -> AiFixRequest`.
   - Extracts surrounding source, inferred types, call sites, available types from `ResolvedModule`.
   - Respects `max-source-lines` and `max-call-sites` config.

5. **Context truncation**:
   - `src/truncation.rs` — `fn truncate_request(request: &mut AiFixRequest, max_tokens: usize)`.
   - Priority-based truncation per spec.

### Deliverables
- Compiles. All existing tests pass. Zero behavioral changes.
- `AiProvider` trait is usable by provider implementations.
- Context builder produces correct payloads from `ResolvedModule`.

---

## Phase 2: OpenAI-Compatible Provider

**Goal:** Any model behind an OpenAI-compatible API works. This covers Ollama, LM Studio, vLLM, llama.cpp, OpenAI, Azure, Groq, Together, Fireworks, and anything else that speaks this protocol.

### Tasks

1. **Implement `OpenAiCompatibleProvider`** in `basilisk-ai/src/providers/openai_compatible.rs`:
   - HTTP client: `ureq` (blocking, no tokio dependency, already used elsewhere in the project — check first; if not, `minreq` for minimal footprint).
   - Endpoint: `POST {base_url}/chat/completions`.
   - Prompt construction: system prompt explains the task (type annotation, type fix, etc.), user prompt contains the serialized `AiFixRequest` as structured context.
   - Response parsing: extract the fix from the model's response. Parse JSON if the model returns structured output, otherwise extract from markdown code blocks.
   - Timeout: configurable, default 30s.
   - Error mapping: HTTP errors → `AiProviderError` variants.

2. **System prompt design** (critical for quality):
   - Role: "You are a Python type system expert working with the Basilisk type checker."
   - Task-specific prompts for each feature:
     - **Fix/annotate**: "Suggest a type annotation..." / "Fix this type error..."
     - **Import**: "Suggest the correct import for this unresolved name..."
     - **Rename**: "Suggest better names for this symbol based on its usage..."
     - **Refactor**: "Suggest a refactoring for this code region..."
     - **Explain**: "Explain this type error in plain language..."
     - **Docstring**: "Generate a docstring in {style} format..."
     - **Stub**: "Generate type stubs for this module..."
     - **Completion**: "Re-rank these completion items by relevance..."
     - **Search**: "Which of these symbols are related to the query..."
     - **Dead code**: "Is this symbol reachable via framework/dynamic mechanisms..."
     - **Modernize**: "Suggest a modern Python equivalent for this pattern..."
     - **Next-edit**: "What follow-up edits are needed after this change..."
   - Context: each request type's fields formatted for readability.
   - Output format: JSON matching the specific response type.
   - Constraints: only use types from `available_types`, prefer specific types over `Any`, explain reasoning, respect Python version constraints.

3. **Capability detection**:
   - Default capabilities based on model name heuristics (large models get all capabilities, small models get type-annotation only).
   - User can override via config.

4. **E2E test with MockProvider**:
   - Test that the provider is selected from config.
   - Test that requests are built correctly.
   - Test that responses are parsed into `AiFixResponse`.
   - Test timeout handling.
   - Test missing API key handling.

### Deliverables
- `basilisk.ai.provider = "openai-compatible"` with local Ollama works end-to-end.
- Same provider config works against OpenAI, Azure, etc.

---

## Phase 3: Anthropic & Copilot Providers

**Goal:** First-party support for Claude and GitHub Copilot.

### Tasks

1. **Implement `AnthropicProvider`** in `basilisk-ai/src/providers/anthropic.rs`:
   - HTTP client: same as OpenAI provider (`ureq`/`minreq`).
   - Endpoint: `POST https://api.anthropic.com/v1/messages`.
   - Anthropic-specific headers: `x-api-key`, `anthropic-version`.
   - Message format: system + user messages (Anthropic format, not OpenAI).
   - Same prompt strategy and response parsing as OpenAI provider.

2. **Implement `CopilotProvider`** in `basilisk-ai/src/providers/copilot.rs`:
   - Discover the Copilot agent: check for VS Code's Copilot extension socket, Neovim's copilot.lua socket, or standalone copilot-node-server.
   - Authentication: piggyback on existing Copilot auth (the user is already signed in).
   - Request format: Copilot's internal API for completions/chat.
   - Fallback: if Copilot agent not found, `is_available() = false`.

3. **Implement `ProcessProvider`** in `basilisk-ai/src/providers/process.rs`:
   - Spawn subprocess from configured command.
   - Send any request type (fix, import, rename, refactor, etc.) as JSON to stdin with a `"type"` discriminator field.
   - Read the corresponding response type as JSON from stdout.
   - Timeout handling (kill process on timeout).
   - Process lifecycle: spawn per-request or keep alive (configurable).
   - Document the JSON wire protocol so users can build custom bridges in any language.

### Deliverables
- Claude, Copilot, and custom process providers work.
- Users with existing Copilot subscriptions get AI features for free.

---

## Phase 4: LSP Feature Integration

**Goal:** AI features appear in the editor as code actions, commands, and hover info.

### Tasks

1. **AI code actions in `code_actions/`**:
   - New module: `code_actions/ai.rs`.
   - When generating code actions for a diagnostic:
     - First: deterministic fixes (existing behavior).
     - Then: if AI provider is available AND the diagnostic has no deterministic fix (or only a low-confidence heuristic), call the AI provider.
     - AI code actions get kind `quickfix.ai.*` and title prefix `"(AI) "`.
     - Include confidence score and reasoning in the code action `data`.

2. **Async handling**:
   - AI requests may be slow (especially cloud). The LSP must not block.
   - Strategy: return deterministic code actions immediately. AI code actions arrive via a follow-up `codeAction/resolve` or a refresh notification.
   - Alternative (simpler): make AI code actions on-demand only — user triggers `basilisk/ai/suggestFix` command explicitly.

3. **Mass autofix AI enhancement** (optional layer on top of deterministic mass autofix):
   - Mass autofix runs first — deterministic, independent of AI, as defined in `MASS-AUTOFIX-SPEC.md`.
   - After deterministic fixes are applied, if AI is enabled:
     - Collect remaining unfixed diagnostics (those with no deterministic fix).
     - Batch them to `suggest_fixes_batch`.
     - Present AI suggestions in a review list (not auto-applied, always Unsafe).
   - If AI is disabled: mass autofix works exactly as before. No change.
   - VS Code: show AI suggestions in a diff view or quick pick.

4. **Diagnostic explanation**:
   - Code action: `"(AI) Explain this error"` on any diagnostic.
   - Calls `basilisk/ai/explain` command.
   - Shows explanation in a hover popup or output panel.

5. **Docstring generation**:
   - Code action on function/class definitions: `"(AI) Generate docstring"`.
   - Calls `basilisk/ai/generateDocstring`.
   - Detects existing docstring style in the file (Google, NumPy, reST).

6. **Status bar / provider status**:
   - `basilisk/ai/status` command.
   - On init: log provider name and availability.
   - VS Code status bar item: "Basilisk AI: [provider name]" or "Basilisk AI: Off".

### Deliverables
- AI code actions appear in all editors.
- Mass autofix uses AI for unfixable diagnostics.
- Explain and docstring features work.
- Provider status is visible.

---

## Phase 5: CLI Integration

**Goal:** AI features available from the command line for CI/batch workflows.

### Tasks

1. **`basilisk fix --ai`**:
   - Runs deterministic fixes first (existing behavior).
   - Then runs AI provider on remaining diagnostics.
   - Prints AI suggestions with confidence and reasoning.
   - `--ai-apply` flag: apply AI fixes without interactive review (for brave users).
   - `--ai-review` flag (default): print suggestions as a diff for review.

2. **`basilisk ai explain <file> <line>`**:
   - Explain the diagnostic at the given location.
   - Output: plain text explanation.

3. **`basilisk ai status`**:
   - Show configured provider, availability, capabilities (all feature flags).

4. **`basilisk ai debug-context <file> <line>`**:
   - Dump the exact request payload that would be sent for the diagnostic at the given location.
   - Users can inspect exactly what data leaves their machine.

5. **`basilisk stubs generate --ai <module>`**:
   - Generate type stubs for an untyped module using AI.
   - Write to `typings/` directory (configurable).
   - Show warnings for low-confidence symbol types.

6. **`basilisk ai search <query>`**:
   - Semantic search from the command line.
   - Output: matching symbols with file paths, line numbers, and relevance scores.

7. **`basilisk ai dead-code <file|directory>`**:
   - Analyze potential dead code with AI framework awareness.
   - Output: list of symbols, dead/alive verdict, confidence, reasoning.

### Deliverables
- CLI users get all AI features without an editor.
- `debug-context` enables privacy auditing.
- `stubs generate --ai` enables batch stub generation for CI.

---

## Phase 6: AI Import Resolution & Rename Suggestions

**Goal:** AI enhances two existing LSP features — import resolution and rename.

### Tasks

1. **Context builders for import and rename**:
   - `src/context.rs` — `fn build_import_request(name: &str, usage: &SyntaxNode, resolved: &ResolvedModule) -> AiImportRequest`.
   - `src/context.rs` — `fn build_rename_request(symbol: &Symbol, resolved: &ResolvedModule) -> AiRenameRequest`.
   - Import context: extract usage pattern (call, annotation, attribute access), existing imports, installed packages.
   - Rename context: extract symbol kind, type, usage sites, detect file naming conventions.

2. **AI import code action**:
   - In `code_actions/ai.rs`: when an unresolved-name diagnostic fires and deterministic import resolution returns no results or ambiguous results, add `"(AI) Add import for {name}"` code action.
   - On acceptance: insert the import statement at the correct location (respecting isort conventions).

3. **AI rename integration**:
   - In the rename handler: after the user triggers rename (F2), send `AiRenameRequest` to the provider if `rename_suggestion` capability is true.
   - Return suggestions as `textDocument/prepareRename` additional data (or via a custom command `basilisk/ai/suggestRename`).
   - The actual rename (reference updates) remains deterministic.

4. **Naming convention detector**:
   - Analyze the file's function names, variable names, class names to determine dominant casing style.
   - Feed this to the AI so it suggests names that match the project's style.

### Deliverables
- AI import suggestions appear when deterministic resolution fails.
- AI name suggestions appear in the rename dialog.
- Both features gracefully degrade (no AI → no suggestions, no error).

---

## Phase 7: AI Refactoring & Code Modernization

**Goal:** AI suggests structural refactoring and modern Python patterns.

### Tasks

1. **Refactoring context builder**:
   - `src/context.rs` — `fn build_refactor_request(range: TextRange, resolved: &ResolvedModule) -> AiRefactorRequest`.
   - Extract selected code region, enclosing scope, types in scope.
   - Determine applicable refactoring kinds based on code patterns.

2. **Refactoring code actions**:
   - When user selects code and requests code actions: if AI provider has `refactoring` capability, analyze the selection and offer refactoring suggestions.
   - Each refactoring kind gets its own `CodeActionKind`: `refactor.ai.extract-method`, `refactor.ai.convert-dataclass`, etc.
   - Refactoring edits are validated: the AI's suggested edits must parse as valid Python AST.

3. **Proactive refactoring hints** (optional, configurable):
   - Background analysis: when a function exceeds a complexity threshold, send to AI for refactoring suggestions.
   - Show as hint-level diagnostics with attached code actions.
   - Off by default (`basilisk.ai.proactiveRefactoring = false`).

4. **Modernization context builder**:
   - `src/context.rs` — `fn build_modernize_request(range: TextRange, resolved: &ResolvedModule, python_version: (u32, u32)) -> AiModernizeRequest`.
   - Check project's minimum Python version from `pyproject.toml` `[project] requires-python`.

5. **Modernization code actions**:
   - Identify code patterns with modern alternatives (isinstance chains → match, Union → |, etc.).
   - Deterministic modernizations (simple syntax swaps) are NOT sent to AI.
   - AI handles nuanced transformations where intent matters.
   - Code action kind: `refactor.ai.modernize`.

6. **AST validation**:
   - All AI-suggested refactorings are validated: parse the result as Python AST. If it doesn't parse, discard and report to user.
   - This is a safety net — the AI might produce syntactically invalid code.

### Deliverables
- AI refactoring suggestions appear for selected code regions.
- AI modernization suggestions appear for legacy patterns.
- All suggestions are validated against the Python parser before presentation.

---

## Phase 8: AI Completions, Stubs, Search, Dead Code & Next-Edit

**Goal:** The remaining AI features — each independent, can be implemented in any order.

### Tasks

#### 8a: AI-Enhanced Completions

1. **Completion enhancement pipeline**:
   - After deterministic completion produces items, if AI provider has `completion_enhancement` capability:
     - Build `AiCompletionRequest` with cursor context and deterministic items.
     - Send async. If response arrives within `completion-timeout-ms`, merge.
     - If too slow, user sees deterministic list unchanged.
   - Merge strategy: AI-reranked items first, then remaining deterministic items in original order, then AI-only additions with `(AI)` prefix.

2. **Enhanced documentation**:
   - AI can provide richer documentation strings for completion items.
   - Merged into `documentation` field of `CompletionItem`.

#### 8b: AI Stub Generation

1. **Stub context builder**:
   - `fn build_stub_request(module_path: &str, module_source: Option<&str>, usages: &[StubUsagePattern]) -> AiStubRequest`.
   - Find module source in installed packages (site-packages).
   - Find usage patterns across the project via cross-file analysis.

2. **Stub generation flow**:
   - Code action on untyped-module diagnostic: `"(AI) Generate type stubs for {module}"`.
   - CLI: `basilisk stubs generate --ai {module}`.
   - Generated stub written to `typings/{module_path}.pyi`.
   - Marked as Tier 3 provenance.

3. **Stub validation**:
   - Parse generated `.pyi` as valid Python stub syntax.
   - Check that all referenced types are importable.

#### 8c: AI Semantic Search

1. **Symbol index export**:
   - Export workspace symbol index as `Vec<SymbolIndexEntry>` — name, kind, type, docstring, preview.
   - Pre-filter by text similarity to query (TF-IDF or keyword overlap) to top N symbols.

2. **Semantic search command**:
   - `basilisk/ai/findByIntent` custom command.
   - Workspace symbol provider optionally routes through AI for natural language queries (detected by: no camelCase/snake_case pattern, contains spaces, etc.).

#### 8d: AI Dead Code Detection

1. **Dead code context builder**:
   - When reference analysis finds zero-reference symbols, build `AiDeadCodeRequest`.
   - Extract decorators, module path, relevant config snippets (pyproject.toml `[project.scripts]`, Django urlpatterns, Flask routes, etc.).

2. **Config snippet extraction**:
   - Scan well-known config files for references to the symbol or its module.
   - `pyproject.toml`, `setup.py`, `setup.cfg`, framework-specific configs.

3. **Diagnostic adjustment**:
   - If AI says "not dead": suppress diagnostic or downgrade to hint.
   - If AI says "dead" with high confidence: show as warning.

#### 8e: AI Next-Edit Prediction

1. **Edit event handler**:
   - Listen to `textDocument/didChange` events.
   - Debounce: only process after 500ms of no edits.
   - Build `AiNextEditRequest` with the diff, file context, related symbols.

2. **Related symbol discovery**:
   - Find call sites, implementations, and tests for the edited symbol.
   - Send as context so the model knows where follow-up edits are needed.

3. **Prediction display**:
   - Send predictions to the editor as custom notifications.
   - VS Code extension: render as ghost text at the predicted location.
   - Other editors: show as information diagnostics or code lens.

4. **Latency enforcement**:
   - Hard cutoff at `next-edit-timeout-ms`. If the model doesn't respond in time, discard.
   - Only enabled if provider has `next_edit_prediction` capability AND `max_latency_ms` ≤ timeout.

### Deliverables
- AI-enhanced completions with async merge.
- AI stub generation from CLI and code actions.
- Semantic search for natural language symbol queries.
- Dead code detection with framework awareness.
- Next-edit prediction with ghost text.

---

## Phase Summary

| Phase | What | Depends On | Parallel? |
|-------|------|-----------|-----------|
| 1. Provider Abstraction | Trait, types, NoOp, config, context builder | Mass Autofix Phase 1 (Fix Metadata) | — |
| 2. OpenAI-Compatible Provider | HTTP provider covering Ollama + cloud | Phase 1 | — |
| 3. Anthropic & Copilot Providers | Claude, Copilot, Process providers | Phase 1 | Yes (with Phase 2) |
| 4. LSP Feature Integration | Code actions, mass autofix AI, explain, docstring | Phase 2 or 3 | — |
| 5. CLI Integration | `basilisk fix --ai`, `basilisk ai explain/status/debug-context` | Phase 4 | — |
| 6. Import & Rename | AI import resolution, AI rename suggestions | Phase 1 | Yes (with 2-5) |
| 7. Refactoring & Modernization | AI refactoring suggestions, code modernization | Phase 1 | Yes (with 2-6) |
| 8a. Enhanced Completions | AI completion re-ranking and enhancement | Phase 1 | Yes (with all) |
| 8b. Stub Generation | AI type stub generation for untyped packages | Phase 1 | Yes (with all) |
| 8c. Semantic Search | Natural language workspace symbol search | Phase 1 | Yes (with all) |
| 8d. Dead Code Detection | Framework-aware dead code analysis | Phase 1 | Yes (with all) |
| 8e. Next-Edit Prediction | Predict and suggest follow-up edits | Phase 1 | Yes (with all) |

```
Mass Autofix Phase 1 (Fix Metadata)
                │
                ▼
    AI Phase 1 (Provider Abstraction)
                │
    ┌───────────┼───────────┬───────────┬──────────┐
    ▼           ▼           ▼           ▼          ▼
 Phase 2     Phase 3     Phase 6     Phase 7    Phase 8a-e
 (OpenAI)   (Anthropic   (Import     (Refactor  (Completions,
             Copilot      Rename)     Modern)    Stubs, Search,
             Process)                            Dead Code,
    │           │                                Next-Edit)
    └─────┬─────┘
          ▼
    Phase 4 (LSP Integration: fixes, explain, docstring)
          │
          ▼
    Phase 5 (CLI)
```

Phases 6, 7, and 8a-e all depend on Phase 1 only. They can run in parallel with each other and with Phases 2-5. This means a contributor working on semantic search doesn't block someone working on refactoring.

---

## Non-Goals (Explicit)

Things this plan does NOT do:

- **General code generation.** Basilisk enhances specific LSP features with AI. It doesn't generate arbitrary code from natural language prompts. That's Copilot/Cursor territory.
- **Training or fine-tuning.** Basilisk sends prompts. It doesn't train models.
- **Bundling a model.** No shipping a 4GB GGUF with the binary. Users bring their own model.
- **Agent loops.** No "let the AI iterate until the file has zero errors." One request, one response, user decides.
- **Chat interface.** No chatbot in the sidebar. Structured requests, structured responses, code actions.
- **Full IDE replacement.** Basilisk is a type checker with an LSP. AI enhances the type checking and code intelligence features. It doesn't try to be a general-purpose AI coding assistant.
