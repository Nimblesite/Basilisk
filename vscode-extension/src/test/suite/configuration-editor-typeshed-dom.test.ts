// Tests [LSPCFGED-TYPESHED] in a REAL webview DOM, driven like a user.
//
// The reported failures this suite locks down:
//   * the source selector showed "Latest" while a commit was pinned, because
//     an unapplied choice was never reverted — selecting Latest must CLEAR the
//     pin, and every control must render the configuration that actually holds;
//   * "Reuse downloads" did nothing: the toggle sat behind an impact dialog
//     built for rule severities, so a dismissed dialog left the box flipped
//     with nothing written;
//   * controls for sources that were not selected were rendered at all.
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
import { ACTIVE_COMMIT, OTHER_COMMIT } from "./typeshed-fixture";

const CUSTOM_FOLDER = "/workspace/vendor/typeshed";
const CACHE_FOLDER = "/workspace/.cache/typeshed";

interface Source {
  readonly mode: string;
  readonly checked: boolean;
  readonly disabled: boolean;
  readonly hint: string;
}

function step(steps: DomStep[] | undefined, label: string): DomStep {
  const found = (steps ?? []).find((entry) => entry.label === label);
  assert.ok(found, `driver never recorded step "${label}" (recorded: ${(steps ?? []).map((entry) => entry.label).join(", ")})`);
  return found;
}

function source(entry: DomStep, mode: string): Source {
  const sources = entry.sources as Source[];
  const found = sources.find((candidate) => candidate.mode === mode);
  assert.ok(found, `step "${entry.label}" rendered no ${mode} source choice`);
  return found;
}

/** Exactly one source is selected, and it is the expected one. */
function assertSelected(entry: DomStep, mode: string): void {
  const sources = entry.sources as Source[];
  assert.strictEqual(sources.length, 3, `step "${entry.label}" must offer the three sources`);
  assert.deepStrictEqual(
    sources.filter((candidate) => candidate.checked).map((candidate) => candidate.mode),
    [mode],
    `step "${entry.label}" must show ${mode} as the one active source`,
  );
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
      record('latest');
      // 1. Pin the active commit by choosing the pinned source.
      await chooseSource('ExactCommit');
      record('pinned');
      // 2. Reject an invalid SHA in place — nothing may be written.
      await change(el('[data-typeshed-commit]'), 'not-a-sha');
      record('invalid-sha');
      // 3. A valid SHA repins.
      await change(el('[data-typeshed-commit]'), '${OTHER_COMMIT}');
      record('repinned');
      // 4. Back to Latest: the pin must be CLEARED.
      await chooseSource('Latest');
      record('unpinned');
      // 5. A custom folder replaces the downloaded source entirely.
      await chooseSource('CustomFolder');
      record('custom');
      // 6. Back to Latest, then cancel the folder picker: nothing changes.
      await chooseSource('Latest');
      record('latest-again');
      await chooseSource('CustomFolder');
      record('picker-cancelled');
      report({ ok: true, steps });
    } catch (error) { report({ ok: false, reason: String(error), steps }); }
  })();
`;

const downloadsDriver = String.raw`
  (async () => {
    ${DRIVER_PRELUDE}
    try {
      if (!await waitFor('[data-typeshed-boolean="TypeshedCache"]')) { report({ ok: false, reason: 'download controls never rendered' }); return; }
      await click(document.querySelector('[data-section-target="project"]'));
      record('initial');
      await change(el('[data-typeshed-boolean="TypeshedCache"]'), false);
      record('reuse-off');
      await change(el('[data-typeshed-boolean="TypeshedCache"]'), true);
      record('reuse-on');
      await change(el('[data-typeshed-boolean="TypeshedVerify"]'), false);
      record('verify-off');
      // Advanced settings stay folded away until asked for.
      const foldedByDefault = !el('.typeshed-advanced').open;
      el('.typeshed-advanced').open = true;
      el('.typeshed-advanced').dispatchEvent(new Event('toggle'));
      await change(el('[data-typeshed-text="TypeshedUrl"]'), 'https://mirror.test/{sha}.zip');
      record('mirror-set');
      await change(el('[data-typeshed-text="TypeshedUrl"]'), '');
      record('mirror-cleared');
      await click(el('[data-pick-typeshed-folder="TypeshedCachePath"]'));
      record('cache-folder');
      report({ ok: true, steps, advancedFoldedByDefault: foldedByDefault });
    } catch (error) { report({ ok: false, reason: String(error), steps }); }
  })();
`;

const acquiringDriver = String.raw`
  (async () => {
    ${DRIVER_PRELUDE}
    try {
      if (!await waitFor('[data-typeshed-source]')) { report({ ok: false, reason: 'typeshed controls never rendered' }); return; }
      await click(document.querySelector('[data-section-target="project"]'));
      record('acquiring');
      // Every control is inert, so no second mutation can race the candidate.
      await chooseSource('Latest');
      await chooseSource('CustomFolder');
      record('acquiring-after-clicks');
      window.__realApi.postMessage({ type: 'domTestSettle' });
      await sleep(250);
      record('settled');
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


const pinnedStartDriver = String.raw`
  (async () => {
    ${DRIVER_PRELUDE}
    try {
      if (!await waitFor('[data-typeshed-commit]')) { report({ ok: false, reason: 'commit field never rendered' }); return; }
      await click(document.querySelector('[data-section-target="project"]'));
      record('pinned');
      // Emptying the SHA is how a user unpins from the field itself.
      await change(el('[data-typeshed-commit]'), '   ');
      record('unpinned');
      // The two remaining actions reach the server verbatim.
      await click(el('[data-typeshed-action="AcquireFresh"]'));
      await click(el('[data-typeshed-action="ViewLicense"]'));
      record('after-actions');
      report({ ok: true, steps });
    } catch (error) { report({ ok: false, reason: String(error), steps }); }
  })();
`;

/** 0-1: Latest offers no source-specific field; pinning writes the ACTIVE commit. */
function assertLatestThenPinned(steps: DomStep[] | undefined, intents: readonly Record<string, unknown>[]): void {
  const latest = step(steps, "latest");
  assertSelected(latest, "Latest");
  assert.strictEqual(latest.commitPresent, false, "Latest has no commit field to mislead with");
  assert.strictEqual(latest.pathPresent, false, "Latest has no folder field");
  assert.strictEqual(latest.reusePresent, true, "a downloaded source states its download policy");
  assert.strictEqual(latest.advancedPresent, true);
  assert.strictEqual(source(latest, "ExactCommit").disabled, false, "an active commit can be pinned");
  assert.deepStrictEqual(
    (latest.actions as { action: string }[]).map((action) => action.action),
    ["AcquireFresh", "ViewLicense"],
    "pinning is the source choice itself, not a redundant button",
  );
  assert.strictEqual((latest.status as Record<string, string>).State, "Ready");

  assert.strictEqual(intents[0]?.type, "ready");
  assert.deepStrictEqual(
    { type: intents[1]?.type, action: intents[1]?.action },
    { type: "typeshedAction", action: "PinCurrent" },
    "choosing the pinned source pins the active commit",
  );
  const pinned = step(steps, "pinned");
  assertSelected(pinned, "ExactCommit");
  assert.strictEqual(pinned.commitPresent, true);
  assert.strictEqual(pinned.commitValue, ACTIVE_COMMIT);
  assert.strictEqual(pinned.pathPresent, false, "a pin and a folder can never coexist");
  assert.strictEqual(pinned.reusePresent, true, "a pinned commit is still downloaded");
}

/** 2-4: an invalid SHA is refused in place; Latest CLEARS the pin. */
function assertCommitEditing(steps: DomStep[] | undefined, intents: readonly Record<string, unknown>[]): void {
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
  assert.deepStrictEqual(mutationsOf(intents, 2), [
    { kind: "SetTypeshedSetting", key: { kind: "TypeshedCommit" }, value: { kind: "Text", value: OTHER_COMMIT } },
    { kind: "RemoveTypeshedSetting", key: { kind: "TypeshedPath" } },
  ]);

  assert.deepStrictEqual(mutationsOf(intents, 3), [
    { kind: "RemoveTypeshedSetting", key: { kind: "TypeshedCommit" } },
    { kind: "RemoveTypeshedSetting", key: { kind: "TypeshedPath" } },
  ], "selecting Latest must CLEAR the pinned commit");
  const unpinned = step(steps, "unpinned");
  assertSelected(unpinned, "Latest");
  assert.strictEqual(unpinned.commitPresent, false, "an unpinned source keeps no commit field");
  assert.strictEqual(unpinned.dialogOpen, false, "a source switch is not an impact trade-off");
}

/** 5-6: a custom folder replaces the download policy; a cancelled pick changes nothing. */
function assertCustomFolder(steps: DomStep[] | undefined, intents: readonly Record<string, unknown>[]): void {
  assert.deepStrictEqual(
    { type: intents[4]?.type, key: intents[4]?.key },
    { type: "pickTypeshedFolder", key: "TypeshedPath" },
  );
  const custom = step(steps, "custom");
  assertSelected(custom, "CustomFolder");
  assert.strictEqual(custom.pathValue, CUSTOM_FOLDER);
  assert.strictEqual(custom.commitPresent, false);
  assert.strictEqual(custom.reusePresent, false, "a user-managed folder downloads nothing");
  assert.strictEqual(custom.verifyPresent, false);
  assert.strictEqual(custom.advancedPresent, false);
  const customPin = source(custom, "ExactCommit");
  assert.strictEqual(customPin.disabled, true, "a custom folder has no upstream commit to pin");
  assert.ok(
    customPin.hint.includes("Choose Latest first"),
    `an unavailable source must teach why (got "${customPin.hint}")`,
  );

  const latestAgain = step(steps, "latest-again");
  assertSelected(latestAgain, "Latest");
  assert.strictEqual(latestAgain.pathPresent, false);
  assert.strictEqual(latestAgain.reusePresent, true);

  const cancelled = step(steps, "picker-cancelled");
  assertSelected(cancelled, "Latest");
  assert.strictEqual(cancelled.pathPresent, false, "a cancelled picker must not select the folder source");
  assert.strictEqual(cancelled.reusePresent, true);
}

/** Toggling a download setting writes at once and survives the re-render. */
function assertDownloadToggles(steps: DomStep[] | undefined, intents: readonly Record<string, unknown>[]): void {
  const initial = step(steps, "initial");
  assert.strictEqual(initial.reuseChecked, true, "the default policy reuses gate-accepted downloads");
  assert.strictEqual(initial.verifyChecked, true);
  assert.strictEqual(initial.urlValue, "", "no mirror is configured by default");

  const off = step(steps, "reuse-off");
  assert.deepStrictEqual(mutationsOf(intents, 1), [
    { kind: "SetTypeshedSetting", key: { kind: "TypeshedCache" }, value: { kind: "Boolean", value: false } },
  ]);
  assert.strictEqual(off.reuseChecked, false, "the re-rendered checkbox must hold the written value");
  assert.strictEqual(off.dialogOpen, false, "a Typeshed toggle never opens the impact dialog");
  assert.strictEqual(off.verifyChecked, true, "one toggle changes one setting");

  const on = step(steps, "reuse-on");
  assert.deepStrictEqual(mutationsOf(intents, 2), [
    { kind: "SetTypeshedSetting", key: { kind: "TypeshedCache" }, value: { kind: "Boolean", value: true } },
  ]);
  assert.strictEqual(on.reuseChecked, true, "toggling back must round-trip");

  const verifyOff = step(steps, "verify-off");
  assert.deepStrictEqual(mutationsOf(intents, 3), [
    { kind: "SetTypeshedSetting", key: { kind: "TypeshedVerify" }, value: { kind: "Boolean", value: false } },
  ]);
  assert.strictEqual(verifyOff.verifyChecked, false);
  assert.strictEqual(verifyOff.reuseChecked, true, "the other toggle is untouched");
}

/** The rarely-needed mirror and cache folder live behind Advanced. */
function assertAdvancedDownloads(steps: DomStep[] | undefined, intents: readonly Record<string, unknown>[]): void {
  const mirror = step(steps, "mirror-set");
  assert.deepStrictEqual(mutationsOf(intents, 4), [
    { kind: "SetTypeshedSetting", key: { kind: "TypeshedUrl" }, value: { kind: "Text", value: "https://mirror.test/{sha}.zip" } },
  ]);
  assert.strictEqual(mirror.urlValue, "https://mirror.test/{sha}.zip");
  assert.strictEqual(mirror.advancedOpen, true, "the disclosure must not snap shut under the user's hands");

  const cleared = step(steps, "mirror-cleared");
  assert.deepStrictEqual(mutationsOf(intents, 5), [
    { kind: "RemoveTypeshedSetting", key: { kind: "TypeshedUrl" } },
  ], "emptying a text setting removes the entry rather than writing an empty one");
  assert.strictEqual(cleared.urlValue, "");

  const cacheFolder = step(steps, "cache-folder");
  assert.deepStrictEqual(
    { type: intents[6]?.type, key: intents[6]?.key },
    { type: "pickTypeshedFolder", key: "TypeshedCachePath" },
  );
  assert.strictEqual(cacheFolder.cacheFolderValue, CACHE_FOLDER);
  assert.strictEqual(cacheFolder.reuseChecked, true, "unrelated policy survives a folder pick");
  assert.strictEqual(cacheFolder.verifyChecked, false);
}

/** Unpinning from the field itself, and the two verbatim server actions. */
function assertUnpinAndActions(steps: DomStep[] | undefined, intents: readonly Record<string, unknown>[]): void {
  const pinned = step(steps, "pinned");
  assertSelected(pinned, "ExactCommit");
  assert.strictEqual(pinned.commitValue, OTHER_COMMIT, "the field shows the configured pin");
  assert.strictEqual(pinned.reusePresent, true, "a pinned commit still downloads");
  assert.strictEqual(
    (pinned.status as Record<string, string>)["Active source"],
    "Bundled · 83c2518a9e6a",
    "the status states the ACTIVE source, which may differ from the configured one",
  );

  assert.deepStrictEqual(mutationsOf(intents, 1), [
    { kind: "RemoveTypeshedSetting", key: { kind: "TypeshedCommit" } },
  ], "clearing the field removes the entry, it never writes an empty SHA");
  const unpinned = step(steps, "unpinned");
  assertSelected(unpinned, "Latest");
  assert.strictEqual(unpinned.commitPresent, false);
  assert.strictEqual(unpinned.reuseChecked, true, "download policy survives the source change");

  assert.deepStrictEqual(
    intents.slice(2).map((intent) => [intent.type, intent.action]),
    [["typeshedAction", "AcquireFresh"], ["typeshedAction", "ViewLicense"]],
    "both actions are relayed verbatim; the client executes neither",
  );
  const after = step(steps, "after-actions");
  assertSelected(after, "Latest");
  assert.strictEqual(after.dialogOpen, false, "an action never opens the impact dialog");
}

suite("Configuration editor — Typeshed source in a real webview DOM", () => {
  test("switching sources writes exactly one source and clears the others", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 20_000);
    const host = new ScenarioHost({ folders: [CUSTOM_FOLDER, undefined] });
    const { result, intents } = await runScenario(sourceJourneyDriver, host);
    assert.strictEqual(result.ok, true, `driver failed: ${result.reason ?? "unknown"}`);
    assertLatestThenPinned(result.steps, intents);
    assertCommitEditing(result.steps, intents);
    assertCustomFolder(result.steps, intents);
  });

  test("download policy toggles write immediately and re-render from the configuration", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 20_000);
    const host = new ScenarioHost({ folders: [CACHE_FOLDER] });
    const { result, intents } = await runScenario(downloadsDriver, host);
    assert.strictEqual(result.ok, true, `driver failed: ${result.reason ?? "unknown"}`);
    assert.strictEqual(result.advancedFoldedByDefault, true, "advanced settings start folded away");
    assertDownloadToggles(result.steps, intents);
    assertAdvancedDownloads(result.steps, intents);
  });


  test("emptying the pinned SHA unpins, and the two actions reach the server", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 20_000);
    const host = new ScenarioHost({ config: { commit: OTHER_COMMIT } });
    const { result, intents } = await runScenario(pinnedStartDriver, host);
    assert.strictEqual(result.ok, true, `driver failed: ${result.reason ?? "unknown"}`);

    assertUnpinAndActions(result.steps, intents);
  });

  test("an in-flight acquisition locks every control until it settles", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 20_000);
    const host = new ScenarioHost({ acquiring: true });
    const { result, intents } = await runScenario(acquiringDriver, host);
    assert.strictEqual(result.ok, true, `driver failed: ${result.reason ?? "unknown"}`);
    const steps = result.steps;

    const acquiring = step(steps, "acquiring");
    assertSelected(acquiring, "Latest");
    (acquiring.sources as Source[]).forEach((candidate) => {
      if (candidate.checked) { return; }
      assert.strictEqual(candidate.disabled, true, `${candidate.mode} must be locked while acquiring`);
      assert.ok(
        candidate.hint.includes("being acquired"),
        `a locked source must say why (got "${candidate.hint}")`,
      );
    });
    assert.strictEqual(acquiring.reuseDisabled, true, "download policy is locked while acquiring");
    assert.deepStrictEqual(
      (acquiring.actions as { action: string; disabled: boolean }[]).map((action) => action.disabled),
      [true, true],
      "no action may race an in-flight candidate",
    );
    assert.strictEqual((acquiring.status as Record<string, string>).State, "Acquiring");

    assert.deepStrictEqual(
      intents.filter((intent) => intent.type !== "ready" && intent.type !== "occurrences"),
      [],
      "a locked control must send nothing at all",
    );
    const afterClicks = step(steps, "acquiring-after-clicks");
    assertSelected(afterClicks, "Latest");

    const settled = step(steps, "settled");
    assert.strictEqual((settled.status as Record<string, string>).State, "Ready");
    (settled.sources as Source[]).forEach((candidate) => {
      assert.strictEqual(candidate.disabled, false, `${candidate.mode} must unlock once settled`);
    });
    assert.strictEqual(settled.reuseDisabled, false);
    assert.deepStrictEqual(
      (settled.actions as { disabled: boolean }[]).map((action) => action.disabled),
      [false, false],
    );
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

    const opened = step(steps, "dialog-open");
    assert.strictEqual(opened.dialogOpen, true, "a rule change still shows its exact impact");
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
