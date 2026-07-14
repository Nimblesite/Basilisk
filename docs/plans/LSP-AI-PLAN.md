# AI-assisted LSP plan {#LSPAIPLAN}

## Status {#LSPAIPLAN-STATUS}

Only the narrow `AiTypingProvider` interface and no-op default exist. They are not wired into
the server. Everything below is optional product work and must preserve the deterministic,
offline checker described in [LSP-AI-SPEC.md](../specs/LSP-AI-SPEC.md).

## First usable slice {#LSPAIPLAN-FIRST-SLICE}

- [ ] Choose one provider protocol and implement explicit credentials, timeout, cancellation,
  rate-limit, and malformed-response handling.
- [ ] Add an opt-in server setting; no provider is contacted until a user enables it.
- [ ] Wire only type-annotation suggestions for diagnostics that deterministic inference
  cannot resolve.
- [ ] Convert a suggestion to a previewable unsafe code action; never auto-apply it.
- [ ] Test no-provider, unavailable, timeout, cancellation, redaction, malformed output, and
  successful preview paths through the real LSP protocol.

## Privacy and safety gate {#LSPAIPLAN-SAFETY}

- [ ] Define the exact source/context boundary and redact configured paths and secret-shaped
  values before transport.
- [ ] Show provider identity and transmitted scope before first use.
- [ ] Keep telemetry off by default and never log credentials or full prompts.
- [ ] Validate suggested syntax and edit ranges locally before presenting a result.
- [ ] Ensure provider failure leaves deterministic diagnostics and actions unchanged.

## Expansion criteria {#LSPAIPLAN-EXPANSION}

Do not add explanation, import, rename, refactoring, completion, stub, search, dead-code, or
next-edit surfaces until the first slice has measured usefulness, latency, cost, and privacy
behavior. Each added surface needs its own typed request/response contract and protocol tests;
it must not reuse an unstructured prompt as an implicit API.

## Acceptance {#LSPAIPLAN-ACCEPTANCE}

- [ ] Basilisk remains fully functional offline and with invalid provider configuration.
- [ ] Every AI result is visibly provider-originated and unsafe until reviewed.
- [ ] No network request occurs in default configuration or deterministic test suites.
- [ ] Cancellation prevents stale edits from being offered after the document changes.
