# AI-Powered LSP Features — Specification

> **Goal**: Model-agnostic AI integration in the Basilisk LSP. Any model, anywhere — local, cloud, GitHub Copilot, Claude, Ollama, whatever. The LSP doesn't care what's behind the provider. It sends structured context, gets structured responses back.

This spec defines the AI layer in Basilisk's LSP server. AI enhances every part of the LSP — fixes, completions, refactoring, navigation, explanations, import resolution, rename suggestions, stub generation, and more. Every deterministic LSP feature works without AI. AI is the optional turbocharger, never the engine.

---

## Design Principles

1. **Model-agnostic.** Basilisk never imports an AI SDK. It defines a trait. Providers implement it. Swap models by changing a config line.
2. **Structured in, structured out.** The LSP sends rich analyzer context (AST, types, call graph, diagnostics). The provider returns structured fixes. No "paste this code and ask GPT to fix it" — the AI gets the same context the type checker has.
3. **Always unsafe.** Every AI-generated fix is `FixSafety::Unsafe` and `FixSource::AiAssisted`. Never auto-applied. Always requires user confirmation.
4. **Offline-first.** AI features are disabled by default. The LSP works without them. Local models work without internet. Cloud providers need explicit opt-in.
5. **Privacy-respecting.** The user controls what leaves their machine. Local models = nothing leaves. Cloud providers = user opts in, context payload is visible.
6. **No vendor lock-in.** The provider interface is the contract. Claude, GPT, Gemini, Copilot, Ollama, LM Studio, llama.cpp — all plug in the same way.

---

## Provider Abstraction

### The `AiProvider` Trait

The core abstraction. Every AI integration implements this. The LSP server holds a `Box<dyn AiProvider>` and doesn't know or care what's behind it.

```rust
/// Model-agnostic AI provider interface.
/// Implementations handle authentication, transport, prompt construction,
/// and response parsing. The LSP only sees this trait.
///
/// The provider receives structured requests with full analyzer context
/// and returns structured responses. Every method has a specific request/response
/// type — this is not a generic "send prompt, get text back" interface.
pub trait AiProvider: Send + Sync {
    /// Human-readable provider name for UI display.
    /// Examples: "Claude via API", "Ollama (codellama)", "GitHub Copilot"
    fn name(&self) -> &str;

    /// Whether this provider is configured and ready to accept requests.
    /// Called before every AI feature invocation. If false, the feature
    /// falls back to non-AI behavior silently.
    fn is_available(&self) -> bool;

    /// Provider-reported capabilities. The LSP uses this to decide
    /// which AI features to offer in the UI.
    fn capabilities(&self) -> AiProviderCapabilities;

    // --- Fix suggestions ---

    /// Request a fix for a single diagnostic with full analyzer context.
    /// Returns None if the model declines or cannot produce a fix.
    fn suggest_fix(
        &self,
        request: AiFixRequest,
    ) -> Result<Option<AiFixResponse>, AiProviderError>;

    /// Request fixes for multiple diagnostics in a batch.
    /// Default implementation calls suggest_fix in sequence.
    /// Providers can override for efficient batching (single API call).
    fn suggest_fixes_batch(
        &self,
        requests: Vec<AiFixRequest>,
    ) -> Result<Vec<Option<AiFixResponse>>, AiProviderError> {
        requests.into_iter()
            .map(|req| self.suggest_fix(req))
            .collect()
    }

    // --- Import resolution ---

    /// Suggest imports for an unresolved name. The deterministic resolver
    /// checks the workspace and installed packages first. This is called
    /// only when deterministic resolution fails or is ambiguous.
    fn suggest_import(
        &self,
        request: AiImportRequest,
    ) -> Result<Option<AiImportResponse>, AiProviderError>;

    // --- Rename suggestions ---

    /// Suggest better names for a symbol based on its usage context.
    /// Triggered when the user initiates a rename — the AI provides
    /// name suggestions alongside the user's own choice.
    fn suggest_rename(
        &self,
        request: AiRenameRequest,
    ) -> Result<Option<AiRenameResponse>, AiProviderError>;

    // --- Refactoring ---

    /// Suggest refactoring actions for a selected code region.
    /// Extract method, convert to dataclass, simplify conditionals, etc.
    fn suggest_refactoring(
        &self,
        request: AiRefactorRequest,
    ) -> Result<Option<AiRefactorResponse>, AiProviderError>;

    // --- Explanations ---

    /// Explain a diagnostic in plain language.
    fn explain_diagnostic(
        &self,
        request: AiExplainRequest,
    ) -> Result<Option<AiExplainResponse>, AiProviderError>;

    // --- Docstrings ---

    /// Generate a docstring for a function, class, or module.
    fn generate_docstring(
        &self,
        request: AiDocstringRequest,
    ) -> Result<Option<AiDocstringResponse>, AiProviderError>;

    // --- Stub generation ---

    /// Generate type stubs for an untyped module based on runtime
    /// analysis, usage patterns, and documentation.
    fn generate_stub(
        &self,
        request: AiStubRequest,
    ) -> Result<Option<AiStubResponse>, AiProviderError>;

    // --- Enhanced completions ---

    /// Enhance completion items with AI-powered context awareness.
    /// Called after the deterministic completer produces its list.
    /// The AI can re-rank, add detail, or suggest items the
    /// deterministic completer missed.
    fn enhance_completions(
        &self,
        request: AiCompletionRequest,
    ) -> Result<Option<AiCompletionResponse>, AiProviderError>;
}
```

### Provider Capabilities

Not all models are equal. A 7B local model shouldn't be asked to do complex cross-file refactoring. The provider declares what it can do.

```rust
pub struct AiProviderCapabilities {
    // --- Core fix capabilities ---

    /// Can suggest type annotations for missing types
    pub type_annotation: bool,
    /// Can suggest fixes for type errors (mismatches, incompatible types)
    pub type_error_fix: bool,

    // --- Generation capabilities ---

    /// Can generate docstrings from function signatures and bodies
    pub docstring_generation: bool,
    /// Can generate type stubs for untyped modules
    pub stub_generation: bool,

    // --- Comprehension capabilities ---

    /// Can explain diagnostics in natural language
    pub diagnostic_explanation: bool,
    /// Can find symbols by intent, not just name ("authentication" → verify_jwt_token)
    pub semantic_search: bool,
    /// Can reason about code reachability beyond static analysis
    pub dead_code_analysis: bool,

    // --- Enhancement capabilities ---

    /// Can suggest imports for unresolved names
    pub import_resolution: bool,
    /// Can suggest better symbol names based on usage context
    pub rename_suggestion: bool,
    /// Can suggest refactoring actions (extract method, convert to dataclass, etc.)
    pub refactoring: bool,
    /// Can re-rank and enhance completion items with context awareness
    pub completion_enhancement: bool,
    /// Can suggest modern Python patterns to replace legacy code
    pub modernization: bool,
    /// Can predict the next edit location and change after a user edit
    pub next_edit_prediction: bool,

    // --- Operational ---

    /// Maximum context window in tokens (0 = unknown/unlimited)
    /// Used to truncate context payloads for small models
    pub max_context_tokens: usize,
    /// Whether the provider supports batch requests efficiently
    pub supports_batch: bool,
    /// Maximum acceptable latency in ms for latency-critical features
    /// (completions, next-edit). 0 = no constraint. If the provider
    /// can't respond within this window, latency-critical features
    /// are disabled for it.
    pub max_latency_ms: u32,
}
```

**Capability profiles** — common configurations for well-known model classes:

| Model class | Example | Good at | Bad at | Typical profile |
|-------------|---------|---------|--------|-----------------|
| Large cloud | Claude Opus, GPT-4o | Everything | Cost, latency | All caps on, high latency tolerance |
| Medium cloud | Claude Sonnet, GPT-4o-mini | Most features | Complex refactoring | Most caps on, moderate latency |
| Large local | Codestral 22B, DeepSeek-Coder 33B | Fixes, completions, stubs | Semantic search, next-edit | Core caps on, latency-sensitive caps off |
| Small local | CodeLlama 7B, Phi-3 | Explanations, simple fixes | Refactoring, stubs, search | Only explanation + simple fixes |

Providers should set capabilities honestly. The LSP trusts these flags — it won't offer features the provider can't handle.

### Provider Errors

```rust
pub enum AiProviderError {
    /// Provider is not configured or credentials are missing
    NotConfigured(String),
    /// Network/transport error (timeout, connection refused, etc.)
    Transport(String),
    /// Model returned an unparseable response
    MalformedResponse(String),
    /// Rate limited — includes retry-after duration if available
    RateLimited(Option<Duration>),
    /// Model explicitly refused (content policy, etc.)
    Refused(String),
    /// Provider-specific error
    Other(String),
}
```

---

## Context Payload

The AI doesn't get raw source and a prayer. It gets the same structured data the type checker uses.

### `AiFixRequest`

```rust
pub struct AiFixRequest {
    /// The diagnostic to fix
    pub diagnostic: Diagnostic,

    /// AST context: the function/class/module surrounding the diagnostic.
    /// Serialized as source text with span markers, not raw AST nodes.
    pub surrounding_source: String,

    /// The specific line(s) containing the diagnostic
    pub diagnostic_source: String,

    /// All type information Basilisk has inferred for symbols in scope.
    /// Includes parameter types, return types, variable types.
    pub inferred_types: Vec<InferredTypeInfo>,

    /// Call sites referencing the target symbol (if the diagnostic is
    /// about a function parameter or return type). Shows how the
    /// function is actually called, giving the model usage evidence.
    pub call_sites: Vec<CallSiteContext>,

    /// Types available in scope (imports, builtins, locally defined).
    /// The model should pick from these rather than inventing types.
    pub available_types: Vec<AvailableType>,

    /// The file path (relative to workspace root) for context
    pub file_path: String,

    /// Whether this request is part of a batch (mass autofix).
    /// Providers may use this to adjust response style.
    pub is_batch: bool,
}

pub struct InferredTypeInfo {
    pub symbol_name: String,
    pub inferred_type: String,    // e.g. "int", "Optional[str]", "list[int]"
    pub confidence: TypeConfidence,
}

pub enum TypeConfidence {
    /// Type is known from annotation or unambiguous inference
    Certain,
    /// Type is inferred from usage but could be wrong
    Probable,
    /// Multiple possible types, can't narrow further
    Ambiguous,
}

pub struct CallSiteContext {
    /// Source text of the call expression
    pub call_source: String,
    /// Types of arguments at this call site (if known)
    pub arg_types: Vec<Option<String>>,
}

pub struct AvailableType {
    /// The type name as it would appear in an annotation
    pub name: String,
    /// Where this type comes from
    pub source: AvailableTypeSource,
}

pub enum AvailableTypeSource {
    Builtin,
    Import(String),       // module path
    LocalDefinition,
    TypeStub,
}
```

### `AiFixResponse`

```rust
pub struct AiFixResponse {
    /// The proposed fix — text edits to apply.
    /// Uses the same Fix structure as deterministic fixes.
    pub fix: Fix,

    /// Model's confidence in this fix (0.0 - 1.0).
    /// The LSP uses this for ranking and display.
    pub confidence: f32,

    /// Human-readable explanation of why this fix was chosen.
    /// Shown in the code action detail or hover.
    pub reasoning: String,

    /// Alternative fixes the model considered, ranked by confidence.
    /// Shown as secondary code actions.
    pub alternatives: Vec<AlternativeFix>,
}

pub struct AlternativeFix {
    pub fix: Fix,
    pub confidence: f32,
    pub reasoning: String,
}
```

### `AiImportRequest` / `AiImportResponse`

```rust
pub struct AiImportRequest {
    /// The unresolved name (e.g. "DataFrame", "patch", "HTTPException")
    pub unresolved_name: String,

    /// How the name is used — the model needs this to disambiguate.
    /// "DataFrame(data)" is a constructor call. "x: DataFrame" is a type annotation.
    pub usage_context: ImportUsageContext,

    /// The file's existing imports — the model uses these to infer the ecosystem.
    /// If the file already imports numpy, "DataFrame" is probably pandas, not pyspark.
    pub existing_imports: Vec<String>,

    /// Known installed packages in the environment (from pip list / pyproject.toml)
    pub available_packages: Vec<String>,

    /// The surrounding source where the name appears
    pub surrounding_source: String,

    pub file_path: String,
}

pub enum ImportUsageContext {
    /// Used as a function/constructor call: `Foo(args)`
    Call(String),
    /// Used as a type annotation: `x: Foo`
    TypeAnnotation,
    /// Used as attribute access: `Foo.bar`
    AttributeAccess(String),
    /// Used as a base class: `class Bar(Foo):`
    BaseClass,
    /// Used in a decorator: `@Foo`
    Decorator,
    /// Other usage
    Other(String),
}

pub struct AiImportResponse {
    /// Ranked import suggestions. First = highest confidence.
    pub suggestions: Vec<ImportSuggestion>,
}

pub struct ImportSuggestion {
    /// The full import statement: "from pandas import DataFrame"
    pub import_statement: String,
    /// The package this comes from: "pandas"
    pub package: String,
    /// Why this import was chosen
    pub reasoning: String,
    pub confidence: f32,
}
```

### `AiRenameRequest` / `AiRenameResponse`

```rust
pub struct AiRenameRequest {
    /// Current symbol name
    pub current_name: String,
    /// Symbol kind (function, variable, parameter, class, method, module)
    pub symbol_kind: SymbolKind,
    /// The symbol's type, if known
    pub symbol_type: Option<String>,
    /// Source of the definition site
    pub definition_source: String,
    /// How the symbol is used — source snippets from usage sites
    pub usage_sites: Vec<String>,
    /// Naming conventions observed in the file (snake_case, camelCase, etc.)
    pub file_conventions: NamingConventions,

    pub file_path: String,
}

pub struct NamingConventions {
    /// Dominant casing style for functions in this file
    pub function_style: CasingStyle,
    /// Dominant casing style for variables
    pub variable_style: CasingStyle,
    /// Dominant casing style for classes
    pub class_style: CasingStyle,
}

pub enum CasingStyle {
    SnakeCase,
    CamelCase,
    PascalCase,
    ScreamingSnakeCase,
    Mixed,
}

pub struct AiRenameResponse {
    /// 3-5 suggested names, ranked by fit. First = best.
    pub suggestions: Vec<RenameSuggestion>,
}

pub struct RenameSuggestion {
    pub name: String,
    /// Why this name fits
    pub reasoning: String,
    pub confidence: f32,
}
```

### `AiRefactorRequest` / `AiRefactorResponse`

```rust
pub struct AiRefactorRequest {
    /// The selected code region (or the construct under cursor)
    pub selected_source: String,
    /// The full enclosing scope (function, class, or module)
    pub enclosing_source: String,
    /// Type information for symbols in the selected region
    pub types_in_scope: Vec<InferredTypeInfo>,
    /// What kind of refactoring the user requested (or None for "suggest anything")
    pub requested_kind: Option<RefactorKind>,

    pub file_path: String,
}

pub enum RefactorKind {
    ExtractMethod,
    ExtractVariable,
    ConvertToDataclass,
    ConvertToTypedDict,
    SimplifyConditionals,
    ConvertToPatternMatch,
    InlineVariable,
    ExtractConstant,
    /// Let the model decide what refactoring to suggest
    Auto,
}

pub struct AiRefactorResponse {
    /// One or more refactoring suggestions
    pub suggestions: Vec<RefactorSuggestion>,
}

pub struct RefactorSuggestion {
    pub kind: RefactorKind,
    /// Human-readable title: "Extract method `validate_input`"
    pub title: String,
    /// The edits to apply
    pub edits: Vec<TextEdit>,
    /// Why this refactoring improves the code
    pub reasoning: String,
    pub confidence: f32,
}
```

### `AiExplainRequest` / `AiExplainResponse`

```rust
pub struct AiExplainRequest {
    pub diagnostic: Diagnostic,
    pub surrounding_source: String,
    pub inferred_types: Vec<InferredTypeInfo>,
    pub file_path: String,
}

pub struct AiExplainResponse {
    /// Plain-language explanation of the diagnostic
    pub explanation: String,
    /// Optional: what the user should do about it
    pub suggestion: Option<String>,
    /// Optional: link to relevant documentation
    pub doc_link: Option<String>,
}
```

### `AiDocstringRequest` / `AiDocstringResponse`

```rust
pub struct AiDocstringRequest {
    /// The function/class/module source
    pub source: String,
    /// Inferred or annotated types for parameters and return
    pub types: Vec<InferredTypeInfo>,
    /// Call sites showing how the function is used
    pub call_sites: Vec<CallSiteContext>,
    /// Docstring style preference (detected or configured)
    pub style: DocstringStyle,

    pub file_path: String,
}

pub enum DocstringStyle {
    /// Google style: Args: / Returns: / Raises:
    Google,
    /// NumPy style: Parameters / Returns / Raises sections
    NumPy,
    /// reStructuredText: :param name: / :returns: / :raises:
    ReST,
    /// Auto-detect from existing docstrings in the file
    Auto,
}

pub struct AiDocstringResponse {
    /// The generated docstring content (without the triple quotes)
    pub docstring: String,
    /// The style used (relevant when Auto was requested)
    pub style_used: DocstringStyle,
}
```

### `AiStubRequest` / `AiStubResponse`

```rust
pub struct AiStubRequest {
    /// The module to generate stubs for: "requests.models"
    pub module_path: String,
    /// The module's source code, if available (installed package, readable .py)
    pub module_source: Option<String>,
    /// How symbols from this module are used across the project.
    /// The model uses this to infer types even without source.
    pub usage_patterns: Vec<StubUsagePattern>,
    /// Any docstrings found in the module
    pub docstrings: Vec<(String, String)>,  // (symbol_name, docstring)
}

pub struct StubUsagePattern {
    /// The symbol being used: "Response"
    pub symbol_name: String,
    /// How it's used: "resp = requests.get(url); resp.json()"
    pub usage_source: String,
    /// Types inferred at the usage site
    pub inferred_types: Vec<InferredTypeInfo>,
}

pub struct AiStubResponse {
    /// The complete .pyi stub file content
    pub stub_content: String,
    /// Confidence per symbol (some symbols may be well-understood, others guessed)
    pub symbol_confidences: Vec<(String, f32)>,
    /// Warnings about uncertain types
    pub warnings: Vec<String>,
}
```

### `AiCompletionRequest` / `AiCompletionResponse`

```rust
pub struct AiCompletionRequest {
    /// Source text before the cursor (within the enclosing scope)
    pub prefix: String,
    /// Source text after the cursor (within the enclosing scope)
    pub suffix: String,
    /// The deterministic completion items the LSP already produced
    pub existing_completions: Vec<CompletionSummary>,
    /// Type of the expression being completed (if known)
    pub expected_type: Option<String>,
    /// Current scope's type information
    pub types_in_scope: Vec<InferredTypeInfo>,

    pub file_path: String,
}

pub struct CompletionSummary {
    /// The completion label: "json"
    pub label: String,
    /// The completion kind: Method, Property, Variable, etc.
    pub kind: CompletionItemKind,
    /// Type of the completion item (if known)
    pub item_type: Option<String>,
}

pub struct AiCompletionResponse {
    /// Re-ordered indices into existing_completions (most relevant first).
    /// Items not in this list keep their original order after the re-ranked ones.
    pub reranked_indices: Vec<usize>,
    /// Additional completion items the AI wants to suggest.
    /// These appear after re-ranked deterministic items with "(AI)" prefix.
    pub additional_items: Vec<AiCompletionItem>,
    /// Enhanced documentation for specific items (keyed by index into existing_completions)
    pub enhanced_docs: HashMap<usize, String>,
}

pub struct AiCompletionItem {
    pub label: String,
    pub detail: String,
    pub insert_text: String,
    pub kind: CompletionItemKind,
    pub reasoning: String,
}
```

### `AiSemanticSearchRequest` / `AiSemanticSearchResponse`

```rust
pub struct AiSemanticSearchRequest {
    /// The natural language query: "error handling", "database connection setup"
    pub query: String,
    /// The workspace symbol index — names, kinds, types, docstrings, module paths.
    /// Pre-filtered to top N by text similarity to keep context manageable.
    pub symbol_index: Vec<SymbolIndexEntry>,
}

pub struct SymbolIndexEntry {
    pub name: String,
    pub kind: SymbolKind,
    pub module_path: String,
    pub type_info: Option<String>,
    pub docstring: Option<String>,
    /// First few lines of the definition
    pub preview: String,
}

pub struct AiSemanticSearchResponse {
    /// Symbols matching the query by intent, ranked by relevance
    pub results: Vec<SemanticSearchResult>,
}

pub struct SemanticSearchResult {
    /// Index into the symbol_index
    pub symbol_index: usize,
    /// Relevance score (0.0 - 1.0)
    pub relevance: f32,
    /// Why this symbol matches the query
    pub reasoning: String,
}
```

### `AiDeadCodeRequest` / `AiDeadCodeResponse`

```rust
pub struct AiDeadCodeRequest {
    /// The symbol that static analysis found with zero references
    pub symbol_name: String,
    pub symbol_kind: SymbolKind,
    /// The symbol's source code
    pub source: String,
    /// Decorators on the symbol (might indicate framework registration)
    pub decorators: Vec<String>,
    /// The module path: "myapp.views"
    pub module_path: String,
    /// Project configuration snippets that might reference this symbol:
    /// pyproject.toml entry points, setup.py, framework configs, etc.
    pub project_config_snippets: Vec<ConfigSnippet>,
}

pub struct ConfigSnippet {
    /// Which config file: "pyproject.toml", "setup.py", "urls.py"
    pub file_path: String,
    /// The relevant section of the config
    pub content: String,
}

pub struct AiDeadCodeResponse {
    /// Is this code truly dead?
    pub is_dead: bool,
    /// Confidence (0.0 - 1.0)
    pub confidence: f32,
    /// Why the model thinks this is or isn't dead code
    pub reasoning: String,
    /// If not dead: how the code is reached (entry point, framework hook, etc.)
    pub reachability_explanation: Option<String>,
}
```

### `AiModernizeRequest` / `AiModernizeResponse`

```rust
pub struct AiModernizeRequest {
    /// The code pattern to modernize
    pub source: String,
    /// The enclosing scope for context
    pub surrounding_source: String,
    /// The project's minimum Python version target
    pub python_version: (u32, u32),  // e.g. (3, 12)

    pub file_path: String,
}

pub struct AiModernizeResponse {
    pub suggestions: Vec<ModernizeSuggestion>,
}

pub struct ModernizeSuggestion {
    /// What kind of modernization: "union-syntax", "pattern-match", "f-string", etc.
    pub kind: String,
    /// Human-readable title: "Use X | Y instead of Union[X, Y]"
    pub title: String,
    /// The modernized code
    pub replacement: String,
    /// What Python version introduced this feature
    pub requires_python: (u32, u32),
    /// Why the modern version is better
    pub reasoning: String,
    pub confidence: f32,
}
```

### `AiNextEditRequest` / `AiNextEditResponse`

```rust
pub struct AiNextEditRequest {
    /// The diff of the user's most recent edit
    pub recent_diff: String,
    /// The full file source after the edit
    pub file_source: String,
    /// Symbols related to the edited code (references, call sites, implementations)
    pub related_symbols: Vec<RelatedSymbol>,
    /// Type information for symbols affected by the edit
    pub affected_types: Vec<InferredTypeInfo>,

    pub file_path: String,
}

pub struct RelatedSymbol {
    /// The symbol name
    pub name: String,
    /// Where it's defined
    pub file_path: String,
    pub line: u32,
    /// Source snippet around the symbol
    pub source: String,
    /// Why it's related: "calls edited function", "implements same interface", etc.
    pub relationship: String,
}

pub struct AiNextEditResponse {
    /// Predicted next edits, ranked by likelihood.
    /// The editor shows the top prediction as ghost text at the target location.
    pub predictions: Vec<NextEditPrediction>,
}

pub struct NextEditPrediction {
    /// Where the next edit should happen
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    /// What to change: the old text and new text
    pub old_text: String,
    pub new_text: String,
    /// Why this edit is needed
    pub reasoning: String,
    pub confidence: f32,
}
```

---

## Built-in Provider Implementations

Basilisk ships with provider implementations for common backends. No AI SDK dependencies in the core — providers use HTTP/process I/O only.

### `NoOpProvider` (Default)

```rust
pub struct NoOpProvider;
```

Returns `is_available() = false`. All AI features silently disabled. Zero overhead.

### `OpenAiCompatibleProvider`

Covers any model behind an OpenAI-compatible API: OpenAI, Azure OpenAI, Ollama, LM Studio, vLLM, llama.cpp server, Groq, Together, Fireworks, etc.

```rust
pub struct OpenAiCompatibleProvider {
    base_url: String,           // e.g. "http://localhost:11434/v1" for Ollama
    api_key: Option<String>,    // None for local models
    model: String,              // e.g. "codellama", "gpt-4o", "deepseek-coder"
    capabilities: AiProviderCapabilities,
}
```

Configuration:

```toml
[tool.basilisk.ai]
enabled = true
provider = "openai-compatible"
base-url = "http://localhost:11434/v1"    # Ollama
model = "codellama:13b"
# api-key via BASILISK_AI_API_KEY env var (never in config files)
```

### `AnthropicProvider`

Claude models via the Anthropic API.

```rust
pub struct AnthropicProvider {
    api_key: String,            // from ANTHROPIC_API_KEY env var
    model: String,              // e.g. "claude-sonnet-4-6"
    capabilities: AiProviderCapabilities,
}
```

### `CopilotProvider`

GitHub Copilot integration. Uses the Copilot LSP's existing authentication and model routing — no separate API key needed if the user has Copilot active.

```rust
pub struct CopilotProvider {
    /// Path to the Copilot agent binary or socket
    copilot_agent: CopilotAgentConnection,
    capabilities: AiProviderCapabilities,
}
```

This provider proxies through the GitHub Copilot agent that VS Code / Neovim already runs. If Copilot is installed and authenticated, Basilisk can use it. No extra setup.

### `ProcessProvider`

Run any local model via a subprocess. For users who have their own inference setup (llama.cpp CLI, custom scripts, etc.).

```rust
pub struct ProcessProvider {
    /// Command to execute. Receives JSON on stdin, returns JSON on stdout.
    command: Vec<String>,       // e.g. ["python", "my_ai_bridge.py"]
    capabilities: AiProviderCapabilities,
}
```

The subprocess receives an `AiFixRequest` as JSON on stdin and writes an `AiFixResponse` as JSON to stdout. Simple, universal, works with anything.

---

## AI-Powered LSP Features

### Feature 1: AI Type Annotation Suggestions

**When**: A diagnostic fires for a missing type annotation (BSK-E0001 through BSK-E0005) and the deterministic fix provider either has no fix or only has a low-confidence heuristic.

**Flow**:

1. User sees diagnostic squiggle.
2. User opens code actions (lightbulb).
3. If AI provider is available, an additional code action appears: `"(AI) Add type annotation"` (or multiple if the model suggests alternatives).
4. User selects the AI fix. It's applied as an unsafe edit (can undo).

**Context sent to model**: The function/class source, all inferred types in scope, call sites showing how the parameter is used, available types from imports.

**Why this works**: The model isn't guessing blind. It sees that `data` is passed to `json.loads()` in the function body, that the function is called with `str` arguments in 3 places, and that `str` is available in scope. It returns `data: str` with high confidence.

### Feature 2: AI Type Error Fixes

**When**: A type error diagnostic (BSK-E0010 through BSK-E0025) fires and there's no deterministic fix.

**Flow**:

1. Diagnostic: `BSK-E0015: Argument type "str" is not assignable to parameter type "int"`.
2. AI code action: `"(AI) Fix type error"`.
3. Model sees the mismatch, the surrounding code, and suggests either:
   - Change the annotation (if the annotation is wrong)
   - Add a conversion (if the code is wrong)
   - Widen the parameter type (if the function should accept both)

**Safety**: Always `Unsafe`. The model might suggest `int(x)` when the real fix is changing the annotation. User reviews.

### Feature 3: AI-Enhanced Mass Autofix (Optional Enhancement)

Mass Autofix is a **deterministic feature** defined in `LSP-MASS-AUTOFIX-SPEC.md`. It works without AI. AI is an optional second pass that handles the leftovers — diagnostics that deterministic fixers can't resolve.

**When**: User triggers mass autofix, deterministic fixes have already been applied, and there are remaining diagnostics with no deterministic fix available. AI is enabled and a provider is configured.

**Flow**:

1. User triggers `Basilisk: Fix All in File`.
2. **Deterministic safe fixes are applied immediately** — this is the core mass autofix, completely independent of AI. No change from existing behavior.
3. If AI is enabled: remaining unfixable diagnostics are batched and sent to the AI provider.
4. AI-suggested fixes are presented in a review panel — NOT auto-applied. They are always `Unsafe` and always require confirmation.
5. User reviews each AI fix (accept/reject/modify), with the model's reasoning shown.
6. If AI is disabled: step 2 is the end. Mass autofix works exactly as specified in `LSP-MASS-AUTOFIX-SPEC.md`.

**Batch efficiency**: The `suggest_fixes_batch` method lets providers send all remaining diagnostics in one API call. For cloud models, this means one round-trip instead of N.

### Feature 4: Diagnostic Explanation

**When**: User hovers over a diagnostic or explicitly requests explanation.

**Flow**:

1. User sees `BSK-E0012: Type "list[Any]" has implicit "Any" type argument`.
2. User triggers `"(AI) Explain this error"` from code actions or hover.
3. Model receives the diagnostic + context and returns a plain-language explanation:
   > "This error means your list doesn't specify what type of items it contains. Python's type system requires explicit type parameters. Change `list` to `list[str]` if the list contains strings, or `list[int]` for integers."

**This is the lowest-risk AI feature.** It doesn't modify code. It only explains. Good candidate for small/local models.

### Feature 5: AI Docstring Generation

**When**: User requests docstring generation for a function or class.

**Flow**:

1. Cursor is on a function definition.
2. Code action: `"(AI) Generate docstring"`.
3. Model receives the function signature, body, inferred types, and call sites.
4. Returns a docstring in the project's preferred style (Google, NumPy, reST — detected from existing docstrings or configured).

### Feature 6: AI Import Resolution

**Enhances**: deterministic import resolver (`completion.rs`, cross-module analysis)

**When**: An unresolved name diagnostic fires and the deterministic resolver either found nothing or returned multiple ambiguous candidates.

**Flow**:

1. User writes `df = DataFrame(data)`. Basilisk fires `BSK-E0030: Name "DataFrame" is not defined`.
2. Deterministic resolver checks workspace modules and installed package indexes. If it finds a unique match, it handles it — no AI needed.
3. If no match or multiple matches: AI code action appears: `"(AI) Add import for DataFrame"`.
4. Model sees the name, how it's used (constructor call with a `data` argument), the file's existing imports (`import numpy as np`, `import json`), and the installed packages.
5. Model returns ranked suggestions: `from pandas import DataFrame` (confidence: 0.92), `from pyspark.sql import DataFrame` (confidence: 0.35).
6. User picks one. The import is inserted at the correct location (respecting isort ordering).

**Why AI is better here**: Deterministic resolution requires the full cross-module symbol graph and struggles with re-exported symbols, `__init__.py` barrel exports, and packages with overlapping names. The AI can look at ecosystem context (numpy is imported → this is probably a data science file → DataFrame is pandas) and get it right without full resolution.

**Safety**: `Unsafe`. Wrong import = wrong dependency. User reviews.

### Feature 7: AI Rename Suggestions

**Enhances**: deterministic rename (`references.rs`, F2 in editors)

**When**: User initiates a rename. The rename box opens. Below the box (or in a dropdown), AI-suggested names appear.

**Flow**:

1. User presses F2 on `d` in `def process(d: dict[str, Any])`.
2. LSP sends the symbol info, its type, the function body (where `d` is used), and the file's naming conventions to the AI.
3. Model returns suggestions: `data` (0.88), `mapping` (0.72), `params` (0.65).
4. The editor shows these as clickable suggestions below the rename input. User can pick one or type their own name.
5. The actual rename (updating all references) is deterministic — the LSP does it. The AI only suggests the name.

**Why AI is better here**: A single-character variable name tells you nothing. The AI reads the function body, sees `d["username"]` and `d["email"]`, and suggests `user_data`. No deterministic heuristic can do this.

**Safety**: Safe-ish — it only suggests a name. The rename itself is deterministic. But the user still confirms.

### Feature 8: AI Refactoring Suggestions

**Enhances**: deterministic code actions (`code_actions/`)

**When**: User selects a code region and requests code actions, or the AI proactively identifies refactoring opportunities.

**Flow (user-initiated)**:

1. User selects 15 lines of validation logic inside a function.
2. Code actions include: `"(AI) Extract method"`, `"(AI) Convert to dataclass"`, etc.
3. Model sees the selected code, the enclosing function, types in scope.
4. For "Extract method": model picks a name (`validate_user_input`), determines the parameter list from variables used in the selection, determines the return type, and produces the extracted method + call site.
5. User reviews the refactored code before applying.

**Flow (proactive)**:

1. The LSP background-analyzes functions over a configurable complexity threshold.
2. AI identifies: "This 40-line function has 3 distinct responsibilities. Consider extracting `parse_config()` (lines 5-15) and `validate_config()` (lines 16-30)."
3. These appear as hint-level diagnostics with AI code actions attached.

**Refactoring kinds**:

| Kind | What it does | Example |
|------|-------------|---------|
| `ExtractMethod` | Pull selected code into a new function with correct signature | 15 lines of parsing → `parse_response(data: dict) -> User` |
| `ConvertToDataclass` | Replace a class with manual `__init__` and attrs to a `@dataclass` | Class with 5 attrs and `__init__` → 5-line dataclass |
| `ConvertToTypedDict` | Replace dict literal patterns with a `TypedDict` | `{"name": str, "age": int}` pattern → `class UserInfo(TypedDict)` |
| `SimplifyConditionals` | Flatten nested if/elif/else chains | 4-level nesting → guard clauses with early return |
| `ConvertToPatternMatch` | Replace isinstance chains with `match`/`case` | 5 isinstance checks → structural pattern matching |
| `ExtractConstant` | Pull magic values into named constants | `if retries > 3` → `MAX_RETRIES = 3; if retries > MAX_RETRIES` |

**Safety**: Always `Unsafe`. Refactoring changes behavior if the AI gets the boundaries wrong.

### Feature 9: AI-Enhanced Completions

**Enhances**: deterministic completion (`completion.rs`)

**When**: The deterministic completer has produced its list. AI enhances it — re-ranks, adds documentation, suggests additional items.

**Flow**:

1. User types `response.` after `response = requests.get(url)`.
2. Deterministic completer produces: `status_code`, `text`, `json`, `headers`, `content`, `cookies`, `url`, `encoding`, `elapsed`, etc.
3. AI sees the cursor context: inside a `try:` block, the next line is `data = `. It re-ranks `json` to the top (the user is probably about to parse JSON), adds enhanced documentation ("Returns the JSON-decoded content of the response. Raises `ValueError` if the response body does not contain valid JSON."), and adds `raise_for_status` as a suggestion ("Consider calling this before accessing response data").
4. The completion list shows: deterministic items re-ranked by AI, followed by any AI-only additions marked with `(AI)`.

**Latency contract**: Completions must be fast. The LSP shows deterministic completions immediately. AI re-ranking happens async — if it arrives within `max_latency_ms`, the list is updated. If not, the deterministic list stands. No spinner, no delay.

**Implementation**:

```
User types "response."
    │
    ├── Deterministic completer → immediate results
    │
    └── AI request (async) ──┬── responds in time → merge into list
                             └── too slow → discard, user sees deterministic list
```

### Feature 10: AI Stub Generation

**Enhances**: stub resolution (LSP-ARCHITECTURE-SPEC.md stub system, `basilisk stubs generate` CLI)

**When**: A user imports from an untyped third-party package and no stubs are available — not from typeshed, not bundled with the package, not in the project's stub directory.

**Flow**:

1. User writes `from thirdparty import Widget`. Basilisk fires `BSK-W0010: Module "thirdparty" has no type stubs`.
2. Code action: `"(AI) Generate type stubs for thirdparty"`.
3. LSP gathers context:
   - The module's source code (if installed and readable — most packages have `.py` files).
   - How the module is used across the project (`Widget(name="foo")`, `widget.render()`, etc.).
   - Any docstrings in the module source.
4. Model generates a `.pyi` stub file with type annotations.
5. The stub is written to the project's stub directory (configurable, default `typings/`) and immediately picked up by the type checker.

**Stub quality**: AI-generated stubs are Tier 3 in the provenance system — best-effort, lower confidence than typeshed (Tier 0) or bundled stubs (Tier 1). Diagnostics from Tier 3 stubs are shown as warnings, not errors. The user can promote them to Tier 2 (project stubs) after review.

**CLI integration**: `basilisk stubs generate --ai thirdparty` triggers this from the command line.

### Feature 11: AI Dead Code Detection

**Enhances**: deterministic reference counting (`code_lens.rs` "N references")

**When**: Static analysis finds a symbol with zero references. Before reporting it as dead, the LSP asks the AI to check for dynamic/framework-based reachability.

**Flow**:

1. Basilisk's reference analysis finds `def handle_webhook(request):` with 0 references in the codebase.
2. Before reporting it as dead code, the LSP checks: does the AI provider have `dead_code_analysis` capability?
3. If yes: sends the symbol, its decorators (`@app.route("/webhook")`), its module path (`myapp.views`), and relevant config snippets (`urls.py`, `pyproject.toml` entry points).
4. Model responds: "Not dead. This function is registered as a Flask route via the `@app.route` decorator. It's called by the web framework's request dispatcher, not directly from Python code." (confidence: 0.95)
5. The LSP suppresses the dead code diagnostic, or downgrades it to a hint: `"0 direct references (registered as Flask route)"`.

**Why AI is better here**: Static analysis can't follow framework magic. `@app.route`, Django's `urlpatterns`, Click's `@cli.command()`, pytest's `test_*` discovery, `__init_subclass__`, `__init__.py` re-exports, `setup.py` entry points — all of these register code dynamically. The AI understands these patterns.

**False positive prevention**: Without AI, dead code detection in Python is nearly useless for non-trivial projects because frameworks register everything dynamically. With AI, it becomes actually useful.

### Feature 12: AI Code Modernization

**Enhances**: deterministic diagnostics (could be BSK-W#### modernization rules)

**When**: Code uses patterns that have modern Python alternatives. Deterministic rules catch the straightforward cases. AI handles the nuanced transformations that require understanding intent.

**What deterministic rules handle** (no AI needed):
- `Union[X, Y]` → `X | Y` (simple syntax swap)
- `Optional[X]` → `X | None` (simple syntax swap)
- `Dict[str, int]` → `dict[str, int]` (builtin generics, PEP 585)
- `typing.List` → `list` (same)

**What AI handles** (requires understanding the code):

| Pattern | Modern alternative | Why AI is needed |
|---------|-------------------|-----------------|
| Long `if/elif/else` chains on type checks | `match`/`case` with structural patterns | Need to identify that the chain is a pattern match in disguise, determine correct patterns |
| Repeated `isinstance` checks | `match`/`case` with class patterns | Need to determine exhaustiveness, handle nested checks |
| `.format()` with complex expressions | f-strings | Need to handle edge cases: multi-line, nested quotes, backslash escapes |
| Manual `__init__` + attribute assignment | `@dataclass` | Need to identify which attributes are init params vs computed, handle defaults |
| `TypeAlias = ...` | PEP 695 `type X = ...` | Need to handle generic aliases, forward references |
| Context manager class with `__enter__`/`__exit__` | `@contextmanager` generator | Need to correctly transform the control flow |

**Flow**:

1. AI identifies a 12-line `if/elif/else` chain checking `isinstance(node, ...)` in a function.
2. Code action: `"(AI) Convert to pattern matching (Python 3.10+)"`.
3. Model produces the `match`/`case` equivalent, handling nested patterns and guard clauses.
4. Code action detail shows the reasoning: "Converts 12-line isinstance chain to 6-line pattern match. Requires Python 3.10+."

**Safety**: `Unsafe`. The transformation might change semantics in edge cases (e.g., evaluation order of elif conditions). User reviews.

**Python version awareness**: Suggestions only appear if the project's target Python version supports the feature. The model receives `python_version` in the request. No point suggesting `match` for a Python 3.8 project.

### Feature 13: AI Semantic Search

**Enhances**: workspace symbol search (`symbols.rs`, Ctrl+T / `#` in editors)

**When**: User searches for symbols by intent, not by name. Normal workspace search is substring matching. Semantic search understands what code does.

**Flow**:

1. User opens workspace symbol search and types: `"handles authentication"`.
2. Normal search: finds nothing (no symbol contains the phrase "handles authentication").
3. AI semantic search: the LSP sends the query + a pre-filtered symbol index (top N symbols by rough text similarity + all symbols with docstrings mentioning related terms).
4. Model returns: `verify_jwt_token` (0.95, "This function verifies JWT tokens, which is the core authentication mechanism"), `login_handler` (0.88, "This handles user login requests"), `UserCredentials` (0.75, "This dataclass holds authentication credentials").
5. Results appear in the symbol picker with relevance scores.

**Implementation notes**:

- The symbol index is pre-built by the LSP (it already builds one for workspace symbol search).
- For large projects, sending the entire index would exceed context limits. Pre-filter using text similarity (TF-IDF or simple keyword overlap) to the top ~200 symbols, then let the AI rank those.
- For local models with small context windows, reduce to top ~50 symbols.
- This is one of the most valuable AI features for navigation in unfamiliar codebases.

**Custom command**: `basilisk/ai/findByIntent` — can also be used programmatically.

### Feature 14: AI Next-Edit Prediction

**Enhances**: the entire editing flow (no deterministic equivalent)

**When**: The user just made an edit that implies follow-up edits elsewhere. The AI predicts where and what.

**Flow**:

1. User adds a parameter `timeout: int = 30` to function `def fetch_data(url: str)`.
2. The LSP detects the edit (via `textDocument/didChange`).
3. If the AI provider has `next_edit_prediction` capability: the LSP sends the diff, the file context, and related symbols (call sites of `fetch_data`, any overrides, tests).
4. Model responds with predictions:
   - **Prediction 1** (confidence: 0.92): In `main.py:45`, add `timeout=60` to the call `fetch_data("https://api.example.com")`.
   - **Prediction 2** (confidence: 0.88): In `test_fetch.py:12`, add `timeout=5` to `fetch_data("http://test")`.
   - **Prediction 3** (confidence: 0.75): In `fetch.py:30`, update the docstring to document the `timeout` parameter.
5. The editor shows ghost text at prediction 1's location. User presses Tab to accept, or continues editing normally to dismiss.

**How this differs from autocomplete**:
- Autocomplete: predicts what you'll type at the current cursor.
- Next-edit prediction: predicts what you'll change at a *different* location after your current edit.

**Latency requirement**: This must feel predictive, not slow. Target: <200ms for the first prediction. This means:
- Fast local models are ideal (7B quantized, <100ms inference).
- Cloud models may be too slow unless cached/prefetched.
- The provider's `max_latency_ms` capability is checked. If the provider can't respond in time, this feature is disabled for it.

**Debouncing**: The LSP debounces edit events. It doesn't fire a prediction request on every keystroke — only after the user pauses for ~500ms, indicating a completed edit.

---

## Configuration

### `pyproject.toml`

```toml
[tool.basilisk.ai]
# Master switch. Default: false. AI features are opt-in.
enabled = false

# Provider selection. Options: "none", "openai-compatible", "anthropic",
# "copilot", "process"
provider = "none"

# Provider-specific settings
[tool.basilisk.ai.openai-compatible]
base-url = "http://localhost:11434/v1"
model = "codellama:13b"
# api-key: use BASILISK_AI_API_KEY env var

[tool.basilisk.ai.anthropic]
model = "claude-sonnet-4-6"
# api-key: use ANTHROPIC_API_KEY env var

[tool.basilisk.ai.copilot]
# No config needed if Copilot is already installed and authenticated

[tool.basilisk.ai.process]
command = ["python", "scripts/my_ai_bridge.py"]

# Feature toggles — disable specific AI features even when AI is enabled.
# Each feature is independently toggleable. All default to true except
# latency-sensitive features that need explicit opt-in.
[tool.basilisk.ai.features]
# Core fix features
type-annotation = true
type-error-fix = true
mass-autofix = true

# Comprehension features
diagnostic-explanation = true
semantic-search = true
dead-code-analysis = true

# Generation features
docstring-generation = true
stub-generation = true

# Enhancement features
import-resolution = true
rename-suggestion = true
refactoring = true
completion-enhancement = true
modernization = true

# Latency-sensitive features — off by default, require fast models
next-edit-prediction = false

# Context control — what gets sent to the AI provider
[tool.basilisk.ai.context]
# Maximum lines of surrounding source to include
max-source-lines = 50
# Include call sites from other files (requires cross-module analysis)
include-cross-file-call-sites = false
# Maximum number of call sites to include
max-call-sites = 10
# Maximum symbols to include in semantic search index
max-search-symbols = 200
# Maximum usage patterns to include in stub generation
max-stub-usage-patterns = 50

# Latency control
[tool.basilisk.ai.latency]
# Maximum ms to wait for completion enhancement before showing deterministic list
completion-timeout-ms = 150
# Maximum ms to wait for next-edit prediction
next-edit-timeout-ms = 200
# Debounce interval for next-edit prediction (ms after last keystroke)
next-edit-debounce-ms = 500
```

### Editor Settings

All editors expose these under the `basilisk.ai` namespace:

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `basilisk.ai.enabled` | `boolean` | `false` | Master switch for all AI features |
| `basilisk.ai.provider` | `enum` | `"none"` | Active provider |
| `basilisk.ai.showConfidence` | `boolean` | `true` | Show confidence scores on AI code actions |
| `basilisk.ai.showReasoning` | `boolean` | `true` | Show model reasoning in code action detail |
| `basilisk.ai.maxAlternatives` | `number` | `3` | Max alternative fixes/suggestions to show |
| `basilisk.ai.batchSize` | `number` | `20` | Max diagnostics per batch request |
| `basilisk.ai.timeoutMs` | `number` | `30000` | Request timeout for non-latency-critical features |
| `basilisk.ai.completionTimeoutMs` | `number` | `150` | Max ms to wait for completion re-ranking |
| `basilisk.ai.nextEditEnabled` | `boolean` | `false` | Enable next-edit prediction (requires fast model) |
| `basilisk.ai.nextEditTimeoutMs` | `number` | `200` | Max ms to wait for next-edit prediction |
| `basilisk.ai.proactiveRefactoring` | `boolean` | `false` | Show AI refactoring hints proactively (as diagnostics) |
| `basilisk.ai.semanticSearch` | `boolean` | `true` | Enable semantic search in workspace symbol picker |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `BASILISK_AI_API_KEY` | API key for OpenAI-compatible providers |
| `ANTHROPIC_API_KEY` | API key for Anthropic provider |
| `BASILISK_AI_PROVIDER` | Override provider selection (useful for CI/testing) |
| `BASILISK_AI_ENABLED` | Override enable/disable (useful for CI: `BASILISK_AI_ENABLED=0`) |

---

## LSP Protocol Integration

### Code Actions

AI-generated code actions use distinct `CodeActionKind` values so editors can filter them:

| Kind | Description | Feature |
|------|-------------|---------|
| `quickfix.ai` | AI-suggested fix for a diagnostic | 1, 2 |
| `quickfix.ai.type-annotation` | AI-suggested type annotation | 1 |
| `quickfix.ai.type-error` | AI-suggested type error fix | 2 |
| `quickfix.ai.import` | AI-suggested import for unresolved name | 6 |
| `refactor.ai.docstring` | AI-generated docstring | 5 |
| `refactor.ai.extract-method` | AI extract method suggestion | 8 |
| `refactor.ai.extract-variable` | AI extract variable suggestion | 8 |
| `refactor.ai.extract-constant` | AI extract constant suggestion | 8 |
| `refactor.ai.convert-dataclass` | AI convert to dataclass/TypedDict | 8 |
| `refactor.ai.simplify` | AI simplify conditionals | 8 |
| `refactor.ai.pattern-match` | AI convert to pattern matching | 8 |
| `refactor.ai.modernize` | AI code modernization suggestion | 12 |
| `source.ai.generate-stub` | AI generate type stubs | 10 |

AI code actions carry extra data in the `data` field:

```json
{
    "provider": "Ollama (codellama)",
    "confidence": 0.87,
    "reasoning": "Parameter `data` is passed to json.loads() which expects str | bytes...",
    "isAiGenerated": true,
    "feature": "type-annotation"
}
```

### Custom Commands

| Command | Arguments | Response | Description |
|---------|-----------|----------|-------------|
| `basilisk/ai/suggestFix` | `{uri, diagnostic}` | `AiFixResponse` | Request AI fix for a specific diagnostic |
| `basilisk/ai/suggestFixBatch` | `{uri, diagnostics}` | `AiFixResponse[]` | Batch AI fix request |
| `basilisk/ai/explain` | `{uri, diagnostic}` | `AiExplainResponse` | Explain a diagnostic in plain language |
| `basilisk/ai/generateDocstring` | `{uri, position}` | `AiDocstringResponse` | Generate docstring at position |
| `basilisk/ai/suggestImport` | `{uri, position, name}` | `AiImportResponse` | Suggest imports for an unresolved name |
| `basilisk/ai/suggestRename` | `{uri, position}` | `AiRenameResponse` | Suggest better names for a symbol |
| `basilisk/ai/suggestRefactoring` | `{uri, range}` | `AiRefactorResponse` | Suggest refactoring for a code region |
| `basilisk/ai/generateStub` | `{module}` | `AiStubResponse` | Generate type stubs for an untyped module |
| `basilisk/ai/findByIntent` | `{query}` | `AiSemanticSearchResponse` | Semantic search — find symbols by intent |
| `basilisk/ai/analyzeDeadCode` | `{uri}` | `AiDeadCodeResponse[]` | Analyze potential dead code in a file |
| `basilisk/ai/suggestModernization` | `{uri, range}` | `AiModernizeResponse` | Suggest modern Python patterns |
| `basilisk/ai/status` | `{}` | `{provider, available, capabilities}` | Check AI provider status |

### Status Reporting

When AI is enabled, the LSP reports provider status via `window/showMessage` on initialization:

- AI enabled, provider available: `"Basilisk AI: Connected to Ollama (codellama)"`
- AI enabled, provider unavailable: `"Basilisk AI: Provider configured but not available (connection refused)"`
- AI disabled: no message

---

## Context Truncation

Small local models can't handle 100K tokens of context. The LSP adapts:

1. Check `capabilities.max_context_tokens`.
2. If the full context exceeds the limit, truncate in priority order:
   - Always include: the diagnostic itself, the diagnostic source line(s).
   - High priority: the enclosing function/class source, inferred types for symbols in the diagnostic.
   - Medium priority: call sites (limited to `max-call-sites`), available types.
   - Low priority: surrounding file context beyond the enclosing scope.
3. Truncation is transparent — the provider never receives a payload larger than its declared capacity.

---

## Security & Privacy

1. **API keys never in config files.** Always environment variables. The LSP refuses to read API keys from `pyproject.toml` or editor settings.
2. **Local models = zero data exfiltration.** Ollama, LM Studio, llama.cpp all run on localhost. Nothing leaves the machine.
3. **Cloud provider consent.** When a cloud provider is configured, the LSP shows a one-time confirmation: "AI features will send code context to [provider]. Continue?"
4. **No telemetry.** Basilisk does not collect, store, or transmit any data about AI feature usage.
5. **Context payload is inspectable.** A `basilisk ai debug-context` CLI command dumps the exact payload that would be sent to the AI, so users can audit what leaves their machine.

---

## Relationship to Existing Specs

Mass Autofix is a **deterministic, standalone feature**. It does not require AI. It works without AI. AI does not replace it. This spec defines an **optional enhancement layer** that plugs into the existing fix pipeline for diagnostics that deterministic fixers cannot handle.

| Spec | Relationship |
|------|-------------|
| `LSP-MASS-AUTOFIX-SPEC.md` Features 1 & 2 | **Untouched.** Mass Autofix and Gradual Adoption are deterministic features that work independently of AI. This spec does not modify them. |
| `LSP-MASS-AUTOFIX-SPEC.md` Feature 3 | This spec **expands** the "AI Typing (Hooks)" section with a full provider abstraction. The `AiTypingProvider` trait becomes the more general `AiProvider` trait defined here. The hook points remain the same — AI slots in after deterministic fixes, never instead of them. |
| `LSP-MASS-AUTOFIX-PLAN.md` Phases 1-4 | **Untouched.** Fix metadata, safe autofixes, mass fix engine, gradual adoption — all deterministic, all independent of AI. |
| `LSP-MASS-AUTOFIX-PLAN.md` Phase 5 | **Expanded** by this spec's implementation plan (`LSP-AI-PLAN.md`). The stubs-only approach is upgraded to full provider abstraction, but the integration point is the same: AI is called only for diagnostics that have no deterministic fix. |
| `LSP-ARCHITECTURE-SPEC.md` | This spec **extends** the LSP spec with 13 new code action kinds, 12 custom commands, and enhanced completion/symbol behavior. The LSP spec remains the single source of truth for non-AI features. AI features enhance — never replace — the deterministic LSP behavior defined there. |
| `LSP-ARCHITECTURE-SPEC.md` completion | Feature 9 (AI-Enhanced Completions) augments the deterministic completion pipeline. AI re-ranks and adds items but never removes or delays the deterministic list. |
| `LSP-ARCHITECTURE-SPEC.md` symbols | Feature 13 (Semantic Search) augments workspace symbol search. Text-based search always works. AI results are additive. |
| `LSP-ARCHITECTURE-SPEC.md` references | Feature 11 (Dead Code Detection) uses reference counting from the existing reference system. AI is consulted only for zero-reference symbols. |
| `LSP-ARCHITECTURE-SPEC.md` code actions | Features 6, 8, 12 add new code action kinds to the existing code action infrastructure. Deterministic code actions always appear first. AI code actions are always labeled with `(AI)` prefix. |

---

## Testing Strategy

AI features are tested without real AI models. The provider trait enables this:

### Mock Provider

```rust
pub struct MockProvider {
    fix_responses: HashMap<String, AiFixResponse>,
    import_responses: HashMap<String, AiImportResponse>,
    rename_responses: HashMap<String, AiRenameResponse>,
    refactor_responses: HashMap<String, AiRefactorResponse>,
    explain_responses: HashMap<String, AiExplainResponse>,
    docstring_responses: HashMap<String, AiDocstringResponse>,
    stub_responses: HashMap<String, AiStubResponse>,
    completion_responses: HashMap<String, AiCompletionResponse>,
    search_responses: HashMap<String, AiSemanticSearchResponse>,
    dead_code_responses: HashMap<String, AiDeadCodeResponse>,
    modernize_responses: HashMap<String, AiModernizeResponse>,
    next_edit_responses: HashMap<String, AiNextEditResponse>,
    capabilities: AiProviderCapabilities,
    /// Simulated latency for testing timeout behavior
    simulated_latency_ms: u32,
}
```

A mock provider that returns pre-configured responses for any AI feature. Used in E2E tests. Supports simulated latency for testing timeout behavior.

### Test Categories

| Test | What it checks |
|------|---------------|
| **Provider lifecycle** | `is_available()`, capability reporting, error handling |
| **Context construction** | Correct AST context, inferred types, call sites assembled from `ResolvedModule` |
| **Fix integration** | AI fixes flow through the same `Fix` pipeline as deterministic fixes |
| **Batch handling** | Multiple diagnostics batched and unbatched correctly |
| **Truncation** | Context truncated to provider's max tokens without losing critical info |
| **Code action rendering** | AI code actions have correct kinds, titles, data, ordering for all 13 action kinds |
| **Configuration** | Provider selected from config, env var overrides work, missing keys handled, feature toggles respected |
| **Timeout handling** | Slow/hung providers don't block the LSP; latency-critical features (completions, next-edit) degrade gracefully |
| **No-op default** | With AI disabled, zero overhead, no AI code actions appear |
| **Import resolution** | Unresolved names dispatched to AI only after deterministic resolution fails; correct usage context sent; import inserted at correct location |
| **Rename suggestion** | Name suggestions appear alongside rename dialog; naming conventions correctly detected and sent |
| **Refactoring** | Selected region correctly identified; extract method generates valid function signature; refactoring edits maintain semantics |
| **Completion enhancement** | Deterministic completions shown immediately; AI re-ranking merges correctly within timeout; over-timeout results discarded |
| **Stub generation** | Generated stubs written to correct directory; marked as Tier 3 provenance; stub content is valid `.pyi` syntax |
| **Dead code analysis** | Symbols with framework decorators correctly identified as reachable; config snippets extracted and sent; diagnostics suppressed when AI says "not dead" |
| **Modernization** | Python version checked before suggesting; modern patterns only suggested when transformation is safe; deterministic modernizations NOT sent to AI |
| **Semantic search** | Symbol index correctly pre-filtered; results ranked by relevance; large projects don't exceed context limits |
| **Next-edit prediction** | Edit debouncing works correctly; predictions for correct locations; ghost text shown/dismissed properly; too-slow predictions discarded |
| **Feature isolation** | Disabling one feature doesn't affect others; capability flags respected; feature toggles in config override capabilities |

### No Real API Calls in CI

Tests use `MockProvider` or `NoOpProvider`. Real provider integration is tested manually or in a dedicated integration test suite that requires explicit opt-in (`BASILISK_AI_INTEGRATION_TEST=1`).
