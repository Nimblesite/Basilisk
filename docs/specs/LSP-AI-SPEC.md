# AI-Powered LSP Features — Specification {#LSPAI}

> ⚠️ **Status: ROADMAP (interface-only).** Ships today: a ~250-line slice
> (`crates/basilisk-lsp/src/ai_typing.rs`) — the `AiTypingProvider` trait, the
> `NoOpAiTypingProvider` default (`is_available() == false`), and the
> request/response/error types — wired in but invoked nowhere. No concrete
> providers, per-feature behaviours, config/env, protocol commands, truncation,
> or `MockProvider`. Sections below describe intended behaviour, not current
> reality; see [`CONFAUDIT-ROADMAP`](../plans/SPEC-CONFORMANCE-AUDIT-PLAN.md#CONFAUDIT-ROADMAP).

Model-agnostic AI integration in the Basilisk LSP (local, cloud, Copilot, Claude, Ollama). The LSP sends structured context and receives structured responses; it is agnostic to the provider behind the trait. AI is optional — every deterministic LSP feature works without it.

---

## Design Principles {#LSPAI-PRINCIPLES}

1. **Model-agnostic.** Basilisk defines a trait; providers implement it. Swap models via one config line.
2. **Structured in, structured out.** The LSP sends analyzer context (AST, types, call graph, diagnostics); the provider returns structured fixes.
3. **Always unsafe.** Every AI fix is `FixSafety::Unsafe` and `FixSource::AiAssisted`. Never auto-applied; always requires user confirmation.
4. **Offline-first.** Disabled by default. Local models work offline; cloud providers need explicit opt-in.
5. **Privacy-respecting.** Local models keep everything local; cloud providers require opt-in with an inspectable payload.
6. **No vendor lock-in.** The provider interface is the contract.

---

## Provider Abstraction {#LSPAI-PROVIDER}

### AiProvider Trait {#LSPAI-TRAIT}

Every AI integration implements this trait; the LSP server holds a `Box<dyn AiProvider>`.

```rust
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    fn capabilities(&self) -> AiProviderCapabilities;

    fn suggest_fix(&self, request: AiFixRequest) -> Result<Option<AiFixResponse>, AiProviderError>;
    fn suggest_fixes_batch(&self, requests: Vec<AiFixRequest>) -> Result<Vec<Option<AiFixResponse>>, AiProviderError>;
    fn suggest_import(&self, request: AiImportRequest) -> Result<Option<AiImportResponse>, AiProviderError>;
    fn suggest_rename(&self, request: AiRenameRequest) -> Result<Option<AiRenameResponse>, AiProviderError>;
    fn suggest_refactoring(&self, request: AiRefactorRequest) -> Result<Option<AiRefactorResponse>, AiProviderError>;
    fn explain_diagnostic(&self, request: AiExplainRequest) -> Result<Option<AiExplainResponse>, AiProviderError>;
    fn generate_docstring(&self, request: AiDocstringRequest) -> Result<Option<AiDocstringResponse>, AiProviderError>;
    fn generate_stub(&self, request: AiStubRequest) -> Result<Option<AiStubResponse>, AiProviderError>;
    fn enhance_completions(&self, request: AiCompletionRequest) -> Result<Option<AiCompletionResponse>, AiProviderError>;
}
```

### Provider Capabilities {#LSPAI-CAPS}

The provider declares what it can do; features whose capability is absent are skipped for that provider.

| Capability | Description |
|---|---|
| `type_annotation` | Suggest type annotations for missing types |
| `type_error_fix` | Suggest fixes for type errors |
| `docstring_generation` | Generate docstrings from signatures and bodies |
| `stub_generation` | Generate type stubs for untyped modules |
| `diagnostic_explanation` | Explain diagnostics in natural language |
| `semantic_search` | Find symbols by intent, not just name |
| `dead_code_analysis` | Reason about code reachability beyond static analysis |
| `import_resolution` | Suggest imports for unresolved names |
| `rename_suggestion` | Suggest better symbol names based on usage |
| `refactoring` | Suggest refactoring actions |
| `completion_enhancement` | Re-rank and enhance completion items |
| `modernization` | Suggest modern Python patterns |
| `next_edit_prediction` | Predict next edit location after a user edit |
| `max_context_tokens` | Maximum context window in tokens (0 = unknown) |
| `supports_batch` | Supports efficient batch requests |
| `max_latency_ms` | Maximum acceptable latency for latency-critical features (0 = no constraint) |

### Provider Errors {#LSPAI-ERRORS}

`AiProviderError` variants: `NotConfigured`, `Transport`, `MalformedResponse`, `RateLimited(Option<Duration>)`, `Refused`, `Other`.

---

## Context Payload {#LSPAI-CONTEXT}

The AI gets the same structured data the type checker uses.

### Request/Response Types {#LSPAI-TYPES}

#### AiFixRequest / AiFixResponse {#LSPAI-TYPES-FIX}

**Request** sends: the diagnostic, surrounding source, diagnostic source line(s), inferred types in scope (`InferredTypeInfo` with symbol name, type string, and confidence: Certain/Probable/Ambiguous), call sites showing how the function is used (`CallSiteContext`), available types from imports, file path, and `is_batch` flag.

**Response** returns: a `Fix` (same structure as deterministic fixes), confidence score (0.0-1.0), reasoning string, and ranked alternative fixes.

#### AiImportRequest / AiImportResponse {#LSPAI-TYPES-IMPORT}

**Request** sends: unresolved name, usage context (Call, TypeAnnotation, AttributeAccess, BaseClass, Decorator, Other), existing imports in the file, available packages, surrounding source, file path.

**Response** returns: ranked import suggestions with import statement, package name, reasoning, and confidence.

#### AiRenameRequest / AiRenameResponse {#LSPAI-TYPES-RENAME}

**Request** sends: current name, symbol kind, symbol type, definition source, usage site snippets, file naming conventions (`NamingConventions` with function/variable/class casing styles), file path.

**Response** returns: 3-5 suggested names ranked by fit, each with reasoning and confidence.

#### AiRefactorRequest / AiRefactorResponse {#LSPAI-TYPES-REFACTOR}

**Request** sends: selected source, enclosing scope source, types in scope, optional `RefactorKind` (ExtractMethod, ExtractVariable, ConvertToDataclass, ConvertToTypedDict, SimplifyConditionals, ConvertToPatternMatch, InlineVariable, ExtractConstant, Auto), file path.

**Response** returns: suggestions with kind, title, text edits, reasoning, and confidence.

#### AiExplainRequest / AiExplainResponse {#LSPAI-TYPES-EXPLAIN}

**Request** sends: diagnostic, surrounding source, inferred types, file path.

**Response** returns: plain-language explanation, optional suggestion, optional doc link.

#### AiDocstringRequest / AiDocstringResponse {#LSPAI-TYPES-DOCSTRING}

**Request** sends: function/class source, types, call sites, docstring style preference (Google, NumPy, ReST, Auto), file path.

**Response** returns: docstring content and style used.

#### AiStubRequest / AiStubResponse {#LSPAI-TYPES-STUB}

**Request** sends: module path, module source (if available), usage patterns across the project (`StubUsagePattern`), docstrings.

**Response** returns: complete `.pyi` stub content, per-symbol confidence scores, warnings about uncertain types.

#### AiCompletionRequest / AiCompletionResponse {#LSPAI-TYPES-COMPLETION}

**Request** sends: prefix/suffix around cursor, existing deterministic completions (`CompletionSummary`), expected type, types in scope, file path.

**Response** returns: re-ordered indices into existing completions, additional AI completion items, enhanced documentation for specific items.

#### AiSemanticSearchRequest / AiSemanticSearchResponse {#LSPAI-TYPES-SEARCH}

**Request** sends: natural language query, pre-filtered symbol index (`SymbolIndexEntry` with name, kind, module path, type info, docstring, preview).

**Response** returns: ranked results with symbol index, relevance score, and reasoning.

#### AiDeadCodeRequest / AiDeadCodeResponse {#LSPAI-TYPES-DEADCODE}

**Request** sends: symbol name and kind, source code, decorators, module path, project config snippets (pyproject.toml, setup.py, framework configs).

**Response** returns: `is_dead` boolean, confidence, reasoning, optional reachability explanation.

#### AiModernizeRequest / AiModernizeResponse {#LSPAI-TYPES-MODERNIZE}

**Request** sends: source, surrounding source, project's minimum Python version, file path.

**Response** returns: suggestions with kind, title, replacement code, required Python version, reasoning, and confidence.

#### AiNextEditRequest / AiNextEditResponse {#LSPAI-TYPES-NEXTEDIT}

**Request** sends: recent diff, full file source, related symbols (`RelatedSymbol` with name, location, source, relationship), affected types, file path.

**Response** returns: ranked predictions with target location, old/new text, reasoning, and confidence.

---

## Built-in Provider Implementations {#LSPAI-PROVIDERS}

No AI SDK dependencies in core — providers use HTTP/process I/O only.

| Provider | Description | Config |
|---|---|---|
| `NoOpProvider` | Returns `is_available() = false`. Zero overhead. | Default |
| `OpenAiCompatibleProvider` | Any OpenAI-compatible API: OpenAI, Azure, Ollama, LM Studio, vLLM, llama.cpp, Groq, Together, etc. | `base_url`, optional `api_key`, `model` |
| `AnthropicProvider` | Claude models via Anthropic API. | `api_key` (from `ANTHROPIC_API_KEY` env), `model` |
| `CopilotProvider` | Proxies through GitHub Copilot agent already running in the editor. No extra API key. | Auto-detected |
| `ProcessProvider` | Any local model via subprocess. Receives JSON on stdin, returns JSON on stdout. | `command` (e.g. `["python", "my_ai_bridge.py"]`) |

---

## AI-Powered LSP Features {#LSPAI-FEATURES}

### Feature 1: AI Type Annotation Suggestions {#LSPAI-FEATURE-ANNOTATIONS}

Triggered when BSK-E0001-E0005 fires with no deterministic fix. Model receives function source, inferred types, call sites, and available types; returns a type annotation with confidence.

### Feature 2: AI Type Error Fixes {#LSPAI-FEATURE-TYPEERROR}

Triggered on imports_unresolved-E0025 with no deterministic fix. Model chooses between changing the annotation, adding a conversion, or widening the parameter type.

### Feature 3: AI-Enhanced Mass Autofix {#LSPAI-FEATURE-MASSAUTOFIX}

Mass Autofix is deterministic (see [LSP-MASS-AUTOFIX-SPEC.md §AUTOFIX-MASS](LSP-MASS-AUTOFIX-SPEC.md#AUTOFIX-MASS)); AI is an optional second pass. Deterministic safe fixes apply immediately; remaining unfixable diagnostics are batched via `suggest_fixes_batch` (single round-trip) and presented in a review panel, never auto-applied.

### Feature 4: Diagnostic Explanation {#LSPAI-FEATURE-EXPLAIN}

User triggers `"(AI) Explain this error"`; model returns a plain-language explanation. Lowest-risk feature — no code modification; suits small/local models.

### Feature 5: AI Docstring Generation {#LSPAI-FEATURE-DOCSTRING}

Model receives signature, body, inferred types, and call sites; returns a docstring in the project's preferred style (Google, NumPy, reST — detected or configured).

### Feature 6: AI Import Resolution {#LSPAI-FEATURE-IMPORT}

Called only when the deterministic resolver fails or is ambiguous. Model sees the name, usage context, existing imports (ecosystem signal: numpy imported = probably data science), and installed packages; returns ranked import suggestions, handling re-exports, `__init__.py` barrel exports, and overlapping package names.

### Feature 7: AI Rename Suggestions {#LSPAI-FEATURE-RENAME}

On F2, AI suggests better names from the symbol's body and usage alongside the rename input. The actual rename (updating references) is deterministic.

### Feature 8: AI Refactoring Suggestions {#LSPAI-FEATURE-REFACTOR}

User-initiated (select code → suggested refactorings) or proactive (background analysis surfaces hints as diagnostics). Kinds: ExtractMethod, ConvertToDataclass, ConvertToTypedDict, SimplifyConditionals, ConvertToPatternMatch, ExtractConstant.

### Feature 9: AI-Enhanced Completions {#LSPAI-FEATURE-COMPLETIONS}

Deterministic completions show immediately; AI re-ranking runs async and updates the list only if it arrives within `max_latency_ms`. AI can re-rank, add documentation, and add items marked `(AI)`.

### Feature 10: AI Stub Generation {#LSPAI-FEATURE-STUBS}

For untyped third-party packages with no stubs. LSP gathers module source, usage patterns, and docstrings; model generates `.pyi` stubs into the project's stub directory at Tier 3 provenance (lower confidence than typeshed/bundled). CLI: `basilisk stubs generate --ai thirdparty`.

### Feature 11: AI Dead Code Detection {#LSPAI-FEATURE-DEADCODE}

Before reporting a zero-reference symbol as dead, the LSP sends it to AI with decorators, module path, and config snippets, so framework-implicit references (`@app.route`, Django URLs, Click commands, pytest discovery, `__init_subclass__`, entry points) don't produce false positives. Command: `basilisk/ai/analyzeDeadCode`.

### Feature 12: AI Code Modernization {#LSPAI-FEATURE-MODERNIZE}

Deterministic rules handle simple syntax swaps (`Union[X, Y]` → `X | Y`, `typing.List` → `list`); AI handles nuanced transformations (isinstance chains → pattern match, manual `__init__` → `@dataclass`, `.format()` → f-strings, context manager classes → `@contextmanager`). Suggestions appear only if the project's target Python version supports the feature.

### Feature 13: AI Semantic Search {#LSPAI-FEATURE-SEARCH}

User searches workspace symbols by intent ("handles authentication"); AI ranks symbols pre-filtered by text similarity by semantic relevance. Command: `basilisk/ai/findByIntent`.

### Feature 14: AI Next-Edit Prediction {#LSPAI-FEATURE-NEXTEDIT}

After an edit, AI predicts follow-up edits elsewhere (e.g. updating call sites after a new parameter), shown as ghost text. Target <200ms; edit events debounced 500ms. Suits fast local models; cloud models may be too slow.

---

## Configuration {#LSPAI-CONFIG}

All AI configuration lives in `pyproject.toml` under `[tool.basilisk.ai]`. See [LSP-ARCHITECTURE-SPEC.md §LSPARCH-CONFIG](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG) for the configuration system.

```toml
[tool.basilisk.ai]
enabled = false                    # AI features are opt-in
provider = "none"                  # "none", "openai-compatible", "anthropic", "copilot", "process"

[tool.basilisk.ai.openai-compatible]
base-url = "http://localhost:11434/v1"
model = "codellama:13b"

[tool.basilisk.ai.anthropic]
model = "claude-sonnet-4-6"

[tool.basilisk.ai.process]
command = ["python", "scripts/my_ai_bridge.py"]

[tool.basilisk.ai.features]
type-annotation = true
type-error-fix = true
mass-autofix = true
diagnostic-explanation = true
semantic-search = true
dead-code-analysis = true
docstring-generation = true
stub-generation = true
import-resolution = true
rename-suggestion = true
refactoring = true
completion-enhancement = true
modernization = true
next-edit-prediction = false       # Off by default, requires fast model

[tool.basilisk.ai.context]
max-source-lines = 50
include-cross-file-call-sites = false
max-call-sites = 10
max-search-symbols = 200
max-stub-usage-patterns = 50

[tool.basilisk.ai.latency]
completion-timeout-ms = 150
next-edit-timeout-ms = 200
next-edit-debounce-ms = 500
```

### Environment Variables {#LSPAI-CONFIG-ENV}

| Variable | Description |
|---|---|
| `BASILISK_AI_API_KEY` | API key for OpenAI-compatible providers |
| `ANTHROPIC_API_KEY` | API key for Anthropic provider |
| `BASILISK_AI_PROVIDER` | Override provider selection |
| `BASILISK_AI_ENABLED` | Override enable/disable (`0` to disable in CI) |

---

## LSP Protocol Integration {#LSPAI-PROTOCOL}

### Code Actions {#LSPAI-PROTOCOL-ACTIONS}

AI code actions use distinct `CodeActionKind` values for filtering:

| Kind | Feature |
|---|---|
| `quickfix.ai.type-annotation` | 1 |
| `quickfix.ai.type-error` | 2 |
| `quickfix.ai.import` | 6 |
| `refactor.ai.docstring` | 5 |
| `refactor.ai.extract-method` | 8 |
| `refactor.ai.extract-variable` | 8 |
| `refactor.ai.extract-constant` | 8 |
| `refactor.ai.convert-dataclass` | 8 |
| `refactor.ai.simplify` | 8 |
| `refactor.ai.pattern-match` | 8 |
| `refactor.ai.modernize` | 12 |
| `source.ai.generate-stub` | 10 |

AI code actions carry `provider`, `confidence`, `reasoning`, `isAiGenerated`, and `feature` in their `data` field.

### Custom Commands {#LSPAI-PROTOCOL-COMMANDS}

| Command | Description |
|---|---|
| `basilisk/ai/suggestFix` | AI fix for a specific diagnostic |
| `basilisk/ai/suggestFixBatch` | Batch AI fix request |
| `basilisk/ai/explain` | Explain a diagnostic in plain language |
| `basilisk/ai/generateDocstring` | Generate docstring at position |
| `basilisk/ai/suggestImport` | Suggest imports for unresolved name |
| `basilisk/ai/suggestRename` | Suggest better names for a symbol |
| `basilisk/ai/suggestRefactoring` | Suggest refactoring for a code region |
| `basilisk/ai/generateStub` | Generate type stubs for untyped module |
| `basilisk/ai/findByIntent` | Semantic search — find symbols by intent |
| `basilisk/ai/analyzeDeadCode` | Analyze potential dead code in a file |
| `basilisk/ai/suggestModernization` | Suggest modern Python patterns |
| `basilisk/ai/status` | Check AI provider status |

### Status Reporting {#LSPAI-PROTOCOL-STATUS}

On initialization: enabled + available → connection message; enabled + unavailable → error; disabled → nothing.

---

## Context Truncation {#LSPAI-TRUNCATION}

For small local models, the LSP truncates context payloads by `max_context_tokens` priority:

1. **Always**: diagnostic itself, diagnostic source line(s)
2. **High**: enclosing function/class, inferred types for diagnostic symbols
3. **Medium**: call sites, available types
4. **Low**: surrounding file context beyond enclosing scope

---

## Security & Privacy {#LSPAI-SECURITY}

1. **API keys never in config files** — read from environment variables only ([LSPAI-CONFIG-ENV]).
2. **Cloud provider consent** — one-time confirmation before sending context to a non-local provider.
3. **No telemetry** about AI feature usage.
4. **Context payload inspectable** — `basilisk ai debug-context` dumps the exact payload sent.

---

## Relationship to Existing Specs {#LSPAI-RELATIONSHIPS}

| Spec | Relationship |
|---|---|
| [LSP-MASS-AUTOFIX-SPEC.md §AUTOFIX-AI](LSP-MASS-AUTOFIX-SPEC.md#AUTOFIX-AI) | Mass Autofix is deterministic and standalone. This spec adds an optional AI second pass for unfixable diagnostics. |
| [LSP-ARCHITECTURE-SPEC.md §LSPARCH-CMDS](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CMDS) | This spec extends the LSP with 13 code action kinds, 12 custom commands, and enhanced completion/symbol behavior. AI enhances, never replaces, deterministic behavior. |

---

## Testing Strategy {#LSPAI-TESTING}

AI features are tested without real models via `MockProvider` (pre-configured responses, simulated latency) and `NoOpProvider`.

| Test Category | What it checks |
|---|---|
| Provider lifecycle | `is_available()`, capability reporting, error handling |
| Context construction | Correct AST context, inferred types, call sites from `ResolvedModule` |
| Fix integration | AI fixes flow through the same `Fix` pipeline as deterministic fixes |
| Batch handling | Multiple diagnostics batched and unbatched correctly |
| Truncation | Context truncated to provider's max tokens without losing critical info |
| Code action rendering | Correct kinds, titles, data, ordering for all 13 action kinds |
| Configuration | Provider selection, env var overrides, feature toggles |
| Timeout handling | Slow providers don't block LSP; latency-critical features degrade gracefully |
| No-op default | AI disabled = zero overhead, no AI code actions |
| Feature isolation | Disabling one feature doesn't affect others; capability flags respected |

Real provider integration tested manually or with `BASILISK_AI_INTEGRATION_TEST=1`.
