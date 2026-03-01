# LSP Implementation Plan

> **Spec**: `docs/lsp-spec.md` — read this before touching any code.
>
> **Current state**: Subprocess only. Extension runs `basilisk check --output json <file>` on
> save/open. No LSP server exists.

---

## Work Items

| WI | Title | Crate(s) | Depends on |
|---|---|---|---|
| WI-L1 | tower-lsp scaffolding + `basilisk lsp` subcommand | basilisk-lsp, basilisk-cli | — |
| WI-L2 | Document store + textDocument lifecycle handlers | basilisk-lsp | WI-L1 |
| WI-L3 | Diagnostics push (publishDiagnostics + span→UTF-16) | basilisk-lsp | WI-L2 |
| WI-L4 | Hover (textDocument/hover) | basilisk-lsp | WI-L3 |
| WI-L5 | Code actions (textDocument/codeAction) | basilisk-lsp | WI-L3 |
| WI-L6 | Extension: replace subprocess with vscode-languageclient | vscode-extension | WI-L3 |
| WI-L7 | Integration tests (13 required cases) | basilisk-lsp | WI-L3 |

---

## Sequencing

```
Phase 1 (serial):
  WI-L1 → WI-L2 → WI-L3

Phase 2 (parallel, all depend on WI-L3):
  WI-L4   Hover
  WI-L5   Code actions
  WI-L6   Extension rewrite
  WI-L7   Integration tests
```

---

## Agent Assignments

| Agent | WI |
|---|---|
| Agent A | WI-L1, WI-L2, WI-L3 (serial, foundational) |
| Agent B | WI-L7 (tests — can start writing skeletons during WI-L1/L2) |
| Agent C | WI-L6 (extension rewrite — can start during WI-L1/L2) |
| Agent A (after L3) | WI-L4, WI-L5 (hover + code actions) |

---

## Rules

- Build must stay GREEN at all times
- No `.unwrap()` in server code — exception: top-level `run_server` entry point (documented)
- `cargo clippy` must pass after every WI
- Do NOT delete failing tests — add more
- File locks: register in `coordination/filelocks.md` before touching a file
