# AI-assisted LSP plan {#LSPAIPLAN}

## Status {#LSPAIPLAN-STATUS}

Only the narrow `AiTypingProvider` interface and no-op default exist. They are not wired into
the server. Everything below is optional product work and must preserve the deterministic
checker described in [LSP-AI-SPEC.md](../specs/LSP-AI-SPEC.md): checking never depends on an AI
service or provider. This is *not* an "offline" claim — by default Basilisk clones
`python/typeshed` for standard-library types
([CHECKER-STUB-RESOLUTION-SPEC §STUBRES-TYPESHED](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED));
the AI provider is a separate, opt-in network surface that this plan governs.

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

- [ ] Basilisk remains fully functional with no AI provider and with invalid provider
  configuration. With no network it still checks — standard-library types come from the
  bundled offline baseline and the CLI shows the dim-amber baseline warning
  ([CHECKER-STUB-RESOLUTION-SPEC §STUBRES-TYPESHED-WARN](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN)).
- [ ] Every AI result is visibly provider-originated and unsafe until reviewed.
- [ ] No **AI-provider** network request occurs unless a user enables a provider — no provider is
  contacted in the default configuration or in deterministic test suites. (The default typeshed
  clone is a separate, expected default network operation; tests pin it via `typeshed-commit` or
  run against the offline bundled baseline —
  [CHECKER-STUB-RESOLUTION-SPEC §STUBRES-TYPESHED](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED).)
- [ ] Cancellation prevents stale edits from being offered after the document changes.
