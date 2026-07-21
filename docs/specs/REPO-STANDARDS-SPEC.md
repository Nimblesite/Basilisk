# Repository standards {#REPO-STANDARDS}

Repo-wide gates that are configured at the repository root or under `.github/`
rather than inside a crate. Each section below is the target of a `[ID]` comment
in the file it governs, so the citation resolves in one hop.

| Section | Governs |
|---|---|
| [CI-DESLOP](#CI-DESLOP) | `.deslop.toml`, `Makefile` `_lint_deslop`, `scripts/install-deslop.sh`, the CI `Install deslop` step |
| [COVERAGE-THRESHOLDS-JSON](#COVERAGE-THRESHOLDS-JSON) | `coverage-thresholds.json`, `scripts/common.sh` |
| [GITIGNORE-RULES](#GITIGNORE-RULES) | `.gitignore` IDE/editor block |
| [GITHUB-DEPENDABOT](#GITHUB-DEPENDABOT) | `.github/dependabot.yml`, `.github/workflows/dependabot-automerge.yml`, the Dependabot skips in `ci.yml`/`codeql.yml` |
| [GITHUB-CODE-SCANNING](#GITHUB-CODE-SCANNING) | `.github/workflows/codeql.yml`, `.github/codeql/codeql-config.yml` |
| [GITHUB-DEP-REVIEW](#GITHUB-DEP-REVIEW) | the `security` job in `.github/workflows/ci.yml` |

## Duplication gate {#CI-DESLOP}

`.deslop.toml` at the repository root is the **single source of truth** for this
repo's duplication budget. It is committed and PR-reviewed, and
`[threshold] max_duplication_percent` is ratcheted **down** only — the same
one-way discipline the coverage and conformance gates use.

`[defaults] exclude` drops paths during discovery, so excluded files are never
analysed and never contribute to the measured percentage
([deslop docs](https://deslop.live/docs/for-ai/)). Built-in defaults already
cover `node_modules`, `target`, `dist`, `__pycache__`, and any path containing
`generated`; the committed file adds only project-specific patterns on top —
benchmark fixtures, crate test trees, `examples/` at any depth, vendored Python
`site-packages`/`.venv`, the git-ignored `scratchpad/`, and the mirrored
`conformance/tests/` suite. Each of those is third-party or deliberately
repetitive code whose duplication must not gate the build.

### Where the gate runs {#CI-DESLOP-GATE}

`make lint` depends on `_lint_deslop`, which runs `deslop .` from the repository
root and fails the build on a non-zero exit. Because `make ci` is
`lint test build`, the gate runs locally and in CI through the same target —
there is no CI-only invocation and no second scorer.

### Unpinned CLI {#CI-DESLOP-UNPINNED}

The `deslop` CLI is **deliberately not pinned**. `scripts/install-deslop.sh`
installs `nimblesite/tap/deslop` with Homebrew, whose formula tracks the latest
release, and the same script is shared by `scripts/setup.sh` (local) and the CI
`Install deslop` step. The reason is agreement, not convenience: the CLI must
analyse the same corpus as the engine behind the editor's LSP/MCP panel, and a
pinned-stale CLI would make the gate disagree with what a developer sees in the
editor.

The script is written for both environments: it loads `brew` from its known
install locations because CI steps run with `bash --noprofile --norc`, taps and
non-interactively trusts `nimblesite/tap` (Homebrew ≥ 6 otherwise prompts, which
deadlocks with CI's closed stdin), and appends the brew bin directory to
`$GITHUB_PATH` so the following `make lint` step resolves the binary.
`scripts/audit.sh` requires `deslop` on `PATH`, so a missing CLI is reported as a
missing tool rather than a silently skipped gate.

## Coverage thresholds {#COVERAGE-THRESHOLDS-JSON}

`coverage-thresholds.json` at the repository root is the **sole** source of code
coverage thresholds. There are no environment variables, no GitHub repository
variables, no thresholds in CI YAML, and no hardcoded fallbacks in the scripts.

### Resolution {#COVERAGE-THRESHOLDS-JSON-RESOLUTION}

| Key | Meaning |
|---|---|
| `default_threshold` | The threshold for any project without its own entry. |
| `projects.<name>.threshold` | The threshold for that project, overriding the default. |

`coverage_threshold_for <project>` in `scripts/common.sh` is the reader for
every consumer that can source the shell helpers: it resolves the repository
root from its own path, loads the JSON, and prints `projects.<name>.threshold`
when the project is listed and `default_threshold` otherwise.
`scripts/test-rust.sh` calls it once per Rust crate before comparing measured
line coverage, and `scripts/test-nvim.sh` calls it for the `nvim` project.

The Makefile's VSIX coverage recipe is the one exception: it is an inline make
recipe that never sources `scripts/common.sh`, so it reads
`projects.vsix.threshold` out of the same file with its own `python3 -c`
one-liner. That is a second **reader**, not a second **source** — both paths
resolve every threshold from `coverage-thresholds.json` and nothing else, which
is what the "sole source" rule above requires.

### Enforcement {#COVERAGE-THRESHOLDS-JSON-ENFORCEMENT}

`make test` always computes coverage and always enforces it. A project whose
measured percentage is below its threshold fails the run, and a project that
produced **no** coverage data fails too — that state means the tests died before
coverage could flush, and treating it as a pass would hide the failure.

Thresholds **ratchet up only**. Lowering any `threshold` or `default_threshold`
value is forbidden; a project that cannot meet its number gets more tests.

### Conformance block {#COVERAGE-THRESHOLDS-JSON-CONFORMANCE}

The same file carries the `conformance` block (`threshold` and
`max_false_positives`), which is enforced by the real `python/typing` harness
rather than by the coverage scripts. Its policy — pass percentage up only,
false-positive ceiling down only, and no rule may be disabled to move either —
is normative in
[CHKARCH-CONFORMANCE](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFORMANCE).

## Committed editor directories {#GITIGNORE-RULES}

`.gitignore`'s IDE/editor block ignores **per-user scratch** only — editor swap
and backup files (`*.swp`, `*.swo`, `*~`) and Sublime project/workspace files,
which are local to one machine and one person.

Two editor directories are deliberately **not** ignored, because they are shared
dev tooling rather than personal state:

| Directory | Status | Why it is committed |
|---|---|---|
| `.vscode/` | Tracked: `extensions.json`, `launch.json`, `settings.json` | Recommended extensions, the debug/launch configurations used to run the CLI and LSP, and workspace settings — so every contributor and every agent gets identical tooling without repeating setup instructions in prose. |
| `.idea/` | Not ignored; no files tracked today | JetBrains **shared** run configurations and code style are committable artifacts of the same kind. Leaving the path un-ignored means adding them is a normal commit, not a `.gitignore` edit. |

Ignoring either directory wholesale would take the shared configuration with it,
so a change that adds `.vscode/` or `.idea/` to `.gitignore` is a regression, not
a cleanup.

## Dependabot {#GITHUB-DEPENDABOT}

`.github/dependabot.yml` keeps dependencies current without PR spam and without
burning the CI matrix on `main` for routine bumps.

### Staging branch {#GITHUB-DEPENDABOT-STAGING}

Every bump ends up on the long-lived `dependabot-upgrades` staging branch;
nothing reaches `main` unattended.

- **Version updates** carry `target-branch: dependabot-upgrades`, so Dependabot
  opens them against the staging branch. `ci.yml` and `codeql.yml` trigger on
  `pull_request: [main]`, and that filter matches the PR **base**, so these PRs
  run no build, test, or CodeQL.
- **Security updates** ignore `target-branch` — GitHub always opens them against
  the default branch. `.github/workflows/dependabot-automerge.yml` therefore also
  triggers on `main`, folds the security bump into the same staging branch, and
  retires the PR, so a CVE bump never waits on a human either.
- **Grouping**: each ecosystem collapses all version bumps into one PR, with a
  parallel `*-security` group (`applies-to: security-updates`) for CVE bumps.
  Majors need no separate review PR because review happens once, on the
  `dependabot-upgrades → main` consolidation PR.
- **Schedule**: `weekly`, one grouped PR per ecosystem.

Ecosystems covered: `github-actions` (repo root), `cargo` (the Rust workspace at
the root), and `npm` for both `/vscode-extension` and `/website`. Keeping
`github-actions` current is what stops SHA-pinned actions from going stale —
action pinning itself is an external Shipwright requirement, cited at its own
call site.

### Sweep workflow {#GITHUB-DEPENDABOT-SWEEP}

`dependabot-automerge.yml` merges the incoming branch into `dependabot-upgrades`
with `-X theirs`, so successive bumps of the same lock file never conflict-stall:
the latest bump wins. Two independent gates are both required — the unforgeable
`dependabot[bot]` actor **and** a `dependabot/*` head branch. The workflow runs
git plumbing plus `gh pr close` only; it executes nothing from the merged tree,
and disables hooks via `core.hooksPath=/dev/null`.

The trigger **must** stay `pull_request` and never become `pull_request_target`:
under `pull_request` a fork PR runs with a read-only token and no secrets, which
is what makes merging fork-controlled content here inert.

The file lives at the repository root so it is present on both `main` and
`dependabot-upgrades` — for `pull_request`, the workflow is read from the PR's
base branch.

### CI skips {#GITHUB-DEPENDABOT-CI-SKIP}

`ci.yml`'s change-scope classifier turns every scope off for a
`dependabot[bot]` actor, and `codeql.yml` excludes the same actor. Those PRs are
swept into staging and discarded, so the expensive matrix runs once — on the
consolidation PR.

### Prerequisite {#GITHUB-DEPENDABOT-PREREQ}

The `dependabot-upgrades` branch must exist, cut from `main` **after**
`dependabot.yml` and `dependabot-automerge.yml` are on `main`, so the staging
branch carries the sweep workflow.

## Code scanning {#GITHUB-CODE-SCANNING}

`.github/workflows/codeql.yml` runs CodeQL static analysis and feeds GitHub
code-scanning alerts. It is separate from `ci.yml` because it needs
`security-events: write` and its own schedule, and it owns exactly one concern:
**vulnerable code**. `make lint` owns style/correctness and
[GITHUB-DEP-REVIEW](#GITHUB-DEP-REVIEW) owns vulnerable dependencies — no linter
plugin may re-cover CodeQL's ground.

### Triggers {#GITHUB-CODE-SCANNING-TRIGGERS}

| Trigger | Purpose |
|---|---|
| `pull_request` on `main` | Scans the diff. |
| `push` on `v*` tags | Scans the exact released SHA with the current query set — a release can ship code last scanned weeks earlier. |
| Weekly `schedule` | Re-scans with newly published queries even without a push. |

The job is gated on `github.event.repository.visibility == 'public'`, because
SARIF upload requires GitHub Advanced Security on private repositories; a private
repo skips cleanly and self-enables when made public. Dependabot PRs are excluded
for the reason in [GITHUB-DEPENDABOT-CI-SKIP](#GITHUB-DEPENDABOT-CI-SKIP).

### Language matrix {#GITHUB-CODE-SCANNING-LANGUAGES}

The matrix is (languages actually in this repo) ∩ (languages CodeQL supports):
`rust` (checker/LSP), `javascript-typescript` (VS Code extension and website),
and `actions` (the workflow files themselves). All three use `build-mode: none` —
`ci.yml` already builds the Rust workspace. Python is supported by CodeQL but
deliberately excluded: the only Python here is deliberately malformed
type-checker fixtures under `conformance/`, `examples/`, and `tests/`, and
scanning them would drown real signal in fixture noise.

The query suite is `security-extended`.

### Query filters {#GITHUB-CODE-SCANNING-FILTERS}

Query-level tuning that the `queries:` input cannot express lives in
`.github/codeql/codeql-config.yml`, wired in through the `init` step's
`config-file:`. It currently excludes exactly one id,
`actions/untrusted-checkout/medium`, for the Dependabot merge bot — that query
assumes a privileged workflow that checks out and runs a fork's build script, and
the sweep workflow does neither (see
[GITHUB-DEPENDABOT-SWEEP](#GITHUB-DEPENDABOT-SWEEP)). The dangerous
`actions/untrusted-checkout/high` variant — the `pull_request_target` case —
stays active, so a real misconfiguration is still caught.

## Dependency review {#GITHUB-DEP-REVIEW}

The `security` job in `.github/workflows/ci.yml` runs
`actions/dependency-review-action` with `fail-on-severity: high` and
`comment-summary-in-pr: on-failure`, and holds `pull-requests: write` so the
action can post its summary.

This is the repository's **only** dependency vulnerability gate — there is no
`cargo-deny` or OSV gate, so there is no doubling up. It is separate from
[GITHUB-CODE-SCANNING](#GITHUB-CODE-SCANNING) (vulnerable code) and from
`make lint` (style and correctness): one owner per concern.

It is PR-only by construction, because dependency review diffs the PR base
against its head — which matches `ci.yml`'s `pull_request`-only trigger. Bumps
that arrive through [GITHUB-DEPENDABOT](#GITHUB-DEPENDABOT) are reviewed here on
the consolidation PR, where the full matrix runs.
