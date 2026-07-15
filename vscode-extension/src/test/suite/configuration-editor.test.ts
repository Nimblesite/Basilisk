// Implements [VSIX-CONFIGURATION-EDITOR] / [CONFIGEDITOR-ACCESSIBILITY-SECURITY].
/** Contract, thin-shell, security, accessibility, and lifecycle tests. */

import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";
import type {
  ApplyConfigurationRequest,
  ConfigurationPreview,
  ConfigurationSnapshot,
  EditorMutation,
  PreviewConfigurationRequest,
  RuleOccurrencesRequest,
  RuleOccurrencesResponse,
} from "../../configuration-editor-model";
import {
  ConfigurationEditorController,
  configurationRepairUri,
  supportsConfigurationEditor,
  type ConfigurationEditorTransport,
} from "../../configuration-editor";
import {
  decodeConfigurationChanged,
  IDLE_CONFIGURATION_EDITOR,
} from "../../configuration-editor-state";
import { readBasiliskSettings } from "../../lsp-client";
import { createStore } from "../../store";

const ROOT_URI = "file:///workspace";
const PEP_CODE = "BSK-0001";
const ANALYZE_CODE = "BSK-0060";

/**
 * [CONFIGEDITOR-MODEL]: one pep rule (check scope, never disabled) and one
 * analyze rule, plus one tag with an explicit `rule-tags` entry.
 */
function configurationSnapshot(revision = "revision-1"): ConfigurationSnapshot {
  return {
    rootUri: ROOT_URI,
    configUri: `${ROOT_URI}/pyproject.toml`,
    revision,
    rules: [{
      descriptor: {
        code: PEP_CODE,
        title: "Incompatible assignment",
        summary: "Assignments must satisfy the declared type.",
        docsUrl: `https://example.test/errors/${PEP_CODE}`,
        tags: ["pep", "assignability"],
      },
      entry: undefined,
      effectiveSeverity: { kind: "Error" },
      diagnosticCount: 3,
    }, {
      descriptor: {
        code: ANALYZE_CODE,
        title: "Active code-specific directive",
        summary: "Audit inline suppressions.",
        docsUrl: `https://example.test/errors/${ANALYZE_CODE}`,
        tags: ["basilisk", "suppressions"],
      },
      entry: { kind: "Warning" },
      effectiveSeverity: { kind: "Warning" },
      diagnosticCount: 1,
    }],
    tags: [{
      name: "basilisk",
      kind: { kind: "Provenance" },
      entry: { kind: "Error" },
      ruleCount: 1,
      diagnosticCount: 1,
    }, {
      name: "pep",
      kind: { kind: "Provenance" },
      entry: undefined,
      ruleCount: 1,
      diagnosticCount: 3,
    }],
  };
}

function configurationPreview(baseRevision = "revision-1"): ConfigurationPreview {
  return {
    previewId: "preview-1",
    baseRevision,
    changes: [{
      code: PEP_CODE,
      before: { kind: "Error" },
      after: { kind: "Warning" },
    }],
    impact: {
      errorsBefore: 3,
      errorsAfter: 0,
      warningsBefore: 1,
      warningsAfter: 4,
      infosBefore: 0,
      infosAfter: 0,
    },
  };
}

class RecordingTransport implements ConfigurationEditorTransport {
  public snapshotResult = configurationSnapshot();
  public previewResult = configurationPreview();
  public applyResult = configurationSnapshot("revision-2");
  public occurrenceResult: RuleOccurrencesResponse = { items: [], nextCursor: undefined };
  public readonly snapshotRequests: string[] = [];
  public readonly previewRequests: PreviewConfigurationRequest[] = [];
  public readonly applyRequests: ApplyConfigurationRequest[] = [];
  public readonly occurrenceRequests: RuleOccurrencesRequest[] = [];
  public previewError: Error | undefined;
  public snapshotError: Error | undefined;
  public snapshotHandler: ((rootUri: string) => Promise<ConfigurationSnapshot>) | undefined;
  public previewHandler: ((request: PreviewConfigurationRequest) => Promise<ConfigurationPreview>) | undefined;
  public applyHandler: (() => Promise<ConfigurationSnapshot>) | undefined;
  public occurrenceHandler: ((request: RuleOccurrencesRequest) => Promise<RuleOccurrencesResponse>) | undefined;

  public async snapshot(rootUri: string): Promise<ConfigurationSnapshot> {
    this.snapshotRequests.push(rootUri);
    if (this.snapshotError !== undefined) { throw this.snapshotError; }
    if (this.snapshotHandler !== undefined) { return this.snapshotHandler(rootUri); }
    return this.snapshotResult;
  }

  public async preview(request: PreviewConfigurationRequest): Promise<ConfigurationPreview> {
    this.previewRequests.push(request);
    if (this.previewError !== undefined) { throw this.previewError; }
    if (this.previewHandler !== undefined) { return this.previewHandler(request); }
    return this.previewResult;
  }

  public async apply(request: ApplyConfigurationRequest): Promise<ConfigurationSnapshot> {
    this.applyRequests.push(request);
    if (this.applyHandler !== undefined) { return this.applyHandler(); }
    return this.applyResult;
  }

  public async occurrences(request: RuleOccurrencesRequest): Promise<RuleOccurrencesResponse> {
    this.occurrenceRequests.push(request);
    if (this.occurrenceHandler !== undefined) { return this.occurrenceHandler(request); }
    return this.occurrenceResult;
  }
}

class RevisionConflictError extends Error {
  public readonly data = { kind: "revisionConflict" } as const;
}

class InvalidConfigurationError extends Error {
  public readonly data: unknown;

  constructor(sourceUri: string) {
    super("The rules table is malformed");
    this.data = { kind: "invalidConfiguration", context: { sourceUri } };
  }
}

function occurrence(line: number): RuleOccurrencesResponse["items"][number] {
  return {
    code: PEP_CODE,
    uri: `${ROOT_URI}/source.py`,
    range: { start: { line, character: 0 }, end: { line, character: 1 } },
    severity: { kind: "Error" },
  };
}

async function pollUntil(predicate: () => boolean, timeoutMs = 5_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (!predicate() && Date.now() < deadline) {
    await new Promise<void>((resolve) => setTimeout(resolve, 25));
  }
  assert.ok(predicate(), "condition did not become true before timeout");
}

suite("Configuration editor — generated contract and central state", () => {
  // [CONFIGEDITOR-MODEL]: snapshot carries rule entries + effective severity;
  // preview is the resolved changes plus the errors/warnings/infos partition.
  test("stores snapshots and exact previews without inventing configuration state", () => {
    const store = createStore();
    store.beginConfigurationLoad(ROOT_URI);
    assert.strictEqual(store.configurationEditor.value.phase, "loading");
    store.acceptConfigurationSnapshot(configurationSnapshot());
    assert.strictEqual(store.configurationEditor.value.snapshot?.rules[0]?.descriptor.code, PEP_CODE);
    assert.strictEqual(store.configurationEditor.value.snapshot?.rules[1]?.entry?.kind, "Warning");
    assert.strictEqual(store.configurationEditor.value.snapshot?.tags[0]?.entry?.kind, "Error");

    store.beginConfigurationPreview();
    store.acceptConfigurationPreview(configurationPreview());
    assert.strictEqual(store.configurationEditor.value.phase, "preview");
    assert.deepStrictEqual(
      store.configurationEditor.value.preview?.changes.map((change) => change.code),
      [PEP_CODE],
    );

    store.markConfigurationChanged({ rootUri: "file:///other", revision: "r2" });
    assert.strictEqual(store.configurationEditor.value.refreshRequested, false);
    store.markConfigurationChanged({ rootUri: ROOT_URI, revision: "revision-2" });
    assert.strictEqual(store.configurationEditor.value.refreshRequested, true);
    store.resetConfigurationEditor();
    assert.deepStrictEqual(store.configurationEditor.value, IDLE_CONFIGURATION_EDITOR);
  });

  // [LSPARCH-CONFIG-EDITOR-PROTOCOL]: configurationChanged is rootUri +
  // revision — nothing else (no reason field survives the redesign).
  test("validates server invalidations before shared state consumes them", () => {
    assert.deepStrictEqual(
      decodeConfigurationChanged({ rootUri: ROOT_URI, revision: "r2" }),
      { rootUri: ROOT_URI, revision: "r2" },
    );
    assert.strictEqual(decodeConfigurationChanged({ rootUri: ROOT_URI, revision: 2 }), undefined);
    assert.strictEqual(decodeConfigurationChanged(null), undefined);
  });
});

suite("Configuration editor — typed mutation routing", () => {
  // [CONFIGEDITOR-OPERATIONS] / [CHKARCH-CONFIG-MODEL]: the editor can request
  // exactly four things — set/remove one rule entry or one tag entry. Each is
  // relayed verbatim through preview, and apply sends only root + preview id.
  test("relays each of the four EditorMutation kinds verbatim through preview", async () => {
    const mutations: EditorMutation[] = [
      { kind: "SetRule", code: PEP_CODE, severity: { kind: "Warning" } },
      { kind: "RemoveRule", code: ANALYZE_CODE },
      { kind: "SetTag", tag: "basilisk", severity: { kind: "Info" } },
      { kind: "RemoveTag", tag: "basilisk" },
    ];
    for (const mutation of mutations) {
      const store = createStore();
      const transport = new RecordingTransport();
      const controller = new ConfigurationEditorController(store, transport);
      try {
        controller.open(ROOT_URI);
        await pollUntil(() => store.configurationEditor.value.phase === "ready");
        await controller.receive({ type: "preview", mutations: [mutation] });
        assert.deepStrictEqual(transport.previewRequests, [{
          rootUri: ROOT_URI,
          baseRevision: "revision-1",
          mutations: [mutation],
        }], `${mutation.kind} must be relayed without translation`);
        assert.strictEqual(store.configurationEditor.value.phase, "preview");
      } finally {
        controller.dispose();
      }
    }
  });

  // [CONFIGEDITOR-OPERATIONS]: rootUri + previewId fully identify the cached
  // preview; the preview pins its own base revision, so apply carries none.
  test("applies a preview with only rootUri and previewId", async () => {
    const store = createStore();
    const transport = new RecordingTransport();
    const controller = new ConfigurationEditorController(store, transport);
    try {
      controller.open(ROOT_URI);
      await pollUntil(() => store.configurationEditor.value.phase === "ready");
      await controller.receive({
        type: "preview",
        mutations: [{ kind: "SetRule", code: PEP_CODE, severity: { kind: "Warning" } }],
      });
      await controller.receive({ type: "apply" });
      assert.deepStrictEqual(transport.applyRequests, [{
        rootUri: ROOT_URI,
        previewId: "preview-1",
      }]);
      assert.strictEqual(store.configurationEditor.value.snapshot?.revision, "revision-2");
    } finally {
      controller.dispose();
    }
  });

  // [VSIX-CONFIGURATION-EDITOR-THIN-SHELL]: legacy selector-based mutations,
  // Inherit/Native settings, and scopes are no longer decodable intent.
  test("rejects legacy selector/setting/scope mutation payloads outright", async () => {
    const store = createStore();
    const transport = new RecordingTransport();
    const controller = new ConfigurationEditorController(store, transport);
    try {
      controller.open(ROOT_URI);
      await pollUntil(() => store.configurationEditor.value.phase === "ready");
      await controller.receive({
        type: "preview",
        mutations: [{ selector: { kind: "All" }, setting: { kind: "Native" }, scope: { kind: "Project" } }],
      });
      await controller.receive({
        type: "preview",
        mutations: [{ kind: "SetRule", code: PEP_CODE, severity: { kind: "Inherit" } }],
      });
      await controller.receive({ type: "preview", mutations: [] });
      await controller.receive({ type: "fixSafe" });
      assert.strictEqual(transport.previewRequests.length, 0);
    } finally {
      controller.dispose();
    }
  });
});

suite("Configuration editor — thin LSP shell", () => {
  // [CONFIGEDITOR-OPERATIONS]: cursor-paged occurrences over the read-side
  // all/codes/tags selectors; navigation is allowlisted to loaded items.
  test("routes paged occurrence reads and ignores invalid or untrusted navigation", async () => {
    const store = createStore();
    const transport = new RecordingTransport();
    const controller = new ConfigurationEditorController(store, transport);
    try {
      controller.open(ROOT_URI);
      await pollUntil(() => store.configurationEditor.value.phase === "ready");
      transport.occurrenceResult = {
        items: Array.from({ length: 100 }, (_unused, line) => occurrence(line)),
        nextCursor: "100",
      };
      await controller.receive({
        type: "occurrences", selector: { kind: "Codes", codes: [PEP_CODE] }, cursor: undefined, limit: 100,
      });
      transport.occurrenceResult = { items: [occurrence(100)], nextCursor: undefined };
      await controller.receive({
        type: "occurrences", selector: { kind: "Codes", codes: [PEP_CODE] }, cursor: "100", limit: 100,
      });
      assert.strictEqual(store.configurationEditor.value.occurrences?.items.length, 101);
      assert.strictEqual(store.configurationEditor.value.occurrences?.nextCursor, undefined);
      assert.deepStrictEqual(transport.occurrenceRequests.map((request) => request.cursor), [undefined, "100"]);
      await controller.receive({ type: "preview", mutations: "not-an-array" });
      await controller.receive({ type: "openDocs", uri: "https://attacker.invalid" });
      await controller.receive({ type: "openOccurrence", uri: "file:///etc/passwd", line: 0, character: 0 });
      assert.strictEqual(transport.previewRequests.length, 0);
    } finally {
      controller.dispose();
    }
  });

  test("drops stale occurrence responses and resets loading for a new selector", async () => {
    const store = createStore();
    const transport = new RecordingTransport();
    const pending: ((response: RuleOccurrencesResponse) => void)[] = [];
    transport.occurrenceHandler = async () => new Promise((resolve) => pending.push(resolve));
    const controller = new ConfigurationEditorController(store, transport);
    try {
      controller.open(ROOT_URI);
      await pollUntil(() => store.configurationEditor.value.phase === "ready");
      const stale = controller.receive({
        type: "occurrences", selector: { kind: "Codes", codes: [PEP_CODE] }, cursor: undefined, limit: 100,
      });
      const newest = controller.receive({
        type: "occurrences", selector: { kind: "Tags", tags: ["pep"], matchAll: false }, cursor: undefined, limit: 100,
      });
      await pollUntil(() => pending.length === 2);
      pending[1]?.({ items: [occurrence(9)], nextCursor: undefined });
      await newest;
      pending[0]?.({ items: [occurrence(1)], nextCursor: "100" });
      await stale;
      assert.deepStrictEqual(store.configurationEditor.value.occurrences?.items, [occurrence(9)]);
      assert.strictEqual(store.configurationEditor.value.occurrencesLoading, false);
    } finally {
      controller.dispose();
    }
  });
});

suite("Configuration editor — transaction lifecycle", () => {
  test("keeps the newest preview and submits an applying preview only once", async () => {
    const store = createStore();
    const transport = new RecordingTransport();
    const pending: ((preview: ConfigurationPreview) => void)[] = [];
    transport.previewHandler = async () => new Promise<ConfigurationPreview>((resolve) => pending.push(resolve));
    const controller = new ConfigurationEditorController(store, transport);
    try {
      controller.open(ROOT_URI);
      await pollUntil(() => store.configurationEditor.value.phase === "ready");
      const first = controller.receive({
        type: "preview",
        mutations: [{ kind: "SetTag", tag: "basilisk", severity: { kind: "Warning" } }],
      });
      const second = controller.receive({
        type: "preview",
        mutations: [{ kind: "SetTag", tag: "basilisk", severity: { kind: "Error" } }],
      });
      await pollUntil(() => pending.length === 2);
      pending[1]?.({ ...configurationPreview(), previewId: "newest" });
      await second;
      pending[0]?.({ ...configurationPreview(), previewId: "stale" });
      await first;
      assert.strictEqual(store.configurationEditor.value.preview?.previewId, "newest");

      let finishApply: ((snapshot: ConfigurationSnapshot) => void) | undefined;
      transport.applyHandler = async () => new Promise<ConfigurationSnapshot>((resolve) => { finishApply = resolve; });
      const apply = controller.receive({ type: "apply" });
      await pollUntil(() => transport.applyRequests.length === 1);
      await controller.receive({ type: "apply" });
      assert.strictEqual(transport.applyRequests.length, 1, "an applying preview cannot be submitted twice");
      finishApply?.(configurationSnapshot("revision-2"));
      await apply;
      assert.strictEqual(store.configurationEditor.value.snapshot?.revision, "revision-2");
    } finally {
      controller.dispose();
    }
  });

  test("refreshes the active root after a validated server invalidation", async () => {
    const store = createStore();
    const transport = new RecordingTransport();
    const controller = new ConfigurationEditorController(store, transport);
    try {
      controller.open(ROOT_URI);
      await pollUntil(() => transport.snapshotRequests.length === 1);
      transport.snapshotResult = configurationSnapshot("revision-2");
      store.markConfigurationChanged({ rootUri: ROOT_URI, revision: "revision-2" });
      await pollUntil(() => transport.snapshotRequests.length === 2);
      await pollUntil(() => store.configurationEditor.value.snapshot?.revision === "revision-2");
    } finally {
      controller.dispose();
    }
  });

  test("replays an invalidation that arrives while the same root is loading", async () => {
    const store = createStore();
    const transport = new RecordingTransport();
    const pending: ((snapshot: ConfigurationSnapshot) => void)[] = [];
    const controller = new ConfigurationEditorController(store, transport);
    try {
      controller.open(ROOT_URI);
      await pollUntil(() => store.configurationEditor.value.phase === "ready");
      transport.snapshotHandler = async () => new Promise((resolve) => pending.push(resolve));

      const refresh = controller.receive({ type: "refresh" });
      await pollUntil(() => transport.snapshotRequests.length === 2);
      store.markConfigurationChanged({ rootUri: ROOT_URI, revision: "revision-3" });
      pending[0]?.(configurationSnapshot("revision-2"));
      await refresh;

      await pollUntil(() => transport.snapshotRequests.length === 3);
      pending[1]?.(configurationSnapshot("revision-3"));
      await pollUntil(() => store.configurationEditor.value.snapshot?.revision === "revision-3");
    } finally {
      controller.dispose();
    }
  });
});

interface ScratchConfigWorkspace {
  readonly rootUri: string;
  readonly configUri: vscode.Uri;
  readonly configPath: string;
  readonly appliedToml: string;
  snapshot(revision: string): ConfigurationSnapshot;
  dispose(): void;
}

/** Real on-disk pyproject.toml scratch root, isolated from the fixture workspace. */
function createScratchConfigWorkspace(): ScratchConfigWorkspace {
  const scratchRoot = fs.mkdtempSync(path.join(os.tmpdir(), "bsk-config-apply-"));
  const configPath = path.join(scratchRoot, "pyproject.toml");
  fs.writeFileSync(configPath, '[project]\nname = "demo"\n');
  const configUri = vscode.Uri.file(configPath);
  const rootUri = vscode.Uri.file(scratchRoot).toString();
  return {
    rootUri,
    configUri,
    configPath,
    appliedToml: '[project]\nname = "demo"\n\n[tool.basilisk.rules]\n"BSK-0001" = "warning"\n',
    snapshot: (revision: string): ConfigurationSnapshot => ({
      ...configurationSnapshot(revision),
      rootUri,
      configUri: configUri.toString(),
    }),
    dispose: (): void => { fs.rmSync(scratchRoot, { recursive: true, force: true }); },
  };
}

/** The exact client-side effect vscode-languageclient produces for the server's workspace/applyEdit. */
async function applyWholeDocumentEdit(target: vscode.Uri, newText: string): Promise<void> {
  const document = await vscode.workspace.openTextDocument(target);
  const replacement = new vscode.WorkspaceEdit();
  replacement.replace(
    target,
    new vscode.Range(new vscode.Position(0, 0), document.positionAt(document.getText().length)),
    newText,
  );
  assert.ok(await vscode.workspace.applyEdit(replacement), "harness workspace edit must apply");
}

suite("Configuration editor — apply persistence", () => {
  // Implements [CONFIGEDITOR-SOURCES]: the server keeps its closed-source
  // overlay only "until the client write is visible on disk", so a successful
  // apply must actually reach disk. vscode.workspace.applyEdit (what
  // vscode-languageclient runs for the server's workspace/applyEdit) only
  // edits the in-memory buffer — the applied change must not die there.
  test("apply persists the configuration edit to disk instead of leaving a dirty buffer", async () => {
    const scratch = createScratchConfigWorkspace();
    const store = createStore();
    const transport = new RecordingTransport();
    transport.snapshotResult = scratch.snapshot("revision-1");
    transport.applyHandler = async () => {
      await applyWholeDocumentEdit(scratch.configUri, scratch.appliedToml);
      return scratch.snapshot("revision-2");
    };
    const controller = new ConfigurationEditorController(store, transport);
    try {
      controller.open(scratch.rootUri);
      await pollUntil(() => store.configurationEditor.value.phase === "ready");
      await controller.receive({
        type: "preview",
        mutations: [{ kind: "SetRule", code: PEP_CODE, severity: { kind: "Warning" } }],
      });
      await pollUntil(() => store.configurationEditor.value.phase === "preview");
      await controller.receive({ type: "apply" });
      await pollUntil(() => store.configurationEditor.value.snapshot?.revision === "revision-2");

      const document = await vscode.workspace.openTextDocument(scratch.configUri);
      assert.strictEqual(document.isDirty, false, "apply must not strand pyproject.toml as a dirty buffer");
      assert.strictEqual(
        fs.readFileSync(scratch.configPath, "utf8"),
        scratch.appliedToml,
        "the applied configuration must be visible on disk",
      );
    } finally {
      controller.dispose();
      scratch.dispose();
    }
  });
});

suite("Configuration editor — conflicts, capability, and lifecycle", () => {
  test("carries only an allowlisted root config URI into invalid-config recovery", async () => {
    assert.strictEqual(
      configurationRepairUri(`${ROOT_URI}/pyproject.toml`, ROOT_URI),
      `${ROOT_URI}/pyproject.toml`,
    );
    assert.strictEqual(configurationRepairUri("file:///etc/passwd", ROOT_URI), undefined);
    assert.strictEqual(configurationRepairUri(`${ROOT_URI}/nested/pyproject.toml`, ROOT_URI), undefined);
    assert.strictEqual(configurationRepairUri(`${ROOT_URI}/basilisk.json`, ROOT_URI), undefined);
    assert.strictEqual(configurationRepairUri("https://attacker.invalid/basilisk.json", ROOT_URI), undefined);

    const store = createStore();
    const transport = new RecordingTransport();
    transport.snapshotError = new InvalidConfigurationError(`${ROOT_URI}/pyproject.toml`);
    const controller = new ConfigurationEditorController(store, transport);
    try {
      controller.open(ROOT_URI);
      await pollUntil(() => store.configurationEditor.value.phase === "error");
      assert.strictEqual(store.configurationEditor.value.repairUri, `${ROOT_URI}/pyproject.toml`);
      assert.strictEqual(store.configurationEditor.value.snapshot, undefined);
    } finally {
      controller.dispose();
    }
  });

  test("uses structured JSON-RPC data to identify a revision conflict", async () => {
    const store = createStore();
    const transport = new RecordingTransport();
    transport.previewError = new RevisionConflictError("The write was rejected");
    const controller = new ConfigurationEditorController(store, transport);
    try {
      controller.open(ROOT_URI);
      await pollUntil(() => store.configurationEditor.value.phase === "ready");
      await controller.receive({
        type: "preview",
        mutations: [{ kind: "RemoveTag", tag: "basilisk" }],
      });
      assert.strictEqual(store.configurationEditor.value.phase, "conflict");
      assert.strictEqual(store.configurationEditor.value.message, "The write was rejected");
    } finally {
      controller.dispose();
    }
  });

  // [LSPARCH-CONFIG-EDITOR-PROTOCOL] / [VSIX-CONFIGURATION-EDITOR]: the
  // capability is pure presence — the editor ships with the server, so there
  // is no protocol version to negotiate.
  test("gates on presence of the experimental capability", () => {
    function clientWithCapability(configurationEditor: unknown): LanguageClient {
      return {
        initializeResult: {
          capabilities: { experimental: { basilisk: { configurationEditor } } },
        },
      } as unknown as LanguageClient;
    }
    assert.strictEqual(supportsConfigurationEditor(clientWithCapability(true)), true);
    assert.strictEqual(supportsConfigurationEditor(clientWithCapability({})), true);
    assert.strictEqual(supportsConfigurationEditor(clientWithCapability(false)), false);
    assert.strictEqual(supportsConfigurationEditor(clientWithCapability(undefined)), false);
    assert.strictEqual(supportsConfigurationEditor(clientWithCapability(null)), false);
    assert.strictEqual(supportsConfigurationEditor(undefined), false);
  });

  test("capability loss clears stale configuration and invalidates occurrence loading", async () => {
    const store = createStore();
    const controller = new ConfigurationEditorController(store, new RecordingTransport());
    try {
      controller.open(ROOT_URI);
      await pollUntil(() => store.configurationEditor.value.phase === "ready");
      store.beginRuleOccurrences(false);
      controller.capabilityLost("Capability lost");
      assert.strictEqual(store.configurationEditor.value.phase, "unsupported");
      assert.strictEqual(store.configurationEditor.value.rootUri, ROOT_URI);
      assert.strictEqual(store.configurationEditor.value.snapshot, undefined);
      assert.strictEqual(store.configurationEditor.value.occurrencesLoading, false);
    } finally {
      controller.dispose();
    }
  });

  test("binds one webview message handler across singleton re-renders", async function () {
    this.timeout(15_000);
    const store = createStore();
    const transport = new RecordingTransport();
    const controller = new ConfigurationEditorController(store, transport);
    try {
      controller.open(ROOT_URI);
      await pollUntil(() => controller.readyMessageCount() >= 1, 10_000);
      controller.open(ROOT_URI);
      await pollUntil(() => controller.readyMessageCount() >= 2, 10_000);
      await new Promise<void>((resolve) => setTimeout(resolve, 500));
      assert.strictEqual(controller.readyMessageCount(), 2, "a stacked handler would deliver the second ready twice");
      assert.strictEqual(controller.isOpen(), true);
      await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      await pollUntil(() => !controller.isOpen());
      assert.deepStrictEqual(
        store.configurationEditor.value,
        IDLE_CONFIGURATION_EDITOR,
        "closing the tab must not retain a hidden configuration snapshot",
      );
    } finally {
      controller.dispose();
    }
    assert.strictEqual(controller.isOpen(), false);
  });
});

suite("Configuration editor — diagnostic scope setting relay", () => {
  // [LSPARCH-DIAGNOSTIC-SCOPE]: `basilisk.analyze` is a per-user editor
  // setting relayed as initializationOptions.basilisk.analyze — it restricts
  // publication to check scope and never touches project configuration.
  test("basilisk.analyze defaults to true and is relayed under initializationOptions.basilisk", async () => {
    const cfg = vscode.workspace.getConfiguration("basilisk");
    const inspected = cfg.inspect<boolean>("analyze");
    assert.strictEqual(inspected?.defaultValue, true, "package.json must declare the default");

    const relayedDefault = readBasiliskSettings();
    const nested = relayedDefault.basilisk as Record<string, unknown>;
    assert.strictEqual(nested.analyze, true);

    try {
      await cfg.update("analyze", false, vscode.ConfigurationTarget.Workspace);
      const relayed = readBasiliskSettings();
      const nestedOff = relayed.basilisk as Record<string, unknown>;
      assert.strictEqual(nestedOff.analyze, false, "the opt-out must reach the LSP payload");
    } finally {
      await cfg.update("analyze", undefined, vscode.ConfigurationTarget.Workspace);
    }
  });
});
