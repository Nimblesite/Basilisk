# basilisk-cli

The `basilisk` binary. It is inert.

Basilisk's type checker was producing incorrect results, so it no longer runs. Every invocation — bare `basilisk`, every former subcommand, every flag — prints the approved statement to stderr and exits `4`. Stdout stays empty. No file is read or written, and no server starts. `--version` is the only surface that still answers, so package managers and installed editor extensions get a reply instead of hanging.

Implements [WITHDRAWAL-INERT](../../docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-INERT). The statement itself is generated from that spec into `src/withdrawal_notice.txt` by `scripts/gen_withdrawal_copy.py` and drift-gated in CI, so this crate cannot print its own version of it.

- **Exit code** — `4` (unlisted), always. Never `0`: a pipeline that still calls Basilisk must fail loudly rather than read a clean run into a checker that was wrong. Never `1`: "error diagnostics were found" would be one more incorrect result. See [CHKARCH-CLI-EXITCODES](../../docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI-EXITCODES).
- **Dependencies** — Shipwright only. The parser, resolver, checker and language server are not linked in; the binary cannot analyse anything even by accident.
- **Tests** — `tests/inert_cli.rs` drives the real binary over every argument shape and asserts the exact stderr bytes, the empty stdout, the exit status, and that nothing on disk changed.
