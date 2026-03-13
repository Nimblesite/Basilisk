# Basilisk Debug Integration via debugpy

## Goal

The Basilisk LSP **is** the debug adapter. When the editor needs to debug Python, it sends a custom LSP request. The LSP spawns debugpy on a TCP port and tells the editor where to connect. No separate binary, no separate process, no bundling. One LSP, both editors.

MAX CODE SHARING BETWEEN RUST COMPONENTS!!!

## Architecture

```mermaid
graph TB
    subgraph "Editor (VS Code / Zed)"
        UI[Debug UI — breakpoints, variables, call stack]
        DAP_CLIENT[Built-in DAP Client]
        LSP_CLIENT[LSP Client]
    end

    subgraph "basilisk lsp (Rust)"
        LSP_CORE[Language Server — diagnostics, completions, hover, ...]
        DEBUG_MGR[Debug Session Manager]
        PYRES[Python Resolver]
    end

    subgraph "Python Runtime"
        DEBUGPY["debugpy.adapter (TCP DAP server)"]
        TARGET[User's Python Program]
    end

    LSP_CLIENT -->|"LSP over stdin/stdout"| LSP_CORE
    LSP_CLIENT -->|"basilisk/startDebugSession"| DEBUG_MGR
    DEBUG_MGR --> PYRES
    DEBUG_MGR -->|"Spawns on free TCP port"| DEBUGPY
    DEBUG_MGR -->|"Returns host:port"| DAP_CLIENT
    DAP_CLIENT -->|"DAP over TCP"| DEBUGPY
    DEBUGPY -->|"Launches & controls"| TARGET
```

The LSP already runs as a long-lived process. It already knows the workspace roots and can resolve the Python interpreter. Adding debug session management is a natural extension — not a separate system.

## How It Works

```mermaid
sequenceDiagram
    participant Editor
    participant LSP as basilisk lsp
    participant debugpy as debugpy.adapter
    participant program as User Program

    Note over Editor,LSP: LSP already running (language features active)
    Editor->>LSP: basilisk/startDebugSession { config }
    LSP->>LSP: Resolve Python interpreter
    LSP->>LSP: Verify debugpy installed
    LSP->>LSP: Find free TCP port
    LSP->>debugpy: Spawn "python -m debugpy.adapter --port 54321"
    LSP-->>Editor: { host: "localhost", port: 54321 }
    Editor->>debugpy: DAP Initialize (TCP)
    debugpy-->>Editor: Capabilities
    Editor->>debugpy: Launch (program, args, cwd)
    debugpy->>program: Start with debug hooks
    program-->>debugpy: Hit breakpoint
    debugpy-->>Editor: Stopped event
    Editor->>debugpy: Variables / Stack / Step requests
    debugpy-->>Editor: Responses
    Editor->>debugpy: Disconnect
    debugpy->>program: Terminate
    Editor->>LSP: basilisk/stopDebugSession
    LSP->>LSP: Clean up child process
```

The editor's DAP client connects directly to debugpy over TCP. The LSP just brokers the connection — it doesn't proxy any DAP traffic.

## Part 1: LSP Custom Requests (Rust)

Add two custom requests to the LSP server and a Python resolver module.

### `basilisk/startDebugSession`

The LSP only needs to know which Python to use. All DAP config (program, args, justMyCode, etc.) goes directly from the editor to debugpy after the TCP connection is established.

**Request:**
```json
{
    "python": null
}
```

`python` is optional — if omitted, the LSP resolves it from the workspace venv or system PATH.

**Response:**
```json
{
    "host": "localhost",
    "port": 54321,
    "sessionId": "a1b2c3"
}
```

The LSP waits until debugpy is actually accepting TCP connections before returning. This avoids a race where the editor tries to connect before debugpy is ready.

### `basilisk/stopDebugSession`

**Request:**
```json
{
    "sessionId": "a1b2c3"
}
```

**Response:**
```json
{
    "stopped": true
}
```

### Rust implementation sketch

Add a `debug.rs` module to `basilisk-lsp`:

```rust
use std::collections::HashMap;
use std::net::TcpListener;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::net::TcpStream;
use tokio::time::{sleep, Duration};

/// Tracks active debug sessions spawned by the LSP.
pub struct DebugSessionManager {
    sessions: Mutex<HashMap<String, Child>>,
}

impl DebugSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Spawn debugpy on a free TCP port. Returns (host, port, session_id).
    /// Waits until debugpy is accepting connections before returning.
    pub async fn start_session(
        &self,
        python_path: &str,
    ) -> Result<(String, u16, String), DebugError> {
        let port = find_free_port()?;
        let session_id = generate_session_id();

        let child = Command::new(python_path)
            .args(["-m", "debugpy.adapter", "--port", &port.to_string()])
            .kill_on_drop(true)
            .spawn()
            .map_err(DebugError::SpawnFailed)?;

        self.sessions
            .lock()
            .await
            .insert(session_id.clone(), child);

        // Wait for debugpy to start accepting connections (up to 5s).
        wait_for_port(port, Duration::from_secs(5)).await?;

        Ok(("localhost".to_owned(), port, session_id))
    }

    /// Kill a debug session and clean up.
    pub async fn stop_session(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.lock().await;
        if let Some(mut child) = sessions.remove(session_id) {
            let _ = child.kill().await;
            return true;
        }
        false
    }
}

fn find_free_port() -> Result<u16, DebugError> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(DebugError::PortAllocation)?;
    let port = listener.local_addr()
        .map_err(DebugError::PortAllocation)?
        .port();
    Ok(port)
}

/// Poll until a TCP connection succeeds on the given port.
async fn wait_for_port(port: u16, timeout: Duration) -> Result<(), DebugError> {
    let start = tokio::time::Instant::now();
    loop {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return Ok(());
        }
        if start.elapsed() > timeout {
            return Err(DebugError::Timeout(port));
        }
        sleep(Duration::from_millis(50)).await;
    }
}
```

### Python resolver

The LSP already has workspace roots. The resolver checks them:

```rust
/// Resolve the Python interpreter for a workspace.
pub fn resolve_python(workspace_root: &Path) -> String {
    // 1. BASILISK_PYTHON env var
    if let Ok(p) = std::env::var("BASILISK_PYTHON") {
        return p;
    }

    // 2. Workspace venv
    for venv_dir in &[".venv", "venv"] {
        let bin = if cfg!(windows) {
            workspace_root.join(venv_dir).join("Scripts").join("python.exe")
        } else {
            workspace_root.join(venv_dir).join("bin").join("python")
        };
        if bin.exists() {
            return bin.to_string_lossy().into_owned();
        }
    }

    // 3. System fallback
    if cfg!(windows) { "python".into() } else { "python3".into() }
}

/// Check if debugpy is importable by the given interpreter.
pub async fn check_debugpy(python: &str) -> Result<(), DebugError> {
    let output = tokio::process::Command::new(python)
        .args(["-c", "import debugpy; print(debugpy.__version__)"])
        .output()
        .await
        .map_err(DebugError::SpawnFailed)?;

    if output.status.success() {
        Ok(())
    } else {
        Err(DebugError::DebugpyNotFound(python.to_owned()))
    }
}
```

### Wire into the LSP server

In `server.rs`, add the `DebugSessionManager` to `LspServer` and handle the custom requests via `execute_command`:

```rust
pub struct LspServer {
    client: Client,
    documents: DashMap<Url, DocumentState>,
    workspace_roots: tokio::sync::RwLock<Vec<std::path::PathBuf>>,
    debug_manager: DebugSessionManager,  // NEW
}

// In execute_command handler, add:
"basilisk.startDebugSession" => {
    let python_override = args.first()
        .and_then(|v| v.get("python"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let workspace = self.workspace_roots.read().await;
    let root = workspace.first().map(|p| p.as_path()).unwrap_or(Path::new("."));
    let python = python_override.unwrap_or_else(|| resolve_python(root));

    check_debugpy(&python).await?;

    let (host, port, session_id) = self.debug_manager
        .start_session(&python)
        .await?;

    Ok(Some(serde_json::json!({
        "host": host,
        "port": port,
        "sessionId": session_id
    })))
}

"basilisk.stopDebugSession" => {
    let session_id = args.first()
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let stopped = self.debug_manager.stop_session(session_id).await;
    Ok(Some(serde_json::json!({ "stopped": stopped })))
}
```

Register the commands in `initialize`:

```rust
execute_command_provider: Some(ExecuteCommandOptions {
    commands: vec![
        "basilisk.organizeImports".to_owned(),
        "basilisk.startDebugSession".to_owned(),  // NEW
        "basilisk.stopDebugSession".to_owned(),    // NEW
    ],
    ..Default::default()
}),
```

## Part 2: VS Code Extension

The extension registers a debug type and a factory. When VS Code starts a debug session, the factory asks the LSP to spawn debugpy and returns the TCP port.

### package.json additions

```json
{
  "contributes": {
    "debuggers": [
      {
        "type": "basilisk-debug",
        "label": "Python (Basilisk)",
        "languages": ["python"],
        "configurationAttributes": {
          "launch": {
            "required": ["program"],
            "properties": {
              "program": {
                "type": "string",
                "description": "Absolute path to the Python file to debug.",
                "default": "${file}"
              },
              "args": {
                "type": "array",
                "description": "Command-line arguments passed to the program.",
                "items": { "type": "string" },
                "default": []
              },
              "cwd": {
                "type": "string",
                "description": "Working directory for the program.",
                "default": "${workspaceFolder}"
              },
              "console": {
                "type": "string",
                "enum": ["integratedTerminal", "internalConsole", "externalTerminal"],
                "default": "integratedTerminal"
              },
              "justMyCode": {
                "type": "boolean",
                "default": true
              },
              "stopOnEntry": {
                "type": "boolean",
                "default": false
              },
              "python": {
                "type": "string",
                "description": "Path to the Python interpreter."
              }
            }
          },
          "attach": {
            "properties": {
              "connect": {
                "type": "object",
                "properties": {
                  "host": { "type": "string", "default": "localhost" },
                  "port": { "type": "number" }
                },
                "required": ["port"]
              }
            }
          }
        },
        "configurationSnippets": [
          {
            "label": "Basilisk: Launch Current File",
            "description": "Debug the currently open Python file",
            "body": {
              "name": "Python: Current File (Basilisk)",
              "type": "basilisk-debug",
              "request": "launch",
              "program": "^\"\\${file}\"",
              "console": "integratedTerminal",
              "justMyCode": true
            }
          }
        ],
        "initialConfigurations": [
          {
            "name": "Python: Current File (Basilisk)",
            "type": "basilisk-debug",
            "request": "launch",
            "program": "${file}",
            "console": "integratedTerminal",
            "justMyCode": true
          }
        ]
      }
    ]
  }
}
```

### extension.ts additions

```typescript
// In activate(), after LSP client starts:

class BasiliskDebugAdapterFactory implements vscode.DebugAdapterDescriptorFactory {
  constructor(private lspClient: LanguageClient) {}

  async createDebugAdapterDescriptor(
    session: vscode.DebugSession,
  ): Promise<vscode.DebugAdapterDescriptor> {
    const config = session.configuration;

    // Attach mode: connect directly to user-specified host:port
    if (config.request === 'attach' && config.connect) {
      return new vscode.DebugAdapterServer(
        config.connect.port,
        config.connect.host || 'localhost'
      );
    }

    // Launch mode: ask the LSP to spawn debugpy
    const result = await this.lspClient.sendRequest(
      'workspace/executeCommand',
      {
        command: 'basilisk.startDebugSession',
        arguments: [{ python: config.python ?? null }],
      }
    ) as { host: string; port: number; sessionId: string };

    return new vscode.DebugAdapterServer(result.port, result.host);
  }
}

context.subscriptions.push(
  vscode.debug.registerDebugAdapterDescriptorFactory(
    'basilisk-debug',
    new BasiliskDebugAdapterFactory(client!)
  )
);
```

That's it. The factory sends one LSP request and gets back a port. No process spawning in TypeScript.

## Zed Compatibility

The debug integration is editor-agnostic by design. All logic lives in the LSP — the editor just sends `basilisk.startDebugSession` and connects to the returned TCP port. Nothing in this design is VS Code-specific. When Zed's debug adapter support matures to allow LSP-initiated DAP connections, it will work without changes to the Rust side.

## Data Flow: Attach Session

```mermaid
sequenceDiagram
    participant Editor
    participant debugpy as Remote debugpy.listen()

    Note over Editor: Attach bypasses the LSP entirely
    Editor->>debugpy: TCP connect (host:port)
    Editor->>debugpy: DAP Initialize
    debugpy-->>Editor: Capabilities
    Editor->>debugpy: DAP Attach
    debugpy-->>Editor: Stopped events, variables, etc.
```

For attach, the editor connects directly to the remote debugpy server. The LSP is not involved.

## Component Diagram

```mermaid
graph LR
    subgraph "basilisk lsp (single Rust process)"
        LANG[Language Features<br/>diagnostics, completions, hover, ...]
        DEBUG[Debug Session Manager<br/>spawn debugpy, track sessions]
        PYRES[Python Resolver<br/>venv detection, interpreter lookup]
    end

    subgraph "VS Code Extension (TypeScript)"
        FACTORY["DebugAdapterFactory<br/>(asks LSP for port)"]
        LSPCLIENT["LSP Client<br/>(existing)"]
    end

    subgraph "Any LSP-compatible Editor (Zed, etc.)"
        OTHER["LSP client<br/>(same commands work)"]
    end

    subgraph "User's Python Environment"
        DEBUGPY["debugpy<br/>(pip install debugpy)"]
    end

    LSPCLIENT --> LANG
    FACTORY -->|"basilisk.startDebugSession"| DEBUG
    OTHER --> LANG
    OTHER -->|"basilisk.startDebugSession"| DEBUG
    DEBUG --> PYRES
    LANG --> PYRES
    DEBUG -->|"spawns"| DEBUGPY
```

## Error Handling

When debugpy is not installed, the LSP returns a clear error:

```json
{
    "error": {
        "code": -32001,
        "message": "debugpy not found. Install it: pip install debugpy"
    }
}
```

The editor extension surfaces this as a notification with an action button to run `pip install debugpy` in the terminal.

When the Python interpreter can't be found, the error tells the user exactly what was tried:

```json
{
    "error": {
        "code": -32002,
        "message": "No Python interpreter found. Checked: .venv/bin/python, venv/bin/python, python3. Set BASILISK_PYTHON or create a virtualenv."
    }
}
```

## Design Decisions

**The LSP is the debug adapter.** No separate binary or proxy process. The LSP spawns debugpy and tells the editor where to connect. One process does everything.

**TCP, not stdin/stdout.** The LSP already owns stdin/stdout for LSP traffic. debugpy listens on a TCP port. The editor's DAP client connects directly — zero proxying overhead.

**debugpy from the user's environment.** No bundling. The user's Python has debugpy installed (`pip install debugpy`). If it's missing, the LSP returns a clear error. This avoids platform-specific wheel builds entirely.

**Session lifecycle managed by the LSP.** The LSP tracks spawned debugpy processes and cleans them up on stop requests or shutdown. No orphaned processes.

## Python Version Targeting

**Primary target: Python 3.12** — this is the canonical version for the entire Basilisk project.

debugpy uses `sys.settrace` (via pydevd internally) on Python 3.12. This is the traditional debugging mechanism and is fully supported.

### Python 3.14 and the Future of Debugging

Python 3.14 introduces two significant debugging enhancements that do **not** affect our current implementation but are worth tracking:

1. **PEP 768 — Safe External Debugger Interface** (`sys.remote_exec`): Allows attaching a debugger to a running process by PID without requiring the process to have been started with debugpy. This is a new *attach mechanism*, not a replacement for DAP. debugpy still handles the protocol layer.

2. **`sys.monitoring` backend for pdb/bdb** (built on PEP 669): Near-zero overhead breakpoints compared to `sys.settrace`'s 4-5x slowdown. The `sys.monitoring` API itself exists in 3.12 (PEP 669), but the pdb/bdb backend that uses it is 3.14-only. debugpy/pydevd has not yet adopted `sys.monitoring` internally.

**Neither change breaks debugpy.** debugpy 1.8.20+ has full Python 3.14 support with platform-specific wheels. Our architecture (LSP spawns debugpy, editor connects via DAP over TCP) works identically on 3.12 and 3.14.

**Future opportunity:** When debugpy adopts `sys.monitoring` internally, breakpoint performance will improve dramatically for 3.14+ users with no changes needed on our side. If we later want to support PEP 768's attach-by-PID, that would be an additive feature behind a version check — not a rewrite.

## Prerequisite

Users need debugpy installed in their Python 3.12 environment:

```bash
pip install debugpy
```

The LSP checks for this on the first debug request and returns an actionable error if it's missing.

## Licensing

debugpy is MIT-licensed. Basilisk invokes it as a subprocess — no bundling, no licensing concerns.
