# AI typing hook {#LSPAI}

The repository ships an unused provider interface and a no-op implementation. It does not
ship a model provider, network transport, AI-backed code action, command, configuration, or
editor UI. Planned integration belongs in
[LSP-AI-PLAN.md](../plans/LSP-AI-PLAN.md), not in this shipping contract.

## Principles {#LSPAI-PRINCIPLES}

- Deterministic checker and LSP behavior never depends on an AI service.
- The default provider performs no I/O and adds no analysis cost.
- Any future model suggestion is `FixSource::AiAssisted` and `FixSafety::Unsafe`; the user
  must review it before application.
- Provider failures must not suppress or alter deterministic diagnostics.

## Provider trait {#LSPAI-TRAIT}

`AiTypingProvider: Send + Sync` has two methods:

```rust
fn suggest_fix(
    &self,
    request: &AiTypingRequest,
) -> Result<Option<AiTypingResponse>, AiTypingError>;

fn is_available(&self) -> bool;
```

`Ok(None)` means no suggestion. The trait is exported by `basilisk-lsp` but no server path
currently calls it.

## Errors {#LSPAI-ERRORS}

`AiTypingError` has three variants: `Unavailable`, `ProviderError(String)`, and `Timeout`.
They describe a provider attempt only; none is an LSP diagnostic or checker error.

## Context payload {#LSPAI-CONTEXT}

`AiTypingRequest` is a flat payload containing diagnostic code/message, surrounding source,
file path, and zero-based line/column. It does not contain a syntax tree, workspace files,
imports, call sites, or secrets. A future provider decides how much of that supplied source
to transmit, subject to explicit configuration and review.

### Type-suggestion response {#LSPAI-TYPES-FIX}

`AiTypingResponse` contains one `suggested_type` string, a `confidence` float, and a
human-readable `reasoning` string. It is not a `WorkspaceEdit` and has no automatic apply
path.

## Providers {#LSPAI-PROVIDERS}

`NoOpAiTypingProvider` is the only implementation. It reports unavailable and always returns
`Ok(None)`. Unit tests cover this behavior and error display strings. Concrete model
providers and protocol integration are unimplemented.
