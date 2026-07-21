// Tests [LSPCFGED-TYPESHED] / [LSPCFGED-TYPESHED-DOWNLOAD] in a REAL webview
// DOM, driven like a user.
//
// The reported failures this suite locks down:
//   * clicking a source radio flashed a full-panel spinner screen and locked
//     every control — the deleted lock screen must STAY deleted: no overlay
//     node, no inert shell, no transient disabled state, ever;
//   * a "Latest" source radio was rendered although no such source exists —
//     exactly two sources may ever appear;
//   * a running download must show progress ON the button that started it
//     while every other control stays live and editable;
//   * a missing source must surface as a persistent inline row carrying its
//     own fix (Download pinned), never as a blocking state.
//
// Each test is one continuous user journey: every interaction is followed by a
// full DOM probe, and every probe is asserted.

import * as assert from "assert";
import {
  DRIVER_PRELUDE,
  RESULT_TIMEOUT_MS,
  runScenario,
  ScenarioHost,
  type DomStep,
} from "./webview-dom-harness";
import { ACTIVE_COMMIT, LATEST_COMMIT, OTHER_COMMIT } from "./typeshed-fixture";

const CUSTOM_FOLDER = "/workspace/vendor/typeshed";
const STORE_FOLDER = "/workspace/.basilisk/typeshed-store";
const NO_SOURCE_REASON = "Pinned commit 1f2e3d4c is not in the local store";

interface Source {
  readonly mode: string;
  readonly checked: boolean;
  readonly disabled: boolean;
  readonly hint: string;
}

interface Action {
  readonly action: string;
  readonly disabled: boolean;
  readonly busy: boolean;
}

function step(steps: DomStep[] | undefined, label: string): DomStep {
  const found = (steps ?? []).find((entry) => entry.label === label);
  assert.ok(found, `driver never recorded step "${label}" (recorded: ${(steps ?? []).map((entry) => entry.label).join(", ")})`);
  return found;
}

function action(entry: DomStep, name: string): Action {
  const found = (entry.actions as Action[]).find((candidate) => candidate.action === name);
  assert.ok(found, `step "${entry.label}" rendered no ${name} button`);
  return found;
}

/** Exactly the two sources exist — a "Latest" radio may NEVER render. */
function assertSelected(entry: DomStep, mode: string): void {
  const sources = entry.sources as Source[];
  assert.deepStrictEqual(
    sources.map((candidate) => candidate.mode),
    ["ExactCommit", "CustomFolder"],
    `step "${entry.label}" must offer exactly the two sources — no Latest, ever`,
  );
  assert.deepStrictEqual(
    sources.filter((candidate) => candidate.checked).map((candidate) => candidate.mode),
    [mode],
    `step "${entry.label}" must show ${mode} as the one active source`,
  );
}

/** The deleted lock screen stays deleted and nothing source-shaped is disabled. */
function assertNothingLocked(entry: DomStep): void {
  assert.strictEqual(entry.overlayPresent, false, `step "${entry.label}" must render no full-panel overlay node`);
  assert.strictEqual(entry.shellInert, false, `step "${entry.label}" must never make the shell inert`);
  (entry.sources as Source[]).forEach((candidate) => {
    assert.strictEqual(candidate.disabled, false, `step "${entry.label}" must keep the ${candidate.mode} radio enabled`);
  });
  if (entry.commitPresent === true) {
    assert.strictEqual(entry.commitDisabled, false, `step "${entry.label}" must keep the SHA field editable`);
  }
  if (entry.pickFolderDisabled !== null) {
    assert.strictEqual(entry.pickFolderDisabled, false, `step "${entry.label}" must keep the custom folder picker live`);
  }
  if (entry.storePickerDisabled !== null) {
    assert.strictEqual(entry.storePickerDisabled, false, `step "${entry.label}" must keep the store folder picker live`);
  }
}

function mutationsOf(intents: readonly Record<string, unknown>[], index: number): unknown {
  return intents[index]?.mutations;
}

const sourceJourneyDriver = String.raw`
  (async () => {
    ${DRIVER_PRELUDE}
    try {
      if (!await waitFor('[data-typeshed-source]')) { report({ ok: false, reason: 'typeshed controls never rendered' }); return; }
      await click(document.querySelector('[data-section-target="project"]'));
      record('pinned');
      // 1. Reject an invalid SHA in place — nothing may be written.
      await change(el('[data-typeshed-commit]'), 'not-a-sha');
      record('invalid-sha');
      // 2. A valid SHA repins, atomically clearing any folder.
      await change(el('[data-typeshed-commit]'), '${OTHER_COMMIT}');
      record('repinned');
      // 3. Choose the custom folder. The probe directly after the click — no
      // settle — must find nothing disabled and no overlay: a radio change
      // may never enter a transient locked state.
      el('[data-typeshed-source="CustomFolder"]').click();
      record('custom-sync');
      await sleep(settleDelay);
      record('custom');
      // 4. Back to the pinned source: one mutation clears the folder.
      await chooseSource('ExactCommit');
      record('repinned-from-custom');
      // 5. Custom again, but cancel the folder picker: nothing changes.
      await chooseSource('CustomFolder');
      record('picker-cancelled');
      report({ ok: true, steps });
    } catch (error) { report({ ok: false, reason: String(error), steps }); }
  })();
`;

const downloadLatestDriver = String.raw`
  (async () => {
    ${DRIVER_PRELUDE}
    try {
      if (!await waitFor('[data-typeshed-action="DownloadLatest"]')) { report({ ok: false, reason: 'download button never rendered' }); return; }
      await click(document.querySelector('[data-section-target="project"]'));
      record('ready');
      // The probe directly after the click — before the server's Downloading
      // state lands — must show the spinner on THIS button and nothing else
      // touched.
      el('[data-typeshed-action="DownloadLatest"]').click();
      record('clicked-sync');
      await sleep(settleDelay);
      record('downloading');
      // Nothing is blocked mid-download: an SHA edit still writes.
      await change(el('[data-typeshed-commit]'), '${OTHER_COMMIT}');
      record('edited-mid-download');
      window.__realApi.postMessage({ type: 'domTestSettle' });
      await sleep(250);
      record('settled');
      report({ ok: true, steps });
    } catch (error) { report({ ok: false, reason: String(error), steps }); }
  })();
`;

const noSourceDriver = String.raw`
  (async () => {
    ${DRIVER_PRELUDE}
    try {
      if (!await waitFor('.typeshed-no-source')) { report({ ok: false, reason: 'the NO SOURCE row never rendered' }); return; }
      await click(document.querySelector('[data-section-target="project"]'));
      record('no-source');
      el('[data-typeshed-action="DownloadPinned"]').click();
      record('pinned-clicked-sync');
      await sleep(settleDelay);
      record('pinned-downloading');
      window.__realApi.postMessage({ type: 'domTestSettle' });
      await sleep(250);
      record('resolved');
      report({ ok: true, steps });
    } catch (error) { report({ ok: false, reason: String(error), steps }); }
  })();
`;

const advancedDriver = String.raw`
  (async () => {
    ${DRIVER_PRELUDE}
    try {
      if (!await waitFor('.typeshed-advanced')) { report({ ok: false, reason: 'advanced settings never rendered' }); return; }
      await click(document.querySelector('[data-section-target="project"]'));
      record('initial');
      el('.typeshed-advanced').open = true;
      el('.typeshed-advanced').dispatchEvent(new Event('toggle'));
      await click(el('[data-pick-typeshed-folder="TypeshedStorePath"]'));
      record('store-picked');
      report({ ok: true, steps });
    } catch (error) { report({ ok: false, reason: String(error), steps }); }
  })();
`;

const unpinDriver = String.raw`
  (async () => {
    ${DRIVER_PRELUDE}
    try {
      if (!await waitFor('[data-typeshed-commit]')) { report({ ok: false, reason: 'commit field never rendered' }); return; }
      await click(document.querySelector('[data-section-target="project"]'));
      record('pinned');
      // Emptying the SHA is how a user unpins from the field itself. Focus
      // the field first: the snapshot re-render must not eat that focus.
      el('[data-typeshed-commit]').focus();
      await change(el('[data-typeshed-commit]'), '   ');
      const unpinned = record('unpinned');
      unpinned.commitFocused = document.activeElement !== null
        && document.activeElement.dataset !== undefined
        && document.activeElement.dataset.typeshedCommit === 'TypeshedCommit';
      // The license action reaches the server verbatim and spins nothing.
      await click(el('[data-typeshed-action="ViewLicense"]'));
      record('after-license');
      report({ ok: true, steps });
    } catch (error) { report({ ok: false, reason: String(error), steps }); }
  })();
`;

const dialogDriver = String.raw`
  (async () => {
    ${DRIVER_PRELUDE}
    try {
      if (!await waitFor('select[data-rule-entry]')) { report({ ok: false, reason: 'rule rows never rendered' }); return; }
      const ruleValue = () => {
        const select = el('select[data-rule-entry="pep_rule_000"]');
        return select ? select.value : null;
      };
      const first = record('before');
      first.ruleValue = ruleValue();
      // A rule change still costs an impact review. Wait for the dialog the
      // change causes rather than for a fixed delay: the proposal travels to
      // the extension host and back before anything renders, and that host is
      // shared with every other suite in the run.
      await change(el('select[data-rule-entry="pep_rule_000"]'), 'Warning');
      if (!await waitUntil(() => dialog().open)) { report({ ok: false, reason: 'the impact dialog never opened for the rule change', steps }); return; }
      const opened = record('dialog-open');
      opened.ruleValue = ruleValue();
      // ...and dismissing it discards the change: the control must snap back.
      // Closing a dialog that is already open is what fires the close event, so
      // the wait above is also what makes the discard reach the host at all.
      dialog().close();
      await waitUntil(() => !dialog().open);
      await sleep(settleDelay);
      const cancelled = record('dialog-cancelled');
      cancelled.ruleValue = ruleValue();
      // Re-run it and apply for real. Applying is ignored unless the editor is
      // actually in its preview phase, so wait for the dialog to prove it is
      // there before clicking — otherwise the click is silently discarded.
      await change(el('select[data-rule-entry="pep_rule_000"]'), 'Info');
      if (!await waitUntil(() => dialog().open)) { report({ ok: false, reason: 'the impact dialog never reopened, so apply had nothing to confirm', steps }); return; }
      await click(el('[data-action="apply-preview"]'));
      await waitUntil(() => !dialog().open);
      await sleep(settleDelay);
      const applied = record('applied');
      applied.ruleValue = ruleValue();
      report({ ok: true, steps });
    } catch (error) { report({ ok: false, reason: String(error), steps }); }
  })();
`;

/** 0-2: the pinned default, in-place SHA rejection, and the atomic repin. */
function assertPinnedAndCommitEditing(steps: DomStep[] | undefined, intents: readonly Record<string, unknown>[]): void {
  const pinned = step(steps, "pinned");
  assertSelected(pinned, "ExactCommit");
  assertNothingLocked(pinned);
  assert.strictEqual(pinned.commitPresent, true, "the pinned source renders its SHA field");
  assert.strictEqual(pinned.commitValue, ACTIVE_COMMIT);
  assert.strictEqual(pinned.pathPresent, false, "a pin and a folder can never coexist");
  assert.strictEqual(pinned.booleanControls, 0, "the cache/verify toggles are deleted");
  assert.strictEqual(pinned.textControls, 0, "the alternate-URL text control is deleted");
  assert.strictEqual(pinned.advancedPresent, true, "the store folder lives under Advanced");
  assert.deepStrictEqual(
    (pinned.actions as Action[]).map((entry) => entry.action),
    ["DownloadLatest", "ViewLicense"],
    "Download latest is always offered; Download pinned only without a source",
  );
  assert.strictEqual((pinned.status as Record<string, string>).State, "Ready");
  assert.strictEqual(intents[0]?.type, "ready");

  const invalid = step(steps, "invalid-sha");
  assert.strictEqual(invalid.commitInvalid, "true", "the field must report itself invalid");
  assert.ok(
    String(invalid.commitError).includes("40-character"),
    `the error must teach the format (got "${String(invalid.commitError)}")`,
  );
  assertSelected(invalid, "ExactCommit");
  assert.ok(
    !intents.some((intent) => JSON.stringify(intent).includes("not-a-sha")),
    "an invalid SHA must never reach the configuration",
  );

  const repinned = step(steps, "repinned");
  assert.strictEqual(repinned.commitValue, OTHER_COMMIT);
  assert.strictEqual(repinned.commitError, null, "the error must clear once the SHA is valid");
  assert.strictEqual(repinned.commitInvalid, null);
  assert.strictEqual(repinned.dialogOpen, false, "a Typeshed edit never opens the impact dialog");
  assert.deepStrictEqual(mutationsOf(intents, 1), [
    { kind: "SetTypeshedSetting", key: { kind: "TypeshedCommit" }, value: { kind: "Text", value: OTHER_COMMIT } },
    { kind: "RemoveTypeshedSetting", key: { kind: "TypeshedPath" } },
  ]);
}

/** 3-5: the folder source, the atomic switch back, and the cancelled picker. */
function assertCustomFolder(steps: DomStep[] | undefined, intents: readonly Record<string, unknown>[]): void {
  // The probe straight after the radio click: no overlay, no disabled
  // control, no dialog — the reported spinner-lock cannot come back.
  const customSync = step(steps, "custom-sync");
  assertNothingLocked(customSync);
  assert.strictEqual(customSync.dialogOpen, false, "a source switch is not an impact trade-off");

  assert.deepStrictEqual(
    { type: intents[2]?.type, key: intents[2]?.key },
    { type: "pickTypeshedFolder", key: "TypeshedPath" },
  );
  const custom = step(steps, "custom");
  assertSelected(custom, "CustomFolder");
  assertNothingLocked(custom);
  assert.strictEqual(custom.pathValue, CUSTOM_FOLDER);
  assert.strictEqual(custom.commitPresent, false, "only the ACTIVE source's field exists");
  assert.strictEqual(custom.advancedPresent, false, "a user-managed folder has no store folder");
  assert.strictEqual(action(custom, "ViewLicense").disabled, true, "a custom folder supplies no license document");
  assert.strictEqual(action(custom, "DownloadLatest").disabled, false, "Download latest stays offered");

  assert.deepStrictEqual(mutationsOf(intents, 3), [
    { kind: "RemoveTypeshedSetting", key: { kind: "TypeshedPath" } },
  ], "returning to the pinned source clears exactly the folder");
  const repinned = step(steps, "repinned-from-custom");
  assertSelected(repinned, "ExactCommit");
  assert.strictEqual(
    repinned.commitValue,
    ACTIVE_COMMIT,
    "the folder pick cleared the pin, so the bundled commit serves again",
  );
  assert.strictEqual(repinned.pathPresent, false);

  assert.deepStrictEqual(
    { type: intents[4]?.type, key: intents[4]?.key },
    { type: "pickTypeshedFolder", key: "TypeshedPath" },
  );
  const cancelled = step(steps, "picker-cancelled");
  assertSelected(cancelled, "ExactCommit");
  assert.strictEqual(cancelled.pathPresent, false, "a cancelled picker must not select the folder source");
}

/** A running Download latest: spinner on that button only, everything else live. */
function assertDownloadLatest(steps: DomStep[] | undefined, intents: readonly Record<string, unknown>[]): void {
  const ready = step(steps, "ready");
  assertNothingLocked(ready);
  assert.deepStrictEqual(
    (ready.actions as Action[]).map((entry) => [entry.action, entry.disabled, entry.busy]),
    [["DownloadLatest", false, false], ["ViewLicense", false, false]],
    "Ready offers Download latest live and no Download pinned",
  );

  assert.deepStrictEqual(
    { type: intents[1]?.type, action: intents[1]?.action },
    { type: "typeshedAction", action: "DownloadLatest" },
  );
  const clicked = step(steps, "clicked-sync");
  assertNothingLocked(clicked);
  const clickedButton = action(clicked, "DownloadLatest");
  assert.strictEqual(clickedButton.busy, true, "the invoking button goes busy at once");
  assert.strictEqual(clickedButton.disabled, true, "a second identical download cannot start");

  const downloading = step(steps, "downloading");
  assert.strictEqual((downloading.status as Record<string, string>).State, "Downloading");
  assertSelected(downloading, "ExactCommit");
  assertNothingLocked(downloading);
  assert.strictEqual(action(downloading, "DownloadLatest").busy, true, "the spinner stays on the invoking button");
  assert.strictEqual(downloading.noSourcePresent, false, "a latest download is not a NO SOURCE state");

  const edited = step(steps, "edited-mid-download");
  assert.deepStrictEqual(mutationsOf(intents, 2), [
    { kind: "SetTypeshedSetting", key: { kind: "TypeshedCommit" }, value: { kind: "Text", value: OTHER_COMMIT } },
    { kind: "RemoveTypeshedSetting", key: { kind: "TypeshedPath" } },
  ], "an SHA edit mid-download still writes — configuration never waits on the network");
  assert.strictEqual(edited.commitValue, OTHER_COMMIT);
  assertNothingLocked(edited);

  const settled = step(steps, "settled");
  assert.strictEqual((settled.status as Record<string, string>).State, "Ready");
  assert.strictEqual(settled.commitValue, LATEST_COMMIT, "the finished download wrote the resolved SHA");
  assert.deepStrictEqual(
    (settled.actions as Action[]).map((entry) => [entry.action, entry.disabled, entry.busy]),
    [["DownloadLatest", false, false], ["ViewLicense", false, false]],
    "settling releases the button and removes the spinner",
  );
}

/** NO SOURCE: a persistent inline row whose fix is the Download pinned button. */
function assertNoSource(steps: DomStep[] | undefined, intents: readonly Record<string, unknown>[]): void {
  const noSource = step(steps, "no-source");
  assert.strictEqual((noSource.status as Record<string, string>).State, "NoSource");
  assert.strictEqual(noSource.noSourcePresent, true, "the reason renders as a persistent row in the panel");
  assert.ok(
    String(noSource.noSourceText).includes(NO_SOURCE_REASON),
    `the row must state the server's reason (got "${String(noSource.noSourceText)}")`,
  );
  assert.ok(String(noSource.noSourceText).includes("Download pinned"), "the row carries its fix inline");
  assertSelected(noSource, "ExactCommit");
  assertNothingLocked(noSource);
  assert.strictEqual(noSource.commitValue, OTHER_COMMIT, "the pinned SHA stays visible and editable");
  assert.strictEqual(action(noSource, "DownloadPinned").disabled, false);

  assert.deepStrictEqual(
    { type: intents[1]?.type, action: intents[1]?.action },
    { type: "typeshedAction", action: "DownloadPinned" },
  );
  assert.ok(
    !intents.some((intent) => intent.type === "preview"),
    "a download writes no configuration at all",
  );
  const clicked = step(steps, "pinned-clicked-sync");
  assertNothingLocked(clicked);
  assert.strictEqual(action(clicked, "DownloadPinned").busy, true, "the invoking button goes busy at once");
  assert.strictEqual(action(clicked, "DownloadLatest").busy, false, "the other download button never spins");

  const downloading = step(steps, "pinned-downloading");
  assert.strictEqual((downloading.status as Record<string, string>).State, "Downloading");
  assert.strictEqual(downloading.noSourcePresent, true, "the row keeps the busy button until the source settles");
  assert.strictEqual(action(downloading, "DownloadPinned").busy, true);
  assert.strictEqual(action(downloading, "DownloadLatest").busy, false);
  assert.strictEqual(action(downloading, "DownloadLatest").disabled, true, "one download at a time");
  assertNothingLocked(downloading);

  const resolved = step(steps, "resolved");
  assert.strictEqual((resolved.status as Record<string, string>).State, "Ready");
  assert.strictEqual(resolved.noSourcePresent, false, "the row disappears once a source exists");
  assert.deepStrictEqual(
    (resolved.actions as Action[]).map((entry) => entry.action),
    ["DownloadLatest", "ViewLicense"],
    "Download pinned is offered only while there is no source",
  );
}

suite("Configuration editor — Typeshed source in a real webview DOM", () => {
  test("switching between the two sources writes one atomic mutation and never locks the panel", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 20_000);
    const host = new ScenarioHost({ folders: [CUSTOM_FOLDER, undefined] });
    const { result, intents } = await runScenario(sourceJourneyDriver, host);
    assert.strictEqual(result.ok, true, `driver failed: ${result.reason ?? "unknown"}`);
    assertPinnedAndCommitEditing(result.steps, intents);
    assertCustomFolder(result.steps, intents);
  });

  test("Download latest spins only its own button while every control stays live", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 20_000);
    const host = new ScenarioHost();
    const { result, intents } = await runScenario(downloadLatestDriver, host);
    assert.strictEqual(result.ok, true, `driver failed: ${result.reason ?? "unknown"}`);
    assertDownloadLatest(result.steps, intents);
  });

  test("NO SOURCE renders a persistent inline row fixed by Download pinned", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 20_000);
    const host = new ScenarioHost({ config: { commit: OTHER_COMMIT }, noSourceReason: NO_SOURCE_REASON });
    const { result, intents } = await runScenario(noSourceDriver, host);
    assert.strictEqual(result.ok, true, `driver failed: ${result.reason ?? "unknown"}`);
    assertNoSource(result.steps, intents);
  });

  test("Advanced holds only the store folder picker and remembers its disclosure", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 20_000);
    const host = new ScenarioHost({ folders: [STORE_FOLDER] });
    const { result, intents } = await runScenario(advancedDriver, host);
    assert.strictEqual(result.ok, true, `driver failed: ${result.reason ?? "unknown"}`);

    const initial = step(result.steps, "initial");
    assert.strictEqual(initial.booleanControls, 0, "the cache/verify toggles are deleted");
    assert.strictEqual(initial.textControls, 0, "the alternate-URL text control is deleted");
    assert.strictEqual(initial.advancedOpen, false, "advanced settings start folded away");
    assertNothingLocked(initial);

    assert.deepStrictEqual(
      { type: intents[1]?.type, key: intents[1]?.key },
      { type: "pickTypeshedFolder", key: "TypeshedStorePath" },
    );
    const picked = step(result.steps, "store-picked");
    assert.strictEqual(picked.storeFolderValue, STORE_FOLDER);
    assert.strictEqual(picked.advancedOpen, true, "the disclosure must not snap shut under the user's hands");
  });

  test("emptying the pinned SHA unpins without stealing focus, and ViewLicense relays verbatim", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 20_000);
    const host = new ScenarioHost({ config: { commit: OTHER_COMMIT } });
    const { result, intents } = await runScenario(unpinDriver, host);
    assert.strictEqual(result.ok, true, `driver failed: ${result.reason ?? "unknown"}`);

    const pinned = step(result.steps, "pinned");
    assertSelected(pinned, "ExactCommit");
    assert.strictEqual(pinned.commitValue, OTHER_COMMIT, "the field shows the configured pin");

    assert.deepStrictEqual(mutationsOf(intents, 1), [
      { kind: "RemoveTypeshedSetting", key: { kind: "TypeshedCommit" } },
    ], "clearing the field removes the entry, it never writes an empty SHA");
    const unpinned = step(result.steps, "unpinned");
    assertSelected(unpinned, "ExactCommit");
    assert.strictEqual(unpinned.commitValue, ACTIVE_COMMIT, "the bundled commit serves once unpinned");
    assert.strictEqual(
      unpinned.commitFocused,
      true,
      "the snapshot re-render must hand focus back to the SHA field — no flicker, no lost caret",
    );

    assert.deepStrictEqual(
      { type: intents[2]?.type, action: intents[2]?.action },
      { type: "typeshedAction", action: "ViewLicense" },
      "the license action is relayed verbatim; the client executes nothing",
    );
    const after = step(result.steps, "after-license");
    assert.strictEqual(after.dialogOpen, false, "an action never opens the impact dialog");
    assert.strictEqual(action(after, "ViewLicense").busy, false, "only downloads may spin a button");
  });

  test("a dismissed rule preview discards the change and restores the control", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 20_000);
    const host = new ScenarioHost();
    const { result, intents } = await runScenario(dialogDriver, host);
    assert.strictEqual(result.ok, true, `driver failed: ${result.reason ?? "unknown"}`);
    const steps = result.steps;

    const before = step(steps, "before");
    assert.strictEqual(before.ruleValue, "Error", "an untouched pep rule runs at error");
    assert.strictEqual(before.dialogOpen, false);
    assertNothingLocked(before);

    const opened = step(steps, "dialog-open");
    assert.strictEqual(opened.dialogOpen, true, "a rule change still shows its exact impact");
    assert.strictEqual(opened.overlayPresent, false, "the impact dialog is the only modal surface");
    assert.strictEqual(
      opened.ruleValue,
      "Error",
      "the list keeps showing the configuration while the dialog carries the proposal",
    );
    assert.ok(
      String(opened.dialogChanges).includes("Error → Warning"),
      `the dialog must state the exact resolved change (got "${String(opened.dialogChanges)}")`,
    );

    const cancelled = step(steps, "dialog-cancelled");
    assert.strictEqual(cancelled.dialogOpen, false);
    assert.strictEqual(
      cancelled.ruleValue,
      "Error",
      "dismissing the dialog must restore the configuration's value, never leave the choice on screen",
    );
    assert.ok(
      intents.some((intent) => intent.type === "cancelPreview"),
      "the runtime must tell the host the preview was discarded",
    );

    const applied = step(steps, "applied");
    assert.strictEqual(applied.dialogOpen, false);
    assert.strictEqual(applied.ruleValue, "Info", "an applied change survives the re-render");
    assert.ok(
      intents.some((intent) => intent.type === "apply"),
      "applying must go through the previewed change",
    );
  });
});
