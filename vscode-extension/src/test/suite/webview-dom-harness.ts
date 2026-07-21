// Harness for driving the REAL configuration-editor webview runtime in a real
// webview DOM ([CONFIGEDITOR-VSIX-EXPERIENCE], [LSPCFGED-TYPESHED]).
//
// The page gets the production document and script; every intent it posts is
// answered on the EXTENSION side by ScenarioHost, which reproduces the real
// host/server pair — including that a Typeshed edit applies immediately and a
// dismissed preview returns to the snapshot. Drivers therefore observe exactly
// what a user would: interact, wait for the state push, read the DOM back.

import * as assert from "assert";
import * as vscode from "vscode";
import { buildConfigurationEditorDocument } from "../../configuration-editor-document";
import type {
  ConfigurationSnapshot,
  EditorMutation,
  TypeshedConfigurationState,
} from "../../configuration-editor-model";
import { ACTIVE_COMMIT, typeshedFixture } from "./typeshed-fixture";

export const RESULT_TIMEOUT_MS = 30_000;
const PEP_RULE_COUNT = 40;
const BASILISK_RULE_COUNT = 5;

export interface DomStep {
  readonly label: string;
  readonly [observation: string]: unknown;
}

export interface DomTestResult {
  readonly ok: boolean;
  readonly reason?: string;
  readonly steps?: DomStep[];
  readonly [observation: string]: unknown;
}

export interface ScenarioOutcome {
  readonly result: DomTestResult;
  /** Every intent the runtime posted, in order. */
  readonly intents: readonly Record<string, unknown>[];
}

/** The persisted Typeshed configuration the fake server holds. */
export interface HostConfig {
  commit?: string;
  path?: string;
  archiveUrl?: string;
  cacheFolder?: string;
  reuseDownloads?: boolean;
  verifyContent?: boolean;
}

function typeshedFor(config: HostConfig, acquiring: boolean): TypeshedConfigurationState {
  const source = config.path !== undefined
    ? ({ kind: "CustomFolder", path: config.path } as const)
    : config.commit !== undefined
      ? ({ kind: "ExactCommit", commit: config.commit } as const)
      : ({ kind: "Latest" } as const);
  return typeshedFixture({
    source,
    acquiring,
    downloads: source.kind === "CustomFolder" ? undefined : {
      reuseDownloads: config.reuseDownloads ?? true,
      verifyContent: config.verifyContent ?? true,
      archiveUrl: config.archiveUrl,
      cacheFolder: config.cacheFolder,
    },
    pinnableCommit: !acquiring && source.kind === "Latest" ? ACTIVE_COMMIT : undefined,
  });
}

/** A realistic rule catalog: pep rules first, basilisk rules at the bottom. */
function fixtureRules(): ConfigurationSnapshot["rules"] {
  const pep = Array.from({ length: PEP_RULE_COUNT }, (_ignored, index) => ({
    descriptor: {
      code: `pep_rule_${String(index).padStart(3, "0")}`,
      title: `PEP rule ${index}`,
      summary: `Summary for pep rule ${index}`,
      tags: ["pep", "generics"],
      docsUrl: `https://www.basilisk-python.dev/errors/pep-${index}`,
    },
    entry: undefined,
    effectiveSeverity: { kind: "Error" } as const,
    diagnosticCount: index,
  }));
  const basilisk = Array.from({ length: BASILISK_RULE_COUNT }, (_ignored, index) => ({
    descriptor: {
      code: `BSK-${String(index + 1).padStart(4, "0")}`,
      title: `Basilisk rule ${index + 1}`,
      summary: `Summary for basilisk rule ${index + 1}`,
      tags: ["basilisk", "strictness"],
      docsUrl: `https://www.basilisk-python.dev/errors/BSK-${String(index + 1).padStart(4, "0")}`,
    },
    entry: undefined,
    // The last analyze rule resolves to Disabled ([CHKARCH-CONFIG-MODEL] step 3).
    effectiveSeverity: index === BASILISK_RULE_COUNT - 1
      ? ({ kind: "Disabled" } as const)
      : ({ kind: "Error" } as const),
    diagnosticCount: index + 1,
  }));
  return [...pep, ...basilisk];
}

function fixtureSnapshot(typeshed: TypeshedConfigurationState, revision: string): ConfigurationSnapshot {
  return {
    rootUri: "file:///workspace/project",
    configUri: "file:///workspace/project/pyproject.toml",
    revision,
    rules: fixtureRules(),
    tags: [
      { name: "basilisk", kind: { kind: "Provenance" }, entry: undefined, ruleCount: BASILISK_RULE_COUNT, diagnosticCount: 15 },
      { name: "pep", kind: { kind: "Provenance" }, entry: undefined, ruleCount: PEP_RULE_COUNT, diagnosticCount: 780 },
    ],
    source: { uri: "file:///workspace/project/pyproject.toml", exists: true, readOnly: false },
    pathOverrides: [{
      path: "legacy",
      configUri: "file:///workspace/project/legacy/pyproject.toml",
      rules: [{ code: "BSK-0001", severity: { kind: "Warning" } }],
      tags: [],
    }],
    debt: {
      remainingDiagnostics: 795,
      errorDiagnostics: 780,
      warningDiagnostics: 15,
      infoDiagnostics: 0,
      adoptedRules: 0,
      disabledRules: 1,
    },
    problems: [],
    typeshed,
  };
}

function isTypeshedMutation(mutation: EditorMutation): boolean {
  return mutation.kind === "SetTypeshedSetting" || mutation.kind === "RemoveTypeshedSetting";
}

/**
 * The real host/server pair, reduced to its observable contract. Typeshed
 * mutations are written and re-projected at once; rule mutations open the
 * impact dialog and land only on apply.
 */
export class ScenarioHost {
  public readonly intents: Record<string, unknown>[] = [];
  private readonly config: HostConfig;
  private acquiring: boolean;
  private revision = 0;
  private pending: EditorMutation[] = [];
  private ruleEntries = new Map<string, string>();
  /** Folders the picker returns, in order; `undefined` means the user cancelled. */
  private readonly folders: (string | undefined)[];
  private readonly focusRule: string | null;

  constructor(options: {
    config?: HostConfig;
    acquiring?: boolean;
    folders?: (string | undefined)[];
    focusRule?: string | null;
  } = {}) {
    this.config = { ...options.config };
    this.acquiring = options.acquiring === true;
    this.folders = [...(options.folders ?? [])];
    this.focusRule = options.focusRule ?? null;
  }

  /** Settle an in-flight acquisition, as the server's status notification does. */
  public settle(): Record<string, unknown> {
    this.acquiring = false;
    return this.readyState();
  }

  public snapshot(): ConfigurationSnapshot {
    this.revision += 1;
    const snapshot = fixtureSnapshot(
      typeshedFor(this.config, this.acquiring),
      `fnv1a64:${this.revision}`,
    );
    return {
      ...snapshot,
      rules: snapshot.rules.map((rule) => {
        const entry = this.ruleEntries.get(rule.descriptor.code);
        return entry === undefined ? rule : { ...rule, entry: { kind: entry } as never };
      }),
    };
  }

  /** Answer one intent exactly as the production host would. */
  public receive(message: Record<string, unknown>): Record<string, unknown> | undefined {
    this.intents.push(message);
    switch (message.type) {
      case "ready": return this.readyState();
      case "preview": return this.preview(message.mutations as EditorMutation[]);
      case "apply": return this.applyPending();
      case "cancelPreview": return this.readyState("Change discarded; configuration is unchanged");
      case "typeshedAction": return this.typeshedAction(String(message.action));
      case "pickTypeshedFolder": return this.pickFolder(String(message.key));
      case "occurrences": return this.occurrences(message);
      default: return undefined;
    }
  }

  private readyState(message = "Configuration is up to date"): Record<string, unknown> {
    return {
      phase: "ready",
      rootUri: "file:///workspace/project",
      snapshot: this.snapshot(),
      preview: undefined,
      occurrences: undefined,
      occurrencesLoading: false,
      repairUri: undefined,
      message,
      refreshRequested: false,
      focusRule: this.focusRule,
    };
  }

  private preview(mutations: EditorMutation[]): Record<string, unknown> {
    if (mutations.every(isTypeshedMutation)) {
      mutations.forEach((mutation) => { this.write(mutation); });
      return this.readyState("Applied");
    }
    this.pending = mutations;
    return {
      ...this.readyState("Preview ready"),
      phase: "preview",
      preview: {
        previewId: "preview-1",
        baseRevision: `fnv1a64:${this.revision}`,
        changes: mutations.flatMap((mutation) => mutation.kind === "SetRule"
          ? [{ code: mutation.code, before: { kind: "Error" }, after: mutation.severity }]
          : []),
        typeshedChanges: [],
        impact: {
          errorsBefore: 780, errorsAfter: 779,
          warningsBefore: 15, warningsAfter: 16,
          infosBefore: 0, infosAfter: 0,
        },
      },
    };
  }

  private applyPending(): Record<string, unknown> {
    this.pending.forEach((mutation) => {
      if (mutation.kind === "SetRule") { this.ruleEntries.set(mutation.code, mutation.severity.kind); }
      if (mutation.kind === "RemoveRule") { this.ruleEntries.delete(mutation.code); }
      this.write(mutation);
    });
    this.pending = [];
    return this.readyState("Applied");
  }

  /** The writer's closed allowlist, in the same shape the TOML holds. */
  private write(mutation: EditorMutation): void {
    if (mutation.kind !== "SetTypeshedSetting" && mutation.kind !== "RemoveTypeshedSetting") { return; }
    const value = mutation.kind === "SetTypeshedSetting" ? mutation.value : undefined;
    const text = value?.kind === "Text" ? value.value : undefined;
    const flag = value?.kind === "Boolean" ? value.value : undefined;
    const fields: Record<string, keyof HostConfig> = {
      TypeshedCommit: "commit",
      TypeshedPath: "path",
      TypeshedUrl: "archiveUrl",
      TypeshedCachePath: "cacheFolder",
      TypeshedCache: "reuseDownloads",
      TypeshedVerify: "verifyContent",
    };
    const field = fields[mutation.key.kind];
    if (field === undefined) { return; }
    Object.assign(this.config, { [field]: text ?? flag });
  }

  private typeshedAction(action: string): Record<string, unknown> | undefined {
    if (action !== "PinCurrent") { return undefined; }
    // The server pins the ACTIVE commit and clears any custom folder.
    this.config.commit = ACTIVE_COMMIT;
    this.config.path = undefined;
    return this.readyState("Applied");
  }

  private pickFolder(key: string): Record<string, unknown> {
    const folder = this.folders.shift();
    // A cancelled picker writes nothing — the host re-pushes the state so the
    // controls snap back to the configuration that still holds.
    if (folder === undefined) { return this.readyState(); }
    if (key === "TypeshedPath") {
      this.config.path = folder;
      this.config.commit = undefined;
    } else {
      this.config.cacheFolder = folder;
    }
    return this.readyState("Applied");
  }

  private occurrences(message: Record<string, unknown>): Record<string, unknown> {
    const selector = message.selector as { codes?: string[] } | undefined;
    return {
      ...this.readyState(),
      occurrences: {
        items: [{
          code: selector?.codes?.[0] ?? "",
          uri: "file:///workspace/project/app.py",
          range: { start: { line: 1, character: 0 }, end: { line: 1, character: 4 } },
          severity: { kind: "Error" },
        }],
        nextCursor: undefined,
      },
    };
  }
}

/** Page-side bridge: forward every runtime intent to the extension host. */
function bridgeScript(): string {
  return `
    const __realApi = acquireVsCodeApi();
    window.__realApi = __realApi;
    __realApi.postMessage({ type: 'domTestBoot' });
    window.addEventListener('error', (event) => {
      __realApi.postMessage({ type: 'domTestResult', ok: false, reason: 'page error: ' + event.message });
    });
    window.acquireVsCodeApi = () => ({
      postMessage(message) { __realApi.postMessage({ type: 'domTestIntent', intent: message }); },
      getState() { return undefined; },
      setState() {},
    });
  `;
}

/** Inject the bridge before and the driver after the real runtime, same nonce. */
export function harnessDocument(driver: string): string {
  const html = buildConfigurationEditorDocument();
  const openTag = /<script nonce="[^"]+">/.exec(html);
  assert.ok(openTag, "the configuration editor document must carry one nonce-gated script");
  return html
    .replace(openTag[0], `${openTag[0]}${bridgeScript()}\n;`)
    .replace("</script>\n</body>", `;\n${driver}</script>\n</body>`);
}

/**
 * Run one driver against one host. The panel is created frontmost: a hidden
 * webview throttles timers and pauses requestAnimationFrame, which would
 * starve both the driver and the virtualized rule window.
 */
export async function runScenario(driver: string, host: ScenarioHost): Promise<ScenarioOutcome> {
  await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  const panel = vscode.window.createWebviewPanel(
    "basilisk.configurationEditorDomTest",
    "Configuration Editor DOM Test",
    vscode.ViewColumn.One,
    { enableScripts: true, retainContextWhenHidden: true, localResourceRoots: [] },
  );
  try {
    const result = await new Promise<DomTestResult>((resolve, reject) => {
      let booted = false;
      const timer = setTimeout(() => {
        reject(new Error(
          "the webview driver never reported a result "
          + `(boot beacon ${booted ? "received" : "missing"}; panel visible=${panel.visible})`,
        ));
      }, RESULT_TIMEOUT_MS);
      panel.webview.onDidReceiveMessage((message: Record<string, unknown>) => {
        if (message.type === "domTestBoot") { booted = true; return; }
        if (message.type === "domTestResult") {
          clearTimeout(timer);
          resolve(message as DomTestResult);
          return;
        }
        if (message.type === "domTestSettle") {
          void panel.webview.postMessage({ type: "state", state: host.settle() });
          return;
        }
        if (message.type !== "domTestIntent") { return; }
        const state = host.receive(message.intent as Record<string, unknown>);
        if (state !== undefined) { void panel.webview.postMessage({ type: "state", state }); }
      });
      panel.webview.html = harnessDocument(driver);
    });
    return { result, intents: host.intents };
  } finally {
    panel.dispose();
  }
}

/** Shared driver preamble: waiting, reporting, and reading the DOM back. */
export const DRIVER_PRELUDE = String.raw`
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const settleDelay = 120;
  const report = (result) => window.__realApi.postMessage(Object.assign({ type: 'domTestResult' }, result));
  const el = (selector) => document.querySelector(selector);
  const all = (selector) => Array.from(document.querySelectorAll(selector));
  const text = (node) => (node && node.textContent ? node.textContent.trim() : null);
  const waitFor = async (selector, tries) => {
    for (let attempt = 0; attempt < (tries || 100); attempt += 1) {
      if (el(selector)) return true;
      await sleep(25);
    }
    return false;
  };
  // Wait for an OBSERVABLE consequence instead of guessing how long the
  // extension host will take. Every webview interaction costs two IPC round
  // trips through the host's single event loop plus a full re-render, and that
  // loop is shared with every other suite in the run — a language client
  // pumping messages, the test-explorer poller, live panel effects. A fixed
  // sleep encodes "the host is idle", which is true only when this file runs
  // alone; in full-suite order the reply lands late and the driver samples a
  // DOM that has not reacted yet. Returns false on timeout so the caller's
  // assertion still fails loudly rather than the whole scenario hanging.
  const waitUntil = async (predicate, tries) => {
    for (let attempt = 0; attempt < (tries || 400); attempt += 1) {
      try { if (predicate()) return true; } catch (ignored) { /* not rendered yet */ }
      await sleep(25);
    }
    return false;
  };
  const dialog = () => document.getElementById('preview-dialog');
  // Read every observable fact about the Typeshed panel at this instant.
  const probe = () => {
    const commit = el('[data-typeshed-commit]');
    const commitError = document.getElementById('typeshed-commit-error');
    const path = el('[data-typeshed-path="TypeshedPath"]');
    const reuse = el('[data-typeshed-boolean="TypeshedCache"]');
    const verify = el('[data-typeshed-boolean="TypeshedVerify"]');
    const url = el('[data-typeshed-text="TypeshedUrl"]');
    const cacheFolder = el('[data-typeshed-path="TypeshedCachePath"]');
    const status = {};
    const rows = all('#typeshed-status dt');
    rows.forEach((dt, index) => { status[text(dt)] = text(all('#typeshed-status dd')[index]); });
    return {
      sources: all('[data-typeshed-source]').map((radio) => ({
        mode: radio.dataset.typeshedSource,
        checked: radio.checked,
        disabled: radio.disabled,
        hint: text(radio.parentElement.querySelector('small')),
      })),
      commitPresent: commit !== null,
      commitValue: commit ? commit.value : null,
      commitDisabled: commit ? commit.disabled : null,
      commitInvalid: commit ? commit.getAttribute('aria-invalid') : null,
      commitError: commitError && !commitError.hidden ? text(commitError) : null,
      pathPresent: path !== null,
      pathValue: path ? path.value : null,
      reusePresent: reuse !== null,
      reuseChecked: reuse ? reuse.checked : null,
      reuseDisabled: reuse ? reuse.disabled : null,
      verifyPresent: verify !== null,
      verifyChecked: verify ? verify.checked : null,
      advancedPresent: el('.typeshed-advanced') !== null,
      advancedOpen: el('.typeshed-advanced') ? el('.typeshed-advanced').open : null,
      urlValue: url ? url.value : null,
      cacheFolderValue: cacheFolder ? cacheFolder.value : null,
      actions: all('[data-typeshed-action]').map((button) => ({
        action: button.dataset.typeshedAction,
        label: text(button),
        disabled: button.disabled,
      })),
      status,
      warnings: all('.typeshed-warning').map(text),
      dialogOpen: document.getElementById('preview-dialog').open,
      dialogChanges: text(document.getElementById('preview-changes')),
    };
  };
  const steps = [];
  const record = (label) => { steps.push(Object.assign({ label }, probe())); return steps[steps.length - 1]; };
  const click = async (node) => { node.click(); await sleep(settleDelay); };
  const change = async (node, value) => {
    if (typeof value === 'boolean') { node.checked = value; } else { node.value = value; }
    node.dispatchEvent(new Event('change', { bubbles: true }));
    await sleep(settleDelay);
  };
  // A real click: a disabled radio does nothing, exactly as for a user.
  const chooseSource = async (mode) => {
    el('[data-typeshed-source="' + mode + '"]').click();
    await sleep(settleDelay);
  };
`;
