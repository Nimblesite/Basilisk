// Tests for [EXTACT-MODULES-TREE-STRUCTURE]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-MODULES-TREE-STRUCTURE
//
// Coarse component tests for the Module Explorer's nested folder/package tree
// (#149) and the flat-view sort picker (#151/#189). Per CLAUDE.md we drive the real
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
  readonly totalSymbols?: number;
  readonly annotatedSymbols?: number;
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
  opts: {
    coverage: number;
    symbols?: readonly TestSymbol[];
    totalSymbols?: number;
    annotatedSymbols?: number;
    errors?: number;
    warnings?: number;
    path?: string;
  },
): TestModule {
  return {
    name,
    kind,
    symbols: opts.symbols ?? [],
    coveragePercent: opts.coverage,
    totalSymbols: opts.totalSymbols,
    annotatedSymbols: opts.annotatedSymbols,
    path: opts.path ?? `/ws/${name.split(".").join("/")}.py`,
    errors: opts.errors ?? 0,
    warnings: opts.warnings ?? 0,
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

/** Theme-colour id of a row's icon tint, or undefined when untinted. */
function iconColorId(item: vscode.TreeItem): string | undefined {
  const icon = item.iconPath as vscode.ThemeIcon;
  return (icon.color as { id?: string } | undefined)?.id;
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

  // Tests [EXTACT-MODULES-TOOLBAR] Sort — the explicit, labelled name/path/coverage picker.
  test("flat-view exposes explicit name/path/coverage sort modes with a visible active mode (#151, #189)", async () => {
    const provider = new ModuleExplorerProvider(storeWith(MODULES));
    try {
      await provider.getChildren();
      provider.toggleViewMode(FAKE_CONTEXT); // -> flat

      // Default surfaces the least-typed modules first (ascending coverage).
      assert.strictEqual(provider.getSortMode(), "coverage", "default flat sort is by coverage");
      const byCoverage = labelsOf(await provider.getChildren());
      assert.deepStrictEqual(
        byCoverage,
        ["app.models.user", "app.api.auth", "app.api", "app", "util"],
        "coverage sort orders by ascending coverage (30, 50, 80, 90, 100)",
      );

      provider.setSortMode("name");
      const byName = labelsOf(await provider.getChildren());
      assert.deepStrictEqual(
        byName,
        ["app", "app.api", "app.api.auth", "app.models.user", "util"],
        "name sort orders alphabetically by dotted module name",
      );
      assert.notDeepStrictEqual(byName, byCoverage, "switching sort must change the rendered order");

      provider.setSortMode("path");
      assert.strictEqual(provider.getSortMode(), "path", "explicit selection sticks");

      // The three modes are explicit + labelled, and the active one is marked —
      // never a blind toggle (#189).
      const options = provider.sortOptions();
      assert.deepStrictEqual(
        options.map((option) => option.label),
        ["Module Name", "Path", "Type Coverage"],
        "exactly the three labelled sort modes are offered, in order",
      );
      assert.deepStrictEqual(
        options.filter((option) => option.current).map((option) => option.mode),
        ["path"],
        "exactly the active mode is marked current so the picker can show it",
      );
    } finally {
      provider.dispose();
    }
  });

  // Tests [EXTACT-MODULES-TOOLBAR] Sort (Path mode).
  test("flat-view offers an explicit sort-by-path mode (#189)", async () => {
    // Paths are chosen so file-path order (a/ < b/ < c/) differs from BOTH name
    // order (alpha < beta < gamma) and score order (10 < 50 < 90) — so only a
    // genuine path sort can produce [beta, alpha, gamma].
    const byPath: readonly TestModule[] = [
      mod("beta", "module", { coverage: 10, path: "/ws/a/beta.py" }),
      mod("alpha", "module", { coverage: 90, path: "/ws/b/alpha.py" }),
      mod("gamma", "module", { coverage: 50, path: "/ws/c/gamma.py" }),
    ];
    const provider = new ModuleExplorerProvider(storeWith(byPath));
    try {
      await provider.getChildren();
      provider.toggleViewMode(FAKE_CONTEXT); // -> flat

      // #189 replaces the blind worst/best/alpha cycle with explicit
      // name/path/coverage modes; selecting "path" sorts by file path.
      provider.setSortMode("path");

      assert.deepStrictEqual(
        labelsOf(await provider.getChildren()),
        ["beta", "alpha", "gamma"],
        "path sort orders modules by file path, distinct from name/score order (#189)",
      );
    } finally {
      provider.dispose();
    }
  });

  // Tests [EXTACT-MODULES-TREE-STRUCTURE] coverage rollup: folder/package rows
  // must show the subtree's symbol-weighted type-coverage % — not just error
  // tallies, and not only the package's own __init__.py coverage.
  test("folder/package rows roll up subtree type coverage, symbol-weighted like the workspace header", async () => {
    // Weights are chosen so the honest symbol-weighted rollup for `app`
    // ((2+1+0) annotated / (2+2+6) total = 30%) differs from a naive average of
    // child percentages ((100+50+0)/3 = 50%) — only a weighted rollup passes.
    const modules = [
      mod("app", "package", { coverage: 100, totalSymbols: 2, annotatedSymbols: 2 }),
      mod("app.api.auth", "module", { coverage: 50, totalSymbols: 2, annotatedSymbols: 1 }),
      mod("app.models.user", "module", { coverage: 0, totalSymbols: 6, annotatedSymbols: 0 }),
      mod("util", "module", { coverage: 100, totalSymbols: 1, annotatedSymbols: 1 }),
    ];
    const provider = new ModuleExplorerProvider(storeWith(modules));
    try {
      const roots = await provider.getChildren();

      const app = roots.find((row) => labelOf(row) === "app");
      assert.ok(app instanceof PackageTreeItem, "'app' is a package container");
      const appDesc = String(app.description);
      assert.ok(
        appDesc.includes("30%"),
        `'app' must show the subtree's symbol-weighted coverage (3/10 = 30%), got: ${appDesc}`,
      );
      assert.ok(
        appDesc.includes("█") || appDesc.includes("░"),
        `'app' must render the coverage bar like module rows do, got: ${appDesc}`,
      );

      // A synthesised pure folder (models/ has no __init__.py, so no module of
      // its own) must still show its subtree's coverage — this is the exact
      // "folders show no percentage" bug.
      const appChildren = await provider.getChildren(app);
      const models = appChildren.find((row) => labelOf(row) === "models");
      assert.ok(models instanceof PackageTreeItem, "'models' is a synthesised folder");
      const modelsDesc = String(models.description);
      assert.ok(
        modelsDesc.includes("0%"),
        `pure folder must show its subtree coverage (0/6 = 0%), got: "${modelsDesc}"`,
      );

      const api = appChildren.find((row) => labelOf(row) === "api");
      assert.ok(api instanceof PackageTreeItem, "'api' is a synthesised folder");
      assert.ok(
        String(api.description).includes("50%"),
        `'api' folder must show its subtree coverage (1/2 = 50%), got: "${String(api.description)}"`,
      );
    } finally {
      provider.dispose();
    }
  });

  // Tests [EXTACT-MODULES-TREE-STRUCTURE] + [ANALYSIS-ENABLED] (#119): with type
  // checking disabled the server omits all grading, so folder rows must render
  // NO percentage — never a vacuous 100% conjured from zero data.
  test("folder rows show no coverage percentage while type checking is disabled (#119)", async () => {
    const ungraded = [
      { name: "app", kind: "package", symbols: [], path: "/ws/app/__init__.py" },
      { name: "app.mod", kind: "module", symbols: [], path: "/ws/app/mod.py" },
    ];
    const provider = new ModuleExplorerProvider(storeWith(ungraded as unknown as readonly TestModule[]));
    try {
      const roots = await provider.getChildren();
      const app = roots.find((row) => labelOf(row) === "app");
      assert.ok(app instanceof PackageTreeItem, "'app' is a package container");
      assert.ok(
        !String(app.description ?? "").includes("%"),
        `ungraded folder must show no percentage, got: "${String(app.description)}"`,
      );
    } finally {
      provider.dispose();
    }
  });

  // Tests [EXTACT-MODULES-TREE-STRUCTURE] icon tint: the folder/package icon
  // colour must follow the SUBTREE rollup, never the package's own
  // __init__.py coverage — a green __init__.py over a red subtree reads red.
  test("package icon tint follows the subtree coverage rollup, not the package's own coverage", async () => {
    // `app`'s own module is fully typed (green on its own: 100% ≥ 90), but the
    // subtree rolls up to 2/12 ≈ 17% (< 50) — only the rolled-up tint is red.
    const modules = [
      mod("app", "package", { coverage: 100, totalSymbols: 2, annotatedSymbols: 2 }),
      mod("app.core", "module", { coverage: 0, totalSymbols: 10, annotatedSymbols: 0 }),
    ];
    const provider = new ModuleExplorerProvider(storeWith(modules));
    try {
      const app = (await provider.getChildren()).find((row) => labelOf(row) === "app");
      assert.ok(app instanceof PackageTreeItem, "'app' is a package container");
      assert.strictEqual(
        iconColorId(app),
        "list.errorForeground",
        "tint must come from the subtree rollup (17% → red), not the package's own 100% (green)",
      );
    } finally {
      provider.dispose();
    }
  });

  test("package icon tint bands: subtree errors win, then warnings, then coverage colour, untinted when ungraded", async () => {
    const cases: readonly { readonly modules: readonly TestModule[]; readonly expected: string | undefined; readonly why: string }[] = [
      {
        modules: [
          mod("app", "package", { coverage: 100, totalSymbols: 2, annotatedSymbols: 2 }),
          mod("app.core", "module", { coverage: 100, totalSymbols: 2, annotatedSymbols: 2, errors: 1 }),
        ],
        expected: "list.errorForeground",
        why: "a subtree error tints red even when fully typed",
      },
      {
        modules: [
          mod("app", "package", { coverage: 100, totalSymbols: 2, annotatedSymbols: 2 }),
          mod("app.core", "module", { coverage: 100, totalSymbols: 2, annotatedSymbols: 2, warnings: 3 }),
        ],
        expected: "list.warningForeground",
        why: "a warning-only subtree tints yellow",
      },
      {
        modules: [
          mod("app", "package", { coverage: 100, totalSymbols: 9, annotatedSymbols: 9 }),
          mod("app.core", "module", { coverage: 90, totalSymbols: 1, annotatedSymbols: 1 }),
        ],
        expected: "testing.iconPassed",
        why: "a clean ≥90% subtree tints green",
      },
      {
        modules: [
          mod("app", "package", { coverage: 100, totalSymbols: 1, annotatedSymbols: 1 }),
          mod("app.core", "module", { coverage: 0, totalSymbols: 1, annotatedSymbols: 0 }),
        ],
        expected: "list.warningForeground",
        why: "a clean 50–89% subtree tints yellow",
      },
      {
        modules: [
          { name: "app", kind: "package", symbols: [], path: "/ws/app/__init__.py" },
          { name: "app.core", kind: "module", symbols: [], path: "/ws/app/core.py" },
        ] as unknown as readonly TestModule[],
        expected: undefined,
        why: "an ungraded subtree (Type Checking disabled, #119) stays untinted",
      },
    ];
    for (const { modules, expected, why } of cases) {
      const provider = new ModuleExplorerProvider(storeWith(modules));
      try {
        const app = (await provider.getChildren()).find((row) => labelOf(row) === "app");
        assert.ok(app instanceof PackageTreeItem, `'app' is a package container (${why})`);
        assert.strictEqual(iconColorId(app), expected, why);
      } finally {
        provider.dispose();
      }
    }
  });

  // Tests [EXTACT-MODULES-TREE-STRUCTURE] tooltips: folder tooltips must quote
  // the SUBTREE rollup (labelled as such) and module tooltips the row's stats.
  test("package tooltip quotes the subtree coverage rollup and subtree tallies; module tooltip its own stats", async () => {
    const modules = [
      mod("app", "package", { coverage: 100, totalSymbols: 2, annotatedSymbols: 2 }),
      mod("app.core", "module", {
        coverage: 0, totalSymbols: 10, annotatedSymbols: 0, errors: 1, warnings: 2,
      }),
    ];
    const provider = new ModuleExplorerProvider(storeWith(modules));
    try {
      const roots = await provider.getChildren();
      const app = roots.find((row) => labelOf(row) === "app");
      assert.ok(app instanceof PackageTreeItem, "'app' is a package container");
      const packageTip = String(app.tooltip);
      assert.ok(
        packageTip.includes("Coverage: 17% (subtree)"),
        `package tooltip must quote the rolled-up subtree coverage (2/12 = 17%), not its own 100%, got: ${packageTip}`,
      );
      assert.ok(
        packageTip.includes("Subtree: 1 error, 2 warnings"),
        `package tooltip must tally subtree diagnostics with correct pluralisation, got: ${packageTip}`,
      );

      const core = (await provider.getChildren(app)).find((row) => labelOf(row) === "core");
      assert.ok(core instanceof ModuleTreeItem, "'core' is a leaf module");
      const moduleTip = String(core.tooltip);
      for (const line of ["app.core", "/ws/app/core.py", "Coverage: 0%", "Errors: 1", "Warnings: 2"]) {
        assert.ok(moduleTip.includes(line), `module tooltip must include "${line}", got: ${moduleTip}`);
      }
    } finally {
      provider.dispose();
    }
  });

  // Tests the graded-but-empty branch: a graded subtree with zero symbols is
  // vacuously fully typed — it must render 100%, never NaN or a blank.
  test("a graded folder with zero symbols renders 100%, never NaN", async () => {
    const modules = [
      mod("app", "package", { coverage: 100, totalSymbols: 0, annotatedSymbols: 0 }),
      mod("app.core", "module", { coverage: 100, totalSymbols: 0, annotatedSymbols: 0 }),
    ];
    const provider = new ModuleExplorerProvider(storeWith(modules));
    try {
      const app = (await provider.getChildren()).find((row) => labelOf(row) === "app");
      assert.ok(app instanceof PackageTreeItem, "'app' is a package container");
      const desc = String(app.description);
      assert.ok(desc.includes("100%"), `zero-symbol graded folder shows 100%, got: "${desc}"`);
      assert.ok(!desc.includes("NaN"), `must never render NaN, got: "${desc}"`);
    } finally {
      provider.dispose();
    }
  });

  test("folder/package rows roll up subtree errors/warnings so problems show without drilling in (#149)", async () => {
    const modules = [
      mod("app", "package", { coverage: 90 }),
      mod("app.api.auth", "module", { coverage: 50, errors: 9, warnings: 2 }),
      mod("app.models.user", "module", { coverage: 30, errors: 1 }),
      mod("util", "module", { coverage: 100 }),
    ];
    const provider = new ModuleExplorerProvider(storeWith(modules));
    try {
      const roots = await provider.getChildren();

      const app = roots.find((row) => labelOf(row) === "app");
      assert.ok(app instanceof PackageTreeItem, "'app' is a package container");
      const appDesc = String(app.description);
      assert.ok(appDesc.includes("🔴 10"), `'app' must roll up all descendant errors (9+1), got: ${appDesc}`);
      assert.ok(appDesc.includes("🟠 2"), `'app' must roll up descendant warnings, got: ${appDesc}`);
      assert.strictEqual(app.node.errors, 10, "rolled-up error count on the node");
      assert.strictEqual(app.node.warnings, 2, "rolled-up warning count on the node");

      // A synthesised intermediate folder rolls up too.
      const appChildren = await provider.getChildren(app);
      const api = appChildren.find((row) => labelOf(row) === "api");
      assert.ok(api instanceof PackageTreeItem, "'api' is a synthesised folder");
      assert.ok(
        String(api.description).includes("🔴 9"),
        `'api' folder must surface auth's 9 errors without drilling in, got: ${String(api.description)}`,
      );

      // A clean leaf must NOT show a spurious tally.
      const util = roots.find((row) => labelOf(row) === "util");
      assert.ok(util instanceof ModuleTreeItem, "'util' is a clean leaf module");
      assert.ok(
        !String(util.description).includes("🔴") && !String(util.description).includes("🟠"),
        `clean module must show no error/warning tally, got: ${String(util.description)}`,
      );
    } finally {
      provider.dispose();
    }
  });
});
