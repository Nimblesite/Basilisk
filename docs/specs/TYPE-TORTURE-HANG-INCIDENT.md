# Type-torture hang report — recursive bases {#TORTURE-HANG}

**Audience:** whoever is working on `benchmarks/torture/` — the type-torture
scoreboard, the first slice of
[the narrowing/inference plan](../plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md).
**Where the code lives:** the torture suite (`run_torture.py`, `cases/`,
`status/torture.csv`) is on the **`bidirectionaltype-inference`** branch. On
`main`, `benchmarks/torture/` is empty — every path below resolves only on that
branch.
**Observed:** 2026-08-05.
**Status:** spin stopped once the workspace no longer contained the case. The
checker-side root cause is **not** confirmed — see
[TORTURE-HANG-UNPROVEN](#TORTURE-HANG-UNPROVEN).

One finding drives everything here: **a torture case can score `pass` on the
harness and still hang the binary a user installs, and the harness cannot
currently see the difference.**

For the release-side procedure this feeds, see
[RELEASE-VERIFICATION](RELEASE-MANUAL-VERIFICATION-SPEC.md#RELEASE-VERIFICATION).

## Summary {#TORTURE-HANG-SUMMARY}

A `basilisk lsp` process pinned one full CPU core indefinitely inside a VS Code
workspace. The hot thread was in unbounded self-recursion. The workspace at the
time contained `benchmarks/torture/cases/recursive_bases.py` — the case whose
docstring states that its purpose is **termination**, not diagnostics.

Run through the *installed* binary, that case hung. Run through a *locally
built* binary, it passed in 0.06 s. Same input, same source tree, opposite
outcome — the variable was which binary ran.

## What was measured {#TORTURE-HANG-OBSERVED}

Live process, sustained over ~10 minutes:

| Property | Value |
|---|---|
| Binary | `~/.vscode/extensions/nimblesite.basilisk-0.39.0-darwin-arm64/bin/darwin-arm64/basilisk lsp` |
| CPU | `100.0%` — one saturated core |
| RSS | ~256 MB, **flat** (not a leak) |
| Threads | 21 |

`sample <pid> 6` (6 s at 1 ms) resolved the thread picture:

```
__psynch_cvwait       74954   ← parked
__ulock_wait           9994   ← parked
__psynch_mutexwait     4997   ← parked
__workq_kernreturn     4997   ← parked
kevent                 4993   ← parked
```

**20 of the 21 threads were asleep.** They are the Tokio worker pool and were
never the problem. Exactly one `tokio-rt-worker` was running, and its stack was
one code address calling itself:

```
basilisk + 0x4fc568
  basilisk + 0x4fc568
    basilisk + 0x4fc568
      … 65+ identical nested frames, growing
```

Unbounded recursion — not a busy loop, not a leak.

Two side effects worth recording, both independent of the recursion itself:

- **Toggling diagnostics off did not stop the burn.** In-flight analysis has no
  cancellation path, so the toggle could not interrupt work already running.
- **The whole LSP went dead, not just diagnostics.** Analysis is single-threaded
  on one dedicated large-stack thread
  ([LSPARCH-ARCH-STACK](LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-STACK)), so one
  runaway recursion starved every other feature. The Modules panel froze on
  stale counts rather than degrading.

## Reproduction {#TORTURE-HANG-REPRO}

With the branch carrying the torture cases checked out, `$BIN` pointing at the
**installed** binary:

```bash
BIN=~/.vscode/extensions/nimblesite.basilisk-0.39.0-darwin-arm64/bin/darwin-arm64/basilisk
timeout 20 "$BIN" check benchmarks/torture/cases/recursive_bases.py
```

| Case | Result |
|---|---|
| `recursive_bases.py` | **`rc=124` — timed out at 20.02 s** |
| `recursive_aliases.py` | `rc=1`, 0.06 s |
| `generic_constructor.py` | `rc=0`, 0.03 s |
| `paramspec_decorator.py` | `rc=1`, 0.03 s |

The siblings finish in tens of milliseconds. `recursive_bases.py` does not
finish. The case body is six lines:

```python
class C(C[int], C[bool]):  # E
    pass


class D(D):  # E
    pass
```

A binary built locally from the same branch handled it in **0.06 s**, emitting
two correct `names_undefined` diagnostics ("Class `C` lists itself as a base,
but `C` is not bound until the class statement completes").

## Why the two binaries differed {#TORTURE-HANG-PROVENANCE}

The installed binary and the source tree were not the same code:

```
v0.39.0 tag  → b8ae454   (Aug 2)
fix commit   → da74283   (Aug 4)  "…checker hang on recursive bases…"

git tag --contains da74283 → (empty)
```

Corroborated by dependency skew: the installed binary reported
`Ruff formatter: 0.15.17`; a build from `main` reported `0.16.1`.

So the installed artifact was built from a commit predating the recursive-bases
work, and that later commit is contained in no tag. This is the mechanism the
harness needs to account for, and it leads straight to the next section.

## The harness gap {#TORTURE-HANG-GAP}

This is the actionable finding.

`benchmarks/torture/run_torture.py` already models termination correctly — it
has a `hang` verdict, a `TORTURE_TIMEOUT` (default 30 s), and a docstring
stating that "termination is part of correctness". That design is right and
needs no change.

The gap is **which binary it measures**:

```python
# benchmarks/torture/run_torture.py
def basilisk_bin() -> Path:
    binary = Path(os.environ.get("BASILISK_BIN", ROOT / "target" / "release" / "basilisk"))
```

It defaults to `target/release/basilisk` — a **local build of the current
tree**. It never touches the artifact a user installs. In exactly the situation
above, the harness scores `recursive_bases.py` as `pass` while the shipped
binary hangs on it: scoreboard green, user's CPU pegged.

Note the asymmetry. For the five competitors, `run_torture.py` deliberately
pulls the **latest official release** — "leads are proven against current
upstream, never a stale pin". Basilisk is the only tool in that table measured
from an unreleased working tree, in a scoreboard whose results are published.

## Limits of this evidence {#TORTURE-HANG-UNPROVEN}

Stated plainly so nobody builds on sand:

- **Not proven:** that the live LSP was recursing on `recursive_bases.py`
  specifically. The shipped binary is fully stripped — `nm` returns 201 symbols,
  all undefined imports, and `atos` resolves nothing. There is no symbolicated
  frame and no log line naming the file. The evidence is circumstantial:
  unbounded self-recursion, a known hang reproducer present in the scanned
  workspace, and the spin ending when that file left the tree.
- **Not tested:** whether `main` actually fixes the hang. The case file does not
  exist on `main` — it lives only on the feature branch that introduced the
  torture suite — so the fix commit was never exercised against it here.
- **Observed but not the cause:** the workspace scan covered ~2,880 `.py` files
  including vendored `debugpy`, a checked-out `fastapi` corpus, and a `.venv`
  containing mypy's own sources. Large, but the profile was recursion, not
  volume. Do not conflate the two.

## What to do about it {#TORTURE-HANG-ACTIONS}

In priority order:

1. **Verify `main` actually terminates on `recursive_bases.py`.** This is the
   open question. Build from `main`, run the case, confirm it exits and emits
   the two required `# E` diagnostics. If it hangs, the fix is incomplete and
   the case is doing its job.
2. **Record which binary was measured.** `run_torture.py` should write the
   path, the reported version, and whether it was a local build or an installed
   artifact into `status/torture.csv` and `results/summary.md`. A published
   scoreboard that does not say which binary produced the numbers cannot be
   audited.
3. **Add a released-artifact run.** Keep the working-tree run as the fast inner
   loop, but `hang` regressions are precisely the case where tree and artifact
   disagree. `BASILISK_BIN` already makes this possible with no code change —
   point it at the installed binary, and consider making that mode mandatory
   before results are published.
4. **Keep `recursive_bases.py` exactly as it is.** It caught a real hang in a
   real binary. Do not soften it, shorten its timeout, or reduce it to a
   diagnostics-only assertion.

Field triage for a future spin — identifying the process, sampling it, reading
the extension log, confirming which build is running — is documented once in
[RELEASE-TRIAGE](RELEASE-MANUAL-VERIFICATION-SPEC.md#RELEASE-TRIAGE); follow it
there rather than duplicating it here.
