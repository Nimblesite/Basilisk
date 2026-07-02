# typeDiagram model - https://typediagram.dev
# Source-of-truth model for LSP debug-session failures (LSPDEBUG-ERRORS).
# Implemented by DebugError in crates/basilisk-lsp/src/debug.rs, whose
# jsonrpc_code() maps each variant to a JSON-RPC error code:
#   DebugpyNotFound -> -32001, PythonNotFound -> -32002, all others -> -32000.
# io payloads are std::io::Error in Rust; modelled as String (opaque) here.
# Regenerate the diagram: typediagram models/debug_session.td > docs/models/debug_session.svg

union DebugError {
  PortAllocation { io: String }
  SpawnFailed { io: String }
  Timeout { port: Int }
  AdapterExited { status: String }
  DebugpyNotFound { python: String }
  PythonNotFound { searched: String }
}
