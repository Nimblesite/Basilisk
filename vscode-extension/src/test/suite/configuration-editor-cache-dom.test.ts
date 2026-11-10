// Tests [LSPCFGED-CACHE] in a REAL webview DOM, driven like a user.
//
// What this suite locks down:
//   * the Project view names BOTH caching layers — a panel that showed only
//     the persistent switch would read as "this is all the caching there is",
//     which is exactly the confusion the panel exists to remove;
//   * the in-session Salsa layer renders read-only, with no control of any
//     kind, because it has no configuration key;
//   * a cache edit lands immediately with no impact dialog: there is no
//     rule-severity trade-off to weigh ([CONFIGEDITOR-VSIX-EXPERIENCE]);
//   * the toggle writes an explicit `cache = true|false`, and the folder
//     reset REMOVES `cache-dir` rather than writing the default back as an
//     entry;
//   * a cancelled folder picker writes nothing at all.

import * as assert from "assert";
import {
  DRIVER_PRELUDE,
  RESULT_TIMEOUT_MS,
  runScenario,
  ScenarioHost,
  type DomStep,
  type ScenarioOutcome,
} from "./webview-dom-harness";
import { decodeConfigurationEditorIntent } from "../../configuration-editor-intents";
import type { EditorMutation } from "../../configuration-editor-model";
import { booleanField, rawField, stringField } from "../../unknown-shape";

const DEFAULT_CACHE_DIR = "/workspace/project/.basilisk/cache/check";
const CHOSEN_CACHE_DIR = "/workspace/project/build/bsk-cache";

function step(steps: DomStep[] | undefined, label: string): DomStep {
  const found = (steps ?? []).find((entry) => entry.label === label);
  assert.ok(
    found,
    `driver never recorded step "${label}" (recorded: ${(steps ?? []).map((entry) => entry.label).join(", ")})`,
  );
  return found;
}

/** One row of the read-only in-session table a recorded step rendered. */
function inSessionRow(entry: DomStep, row: string): string | undefined {
  return stringField(rawField(entry, "inSession"), row);
}

function mutationsOf(intents: readonly Record<string, unknown>[], index: number): unknown {
  return intents[index]?.mutations;
}

/** Every `preview` intent the runtime posted, decoded through the real decoder. */
function previewMutations(intents: readonly Record<string, unknown>[]): EditorMutation[][] {
  return intents
    .map((intent, index) => ({ intent, index }))
    .filter(({ intent }) => intent.type === "preview")
    .map(({ index }) => {
      const decoded = decodeConfigurationEditorIntent({
        type: "preview",
        mutations: mutationsOf(intents, index),
      });
      assert.ok(
        decoded?.type === "preview",
        `preview intent ${index} must survive the production decoder`,
      );
      return decoded.mutations;
    });
}

const cacheJourneyDriver = String.raw`
  (async () => {
    ${DRIVER_PRELUDE}
    try {
      if (!await waitFor('[data-cache-enabled]')) { report({ ok: false, reason: 'caching controls never rendered' }); return; }
      await click(document.querySelector('[data-section-target="project"]'));
      record('initial');
      // 1. Turning the cache on must land at once — no impact dialog stands
      //    between a setting switch and the configuration.
      el('[data-cache-enabled]').click();
      record('enabled-sync');
      await waitUntil(() => el('[data-cache-enabled]').checked === true);
      record('enabled');
      // 2. Choose a folder through the native picker.
      await click(el('[data-pick-cache-folder]'));
      await waitUntil(() => el('[data-cache-folder]').value === '${CHOSEN_CACHE_DIR}');
      record('folder-chosen');
      // 3. Reset it: the key is removed, so the default returns.
      await click(el('[data-action="reset-cache-folder"]'));
      await waitUntil(() => el('[data-cache-folder]').value === '${DEFAULT_CACHE_DIR}');
      record('folder-reset');
      // 4. Turn it back off — an explicit false, not an erased key.
      el('[data-cache-enabled]').click();
      await waitUntil(() => el('[data-cache-enabled]').checked === false);
      record('disabled');
      // 5. Cancel the picker: nothing may be written.
      await click(el('[data-pick-cache-folder]'));
      await sleep(settleDelay);
      record('picker-cancelled');
      report({ ok: true, steps });
    } catch (error) { report({ ok: false, reason: String(error), steps }); }
  })();
`;

suite("Configuration editor · caching panel DOM", () => {
  // One journey, asserted from three angles. Driving a real webview costs ~10s
  // of panel setup and IPC per run, so the scenario runs ONCE in suiteSetup
  // and every test reads the same recorded steps — re-running it per test
  // would triple the wall clock to observe the identical DOM.
  let outcome: ScenarioOutcome;

  suiteSetup(async function runOnce() {
    this.timeout(RESULT_TIMEOUT_MS * 3);
    outcome = await runScenario(
      cacheJourneyDriver,
      new ScenarioHost({ folders: [CHOSEN_CACHE_DIR, undefined] }),
    );
    assert.ok(outcome.result.ok, `driver failed: ${outcome.result.reason ?? "unknown"}`);
  });

  test("both layers render, and only the persistent one is configurable", () => {
    const initial = step(outcome.result.steps, "initial");
    assert.strictEqual(
      booleanField(initial, "cacheEnabledPresent"),
      true,
      "the Project view must render the persistent cache toggle",
    );
    assert.strictEqual(
      booleanField(initial, "cacheEnabled"),
      false,
      "an unconfigured project shows the persistent cache off",
    );
    assert.strictEqual(
      stringField(initial, "cacheFolderValue"),
      DEFAULT_CACHE_DIR,
      "the default folder is shown even before the cache is enabled",
    );
    assert.strictEqual(
      booleanField(initial, "cacheResetPresent"),
      false,
      "there is nothing to reset until the project chooses a folder",
    );

    // The whole point of the panel: Salsa is named, and stated as always on.
    assert.strictEqual(inSessionRow(initial, "Engine"), "Salsa incremental queries");
    assert.strictEqual(inSessionRow(initial, "State"), "Always on · no configuration");
    assert.ok(
      (inSessionRow(initial, "Memoized files") ?? "").includes("tracked in this session"),
      "the in-session layer must report its live memo count",
    );
  });

  test("a cache edit lands at once and writes explicit keys", () => {
    const steps = outcome.result.steps;

    // No impact dialog: a setting switch has no severity trade-off to weigh.
    ["enabled-sync", "enabled", "folder-chosen", "folder-reset", "disabled"].forEach((label) => {
      assert.strictEqual(
        booleanField(step(steps, label), "dialogOpen"),
        false,
        `step "${label}" must not open the impact dialog for a cache edit`,
      );
    });

    assert.strictEqual(booleanField(step(steps, "enabled"), "cacheEnabled"), true);
    assert.strictEqual(
      stringField(step(steps, "folder-chosen"), "cacheFolderValue"),
      CHOSEN_CACHE_DIR,
    );
    assert.strictEqual(
      booleanField(step(steps, "folder-chosen"), "cacheResetPresent"),
      true,
      "a chosen folder is a project decision, so it can be undone",
    );
    assert.strictEqual(
      stringField(step(steps, "folder-reset"), "cacheFolderValue"),
      DEFAULT_CACHE_DIR,
      "resetting must fall back to the default, not blank the field",
    );
    assert.strictEqual(booleanField(step(steps, "disabled"), "cacheEnabled"), false);

    // The exact wire vocabulary: explicit booleans, and a REMOVE for the
    // folder reset rather than the default written back as an entry. Choosing
    // a folder posts `pickCacheFolder`, not a mutation — the host builds the
    // write from what the native picker returned.
    assert.deepStrictEqual(previewMutations(outcome.intents), [
      [{ kind: "SetCacheSetting", key: { kind: "CacheEnabled" }, value: "true" }],
      [{ kind: "RemoveCacheSetting", key: { kind: "CacheDir" } }],
      [{ kind: "SetCacheSetting", key: { kind: "CacheEnabled" }, value: "false" }],
    ]);
    assert.strictEqual(
      outcome.intents.filter((intent) => intent.type === "pickCacheFolder").length,
      2,
      "both folder interactions must route through the native picker",
    );
  });

  test("a cancelled folder picker writes nothing and restores the controls", () => {
    const cancelled = step(outcome.result.steps, "picker-cancelled");
    assert.strictEqual(
      stringField(cancelled, "cacheFolderValue"),
      DEFAULT_CACHE_DIR,
      "a cancelled picker leaves the configuration that still holds",
    );
    assert.strictEqual(booleanField(cancelled, "cacheEnabled"), false);
    assert.strictEqual(
      previewMutations(outcome.intents).length,
      3,
      "the cancelled picker must post no further mutation",
    );
    assert.strictEqual(
      booleanField(cancelled, "cachePickerDisabled"),
      false,
      "nothing in the caching panel ever locks",
    );
  });
});
