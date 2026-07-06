# Basilisk Formatting & Import Hygiene — Specification {#LSPFMT}

Formatting and import cleanup are **LSP capabilities served by the binary**, identically to every consumer (VS Code, Zed, Neovim, and the `basilisk` CLI). This document is the single source of truth for both; `LSP-ARCHITECTURE-SPEC.md` ([LSPARCH](LSP-ARCHITECTURE-SPEC.md#LSPARCH)) points here. Editor specs MUST reference this, not duplicate it.

Design principle (per [LSPARCH](LSP-ARCHITECTURE-SPEC.md#LSPARCH)): the LSP owns the feature; editor frontends are thin clients. Nothing here may be implemented per-editor.

---

## Decision: everything in the binary, no `ruff` subprocess {#LSPFMT-DECISION}

Formatting and import hygiene are **self-contained in the `basilisk` binary**. The external `ruff` CLI is **jettisoned** — it is never spawned. Two independent mechanisms replace it:

| Concern | Old (removed) | New |
|---|---|---|
| Formatting | spawn `ruff format` | link the `ruff_python_formatter` crate, call it in-process ([LSPFMT-ENGINE]) |
| Import hygiene | spawn `ruff check --select I/F403/E401 --fix` | native AST fixers in the binary ([LSPFMT-IMPORTS]) |

Rationale — one engine in one binary is the only design uniform across all four consumers, removes the PATH/bundle/silent-no-op failure mode, and eliminates version skew between "the ruff that parses" and "the ruff that formats". The CLI-bundle and VS-Code-extension-dependency approaches were rejected because each serves at most one editor and splits ownership away from the LSP.

---

## Formatter engine: embedded Ruff, pure passthrough {#LSPFMT-ENGINE}

We already compile Ruff into the binary for parsing (`ruff_python_parser`/`ruff_python_ast`, pinned at one rev in `Cargo.toml`). Formatting links **one more crate from the same repo at the same rev** — `ruff_python_formatter` — and calls it directly (no subprocess, no bundled binary, always present).

- **Pure passthrough.** Output MUST be byte-identical to `ruff format` at the pinned rev. Basilisk applies **zero** original transforms to formatting output. The moment we deviate, calling it "Ruff" becomes false and it must be renamed with its own output contract ([LSPFMT-HONESTY]).
- **Config-respecting.** The engine reads the project's `[tool.ruff.format]` options (line length, quote style, indent style, magic trailing comma) from `pyproject.toml` so Basilisk's output matches what the user's own `ruff format` would produce. Options flow from the config loader (`WorkspaceConfig`), not hard-coded defaults.
- **Style is Black-compatible**, with Ruff's [documented deviations](https://docs.astral.sh/ruff/formatter/black/) — link the source, never claim byte-identical-to-Black.

## Provenance & versioning — the user must know which Ruff {#LSPFMT-PROVENANCE}

Because the formatter's output is user-visible, its provenance MUST be user-visible (unlike the invisible parser embedding). The embedded formatter version is a compile-time constant derived from the pinned `Cargo.toml` rev. Surface it in every place a user would look:

1. **`basilisk --version`** lists embedded engine versions (e.g. `Ruff formatter: 0.15.17`) alongside the Shipwright build stamp ([CHKARCH-ARCH-BUILD-VERSIONINFO](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-BUILD-VERSIONINFO)).
2. **LSP `InitializeResult.serverInfo.version`** carries it, so every client (VS Code, Zed, Neovim) surfaces the same identity.
3. **Output/log channel** notes it on format (`Formatted with embedded Ruff <ver>`) — no silent magic.
4. **Release notes** MUST list the Shipwright manifest binary versions AND the embedded Ruff formatter version for the release ([LSPFMT-RELEASE-NOTES]).
5. **`THIRD-PARTY-LICENSES`/NOTICE** ships Ruff's MIT license — the legal answer to "where did this come from".

### Release-notes exposure {#LSPFMT-RELEASE-NOTES}

Each release's notes MUST enumerate, from a single generated source (not hand-typed): every `shipwright.json` component + version, and the embedded Ruff formatter version. Generated so it cannot drift from what actually shipped; a user reading the notes can tell exactly which formatter bytes they will get.

## Configuration {#LSPFMT-CONFIG}

One new setting replaces the two removed `basilisk.ruff.*` settings (there is no `ruff` binary, so `executablePath` is meaningless):

| Setting | Type | Default | Description |
|---|---|---|---|
| `basilisk.formatter` | `enum` | `"ruff"` | Formatter engine: `"ruff"` (embedded, default) or `"none"` (disable). Future: `"basilisk"`. |

- `"ruff"` — the embedded Ruff formatter ([LSPFMT-ENGINE]).
- `"none"` — the server does not advertise formatting capability; editors show no Basilisk formatter.
- Future `"basilisk"` — a Basilisk-native formatter with its own contract (not yet built). Reserved so the flag is forward-stable.

Import hygiene ([LSPFMT-IMPORTS]) is native and always available as code actions — not gated by this flag.

## Server capabilities {#LSPFMT-CAPABILITIES}

The server advertises formatting as LSP capabilities, so all four consumers get them from one change:

- `documentFormattingProvider` — whole-document (existing).
- `documentRangeFormattingProvider` — Format Selection (shares the embedded engine; Ruff widens the selection to whole logical lines).
- `documentOnTypeFormattingProvider` — format-as-you-type (future, optional).

All are attributed to the single identity **"Basilisk"** (`serverInfo.name`); VS Code shows the extension `displayName`, Zed/Neovim show the server id. There is no per-provider display name in LSP or VS Code — the Ruff engine is disclosed via [LSPFMT-PROVENANCE], never a picker label. When `basilisk.formatter` is `"none"`, none of these are advertised.

## Native import hygiene {#LSPFMT-IMPORTS}

Reimplemented in the binary on the Ruff AST we already own — **no `ruff check` subprocess**. Owning the code lets us extend the behavior and removes the version-coupling and silent-no-op failure modes.

| Code action | Kind | Replaces |
|---|---|---|
| Organize imports | `SOURCE_ORGANIZE_IMPORTS` | `ruff check --select I --fix` |
| Expand wildcard imports | `QUICKFIX` | `ruff check --select F403 --fix` |
| Split multiple imports on one line | `QUICKFIX` | `ruff check --select E401 --fix` |

Behavior parity with the corresponding Ruff fixers is the acceptance bar (regression-tested against representative fixtures). "Organize imports" ordering follows isort semantics.

## Per-client wiring (thin) {#LSPFMT-CLIENTS}

Everything substantive is server-side; clients only wire triggers and one opt-in:

- **VS Code** — provider label is the extension `displayName` ("Basilisk"). Format-on-save wiring; a one-time, dismissible **opt-in** prompt to set `editor.defaultFormatter` to Basilisk. MUST NOT hijack a user's existing `editor.defaultFormatter`; if two formatters are present, VS Code's standard one-time picker governs.
- **Zed** — `"formatter": "language_server"` for Python.
- **Neovim** — `vim.lsp.buf.format({ name = "basilisk" })`.
- **CLI** — `basilisk format [paths]` uses the same embedded engine directly.

## Honesty guardrail {#LSPFMT-HONESTY}

- Formatter = **pure Ruff passthrough**; all Basilisk "control/flexibility" lives on the import-hygiene side ([LSPFMT-IMPORTS]) and future `"basilisk"` provider, never in the `"ruff"` formatter's output.
- Docs MUST state plainly that the formatter **is the Ruff formatter, embedded** (no separate install), and mostly link to the [Ruff formatter docs](https://docs.astral.sh/ruff/formatter/) rather than re-document formatting behavior we don't own.
