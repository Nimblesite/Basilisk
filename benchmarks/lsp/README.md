# LSP editing-loop benchmark

Implements [CHKARCH-INCREMENTAL-SALSA] measurement support. See
docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA

`lsp_bench.py` drives a real `basilisk lsp` binary over stdio JSON-RPC and
times the editing-loop scenarios where incrementality matters. It is
**binary-agnostic** — point it at any two builds (e.g. `main` vs a feature
branch) to get an A/B comparison of the same scripted session.

This is a manual dev tool: no `make` target, no CI gate, no committed
baseline. The CLI batch-path ratchet remains `make bench`
([CHKARCH-TESTING-BENCH-RATCHET]); this harness exists because the salsa
engine's wins live in the *editing loop*, which the CLI benchmark cannot see.

## Scenarios

The driver generates a synthetic **crossModule** workspace — `lib.py`
exporting 20 typed functions plus N files, a configurable fraction importing
lib and the rest unrelated modules — then measures (medians over in-run
samples; time is "notification sent → last expected `publishDiagnostics`
received", tracked by exact URI set, not a quiet-period guess):

| scenario | what it exercises |
|---|---|
| `scan` | initialize → startup diagnostics complete for all N files |
| `rename` | didChange on the open `lib.py` renaming an export → dependent refresh |
| `restore` | the inverse rename (second sample of the same path) |
| `body` | didChange on `lib.py` editing a function body only (exports unchanged) |
| `keystroke` | didChange inside an open consumer's function body |

## Usage

```bash
cargo build --release --bin basilisk

# 1000-file workspace, 50% importers (default), labelled output as JSON lines
python3 benchmarks/lsp/lsp_bench.py target/release/basilisk 1000 my-branch

# 10% importers — shows refresh cost scaling with the affected set
IMPORTER_EVERY=10 python3 benchmarks/lsp/lsp_bench.py target/release/basilisk 1000 my-branch

# A/B against another build
python3 benchmarks/lsp/lsp_bench.py /path/to/main/basilisk 1000 main
```

Each invocation prints one JSON object:
`{"label": ..., "n_files": ..., "scan": ..., "rename": ..., "restore": ..., "body": ..., "keystroke": ...}`
(seconds). Run each configuration ≥3 times and compare medians — single runs
jitter by a few milliseconds.

Interpretation guide: `main`-era refresh cost scales with the workspace;
the salsa engine's scales with the affected set, so widen the gap by lowering
the importer fraction (`IMPORTER_EVERY`). `body` isolates export-set
backdating (nothing downstream should recompute). `scan` is the one-time
startup cost of priming the engine.
