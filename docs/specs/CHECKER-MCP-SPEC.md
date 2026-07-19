# Checker MCP service {#MCP}

## Scope {#MCP-SCOPE}

Basilisk ships one deliberately narrow Model Context Protocol surface:
`basilisk mcp --workspace <DIR>`. It is a subcommand of the same `basilisk`
binary already carried by the binary archives, wheels, editor packages, and
other release artifacts; it is not a second executable or workspace index.
The server exposes source status only. It MUST NOT perform type-checking,
publish diagnostics, or invent a second typeshed acquisition state.

The tool invokes the shared typeshed startup gate and serializes its canonical
status object ([STUBRES-TYPESHED-WARN](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN)).
The underlying step-3 source is the custom canonical tree or typeshed stdlib
defined by the pinned typing specification
([`python/typing@6ef9f7719ecfff09dad8724ef42b621fd994fb5e`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst));
MCP changes only status transport, never that resolution order.
It is read-only with respect to user projects. Shared Latest acquisition may
contact the configured upstream and populate Basilisk's internal immutable
cache before status is available; MCP MUST NOT add another fetch or cache path.

## Stdio lifecycle {#MCP-STDIO}

The server implements MCP `2025-11-25` JSON-RPC over
[stdio](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports):
one UTF-8 JSON message per line, no embedded newlines, and no non-protocol bytes
on stdout. Logs go to stderr. An input line over 1 MiB is rejected.

The supported lifecycle is `initialize`, `notifications/initialized`, `ping`,
`tools/list`, and `tools/call`, following the official
[initialization contract](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle).
Before a valid `initialize`, an `initialized` notification has no effect. If a
client requests an unsupported protocol version, the initialize result returns
the server's supported version so the client can continue or disconnect.
Operational calls are rejected until the notification completes initialization.
There is no MCP-over-HTTP transport and no server-to-client request surface.

## Typeshed status tool {#MCP-TYPESHED-STATUS}

`tools/list` declares exactly one tool, `basilisk_typeshed_status`, with an empty
object input schema and read-only, non-destructive, idempotent annotations. Its
`openWorldHint` is true because shared Latest acquisition can access the
configured upstream. The result follows MCP
[structured tool output](https://modelcontextprotocol.io/specification/2025-11-25/server/tools):
the same JSON object appears as `structuredContent` and as serialized JSON in a
text content block.

The closed output schema contains:

| Field | Contract |
|---|---|
| `active_source` | Active custom, exact, latest, or bundled source label. |
| `commit_identity` | Full commit SHA, or `null` when the source has none. |
| `tree_identity` | Full Git tree identity, or `null` when unavailable. |
| `transport` | Shared source transport, such as custom path or embedded ZIP. |
| `provenance` | `github-tls-attested`, `bundle-vetted`, `unverified`, or `user-managed`; attested does not imply a signed typeshed release. |
| `license_status` | Approved/review state; custom may say `not supplied`. |
| `license_reference` | Safe immutable license reference, or `null` for custom. |
| `signed_release` | Always `false`: current typeshed sources are commits/archives, not signed releases. |
| `warnings` | Canonically ordered `{code, message}` status warnings. |

Field values and warning order MUST match the CLI and LSP status produced for
the same acquired source. Warning codes are stable display codes including
`UNPINNED`, `DOWNLOAD FAILED`, `LICENSE CHANGED`, `UNVERIFIED`, and
`USER-MANAGED SOURCE`. They are service status, never Python diagnostics.

Unknown methods/tools and invalid arguments return JSON-RPC errors. A shared
acquisition/status failure returns a tool result with `isError: true`; it MUST
NOT return partial provenance or substitute stale state. Responses contain no
archive bytes, source text, credentials, configured mirror URL, or user files.

## Acceptance {#MCP-ACCEPTANCE}

Protocol unit tests MUST cover lifecycle ordering, version negotiation, invalid
JSON, pre-initialization calls, tool discovery, the closed schemas, dual text
and structured output, and ordered warnings. A subprocess test against the
packaged `basilisk` binary MUST exercise a real custom-typeshed workspace,
assert the user-managed license/status fields, and prove stdout contains only
JSON-RPC messages. `basilisk --version --json` advertises the `mcp` capability.
