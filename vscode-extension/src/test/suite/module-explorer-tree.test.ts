// Tests for [EXTACT-MODULES-TREE-STRUCTURE]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-MODULES-TREE-STRUCTURE
//
// Coarse component tests for the Module Explorer's nested folder/package tree
// (#149) and the flat-view sort toggle (#151). Per CLAUDE.md we drive the real
// provider: a stubbed WorkspaceModulesResponse is fed through a fake LSP client
// and getChildren() output is asserted. Crucially the LSP returns a FLAT list of
// dotted module names — the provider must rebuild the hierarchy client-side, so
// these tests guard that reconstruction and that flat view never dumps bare
// symbols at the tree root.

import * as assert from "assert";
import type * as vscode from "vscode";
import { type LanguageClient } from "vscode-languageclient/node";
import {
  ModuleExplorerProvider,
  ModuleTreeItem,
  PackageTreeItem,
} from "../../module-explorer";
import { createStore, type Store } from "../../store";

// ── Fixtures ────────────────────────────────────────────────────────────────

interface TestSymbol {
  readonly name: string;
  readonly kind: string;
  readonly line: number;
  readonly annotated: boolean;
  readonly exported: boolean;
  readonly children?: readonly TestSymbol[];
}

interface TestModule {
  readonly name: string;
  readonly path: string;
  readonly kind: "package" | "module";
  readonly symbols: readonly TestSymbol[];
  readonly coveragePercent: number;
  readonly errors: number;
  readonly warnings: number;
  readonly adopted: boolean;
}

function sym(name: string): TestSymbol {
  return { name, kind: "function", line: 0, annotated: true, exported: false };
}

function mod(
  name: string,
  kind: "package" | "module",
  opts: { coverage: number; symbols?: readonly TestSymbol[] },
): TestModule {
  return {
    name,
    kind,
    symbols: opts.symbols ?? [],
    coveragePercent: opts.coverage,
    path: `/ws/${name.split(".").join("/")}.py`,
    errors: 0,
    warnings: 0,
    adopted: false,
  };
}

/**
 * A representative flat module list — exactly the shape the LSP returns. Note
 * `app.models` has NO entry of its own: `models/` is a plain folder (no
 * `__init__.py`), so the provider must synthesise it as a container node.
 */
const MODULES: readonly TestModule[] = [
  mod("app", "package", { coverage: 90, symbols: [sym("app_init")] }),
  mod("app.api", "package", { coverage: 80 }),
  mod("app.api.auth", "module", { coverage: 50, symbols: [sym("login"), sym("logout")] }),
  mod("app.models.user", "module", { coverage: 30, symbols: [sym("User")] }),
  mod("util", "module", { coverage: 100, symbols: [sym("helper")] }),
];

const WORKSPACE = {
  totalSymbols: 6,
  annotatedSymbols: 6,
  coveragePercent: 100,
  errors: 0,
  warnings: 0,
  adoptedFiles: 0,
  totalFiles: 5,
};

/** Minimal ExtensionContext for toggleViewMode (only workspaceState is touched). */
const FAKE_CONTEXT = {
  workspaceState: { update: (): Thenable<void> => Promise.resolve() },
} as unknown as vscode.ExtensionContext;

// ── Stubs ─────────────────────────────────────────────────────────────────

/** Build a Store whose LSP client returns the given flat module list. */
function storeWith(modules: readonly TestModule[]): Store {
  const store = createStore();
  const client = {
    isRunning: (): boolean => true,
    onDidChangeState: (): vscode.Disposable => ({ dispose: (): undefined => undefined }),
    sendRequest: async (): Promise<unknown> => ({ modules, workspace: WORKSPACE }),
  } as unknown as LanguageClient;
  store.setClient({ subscriptions: [] } as unknown as vscode.ExtensionContext, client);
  return store;
}

function labelOf(item: vscode.TreeItem): string {
  const { label } = item;
  return typeof label === "string" ? label : label?.label ?? "";
}

function labelsOf(items: readonly vscode.TreeItem[]): string[] {
  return items.map(labelOf);
}

// ── Tests ─────────────────────────────────────────────────────────────────

// eslint-disable-next-line max-lines-per-function
suite("Module Explorer tree structure [EXTACT-MODULES-TREE-STRUCTURE]", () => {

  test("tree view renders a nested folder/package tree, never a flat dotted list (#149)", async () => {
    const provider = new ModuleExplorerProvider(storeWith(MODULES));
    try {
      const roots = await provider.getChildren();
      assert.deepStrictEqual(
        labelsOf(roots),
        ["app", "util"],
        "root shows top-level packages/folders by segment (containers first), not dotted names",
      );
      assert.ok(
        !labelsOf(roots).some((label) => label.includes(".")),
        "no fully-qualified dotted module names at the root",
      );
      assert.ok(
        roots.find((row) => labelOf(row) === "app") instanceof PackageTreeItem,
        "'app' is a package container row",
      );
      assert.ok(
        roots.find((row) => labelOf(row) === "util") instanceof ModuleTreeItem,
        "'util' is a leaf module row",
      );
    } finally {
      provider.dispose();
    }
  });

  test("packages nest child packages/modules; modules nest symbols (#149)", async () => {
    const provider = new ModuleExplorerProvider(storeWith(MODULES));
    try {
      const roots = await provider.getChildren();
      const appNode = roots.find((row) => labelOf(row) === "app");
      assert.ok(appNode, "'app' node should exist");

      const appChildren = await provider.getChildren(appNode);
      assert.deepStrictEqual(
        labelsOf(appChildren),
        ["api", "models", "app_init"],
        "package expands to child packages/folders first, then its own symbols",
      );

      const api = appChildren.find((row) => labelOf(row) === "api");
      assert.ok(api instanceof PackageTreeItem, "'api' is a package container");
      assert.deepStrictEqual(
        labelsOf(await provider.getChildren(api)),
        ["auth"],
        "'api' nests the 'auth' module",
      );

      const auth = (await provider.getChildren(api)).find((row) => labelOf(row) === "auth");
      assert.ok(auth instanceof ModuleTreeItem, "'auth' is a leaf module");
      assert.deepStrictEqual(
        labelsOf(await provider.getChildren(auth)),
        ["login", "logout"],
        "module expands to its symbols",
      );

      const models = appChildren.find((row) => labelOf(row) === "models");
      assert.ok(
        models instanceof PackageTreeItem,
        "'models' is a synthesised folder node (no __init__.py of its own)",
      );
      assert.deepStrictEqual(
        labelsOf(await provider.getChildren(models)),
        ["user"],
        "synthesised folder nests its module",
      );
    } finally {
      provider.dispose();
    }
  });

  test("flat view lists modules (full names) with symbols grouped under them, never at the root (#149)", async () => {
    const provider = new ModuleExplorerProvider(storeWith(MODULES));
    try {
      await provider.getChildren(); // prime the cache (tree mode)
      provider.toggleViewMode(FAKE_CONTEXT); // tree -> flat

      const roots = await provider.getChildren();
      for (const row of roots) {
        assert.ok(
          row instanceof ModuleTreeItem,
          `flat root rows must be modules, not bare symbols — got "${labelOf(row)}"`,
        );
      }
      assert.ok(
        labelsOf(roots).includes("app.api.auth"),
        "flat rows are labelled by full dotted module name",
      );
      assert.ok(
        !labelsOf(roots).includes("login") && !labelsOf(roots).includes("logout"),
        "symbols must never be dumped at the flat-view root (#149 §2)",
      );

      const auth = roots.find((row) => labelOf(row) === "app.api.auth");
      assert.ok(auth, "'app.api.auth' module present in flat view");
      assert.deepStrictEqual(
        labelsOf(await provider.getChildren(auth)),
        ["login", "logout"],
        "symbols remain reachable as children of their owning module",
      );
    } finally {
      provider.dispose();
    }
  });

  test("flat-view sort toggle visibly reorders the module list — never a no-op (#151)", async () => {
    const provider = new ModuleExplorerProvider(storeWith(MODULES));
    try {
      await provider.getChildren();
      provider.toggleViewMode(FAKE_CONTEXT); // -> flat

      const worst = labelsOf(await provider.getChildren());
      assert.deepStrictEqual(
        worst,
        ["app.models.user", "app.api.auth", "app.api", "app", "util"],
        "worst-first orders by ascending coverage (30, 50, 80, 90, 100)",
      );

      provider.cycleSortMode(); // worst -> best
      const best = labelsOf(await provider.getChildren());
      assert.deepStrictEqual(
        best,
        ["util", "app", "app.api", "app.api.auth", "app.models.user"],
        "best-first orders by descending coverage",
      );
      assert.notDeepStrictEqual(best, worst, "toggling sort must change the rendered order");

      provider.cycleSortMode(); // best -> alpha
      assert.deepStrictEqual(
        labelsOf(await provider.getChildren()),
        ["app", "app.api", "app.api.auth", "app.models.user", "util"],
        "alphabetical orders by module name",
      );
    } finally {
      provider.dispose();
    }
  });
});
