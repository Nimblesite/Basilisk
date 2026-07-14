// Implements [VSIX-CONFIGURATION-EDITOR] / [CONFIGEDITOR-ACCESSIBILITY-SECURITY].
/** Contract, thin-shell, security, accessibility, and lifecycle tests. */

import * as assert from "assert";
import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";
import type {
  ConfigurationPreview,
  ConfigurationSnapshot,
  PreviewConfigurationRequest,
  RuleOccurrencesRequest,
  RuleOccurrencesResponse,
} from "../../configuration-editor-model";
import {
  ConfigurationEditorController,
  configurationRepairUri,
  configurationEditorCapabilityVersion,
  supportsConfigurationEditor,
  type ConfigurationEditorTransport,
} from "../../configuration-editor";
import {
  decodeConfigurationChanged,
  IDLE_CONFIGURATION_EDITOR,
} from "../../configuration-editor-state";
import { createStore } from "../../store";

const ROOT_URI = "file:///workspace";

function configurationSnapshot(revision = "revision-1"): ConfigurationSnapshot {
  return {
    rootUri: ROOT_URI,
    revision,
    source: {
      uri: `${ROOT_URI}/pyproject.toml`,
      format: { kind: "PyprojectToml" },
      exists: true,
      readOnly: false,
      shadowedSources: [],
    },
    rules: [{
      descriptor: {
        code: "B001",
        title: "No implicit Any",
        summary: "Keep boundaries explicit.",
        docsUrl: "https://example.test/rules/B001",
        tags: ["strictness"],
        defaultSeverity: { kind: "Error" },
        defaultEnabled: true,
      },
      configuredSeverity: undefined,
      effectiveSeverity: { kind: "Error" },
      inherited: true,
      diagnosticCount: 3,
      affectedFileCount: 2,
      safeFixCount: 1,
      unsafeFixCount: 0,
      adoptionExceptionCount: 1,
    }],
    tags: [{
      name: "strictness",
      kind: { kind: "Descriptive" },
      ruleCount: 1,
      diagnosticCount: 3,
    }],
    presets: [{
      id: "strict",
      name: "Strict",
      summary: "Enable the complete live catalog at each rule's native severity.",
      mutations: [{
        selector: { kind: "All" },
        setting: { kind: "Native" },
        scope: { kind: "Project" },
      }],
    }],
    pathOverrides: [{
      pattern: "legacy/**",
      adoption: true,
      rules: [{ ruleCode: "B001", severity: { kind: "Warning" } }],
    }],
    debt: {
      remainingDiagnostics: 3,
      adoptedFiles: 1,
      adoptionExceptions: 1,
      suppressionDiagnostics: 0,
      disabledRules: 0,
    },
    problems: [],
  };
}

function configurationPreview(baseRevision = "revision-1"): ConfigurationPreview {
  return {
    previewId: "preview-1",
    baseRevision,
    expandedRuleCodes: ["B001"],
    changes: [{
      ruleCode: "B001",
      scope: { kind: "Project" },
      previousSetting: { kind: "Inherit" },
      resultingSetting: { kind: "Warning" },
    }],
    impact: {
      changedRules: 1,
      enabledRules: 1,
      disabledRules: 0,
      diagnosticsBefore: 3,
      diagnosticsAfter: 1,
      errorsBefore: 3,
      errorsAfter: 1,
      warningsBefore: 0,
      warningsAfter: 0,
    },
    problems: [],
  };
}

class RecordingTransport implements ConfigurationEditorTransport {
  public snapshotResult = configurationSnapshot();
  public previewResult = configurationPreview();
  public applyResult = configurationSnapshot("revision-2");
  public occurrenceResult: RuleOccurrencesResponse = { items: [], nextCursor: undefined };
  public readonly snapshotRequests: string[] = [];
  public readonly previewRequests: PreviewConfigurationRequest[] = [];
  public readonly applyRequests: { rootUri: string; previewId: string; baseRevision: string }[] = [];
  public readonly occurrenceRequests: RuleOccurrencesRequest[] = [];
  public readonly safeFixRequests: string[] = [];
  public safeFixResult = { fixed: 2, files: 1 };
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

  public async apply(request: { rootUri: string; previewId: string; baseRevision: string }): Promise<ConfigurationSnapshot> {
    this.applyRequests.push(request);
    if (this.applyHandler !== undefined) { return this.applyHandler(); }
    return this.applyResult;
  }

  public async occurrences(request: RuleOccurrencesRequest): Promise<RuleOccurrencesResponse> {
    this.occurrenceRequests.push(request);
    if (this.occurrenceHandler !== undefined) { return this.occurrenceHandler(request); }
    return this.occurrenceResult;
  }

  public async fixSafe(rootUri: string): Promise<{ fixed: number; files: number }> {
    this.safeFixRequests.push(rootUri);
    return this.safeFixResult;
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
    ruleCode: "B001",
    uri: `${ROOT_URI}/source.py`,
    range: { start: { line, character: 0 }, end: { line, character: 1 } },
    effectiveSeverity: { kind: "Error" },
    fixSafety: undefined,
    configurationSource: `${ROOT_URI}/pyproject.toml`,
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
  test("stores snapshots and exact previews without inventing policy state", () => {
    const store = createStore();
    store.beginConfigurationLoad(ROOT_URI);
    assert.strictEqual(store.configurationEditor.value.phase, "loading");
    store.acceptConfigurationSnapshot(configurationSnapshot());
    assert.strictEqual(store.configurationEditor.value.snapshot?.rules[0]?.descriptor.code, "B001");

    store.beginConfigurationPreview();
    store.acceptConfigurationPreview(configurationPreview());
    assert.strictEqual(store.configurationEditor.value.phase, "preview");
    assert.deepStrictEqual(store.configurationEditor.value.preview?.expandedRuleCodes, ["B001"]);

    store.markConfigurationChanged({ rootUri: "file:///other", revision: "r2", reason: "other" });
    assert.strictEqual(store.configurationEditor.value.refreshRequested, false);
    store.markConfigurationChanged({ rootUri: ROOT_URI, revision: "revision-2", reason: "Changed on disk" });
    assert.strictEqual(store.configurationEditor.value.refreshRequested, true);
    assert.strictEqual(store.configurationEditor.value.message, "Changed on disk");
    store.resetConfigurationEditor();
    assert.deepStrictEqual(store.configurationEditor.value, IDLE_CONFIGURATION_EDITOR);
  });

  test("validates server invalidations before shared state consumes them", () => {
    assert.deepStrictEqual(
      decodeConfigurationChanged({ rootUri: ROOT_URI, revision: "r2", reason: "Updated" }),
      { rootUri: ROOT_URI, revision: "r2", reason: "Updated" },
    );
    assert.strictEqual(decodeConfigurationChanged({ rootUri: ROOT_URI, revision: 2, reason: "bad" }), undefined);
    assert.strictEqual(decodeConfigurationChanged(null), undefined);
  });
});

suite("Configuration editor — thin LSP shell", () => {
  test("relays exact preset mutations through preview then applies only the preview id", async () => {
    const store = createStore();
    const transport = new RecordingTransport();
    const controller = new ConfigurationEditorController(store, transport);
    try {
      controller.open(ROOT_URI);
      await pollUntil(() => store.configurationEditor.value.phase === "ready");
      assert.deepStrictEqual(transport.snapshotRequests, [ROOT_URI]);

      const strictMutation = configurationSnapshot().presets[0]?.mutations[0];
      assert.ok(strictMutation);
      await controller.receive({ type: "preview", mutations: [strictMutation] });
      assert.deepStrictEqual(transport.previewRequests, [{
        rootUri: ROOT_URI,
        baseRevision: "revision-1",
        mutations: [strictMutation],
      }]);
      assert.strictEqual(store.configurationEditor.value.phase, "preview");

      await controller.receive({ type: "apply" });
      assert.deepStrictEqual(transport.applyRequests, [{
        rootUri: ROOT_URI,
        previewId: "preview-1",
        baseRevision: "revision-1",
      }]);
      assert.strictEqual(store.configurationEditor.value.snapshot?.revision, "revision-2");
    } finally {
      controller.dispose();
    }
  });

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
        type: "occurrences", selector: { kind: "Codes", codes: ["B001"] }, cursor: undefined, limit: 100,
      });
      transport.occurrenceResult = { items: [occurrence(100)], nextCursor: undefined };
      await controller.receive({
        type: "occurrences", selector: { kind: "Codes", codes: ["B001"] }, cursor: "100", limit: 100,
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
        type: "occurrences", selector: { kind: "Codes", codes: ["B001"] }, cursor: undefined, limit: 100,
      });
      const newest = controller.receive({
        type: "occurrences", selector: { kind: "WithoutSafeFix" }, cursor: undefined, limit: 100,
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
  test("keeps the newest preview, applies once, and delegates safe fixes to the LSP", async () => {
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
        mutations: [{ selector: { kind: "All" }, setting: { kind: "Warning" }, scope: { kind: "Project" } }],
      });
      const second = controller.receive({
        type: "preview",
        mutations: [{ selector: { kind: "All" }, setting: { kind: "Error" }, scope: { kind: "Project" } }],
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

      await controller.receive({ type: "fixSafe" });
      assert.deepStrictEqual(transport.safeFixRequests, [ROOT_URI]);
      assert.strictEqual(transport.snapshotRequests.length, 2, "safe fixes refresh exact LSP-owned counts");
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
      store.markConfigurationChanged({ rootUri: ROOT_URI, revision: "revision-2", reason: "Changed on disk" });
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
      store.markConfigurationChanged({ rootUri: ROOT_URI, revision: "revision-3", reason: "Changed again" });
      pending[0]?.(configurationSnapshot("revision-2"));
      await refresh;

      await pollUntil(() => transport.snapshotRequests.length === 3);
      pending[1]?.(configurationSnapshot("revision-3"));
      await pollUntil(() => store.configurationEditor.value.snapshot?.revision === "revision-3");
    } finally {
      controller.dispose();
    }
  });

  test("does not return to an old root when a safe-fix request finishes late", async () => {
    const store = createStore();
    const transport = new RecordingTransport();
    let finishFix: ((result: { fixed: number; files: number }) => void) | undefined;
    transport.fixSafe = async (rootUri: string) => {
      transport.safeFixRequests.push(rootUri);
      return new Promise((resolve) => { finishFix = resolve; });
    };
    const controller = new ConfigurationEditorController(store, transport);
    try {
      controller.open(ROOT_URI);
      await pollUntil(() => store.configurationEditor.value.phase === "ready");
      const fix = controller.receive({ type: "fixSafe" });
      await pollUntil(() => transport.safeFixRequests.length === 1);

      const otherRoot = "file:///other-workspace";
      transport.snapshotResult = {
        ...configurationSnapshot("other-revision"),
        rootUri: otherRoot,
        source: {
          ...configurationSnapshot().source,
          uri: `${otherRoot}/pyproject.toml`,
        },
      };
      controller.open(otherRoot);
      await pollUntil(() => store.configurationEditor.value.snapshot?.rootUri === otherRoot);
      finishFix?.({ fixed: 1, files: 1 });
      await fix;

      assert.deepStrictEqual(transport.snapshotRequests, [ROOT_URI, otherRoot]);
      assert.strictEqual(store.configurationEditor.value.snapshot?.rootUri, otherRoot);
    } finally {
      controller.dispose();
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
        mutations: [{ selector: { kind: "All" }, setting: { kind: "Native" }, scope: { kind: "Project" } }],
      });
      assert.strictEqual(store.configurationEditor.value.phase, "conflict");
      assert.strictEqual(store.configurationEditor.value.message, "The write was rejected");
    } finally {
      controller.dispose();
    }
  });

  test("recognizes only the versioned experimental capability", () => {
    const supported = {
      initializeResult: {
        capabilities: { experimental: { basilisk: { configurationEditor: { version: 1 } } } },
      },
    } as unknown as LanguageClient;
    const future = {
      initializeResult: {
        capabilities: { experimental: { basilisk: { configurationEditor: { version: 2 } } } },
      },
    } as unknown as LanguageClient;
    assert.strictEqual(configurationEditorCapabilityVersion(supported), 1);
    assert.strictEqual(supportsConfigurationEditor(supported), true);
    assert.strictEqual(supportsConfigurationEditor(future), false);
    assert.strictEqual(supportsConfigurationEditor(undefined), false);
  });

  test("capability loss clears stale policy and invalidates occurrence loading", async () => {
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
