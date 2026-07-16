# Real-World Workspace E2E Suites {#VSIX-REALWORLD}

The unit/e2e suites in `src/test/suite/` exercise the extension against tiny synthetic fixtures. That never proves the product survives what users actually do: open a real, popular, medium-to-large Python codebase and lean on the LSP all day. These suites close that gap — each opens a **pinned real-world repository** as the VS Code workspace, drives a long interaction journey through it, and holds the basilisk server process to **hard memory and CPU budgets measured at the OS level**.

They run as part of the standard VS Code extension gate (`make _test_vsix` → `npm test`), one VS Code session per corpus repo.

| Artifact | Path |
|---|---|
| Corpus manifest (single source of truth) | `vscode-extension/test-fixtures/real-world-corpus.json` |
| Fetch script (pretest) | `vscode-extension/scripts/fetch-real-world-repos.mjs` |
| Suite + engine | `vscode-extension/src/test/real-world/` |
| Per-repo test configs | `vscode-extension/.vscode-test.mjs` |

## Corpus {#VSIX-REALWORLD-CORPUS}

Three medium-to-large, heavily-typed, pure-Python repositories, each pinned to the **exact commit SHA** of a named release tag so every probe token in the manifest is verified against immutable content:

| Repo | Pin | Scale (at pin) |
|---|---|---|
| `pallets/flask` | `3.1.1` → `7fff56f5172c48b6f3aedf17ee14ef5c2533dfd1` | ~83 py files / ~18k LOC |
| `Textualize/rich` | `v14.3.4` → `ee8378c3bbbd7c75abc2f55c6c19e83b218ae81d` | ~213 py files / ~52k LOC |
| `fastapi/fastapi` | `0.116.1` → `313723494be79d4b24ccaa60e4f6d1f96c150fed` | ~1129 py files / ~88k LOC |

- `fetch-real-world-repos.mjs` downloads each pin as a GitHub tarball (`codeload…/tar.gz/<sha>` — no git dependency), extracts it to `.real-world/<name>/` (git-ignored), verifies a sentinel file, asserts a **minimum `.py` file count** (`minPythonFiles` — a truncated tree fails, never silently passes), and stamps `.bsk-real-world-ok` with the SHA. Repeat runs are no-ops; a stale marker rebuilds the tree.
- The fetch script also **purges VS Code's hot-exit backups** (`.vscode-test/user-data/Backups`): all test configs share a persistent user-data dir, so a dirty buffer left by an aborted edit-churn run would otherwise be silently restored into the next run's identical workspace, poisoning its baseline.
- The script is wired as `pretest`, so `npm test` can never launch against a missing corpus. **There is no offline skip** — a fetch failure fails the run (a silent skip would disarm the suites, forbidden by the testing rules).
- Repos are opened **without a venv**: third-party imports (werkzeug, starlette, pydantic…) are legitimately unresolved, exactly like a user opening a repo before `pip install`. Probes therefore target **intra-repo** symbols only.

## Interaction journey {#VSIX-REALWORLD-JOURNEY}

Per repo, the suite (`real-world.test.ts` + `journey.ts`) runs these phases in order (`bail: true` — the first failure halts the session):

1. **Pin verification** — workspace folder IS the pinned tree: marker SHA, sentinel, `minPythonFiles`.
2. **Whole-workspace analysis** — waits until the workspace-symbol index answers a pinned query AND the workspace-wide basilisk diagnostic set is **non-empty** and holds still for 6s. The non-empty gate is load-bearing: the server computes the whole scan before publishing in one burst while the symbol index answers incrementally mid-scan, so an empty set is deceptively "stable" — and every corpus repo is fetched without its dependencies, guaranteeing unresolved-import diagnostics. Then the server's CPU must **settle to idle** and memory must be in budget ([VSIX-REALWORLD-RESOURCES]).
3. **Diagnostic invariants** — a **fresh** workspace snapshot (never a stale one captured mid-scan) must be non-empty, and every published basilisk diagnostic in it is checked structurally: Python-file target, non-empty message, ordered range, valid severity, non-empty code (snake_case PEP rule name or opt-in `BSK-XXXX`) whose docs link points at its `/errors/<code>` page. (No count assertions — real-repo diagnostic counts are checker behavior, not a test contract.)
4. **Per-file journeys** — for each manifest file: document symbols (floor + named expectations), hover probes, definition probes (must land in the pinned target file), member-completion probes (positioned after a real `self.`/module dot in the pinned source), reference probes (location floors). Every probe token is located in the live document text; a missing token fails loudly (manifest/tree drift).
5. **Workspace symbol search** — every pinned query must surface the expected symbol in the expected file.
6. **Edit churn** — repeatedly appends a guaranteed type error (`def …() -> str: return <int literal>` — flagged by `returns_compatibility`; a mistyped *parameter* return is deliberately not used because the checker does not currently flag it) to an open buffer, asserts an **Error-severity diagnostic inside the appended probe**, reverts, and asserts the diagnostic set returns to baseline (live analysis, no stale leftovers). The buffer is never saved; the file is reverted clean.
7. **Open blitz + leak check** — opens a dozen real files back-to-back (symbols asserted on each, resources sampled during), then asserts CPU settles again and RSS **grew at most `maxServerLeakMb`** since the post-analysis baseline.
8. **Assertion-density floor** — every journey assertion runs through a counting `check()`; the suite fails if fewer than the floor (2,000/repo; measured runs count 7.5k–28k) executed. Density is a tested invariant, not a hope.

Every mocha `this.timeout()` in the suite is computed to cover the worst case its phase's own poll deadlines sanction (e.g. analysis = 3× `cpuSettleTimeoutMs`, churn = cycles × 2 polls) — a slow-but-in-budget run must fail on a descriptive assertion, never an opaque mocha timeout.

Shared plumbing (hover/nav/poll helpers, `flattenSymbolNames`, `filterBasiliskDiagnostics`) lives in `src/test/suite/test-helpers.ts` — the journeys reuse it rather than duplicating it.

## Resource budgets {#VSIX-REALWORLD-RESOURCES}

`src/test/real-world/metrics.ts` samples the **basilisk server process from outside** (never self-reported): PowerShell `Get-Process` on Windows, `/proc/<pid>/{status,stat}` on Linux, `ps -o rss=,cputime=` on macOS — RSS bytes + cumulative CPU ms. The server PID comes from the language client's child process.

The `ResourceMonitor` enforces, per corpus entry (`budgets` in the manifest):

| Budget | Meaning |
|---|---|
| `maxServerRssMb` | Ceiling on server RSS — current AND peak — asserted after every phase and every file journey. |
| `maxServerLeakMb` | Max RSS growth from the post-analysis baseline to the end of the open blitz. |
| `maxExtHostRssMb` | Ceiling on the extension host's own peak RSS (`process.memoryUsage()`). |
| `maxIdleCpuPercent` | The server must idle below this for two consecutive ~2s windows after analysis and after the blitz — catches busy-loop / re-analysis-storm regressions. |
| `cpuSettleTimeoutMs` | How long analysis/settle may take before it is a failure. |

Additional hard rule: the **server PID must not change** across the journey — a mid-run change means the server crashed and was restarted by the client (the #278 crash-loop class) and fails the suite immediately.

Budgets are **ratchets**: calibrated from measured runs with headroom, they may only tighten. Loosening a budget to make a regression pass is scoreboard-tampering (same crime as touching `coverage-thresholds.json`).

## Wiring {#VSIX-REALWORLD-WIRING}

- `.vscode-test.mjs` builds one desktop test config per manifest repo — label `real-world-<name>`, `workspaceFolder: .real-world/<name>`, `env.BSK_REAL_WORLD_REPO=<name>`, `files: out/test/real-world/**` — appended after the default `workspace-suite` config, so plain `npm test` (and therefore CI's `make _test_vsix`) runs them. The screenshots flow (`BASILISK_SCREENSHOTS`) skips them: it drives `vscode-test` directly without the corpus staged.
- `npm test` passes `--bail` to `vscode-test`: per-config `mocha.bail` only halts within one VS Code session, and without the CLI flag a failed config would still boot every remaining corpus session — regressing the repo's fail-fast testing rule and burning CI minutes after the verdict is known.
- `npm run test:real-world` compiles, fetches, and runs only the three real-world labels.
- CI caches `.real-world/` keyed on the manifest hash (a pin change re-fetches) and the `test-vscode` job timeout accounts for the three extra VS Code sessions.
