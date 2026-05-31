# Activity Panel — Action Wiring & Affordance Verification Plan

Implements / verifies:
- [EXTACT-INFO-AFFORDANCE](../specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO-AFFORDANCE)
- [EXTACT-INFO-ACTION-WIRING](../specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO-ACTION-WIRING)
- [EXTACT-INFO-FEATURE-STATUS](../specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO-FEATURE-STATUS)
- [EXTACT-INFO-QUICK-ACTIONS](../specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO-QUICK-ACTIONS)
- [EXTACT-INFO-SERVER-INFO](../specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO-SERVER-INFO)

## Problem

Two defects in the `basilisk.info` panel (`vscode-extension/src/info-panel.ts`):

1. Actionable rows and read-only rows are visually indistinguishable.
2. Several Quick Actions render as clickable but have **no registered handler** —
   clicking is a silent no-op.

Confirmed dead actions (contributed, no `registerCommand`): `basilisk.fixWorkspace`,
`basilisk.organizeImports`, `basilisk.uv.sync`, `basilisk.uv.add`, `basilisk.uv.lock`,
`basilisk.uv.createEnv`. Working: `basilisk.restartServer`, `basilisk.showOutput`.

## Goal

**Every actionable row in the panel does something observable when invoked, and is visually
unmistakable as a button. Every read-only row carries no action affordance.** Proven by
coarse e2e tests that drive rows through the UI / VSIX state — never via
`vscode.commands.getCommands(true)` or `whenCommandReady` (per CLAUDE.md IDE testing rules).

## Inventory — single source of truth for the tests

The test enumerates rows by reading the live tree from `InfoPanelProvider`, then asserts per
interaction class. Expected wiring:

| Row | Section | Class | Command id | Registered handler |
|---|---|---|---|---|
| Type Checking … AI Typing | Feature Status | actionable | `basilisk.toggleFeature` | ✅ already |
| Restart Server | Quick Actions | actionable | `basilisk.restartServer` | ✅ already |
| Show Output | Quick Actions | actionable | `basilisk.showOutput` | ✅ already |
| Fix All in Workspace | Quick Actions | actionable | `basilisk.fixWorkspace` | ⛔ implement |
| Organize Imports (Workspace) | Quick Actions | actionable | `basilisk.organizeImports` | ⛔ implement |
| uv Sync | Quick Actions | actionable | `basilisk.uv.sync` | ⛔ implement |
| uv Add Package | Quick Actions | actionable | `basilisk.uv.add` | ⛔ implement |
| uv Lock | Quick Actions | actionable | `basilisk.uv.lock` | ⛔ implement |
| uv Create Env | Quick Actions | actionable | `basilisk.uv.createEnv` | ⛔ implement |
| Server, Version, Analysis Mode, Python, uv, uv Auto-Sync, Stub Suggestions, Binary | Server Info | read-only | none | n/a |

## Phase 1 — Centralize the affordance (kills drift)

Per [EXTACT-INFO-AFFORDANCE], one helper owns each interaction class so the rules cannot drift
per call site. In `info-panel.ts`:

1. Keep `ActionItem` / `FeatureItem` as the **only** constructors that may set `.command`, and
   have them set an imperative `tooltip` and the action-connoting icon.
2. Make `InfoTextItem` structurally unable to be actionable: never sets `.command`, never
   accepts a button-style icon, keeps `contextValue = "info"`.
3. Add an `inline` action-button contribution in `package.json` `view/item/context` gated on
   `viewItem == action || viewItem == feature` so a literal button shows on every actionable
   row (not just a whole-row click target). Read-only rows (`viewItem == info`) get none.
4. Add a single source list of Quick Actions (label, command id, icon, when-clause) so rows and
   the e2e inventory derive from the same array.

## Phase 2 — Implement the dead handlers (TDD, per row)

For each dead command, follow the bug-fix process in CLAUDE.md (failing e2e first, confirm it
fails for the right reason, then implement):

- `basilisk.organizeImports` — workspace-wide organize-imports via the LSP code-action /
  `workspace/executeCommand` path already used elsewhere; reuse, do not reinvent.
- `basilisk.fixWorkspace` — workspace-wide "fix all" via the same LSP execute-command surface.
- `basilisk.uv.sync` / `uv.add` / `uv.lock` / `uv.createEnv` — drive the uv integration that
  backs the existing Server Info uv rows; surface progress + errors per the logging standards.

Each handler must produce an **observable** effect a test can assert (a terminal/task created,
an LSP request issued, a notification, or a `Store` state change) — never a silent return.

If a handler genuinely cannot exist yet, the row must be **removed** from the panel and from
`contributes.commands`, not left dead ([EXTACT-INFO-ACTION-WIRING]).

## Phase 3 — E2E tests (coarse, through the UI)

Add to `vscode-extension/src/test/suite/activity-panel.test.ts` (and the accessibility
sibling). Rules: no `getCommands(true)`, no `whenCommandReady`; assert through the provider's
tree items and observable side effects.

1. **Affordance partition** — walk the live tree; assert every Feature Status and Quick Actions
   row has a `.command` and an imperative `.tooltip`, and every Server Info row has **no**
   `.command` and `contextValue === "info"`. No row appears in both classes.
2. **Inline button presence** — assert `package.json` contributes an `inline`
   `view/item/context` entry whose `when` matches `action`/`feature` and excludes `info`
   (parse the manifest the extension actually ships).
3. **No dead actions** — for each actionable row, invoke its command via the row's own
   `command` object and assert it resolves to a working handler by observing the side effect
   (Phase 2). A "command not found" rejection fails the test. This is the assertion that would
   have caught the current bug.
4. **Context gating** — with `basilisk.uv.enabled=false`, assert the uv rows are absent (hidden,
   not dead); with it true, assert they are present and wired.
5. **Read-only inertness** — a Server Info row (no command) is a no-op; assert no state change /
   no command dispatch.

## Phase 4 — Guardrails

- Add a build/test-time check that every `command` referenced by an `ActionItem`/`FeatureItem`
  is present in `contributes.commands` **and** registered, derived from the Phase-1 source list,
  so a future added row cannot ship dead.
- Run `deslop:find-similar` before adding handler code and `deslop:top-offenders` after, merging
  duplicates (handlers likely share an LSP execute-command helper).
- `make ci` green; coverage at/above `coverage-thresholds.json` (ratchet only).

## Done when

- All rows in the table above are either wired-and-working or removed.
- Tests 1–5 pass and would fail if any action regressed to a no-op or any class boundary blurred.
- Issue closed referencing the merged PR.
