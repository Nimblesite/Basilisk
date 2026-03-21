# Fix: VSCode command double-registration / timing crash

## Context

The Basilisk VSCode extension crashes on startup with `command 'basilisk.uv.sync' already exists`. Root cause:

1. The extension registers commands client-side via `vscode.commands.registerCommand()`
2. The LSP server advertises the SAME commands in `executeCommandProvider` capabilities
3. `vscode-languageclient`'s `ExecuteCommandFeature.initialize()` calls `registerCommand()` again — crash

## Ironclad Rule

**The LSP server is the single source of truth for commands.** It MUST advertise ALL commands it handles via `executeCommandProvider` capabilities. No editor extension (VS Code, Neovim, Zed) is allowed to pre-register commands that the server advertises. The client discovers commands from the server's capabilities response — never the other way around.

This rule is codified in `LSP-ARCHITECTURE-SPEC.md` § Command Registration Rule, and referenced from `VSIX-SPEC.md`, `ZED-SPEC.md`, and `NEOVIM-SPEC.md`.

## Plan

### Step 1: Rust — Server advertises ALL commands

**File:** `crates/basilisk-common/src/lib.rs`

`ALL` must contain every command the server handles. No exceptions. Including `ORGANIZE_IMPORTS`.

### Step 2: TypeScript — Remove all client-side command registrations for server commands

**File:** `vscode-extension/src/commands.ts`

Remove `registerFixCommands`, `registerAdoptCommands`, `registerUvCommands`. The extension must NOT call `vscode.commands.registerCommand()` for any command that the server advertises. Only client-only commands (`restartServer`, `showOutput`) stay.

`organizeImports`: in LSP mode, the server advertises it — no client registration needed. In subprocess mode (no LSP), the extension registers it with a ruff CLI fallback. Registration is gated on `!useLsp` in `extension.ts`.

### Step 3: TypeScript — Client-side UI via executeCommand middleware

**File:** `vscode-extension/src/lsp-client.ts`

The `executeCommand` middleware intercepts server-advertised commands to inject client-side UI:
- **Editor URI injection**: `fixFile`, `adoptFile`, `unadoptFile` need the active editor URI as arg
- **Input prompts**: `uv.add`, `uv.addDev`, `uv.remove` prompt for package name before sending
- **Toast messages**: `uv.sync`, `uv.lock`, `uv.createEnv` show success notification after

This is the correct place for client-side behavior — NOT `registerCommand`.


## Files to modify

1. `crates/basilisk-common/src/lib.rs` — `ALL` contains every command
2. `vscode-extension/src/commands.ts` — no server command registrations
3. `vscode-extension/src/lsp-client.ts` — middleware handles client-side UI
4. `vscode-extension/src/extension.ts` — organizeImports only in subprocess mode
5. `crates/basilisk-lsp/tests/lsp/ws_test_capabilities.rs` — assert all commands advertised

## Deployment notes

No changes to binary distribution or release pipeline. The `executeCommandProvider` change is purely LSP protocol level.

## Verification

1. `cd vscode-extension && npm run compile && npm run lint` — zero errors
2. `cargo check -p basilisk-common -p basilisk-lsp` — compiles
3. `cargo test -p basilisk-lsp --test ws_core_tests test_ws_initialize_advertises_execute_command_provider` — passes
4. Extension starts without "command already exists" crash

## Todo

- [x] Step 1: Server advertises ALL commands in `crates/basilisk-common/src/lib.rs`
- [x] Step 2: Remove client-side server command registrations in `commands.ts`
- [x] Step 3: Client-side UI via middleware in `lsp-client.ts`
- [x] Step 4a: `extension.ts` — organizeImports only in subprocess mode
- [x] Step 4b: Rust capability test — assert all commands advertised
- [x] Step 4c: `LSP-ARCHITECTURE-SPEC.md` — Command Registration Rule added
- [x] Step 4d: `VSIX-SPEC.md` — reference added
- [x] Step 4e: `ZED-SPEC.md` — reference added
- [x] Step 4f: `NEOVIM-SPEC.md` — reference added
- [x] Verify: TypeScript compiles and ESLint passes
- [x] Verify: Rust compiles and all tests pass
