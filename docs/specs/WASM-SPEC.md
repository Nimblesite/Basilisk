# Basilisk WASM {#WASM}

The Basilisk checker compiled to WebAssembly, so a browser can check a Python
source string with no server, no filesystem, and no network. This is the engine
behind the playground site requested in
[#323](https://github.com/Nimblesite/Basilisk/issues/323); the playground UI
itself is out of scope here and tracked in
[WASM-PLAN.md](../plans/WASM-PLAN.md).

Crate: `crates/basilisk-wasm`. Target: `wasm32-unknown-unknown`.

The whole point is that the browser answers **exactly** what the CLI answers.
The crate therefore contains no checking logic of its own — it is an adapter
that drives the same pipeline stages `basilisk check` drives, and any rule
change reaches the browser for free.

## Pipeline {#WASM-PIPELINE}

One-shot, in-memory, stage-for-stage identical to the CLI's `resolve_file_imports`
+ `process_file` ([`pipeline/mod.rs`](../../crates/basilisk-cli/src/pipeline/mod.rs)),
with `parse_file` swapped for `parse_source` because there is no file to read:

| # | Stage | Call |
|---|---|---|
| 1 | Parse | `basilisk_parser::parse_source(source, path)` |
| 2 | Resolve scopes | `basilisk_resolver::resolve_with_target` / `resolve` |
| 3 | Resolve imports | `basilisk_checker::imports::resolve_module_imports` |
| 4 | Check | `basilisk_checker::check_with_config` |

Stage 3 is the same engine the LSP uses: `basilisk_lsp::import_resolver` only
re-exports it from `basilisk_checker::imports`, so reaching it costs no LSP
dependency (and tokio/tower-lsp never enter the wasm graph).

A parse failure is reported as a diagnostic rather than an exception, matching
how the CLI renders an unanalysable file ([WASM-DIAGNOSTIC]).

## No ambient authority {#WASM-NOFS}

The module cannot touch a filesystem, a network, a clock, or a thread, and does
not ask for permission to:

- **Every search root is empty.** `ImportSearchPaths` is built with no `roots`,
  `extra_paths`, `stub_paths`, `workspace_members`, `site_packages`, or uv
  `registry`. The directory-listing cache that backs import resolution is
  therefore never asked to read a directory.
- **The stdlib is embedded, not read.** `basilisk_stubs::typeshed::bundled_snapshot()`
  decodes the `include_bytes!`-embedded typeshed ZIP into an in-memory snapshot
  ([STUBRES-TYPESHED-BASELINE](CHECKER-STUB-RESOLUTION-SPEC.md)), which is
  handed to the checker as the active `ActiveTypeshed`. `import typing` resolves
  in the browser exactly as it does on disk.
- **No threads are spawned.** Salsa's `rayon` feature supplies one trait impl
  for `rayon::iter::Either` and nothing else — salsa's source contains no
  `par_iter`, `ThreadPool`, `rayon::spawn`, `par_bridge` or `install(` call. The
  module needs no `SharedArrayBuffer` and no COOP/COEP headers, so it can be
  served from plain static hosting.
- **Nothing is downloaded.** `basilisk-stubs` is forbidden an HTTP client by
  [TYPESHEDRT-SEGREGATION](CHECKER-STUB-RESOLUTION-SPEC.md), which the wasm
  build inherits.

## Public API {#WASM-API}

One entry point, taking and returning JSON strings so the boundary stays a
stable contract rather than a generated object graph:

```ts
check(source: string, options_json: string): string
```

`options_json` accepts `{}` and every field is optional:

| Field | Type | Meaning when absent |
|---|---|---|
| `path` | string | `"<playground>.py"` — the label diagnostics are reported against |
| `python_version` | string, `"3.13"` | No target evidence; version-gated stubs and `sys.version_info` guards are not narrowed. Deliberately not a default release ([CHKARCH-VERSION-TARGET](CHECKER-ARCHITECTURE-SPEC.md)) |

Rule selection and severity are **not** callable options: every check runs the
default configuration — every PEP typing-spec rule at `error`, house-style rules
off ([CHKARCH-CONFIGURATION-ONLY](CHECKER-ARCHITECTURE-SPEC.md)) — so the
playground answers what a user gets out of the box. Per-call rule configuration
is deferred ([WASM-PLAN.md](../plans/WASM-PLAN.md)).

Unknown fields are refused rather than ignored: a playground that silently
discarded `python_verison` would answer a question the reader did not ask.
Malformed `options_json` is an error result, never a panic. There is no
`unwrap`, `panic!`, or `unsafe` in the engine.

## Result shape {#WASM-DIAGNOSTIC}

The result is `{"diagnostics": [...]}`, whose entries are **field-for-field the
CLI's `--output json` entries** ([`output/json.rs`](../../crates/basilisk-cli/src/output/json.rs)) —
`code`, `severity`, `message`, `path`, `line`, `col`, `end_line`, `end_col`,
with 1-based positions and `code: null` for a file that could not be analysed.
A consumer can move between the CLI and the browser without a second parser.

Byte offsets become line/column through the shared
`basilisk_common::text::LineIndex`, the same index the CLI renders with, so a
span never lands one column apart between the two.

The DTO is redeclared rather than imported because the CLI's is `pub(super)`
inside a crate that cannot compile to wasm (it spawns processes). The field
list is asserted against the CLI's in test, so drift fails the build rather
than shipping.

## Deliberate limits {#WASM-LIMITS}

These are design decisions, not gaps to be quietly filled:

- **One file per call.** No sibling-module or relative-import resolution.
  Multi-file support needs an in-memory VFS behind import resolution and is
  planned, not shipped ([WASM-PLAN.md](../plans/WASM-PLAN.md)).
- **No third-party packages.** With no `site_packages`, `import numpy` reports
  unresolved. That is the correct answer for an environment with nothing
  installed.
- **No execution.** Basilisk checks Python; it does not run it. Running user
  code is the one feature that would require a server, and it is not wanted.

## Build {#WASM-BUILD}

`crate-type = ["cdylib", "rlib"]` — `cdylib` for the browser, `rlib` so the
engine is unit-testable on the host.

The binding layer is `#[cfg(target_arch = "wasm32")]`, so a native `cargo test`
compiles and exercises the engine without `wasm-bindgen` in the graph. This is
the split [ZED-SPEC.md](ZED-SPEC.md) already uses: pure logic in its own module,
thin generated glue beside it.

Type inference recurses deeper than the 1 MiB wasm default stack allows, and a
wasm stack overflow aborts the module rather than unwinding — the playground
would die with nothing to render. `.cargo/config.toml` therefore links
`wasm32-unknown-unknown` with `-C link-arg=-zstack-size=16777216`, the same
order as the dedicated large-stack analysis thread the native server runs on
([LSPARCH-ARCH-STACK](LSP-ARCHITECTURE-SPEC.md)). That is reserved address
space, not resident memory.

The unoptimised release artefact measures **7.2 MB**, most of it the embedded
typeshed. Size work (`opt-level="z"`, `wasm-opt`, lazy loading) and the ratchet
that holds it are in [WASM-PLAN.md](../plans/WASM-PLAN.md).

The engine is a **separate build step from the site**. `npm run build` is
Eleventy alone and has no Rust dependency; `npm run build:wasm` compiles this
crate into `website/src/assets/wasm`. The site is otherwise generated from
committed data, so a checker that does not compile must not be able to take
every page down with it — it can only cost the playground its engine. CI and
the release deploy run `build:wasm` as their own explicit step, and a site
served without one reports the missing engine on the playground page instead of
hanging on a spinner.

## Testing {#WASM-TESTING}

Because the engine is an `rlib`, every test runs on the host under the normal
`make test` coverage gate — no browser and no wasm runtime in the loop:

| Test | Proves |
|---|---|
| `stdlib_import_resolves_from_the_embedded_bundle` | [WASM-NOFS] — `import typing` resolves with every search root empty |
| `clean_source_reports_no_diagnostics` | The default config does not fire house-style rules |
| `type_error_is_reported_with_cli_positions` | [WASM-DIAGNOSTIC] — 1-based line/col agree with `LineIndex` |
| `third_party_import_is_unresolved` | [WASM-LIMITS] — no `site_packages` means no silent resolution |
| `parse_failure_becomes_a_diagnostic` | [WASM-PIPELINE] — a syntax error is data, not an exception |
| `malformed_options_json_is_an_error_not_a_panic` | [WASM-API] |
| `diagnostic_fields_match_the_cli_json_contract` | [WASM-DIAGNOSTIC] — drift between the two DTOs fails the build |

That the crate genuinely builds for the browser is proved by compiling it for
`wasm32-unknown-unknown` in CI, not by asserting it in a host test.
