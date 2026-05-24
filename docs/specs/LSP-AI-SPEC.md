# AI-Powered LSP Features — Specification {#LSPAI}

> **Goal**: Model-agnostic AI integration in the Basilisk LSP. Any model, anywhere — local, cloud, GitHub Copilot, Claude, Ollama, whatever. The LSP doesn't care what's behind the provider. It sends structured context, gets structured responses back.

AI enhances every part of the LSP — fixes, completions, refactoring, navigation, explanations, import resolution, rename suggestions, stub generation, and more. Every deterministic LSP feature works without AI. AI is the optional turbocharger, never the engine.

---

## Design Principles {#LSPAI-PRINCIPLES}

1. **Model-agnostic.** Basilisk defines a trait. Providers implement it. Swap models by changing a config line.
2. **Structured in, structured out.** The LSP sends rich analyzer context (AST, types, call graph, diagnostics). The provider returns structured fixes. No "paste this code and ask GPT to fix it."
3. **Always unsafe.** Every AI-generated fix is `FixSafety::Unsafe` and `FixSource::AiAssisted`. Never auto-applied. Always requires user confirmation.
4. **Offline-first.** AI features disabled by default. Local models work without internet. Cloud providers need explicit opt-in.
5. **Privacy-respecting.** Local models = nothing leaves. Cloud providers = user opts in, context payload is visible.
6. **No vendor lock-in.** The provider interface is the contract.

---

## Provider Abstraction {#LSPAI-PROVIDER}

### AiProvider Trait {#LSPAI-TRAIT}

The core abstraction. Every AI integration implements this. The LSP server holds a `Box<dyn AiProvider>`.

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

Not all models are equal. The provider declares what it can do.

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

**Capability profiles:**

| Model class | Example | Good at | Bad at |
|---|---|---|---|
| Large cloud | Claude Opus, GPT-4o | Everything | Cost, latency |
| Medium cloud | Claude Sonnet, GPT-4o-mini | Most features | Complex refactoring |
| Large local | Codestral 22B, DeepSeek-Coder 33B | Fixes, completions, stubs | Semantic search, next-edit |
| Small local | CodeLlama 7B, Phi-3 | Explanations, simple fixes | Refactoring, stubs, search |

### Provider Errors {#LSPAI-ERRORS}

`AiProviderError` variants: `NotConfigured`, `Transport`, `MalformedResponse`, `RateLimited(Option<Duration>)`, `Refused`, `Other`.

---

## Context Payload {#LSPAI-CONTEXT}

The AI gets the same structured data the type checker uses. Each request type sends rich context, not raw source and a prayer.

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

Triggered when BSK-E0001-E0005 fires and deterministic fix is unavailable. The model sees the function source, inferred types, call sites, and available types. Returns `data: str` with high confidence because it sees `data` passed to `json.loads()` and called with `str` arguments.

### Feature 2: AI Type Error Fixes {#LSPAI-FEATURE-TYPEERROR}

Triggered on BSK-E0010-E0025 with no deterministic fix. Model chooses between changing the annotation, adding a conversion, or widening the parameter type based on context.

### Feature 3: AI-Enhanced Mass Autofix {#LSPAI-FEATURE-MASSAUTOFIX}

Mass Autofix is a **deterministic feature** (see [LSP-MASS-AUTOFIX-SPEC.md §AUTOFIX-MASS](LSP-MASS-AUTOFIX-SPEC.md#AUTOFIX-MASS)). AI is an optional second pass for diagnostics that deterministic fixers cannot resolve. Deterministic safe fixes apply immediately. Remaining unfixable diagnostics are batched to the AI provider. AI fixes are presented in a review panel, never auto-applied. `suggest_fixes_batch` enables single-round-trip efficiency.

### Feature 4: Diagnostic Explanation {#LSPAI-FEATURE-EXPLAIN}

User triggers `"(AI) Explain this error"`. Model returns plain-language explanation. Lowest-risk AI feature — doesn't modify code. Good candidate for small/local models.

### Feature 5: AI Docstring Generation {#LSPAI-FEATURE-DOCSTRING}

Model receives function signature, body, inferred types, call sites. Returns docstring in project's preferred style (Google, NumPy, reST — detected from existing docstrings or configured).

### Feature 6: AI Import Resolution {#LSPAI-FEATURE-IMPORT}

Called only when deterministic resolver fails or returns ambiguous results. Model sees the name, usage context, existing imports (ecosystem signal: numpy imported = probably data science), and installed packages. Returns ranked import suggestions. AI handles re-exports, `__init__.py` barrel exports, and overlapping package names that defeat deterministic resolution.

### Feature 7: AI Rename Suggestions {#LSPAI-FEATURE-RENAME}

When user presses F2, AI suggests better names alongside the rename input. Model reads the function body (sees `d["username"]` and `d["email"]`), suggests `user_data`. The actual rename (updating references) is deterministic.

### Feature 8: AI Refactoring Suggestions {#LSPAI-FEATURE-REFACTOR}

User-initiated: select code, get AI-suggested refactorings (extract method, convert to dataclass, etc.). Proactive: LSP background-analyzes complex functions and surfaces refactoring hints as diagnostics.

Refactoring kinds: ExtractMethod, ConvertToDataclass, ConvertToTypedDict, SimplifyConditionals, ConvertToPatternMatch, ExtractConstant.

### Feature 9: AI-Enhanced Completions {#LSPAI-FEATURE-COMPLETIONS}

Deterministic completions show immediately. AI re-ranking happens async — if it arrives within `max_latency_ms`, the list updates; if not, the deterministic list stands. AI can re-rank, add documentation, and suggest additional items marked with `(AI)`.

### Feature 10: AI Stub Generation {#LSPAI-FEATURE-STUBS}

For untyped third-party packages with no stubs. LSP gathers module source, usage patterns, and docstrings. Model generates `.pyi` stubs written to the project's stub directory. AI stubs are Tier 3 provenance (best-effort, lower confidence than typeshed or bundled stubs). CLI: `basilisk stubs generate --ai thirdparty`.

### Feature 11: AI Dead Code Detection {#LSPAI-FEATURE-DEADCODE}

Before reporting a zero-reference symbol as dead, the LSP sends it to AI with decorators, module path, and config snippets. AI understands framework magic (`@app.route`, Django URLs, Click commands, pytest discovery, `__init_subclass__`, entry points) that static analysis can't follow. Without AI, dead code detection is nearly useless in framework-heavy Python projects.

### Feature 12: AI Code Modernization {#LSPAI-FEATURE-MODERNIZE}

Deterministic rules handle simple syntax swaps (`Union[X, Y]` to `X | Y`, `typing.List` to `list`). AI handles nuanced transformations: isinstance chains to pattern matching, manual `__init__` to `@dataclass`, `.format()` to f-strings, context manager classes to `@contextmanager`. Suggestions only appear if the project's target Python version supports the feature.

### Feature 13: AI Semantic Search {#LSPAI-FEATURE-SEARCH}

User searches workspace symbols by intent ("handles authentication"). Normal search finds nothing; AI ranks pre-filtered symbols by semantic relevance (`verify_jwt_token`, `login_handler`, `UserCredentials`). Symbol index pre-filtered by text similarity to keep context manageable. Custom command: `basilisk/ai/findByIntent`.

### Feature 14: AI Next-Edit Prediction {#LSPAI-FEATURE-NEXTEDIT}

After a user edit, AI predicts follow-up edits at other locations (e.g., adding new parameter to call sites after adding it to a function signature). Shows ghost text at predicted location. Target: <200ms. The LSP debounces edit events (500ms after last keystroke). Fast local models are ideal; cloud models may be too slow.

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

On initialization: AI enabled + available shows connection message. AI enabled + unavailable shows error. AI disabled shows nothing.

---

## Context Truncation {#LSPAI-TRUNCATION}

For small local models, the LSP truncates context payloads based on `max_context_tokens`:

1. **Always include**: diagnostic itself, diagnostic source line(s)
2. **High priority**: enclosing function/class, inferred types for symbols in the diagnostic
3. **Medium priority**: call sites, available types
4. **Low priority**: surrounding file context beyond enclosing scope

---

## Security & Privacy {#LSPAI-SECURITY}

1. **API keys never in config files.** Always environment variables.
2. **Local models = zero data exfiltration.**
3. **Cloud provider consent.** One-time confirmation before sending code context.
4. **No telemetry.** Basilisk collects nothing about AI feature usage.
5. **Context payload is inspectable.** `basilisk ai debug-context` CLI dumps the exact payload.

---

## Relationship to Existing Specs {#LSPAI-RELATIONSHIPS}

| Spec | Relationship |
|---|---|
| [LSP-MASS-AUTOFIX-SPEC.md §AUTOFIX-AI](LSP-MASS-AUTOFIX-SPEC.md#AUTOFIX-AI) | Mass Autofix is deterministic and standalone. This spec adds an optional AI second pass for unfixable diagnostics. |
| [LSP-ARCHITECTURE-SPEC.md §LSPARCH-CMDS](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CMDS) | This spec extends the LSP with 13 code action kinds, 12 custom commands, and enhanced completion/symbol behavior. AI enhances, never replaces, deterministic behavior. |

---

## Testing Strategy {#LSPAI-TESTING}

AI features are tested without real AI models via `MockProvider` (pre-configured responses, simulated latency) and `NoOpProvider`.

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
