# Basilisk Debug Integration via debugpy {#LSPDEBUG}

## Goal {#LSPDEBUG-GOAL}

The Basilisk LSP **is** the debug adapter. When the editor needs to debug Python, it sends a custom LSP request. The LSP spawns debugpy on a TCP port and tells the editor where to connect. No separate binary, no separate process, no bundling. One LSP, both editors.

## Architecture {#LSPDEBUG-ARCH}

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

The LSP already runs as a long-lived process. It already knows the workspace roots and can resolve the Python interpreter. Adding debug session management is a natural extension — not a separate system. The design is editor-agnostic: any LSP-compatible editor sends `basilisk.startDebugSession` and connects to the returned TCP port.

## How It Works {#LSPDEBUG-FLOW}

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

## LSP Custom Requests {#LSPDEBUG-LSP}

### startDebugSession {#LSPDEBUG-START}

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

### stopDebugSession {#LSPDEBUG-STOP}

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

### Rust Implementation {#LSPDEBUG-RUST}

`DebugSessionManager` in `basilisk-lsp/src/debug.rs` owns session lifecycle: spawning debugpy on a free TCP port, polling until the port accepts connections, tracking active sessions, and killing child processes on stop or shutdown.

### Python Resolver {#LSPDEBUG-PYRES}

The resolver finds the Python interpreter using a three-step cascade: (1) `BASILISK_PYTHON` environment variable, (2) workspace virtualenv (`.venv/bin/python` or `venv/bin/python`), (3) system `python3` (or `python` on Windows). Before spawning debugpy, `check_debugpy` verifies the interpreter can import debugpy and returns `DebugError::DebugpyNotFound` if it cannot.

### LSP Server Wiring {#LSPDEBUG-WIRE}

Add `DebugSessionManager` to `LspServer` and handle `basilisk.startDebugSession` / `basilisk.stopDebugSession` in the `execute_command` dispatch. Register both commands in the `initialize` response's `executeCommandProvider`.

## VS Code Extension {#LSPDEBUG-VSCODE}

See [VSIX-SPEC.md VSIX-DAP](VSIX-SPEC.md#VSIX-DAP) for VS Code-specific DAP implementation.

## Attach Session Flow {#LSPDEBUG-ATTACH}

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

## Component Diagram {#LSPDEBUG-COMPONENTS}

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

## Error Handling {#LSPDEBUG-ERRORS}

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

## Python Version Targeting {#LSPDEBUG-PYTHON}

**Primary target: Python 3.12** — this is the canonical version for the entire Basilisk project. debugpy uses `sys.settrace` (via pydevd internally) on Python 3.12, which is fully supported.
