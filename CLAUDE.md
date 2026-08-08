<!-- agent-pmo:f87d349 -->
# CLAUDE.md

Basilisk is a strict Python type checker written in Rust. It must be correct on Python it has never seen.

# The Job

1. **Find text-pattern-matching logic and delete it.** A rule whose behaviour depends on the *spelling* of its input rather than its *meaning* is broken, whatever it scores. Finding and removing it outranks every other task here.
2. **Rebuild what was deleted on the real AST** — every decision from resolved bindings, types, and symbol identity; never from how the source happens to be spelled. `from typing import Final as F` behaves identically to `typing.Final`; reformatting a file changes no diagnostic.
3. **Prove which rules actually work, and name the ones that never did.** A rule carried by text matching was never a rule. Saying so out loud is the deliverable.
4. **Add spelling mutations to tests that currently prove nothing**. Illegal: tests that observe diagnostics and assert nothing, reporting "ok" or similar while proving nothing.

**Not the job: raising the conformance score.** That number is what caused this. A change that lifts it without improving analysis of arbitrary Python is a regression.

RULE PRIORITY: 

1) PEP RULES THAT WERE ALREADY WORKING CORRECTLY BEFORE THE CULL
2) PEP RULES THAT WERE SOMEHWHAT CORRECT
3) PEP RULES THAT WERE NOT CORRECT MUST BE FLAGGED AS MOSTLY BROKEN. THESE ARE NOT A PRIORITY RIGHT NOW

OTHER RULES LIKE BASILISK RULES ARE ONLY TO BE WORKED ON IF THEY ARE SIMPLE AND EASY TO FIX

## Why

Basilisk was **removed from the python/typing conformance results** on 2026-08-05, at its own author's request — [python/typing#2330](https://github.com/python/typing/pull/2330), reverting [#2316](https://github.com/python/typing/pull/2316). The reason: *"Many of Basilisk's rules match against raw source text and hard-coded typing symbol names instead of resolved symbols on the AST."* Semantics-preserving edits to the suite — renaming imports, adjusting whitespace — broke **113 of 141 test files**. The score was real; the checker under it was not.

## How to find it

- Raw source-text matching — `.contains` / `starts_with` / `ends_with` on user code.
- Hard-coded symbol spellings instead of resolved identity: `t == "typing.Final"`, `text.starts_with("Callable[")`, `import.module == "typing"`, a whitelist of `int`/`str`/`isinstance` names. **Builtins are not an exception** — Python lets any name be shadowed, rebound, or aliased, so builtin uses resolve through the binding table like everything else.
- Any regex over Python source.
- Logic keyed to a test fixture: rule files named after conformance tests (`generics_base_class_2.rs`, `constructors_call_init`), branches for shapes only the suite contains, comments citing a test file as justification.
- Detection that fires on formatting — line breaks, spacing, quote style, comment text, statement order.

## What to do when you find it

In this order — **do not fix it in place, do not leave a TODO**:

1. **Write a test that fails** because of the incorrect code — pin the real defect: an aliased import, a reformatted source, a shape the conformance suite never contains.
2. **Delete the offending code.** Delete the text-matching function body, not its call site: the call sites are the map of what has to be rebuilt.
3. **Report what you deleted, why, and which tests now fail.**
4. **STOP**

Never restore a deleted text helper to unblock a compile, and never patch the text path while "waiting for" its replacement — no production verdict may come from text. A checker with fewer rules and visible failing tests is the correct outcome; a diagnostic that only fires on one spelling looks like coverage and isn't.

**A failing test that pins real incorrect behaviour is worth more than a passing fixture carried by logic that does not analyse code.** Given the choice, take the failing test — every time.

## What a correct rule looks like

- Decides on the **resolved semantic model** from `basilisk-resolver`, never tokens or text.
- Named for the **typing-spec concept** it implements ([the typing spec](https://typing.python.org/en/latest/spec/) and [`typing` docs](https://docs.python.org/3/library/typing.html)), not a test file.
- Survives **semantics-preserving mutation**: aliased imports, reformatting, reordering → identical diagnostics.
- Tested against Python the conformance suite has never contained.

# Proving Rules Work

Judge a test by what it would catch, never by whether it's green.

- **Meaning, not spelling**: every rule test gets an aliased-import and a reformatted variant asserting identical diagnostics. The harness that would enforce this suite-wide ([CHKARCH-TESTING-SEMANTIC-MUTATION]) **does not exist yet** — until it does, every rule is unverified and must be described that way.
- A test copied from `conformance/tests/` cannot detect a rule fitted to `conformance/tests/`. Write new Python.
- `let _ = run(source)?` proves a function returned. It is not a test. Every test asserts a specific diagnostic present or absent.
- **NEVER delete a failing test, remove a failure-causing assertion, reduce assertiveness, or ignore tests.** Broken functionality gets MORE failing tests, never fewer. Failures left by a deletion are the accurate map — keep them.

# Conformance

A regression indicator to read and report. Nothing else.

- **Never publish, quote, or market a figure** — nothing may imply Basilisk is in the official results. Never re-submit to python/typing until the mutation harness passes clean and an external audit has run.
- **Never a gate, threshold, or ratchet anywhere** — that floor was deleted from `coverage-thresholds.json` on 2026-08-08 at the user's direction because it was the incentive that produced the fitting. Do not reintroduce it in that file, `make test`, CI, or a script.
- **A drop caused by removing text-matched logic is progress.** Record it and say so plainly — never restore the code or fake a pass.
- Never move the number by touching the scoreboard: rule-suppressing config, deleting source to dodge a failure, hand-editing `conformance/conformance_status.csv`.
- `python3 conformance/run_conformance.py` stays honest: fresh `git clone` of `python/typing@main`, clean `cargo build --release` from THIS checkout, the suite's own unmodified `src/main.py --only-run basilisk` via `BASILISK_BIN`. A vendored scorer, injected adapter, cached fixtures, or committed results standing in for a live run is a **BUILD FAILURE**.

# Ground Rules

- **Never parse with strings or regex** — `ruff_python_parser`, `basilisk-resolver`, and the `basilisk-canonical` binding table only.
- [Pyrefly](https://pyrefly.org/en/docs/) and [Pyright](https://microsoft.github.io/pyright/#/) are references to compare against — NEVER copy their code.
- Use your judgment — do NOT stop to ask questions. Reporting a deletion isn't a question: report and continue.
- No `unsafe`, no `unwrap()`, no `panic!`/`todo!`/`unimplemented!` — `Result`/`Option` with `?` and real error types.
- Build scripts live in the Makefile.
- Don't use Git unless asked. Never push to `main`. Never list the agent as co-author.
- NEVER kill a VS Code process — it disrupts active debugging and test sessions.
