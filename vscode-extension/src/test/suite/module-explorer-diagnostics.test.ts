// Tests for [EXTACT-MODULES-DIAGNOSTICS]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-MODULES-DIAGNOSTICS
//
// Regression guard for GitHub #235: the module rows advertise `🔴 n 🟠 n`
// tallies, so expanding a module MUST list the actual diagnostics as the first
// children — above its symbols — each one a navigable row (message label,
// `code · Ln n` description, open-at-range click action). Before the fix the
// provider rendered symbol rows only, making every tally a dead number, and a
// symbol-less module with errors could not even be expanded.
//
// Same harness as module-explorer-tree.test.ts: a stubbed
// WorkspaceModulesResponse is fed through a fake LSP client and the REAL
// provider's getChildren() output is asserted.

import * as assert from "assert";
import * as vscode from "vscode";
import { type LanguageClient } from "vscode-languageclient/node";
import {
  ModuleExplorerProvider,
  ModuleTreeItem,
  PackageTreeItem,
} from "../../module-explorer";
import { createStore, type Store } from "../../store";
import { rawField } from "../../unknown-shape";

// ── Fixtures ────────────────────────────────────────────────────────────────

interface TestDiagnostic {
  readonly severity: "error" | "warning";
  readonly code: string;
  readonly message: string;
  readonly line: number;
  readonly character: number;
}

interface TestSymbol {
  readonly name: string;
  readonly kind: string;
  readonly line: number;
  readonly annotated: boolean;
  readonly exported: boolean;
}

interface TestModule {
  readonly name: string;
  readonly path: string;
  readonly kind: "package" | "module";
  readonly symbols: readonly TestSymbol[];
  readonly diagnostics: readonly TestDiagnostic[];
  readonly coveragePercent: number;
  readonly errors: number;
  readonly warnings: number;
  readonly adopted: boolean;
}

function sym(name: string): TestSymbol {
  return { name, kind: "function", line: 0, annotated: true, exported: false };
}

function diag(
  severity: "error" | "warning",
  message: string,
  opts: { code?: string; line?: number; character?: number } = {},
): TestDiagnostic {
  return {
    severity,
    message,
    code: opts.code ?? "returns_compatibility",
    line: opts.line ?? 0,
    character: opts.character ?? 0,
  };
}

function mod(
  name: string,
  kind: "package" | "module",
  opts: { symbols?: readonly TestSymbol[]; diagnostics?: readonly TestDiagnostic[] },
): TestModule {
  const diagnostics = opts.diagnostics ?? [];
  return {
    name,
    kind,
    symbols: opts.symbols ?? [],
    diagnostics,
    coveragePercent: 80,
    path: `/ws/${name.split(".").join("/")}.py`,
    errors: diagnostics.filter((d) => d.severity === "error").length,
    warnings: diagnostics.filter((d) => d.severity === "warning").length,
    adopted: false,
  };
}

const WORKSPACE = {
  typeCheckingEnabled: true,
  totalSymbols: 2,
  annotatedSymbols: 2,
  coveragePercent: 100,
  errors: 2,
  warnings: 1,
  adoptedFiles: 0,
  totalFiles: 2,
  scanComplete: true,
};

/** Build a Store whose LSP client returns the given flat module list. */
function storeWith(modules: readonly TestModule[]): Store {
  const store = createStore();
  // A stand-in for the members the code under test calls. No runtime check
  // can produce the rest of `LanguageClient`, so the test double itself is
  // the one assertion here — it is not a payload being read.
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- see above.
  const client = {
    isRunning: (): boolean => true,
    onDidChangeState: (): vscode.Disposable => ({ dispose: (): undefined => undefined }),
    sendRequest: async (): Promise<unknown> => ({ modules, workspace: WORKSPACE }),
  } as unknown as LanguageClient;
  store.setClient({ subscriptions: [] }, client);
  return store;
}

function labelOf(item: vscode.TreeItem): string {
  const { label } = item;
  return typeof label === "string" ? label : label?.label ?? "";
}

/** The provider's root rows for the given stubbed modules (tree view). */
async function rootItems(modules: readonly TestModule[]): Promise<{
  provider: ModuleExplorerProvider;
  roots: vscode.TreeItem[];
}> {
  const provider = new ModuleExplorerProvider(storeWith(modules));
  const roots = await provider.getChildren();
  return { provider, roots };
}

// ── Tests ─────────────────────────────────────────────────────────────────

suite("Module Explorer diagnostics drill-down [EXTACT-MODULES-DIAGNOSTICS] (#235)", () => {

  test("expanding a module lists its diagnostics first — above its symbols — not just a dead tally", async () => {
    const modules = [
      mod("util", "module", {
        symbols: [sym("helper")],
        // Deliberately out of order (warning first, later-line error before
        // earlier-line error): the client must render errors before warnings,
        // then ascending line, even if a server ever sends them unsorted.
        diagnostics: [
          diag("warning", "unused import", { line: 1 }),
          diag("error", "later error", { line: 9 }),
          diag("error", "earlier error", { line: 4 }),
        ],
      }),
    ];
    const { provider, roots } = await rootItems(modules);
    const moduleRow = roots.find((row) => row instanceof ModuleTreeItem);
    assert.ok(moduleRow, "the util module row should render");

    const children = await provider.getChildren(moduleRow);
    assert.strictEqual(
      children.length,
      4,
      `a module with 3 diagnostics and 1 symbol must expand to 4 rows (diagnostics + symbols), got: [${children.map(labelOf).join(", ")}]`,
    );
    assert.deepStrictEqual(
      children.map(labelOf),
      ["earlier error", "later error", "unused import", "helper"],
      "diagnostics come FIRST (errors before warnings, then ascending line), then the symbols",
    );
  });

  test("each diagnostic row is navigable: code · 1-based line description and an open-at-range click action", async () => {
    const modules = [
      mod("util", "module", {
        symbols: [],
        diagnostics: [diag("error", "bad assignment", { code: "assignment_type", line: 41, character: 7 })],
      }),
    ];
    const { provider, roots } = await rootItems(modules);
    const moduleRow = roots.find((row) => row instanceof ModuleTreeItem);
    assert.ok(moduleRow, "the util module row should render");

    const children = await provider.getChildren(moduleRow);
    assert.strictEqual(children.length, 1, "the diagnostic must render as a child row");
    const row = children[0];

    assert.strictEqual(labelOf(row), "bad assignment", "row label is the diagnostic message");
    assert.strictEqual(
      row.description,
      "assignment_type · Ln 42",
      "row description is `code · Ln n` with the 1-based line",
    );
    assert.strictEqual(row.command?.command, "vscode.open", "clicking opens the file");
    const args: readonly unknown[] = row.command?.arguments ?? [];
    const target = args[0];
    assert.ok(target instanceof vscode.Uri, "the open command targets a Uri");
    assert.strictEqual(target.fsPath, "/ws/util.py");
    const selection = rawField(args[1], "selection");
    assert.ok(
      selection instanceof vscode.Range,
      "the open command must carry the diagnostic's range as the selection",
    );
    assert.strictEqual(selection.start.line, 41, "selection anchors to the zero-based line");
    assert.strictEqual(selection.start.character, 7, "selection anchors to the zero-based character");
  });

  test("a symbol-less module with diagnostics is expandable (its errors must be reachable)", async () => {
    const modules = [
      mod("empty_but_broken", "module", {
        symbols: [],
        diagnostics: [diag("error", "syntax-adjacent error")],
      }),
    ];
    const { roots } = await rootItems(modules);
    const moduleRow = roots.find((row) => row instanceof ModuleTreeItem);
    assert.ok(moduleRow, "the module row should render");
    assert.notStrictEqual(
      moduleRow.collapsibleState,
      vscode.TreeItemCollapsibleState.None,
      "a module whose only children are diagnostics must still be expandable (#235)",
    );
  });

  test("package rows surface their own diagnostics above their symbols too", async () => {
    const modules = [
      mod("app", "package", {
        symbols: [sym("app_init")],
        diagnostics: [diag("error", "package-level error", { line: 2 })],
      }),
      mod("app.api", "module", { symbols: [sym("route")], diagnostics: [] }),
    ];
    const { provider, roots } = await rootItems(modules);
    const packageRow = roots.find((row) => row instanceof PackageTreeItem);
    assert.ok(packageRow, "the app package row should render");

    const children = await provider.getChildren(packageRow);
    const labels = children.map(labelOf);
    const diagnosticIndex = labels.indexOf("package-level error");
    const symbolIndex = labels.indexOf("app_init");
    assert.ok(
      diagnosticIndex !== -1,
      `the package's own diagnostic must render as a child row, got: [${labels.join(", ")}]`,
    );
    assert.ok(symbolIndex !== -1, "the package's own symbols still render");
    assert.ok(
      diagnosticIndex < symbolIndex,
      `diagnostics render above the package's symbols, got: [${labels.join(", ")}]`,
    );
  });

  test("a clean module drills straight to its symbols (no empty diagnostic section)", async () => {
    const modules = [
      mod("clean", "module", { symbols: [sym("fine")], diagnostics: [] }),
    ];
    const { provider, roots } = await rootItems(modules);
    const moduleRow = roots.find((row) => row instanceof ModuleTreeItem);
    assert.ok(moduleRow, "the clean module row should render");

    const children = await provider.getChildren(moduleRow);
    assert.deepStrictEqual(
      children.map(labelOf),
      ["fine"],
      "a diagnostics-free module shows exactly its symbols",
    );
  });
});
