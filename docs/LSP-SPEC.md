# Basilisk LSP — Implementation Specification

> **Authoritative reference** for all LSP implementation work.
> See `docs/lsp-plan.md` for agent assignments and sequencing.

---

## Current Architecture (Pre-LSP)

```
VS Code Extension (extension.ts)
  │
  │  execFile("basilisk check --output json <file>")
  ▼
basilisk CLI (basilisk-cli)
  │  check subcommand
  ▼
basilisk-lsp::check_source(&str) → Vec<String>
  │
  ├── basilisk-parser::parse_source
  ├── basilisk-resolver::resolve
  └── basilisk-checker::check → Vec<Diagnostic>
```

**Limitations of subprocess approach**:
- Check only fires on save or open — not as the user types
- Subprocess startup cost (~50–100ms) on every save
- No hover, no go-to-definition, no code actions
- Cannot share parse/resolve state between checks
- Unsaved buffers cannot be checked (file must be on disk)

---

## Target Architecture

```
VS Code Extension (extension.ts)
  │
  │  vscode-languageclient  (JSON-RPC over stdio)
  ▼
basilisk lsp  (basilisk-cli subcommand)
  │
  │  tower-lsp LanguageServer trait
  ▼
LspServer struct
  │  owns a document store (uri → DocumentState)
  │
  ├── on didOpen / didChange → re-check in-memory text → publishDiagnostics
  ├── on didSave → full re-check of saved file
  ├── on hover → query InferredType at position → Hover response
  └── on codeAction → suggest quick-fixes for diagnostics at cursor
  │
  ▼
basilisk-lsp::check_source_with_types(&str) → CheckResult
  │
  ├── basilisk-parser::parse_source
  ├── basilisk-resolver::resolve
  ├── basilisk-checker::check → Vec<Diagnostic>
  └── (future) type_map: HashMap<Span, InferredType>
```

---

## WI-L1 — tower-lsp Scaffolding + `basilisk lsp` Subcommand

### Cargo.toml changes

**`crates/basilisk-lsp/Cargo.toml`** — add dependencies:
```toml
[dependencies]
basilisk-parser.workspace   = true
basilisk-resolver.workspace = true
basilisk-checker.workspace  = true

tower-lsp  = "0.20"
tokio      = { version = "1", features = ["rt-multi-thread", "macros", "io-std"] }
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
dashmap    = "6"
```

**`crates/basilisk-cli/Cargo.toml`**:
```toml
basilisk-lsp.workspace = true
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

### New CLI subcommand

**`crates/basilisk-cli/src/main.rs`** — add `Lsp` variant:

```rust
#[derive(Subcommand)]
enum Command {
    /// Type check one or more files or directories.
    Check { /* existing */ },

    /// Start the Basilisk Language Server (JSON-RPC over stdio).
    Lsp,
}
```

In `main()`:
```rust
Command::Lsp => {
    basilisk_lsp::run_server();
    0
}
```

### `run_server` in basilisk-lsp

**`crates/basilisk-lsp/src/server.rs`** (new file):

```rust
use tower_lsp::{LspService, Server};

pub fn run_server() {
    // expect() is acceptable here: this is the process entry point.
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        let stdin  = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let (service, socket) = LspService::new(|client| LspServer::new(client));
        Server::new(stdin, stdout, socket).serve(service).await;
    });
}
```

### `LspServer` struct

```rust
use dashmap::DashMap;
use tower_lsp::lsp_types::*;
use tower_lsp::Client;

pub struct LspServer {
    client: Client,
    documents: DashMap<Url, DocumentState>,
}

struct DocumentState {
    text:    String,
    version: i32,
}

impl LspServer {
    pub fn new(client: Client) -> Self {
        Self { client, documents: DashMap::new() }
    }
}
```

### `initialize` handler

```rust
#[tower_lsp::async_trait]
impl LanguageServer for LspServer {
    async fn initialize(&self, _params: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name:    "basilisk".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider:       Some(HoverProviderCapability::Simple(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "Basilisk LSP initialized").await;
    }

    async fn shutdown(&self) -> LspResult<()> { Ok(()) }
}
```

### Deliverables
- `crates/basilisk-lsp/src/server.rs` — struct + initialize/initialized/shutdown
- `crates/basilisk-lsp/src/lib.rs` — re-export `run_server`
- `crates/basilisk-cli/src/main.rs` — `Lsp` subcommand wired in
- `cargo build` and `cargo clippy` pass

---

## WI-L2 — Document Store + textDocument Lifecycle

### didOpen

```rust
async fn did_open(&self, params: DidOpenTextDocumentParams) {
    let uri     = params.text_document.uri;
    let text    = params.text_document.text;
    let version = params.text_document.version;
    self.documents.insert(uri.clone(), DocumentState { text: text.clone(), version });
    self.check_and_publish(uri, &text).await;
}
```

### didChange (full sync — Phase 1)

```rust
async fn did_change(&self, params: DidChangeTextDocumentParams) {
    let uri     = params.text_document.uri;
    let version = params.text_document.version;
    let Some(change) = params.content_changes.into_iter().next() else { return; };
    let text = change.text;
    self.documents.insert(uri.clone(), DocumentState { text: text.clone(), version });
    self.check_and_publish(uri, &text).await;
}
```

### didSave

```rust
async fn did_save(&self, params: DidSaveTextDocumentParams) {
    let uri = params.text_document.uri;
    if let Some(entry) = self.documents.get(&uri) {
        self.check_and_publish(uri, &entry.text).await;
    }
}
```

### didClose

```rust
async fn did_close(&self, params: DidCloseTextDocumentParams) {
    let uri = params.text_document.uri;
    self.documents.remove(&uri);
    self.client.publish_diagnostics(uri, vec![], None).await;
}
```

### Deliverables
- All four handlers implemented on `LspServer`
- `documents` populated and cleared correctly

---

## WI-L3 — Diagnostics Push (publishDiagnostics)

### `check_and_publish`

```rust
impl LspServer {
    async fn check_and_publish(&self, uri: Url, text: &str) {
        let diagnostics = self.run_checker(uri.clone(), text);
        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }

    fn run_checker(&self, uri: Url, text: &str) -> Vec<lsp_types::Diagnostic> {
        let file_path = uri.to_file_path().unwrap_or_default();
        let path_str  = file_path.to_string_lossy().into_owned();

        let parsed = match basilisk_parser::parse_source(text.to_owned(), path_str.clone()) {
            Ok(p)  => p,
            Err(e) => return vec![parse_error_diagnostic(&e.to_string())],
        };
        let resolved = match basilisk_resolver::resolve(&parsed) {
            Ok(r)  => r,
            Err(_) => return vec![],
        };
        basilisk_checker::check(&resolved)
            .into_iter()
            .map(|d| bsk_to_lsp(d, text))
            .collect()
    }
}
```

### Span → LSP Position (UTF-16)

LSP character offsets are **UTF-16 code units** — surrogate pairs count as 2.

```rust
fn byte_offset_to_position(text: &str, byte_offset: usize) -> Position {
    let clamped  = byte_offset.min(text.len());
    let before   = &text[..clamped];
    let line     = before.chars().filter(|&c| c == '\n').count() as u32;
    let last_nl  = before.rfind('\n').map_or(0, |p| p + 1);
    let character = before[last_nl..]
        .chars()
        .map(|c| if c as u32 > 0xFFFF { 2u32 } else { 1u32 })
        .sum::<u32>();
    Position { line, character }
}
```

### Diagnostic mapper

```rust
fn bsk_to_lsp(d: basilisk_checker::Diagnostic, text: &str) -> lsp_types::Diagnostic {
    let start = byte_offset_to_position(text, d.span.start as usize);
    let end   = byte_offset_to_position(text, d.span.end   as usize);
    let severity = match d.severity {
        basilisk_checker::Severity::Error           => DiagnosticSeverity::ERROR,
        basilisk_checker::Severity::Warning         => DiagnosticSeverity::WARNING,
        basilisk_checker::Severity::SafetyViolation => DiagnosticSeverity::ERROR,
    };
    lsp_types::Diagnostic {
        range:    Range { start, end },
        severity: Some(severity),
        code:     Some(NumberOrString::String(d.code.code.to_owned())),
        code_description: Some(CodeDescription {
            href: Url::parse(d.code.docs_url).unwrap_or_else(|_| {
                Url::parse("https://basilisk-lang.org").expect("fallback URL is valid")
            }),
        }),
        source:  Some("basilisk".to_owned()),
        message: d.message,
        ..Default::default()
    }
}

fn parse_error_diagnostic(message: &str) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic {
        range:    Range { start: Position::new(0, 0), end: Position::new(0, 0) },
        severity: Some(DiagnosticSeverity::ERROR),
        code:     Some(NumberOrString::String("BSK-PARSE".to_owned())),
        source:   Some("basilisk".to_owned()),
        message:  format!("Parse error: {message}"),
        ..Default::default()
    }
}
```

### Deliverables
- `check_and_publish`, `run_checker`, `bsk_to_lsp`, `byte_offset_to_position` implemented
- Diagnostics appear in VS Code Problems panel

---

## WI-L4 — Hover (textDocument/hover)

### Position → Byte Offset (inverse of WI-L3)

```rust
fn position_to_byte_offset(text: &str, pos: Position) -> usize {
    let mut line      = 0u32;
    let mut char_cu   = 0u32;   // UTF-16 code units on current line
    let mut byte_pos  = 0usize;

    for (byte_idx, ch) in text.char_indices() {
        if line == pos.line && char_cu == pos.character {
            return byte_idx;
        }
        if ch == '\n' {
            line += 1;
            char_cu = 0;
            if line > pos.line {
                return byte_idx;
            }
        } else {
            char_cu += if ch as u32 > 0xFFFF { 2 } else { 1 };
        }
        byte_pos = byte_idx + ch.len_utf8();
    }
    byte_pos
}
```

### Hover handler

Phase 1: hover shows the diagnostic message covering the cursor position.
Phase 2 (post type-inference): hover shows `Type: \`int | str\`` from the type map.

```rust
async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
    let uri  = params.text_document_position_params.text_document.uri;
    let pos  = params.text_document_position_params.position;
    let Some(entry) = self.documents.get(&uri) else { return Ok(None); };
    let text        = entry.text.clone();
    drop(entry);  // release DashMap lock before await

    let byte_offset = position_to_byte_offset(&text, pos);
    let file_path   = uri.to_file_path().unwrap_or_default();
    let path_str    = file_path.to_string_lossy().into_owned();

    let parsed   = match basilisk_parser::parse_source(text.clone(), path_str) {
        Ok(p)  => p,
        Err(_) => return Ok(None),
    };
    let resolved = match basilisk_resolver::resolve(&parsed) {
        Ok(r)  => r,
        Err(_) => return Ok(None),
    };
    let diags = basilisk_checker::check(&resolved);

    let hit = diags.into_iter().find(|d| {
        (d.span.start as usize) <= byte_offset && byte_offset < (d.span.end as usize)
    });

    let Some(d) = hit else { return Ok(None); };
    let mut md = format!("**{}** — {}", d.code.code, d.message);
    if let Some(help) = d.help { md.push_str(&format!("\n\n_{help}_")); }

    Ok(Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown, value: md,
        }),
        range: None,
    }))
}
```

### Deliverables
- `hover` handler implemented
- `position_to_byte_offset` utility
- Hovering over a squiggle shows the error message in VS Code

---

## WI-L5 — Code Actions (textDocument/codeAction)

### Quick-fix table

| Diagnostic | Action title | Transformation |
|---|---|---|
| BSK-E0001 | "Add `: Any` annotation" | Insert `: Any` after param name span end |
| BSK-E0002 | "Add `-> None` return type" | Insert `-> None` before colon at end of def line |

### Handler

```rust
async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
    let uri   = params.text_document.uri;
    let diags = params.context.diagnostics;
    let Some(entry) = self.documents.get(&uri) else { return Ok(None); };
    let text = entry.text.clone();
    drop(entry);

    let mut actions: Vec<CodeActionOrCommand> = vec![];
    for diag in &diags {
        let Some(NumberOrString::String(code)) = &diag.code else { continue; };
        let action = match code.as_str() {
            "BSK-E0001" => fix_missing_param_annotation(&uri, &text, diag),
            "BSK-E0002" => fix_missing_return_annotation(&uri, &text, diag),
            _           => None,
        };
        if let Some(a) = action { actions.push(CodeActionOrCommand::CodeAction(a)); }
    }
    Ok(if actions.is_empty() { None } else { Some(actions) })
}
```

### Fix helpers

```rust
fn fix_missing_param_annotation(
    uri:  &Url,
    _text: &str,
    diag: &lsp_types::Diagnostic,
) -> Option<CodeAction> {
    let insert_pos = diag.range.end;
    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), vec![TextEdit {
        range:    Range { start: insert_pos, end: insert_pos },
        new_text: ": Any".to_owned(),
    }]);
    Some(CodeAction {
        title:        "Add `: Any` annotation (basilisk)".to_owned(),
        kind:         Some(CodeActionKind::QUICKFIX),
        diagnostics:  Some(vec![diag.clone()]),
        edit:         Some(WorkspaceEdit { changes: Some(changes), ..Default::default() }),
        is_preferred: Some(true),
        ..Default::default()
    })
}

fn fix_missing_return_annotation(
    uri:  &Url,
    _text: &str,
    diag: &lsp_types::Diagnostic,
) -> Option<CodeAction> {
    // Insert `-> None` at the start of the diagnostic range (before the colon on the def line).
    let insert_pos = diag.range.start;
    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), vec![TextEdit {
        range:    Range { start: insert_pos, end: insert_pos },
        new_text: "-> None ".to_owned(),
    }]);
    Some(CodeAction {
        title:        "Add `-> None` return type (basilisk)".to_owned(),
        kind:         Some(CodeActionKind::QUICKFIX),
        diagnostics:  Some(vec![diag.clone()]),
        edit:         Some(WorkspaceEdit { changes: Some(changes), ..Default::default() }),
        is_preferred: Some(true),
        ..Default::default()
    })
}
```

### Deliverables
- `code_action` handler + two fix helpers
- Lightbulb appears in VS Code over E0001 and E0002 squiggles

---

## WI-L6 — Extension: Replace Subprocess with vscode-languageclient

### package.json changes

Add dependency and new config option:
```json
{
  "dependencies": {
    "vscode-languageclient": "^9.0.1"
  },
  "contributes": {
    "configuration": {
      "properties": {
        "basilisk.executablePath": { "type": "string", "default": "basilisk" },
        "basilisk.enabled":        { "type": "boolean", "default": true },
        "basilisk.useLsp": {
          "type":        "boolean",
          "default":     false,
          "description": "Use the LSP server instead of the subprocess approach. Enable once basilisk lsp is stable."
        },
        "basilisk.trace.server": {
          "type":    "string",
          "enum":    ["off", "messages", "verbose"],
          "default": "off",
          "description": "Trace LSP communication for debugging."
        }
      }
    }
  }
}
```

### extension.ts LSP path

```typescript
import * as vscode from "vscode";
import * as path from "path";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  const cfg            = vscode.workspace.getConfiguration("basilisk");
  const executablePath = cfg.get<string>("executablePath") ?? "basilisk";
  const useLsp         = cfg.get<boolean>("useLsp") ?? false;

  if (useLsp) {
    startLspClient(context, executablePath);
  } else {
    startSubprocessMode(context, executablePath);  // existing code, unchanged
  }
}

function startLspClient(context: vscode.ExtensionContext, executablePath: string): void {
  const serverOptions: ServerOptions = {
    command:   executablePath,
    args:      ["lsp"],
    transport: TransportKind.stdio,
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "python" }],
    synchronize:      { configurationSection: "basilisk" },
    traceOutputChannel: vscode.window.createOutputChannel("Basilisk LSP Trace"),
  };
  client = new LanguageClient("basilisk", "Basilisk Type Checker", serverOptions, clientOptions);
  client.start();
  context.subscriptions.push(client);
}

export function deactivate(): Promise<void> | undefined {
  return client?.stop();
}
```

### Deliverables
- `basilisk.useLsp` flag gates the two paths
- LSP path uses `vscode-languageclient`, subprocess path unchanged
- `npm run compile` passes
- When `useLsp: true`, diagnostics appear without save

---

## WI-L7 — LSP Integration Tests

### Test location
`crates/basilisk-lsp/tests/lsp_integration.rs`

### Required tests (all must pass)

| Test | Asserts |
|---|---|
| `initialize_returns_full_sync` | Capabilities include FULL sync, hover, code-action |
| `did_open_stores_document` | `server.documents` contains entry after didOpen |
| `did_close_clears_document` | `server.documents` is empty after didClose |
| `run_checker_e0001_missing_param` | `"BSK-E0001"` in diagnostics for `def f(x): pass` |
| `run_checker_e0002_missing_return` | `"BSK-E0002"` in diagnostics for unannotated public fn |
| `run_checker_clean_file` | Empty diagnostics for fully annotated code |
| `run_checker_parse_error` | `"BSK-PARSE"` returned for syntactically invalid Python |
| `byte_offset_ascii` | `byte_offset_to_position` correct on ASCII text |
| `byte_offset_utf16_surrogate` | Correct character count for emoji (non-BMP) characters |
| `position_roundtrip` | `position_to_byte_offset(byte_offset_to_position(text, off)) == off` |
| `hover_over_error_span` | Returns `Some(Hover)` with error message |
| `hover_outside_span` | Returns `None` |
| `code_action_e0001_inserts_any` | Action for E0001 inserts `: Any` at correct position |

### Test skeleton

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::*;

    fn make_server() -> LspServer {
        // Use tower_lsp test harness to get a client stub.
        let (service, _socket) = tower_lsp::LspService::new(|client| LspServer::new(client));
        // Extract inner server for direct method calls.
        // (Actual extraction depends on tower-lsp API — see tower_lsp::LspService::inner)
        todo!("wire up test client")
    }

    #[test]
    fn run_checker_e0001_missing_param() {
        let server = make_server();
        let uri    = Url::parse("file:///test.py").unwrap();
        let diags  = server.run_checker(uri, "def f(x): pass\n");
        assert!(diags.iter().any(|d| d.code == Some(NumberOrString::String("BSK-E0001".to_owned()))));
    }
}
```

### Deliverables
- All 13 tests implemented and `cargo test -p basilisk-lsp` passes
- `cargo clippy -p basilisk-lsp` is clean

---

## Future Phases

### Phase 2 — Incremental Sync
- Change `TextDocumentSyncKind::FULL` → `INCREMENTAL`
- Apply `TextDocumentContentChangeEvent` patches to stored text
- Requires incremental parser API

### Phase 3 — Type Hover
Once Sprint TI-1 delivers `InferredType`:
- `check_with_types(resolved) -> (Vec<Diagnostic>, TypeMap)` where `TypeMap = HashMap<Span, InferredType>`
- Store TypeMap alongside diagnostics in `DocumentState`
- Hover returns `Type: \`int | str\`` from TypeMap

---

## Acceptance Criteria

- [ ] `basilisk lsp` starts a JSON-RPC server on stdio
- [ ] VS Code extension connects via `vscode-languageclient`
- [ ] Diagnostics appear in Problems panel on open (no save required)
- [ ] Diagnostics update as user types (didChange triggers re-check)
- [ ] Hovering over a squiggle shows the diagnostic message
- [ ] E0001 lightbulb inserts `: Any`
- [ ] E0002 lightbulb inserts `-> None`
- [ ] All 13 integration tests pass
- [ ] `cargo clippy` clean
- [ ] No `.unwrap()` in server code (entry-point `expect` documented as exception)
