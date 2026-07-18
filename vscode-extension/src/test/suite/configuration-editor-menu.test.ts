// Implements [VSIX-CONFIGURATION-EDITOR]: file-explorer context-menu entry point.
/**
 * Right-clicking pyproject.toml in the Explorer must offer an "Edit Config"
 * item at the very top of the context menu, opening the configuration editor.
 * Manifest-level contract, same convention as activity-panel.test.ts.
 */

import * as assert from "assert";
import {
  getPackageJsonCommands,
  getPackageJsonMenu,
} from "./profiler-test-constants";

suite("Configuration editor — pyproject.toml explorer context menu", () => {
  test("pyproject.toml context menu has an Edit Config item at the top", function () {
    const commands = getPackageJsonCommands();
    const editConfig = commands.find((entry) => entry.title === "Edit Config");
    assert.ok(
      editConfig,
      'package.json must declare a command titled "Edit Config" for the explorer context menu',
    );

    const explorerMenu = getPackageJsonMenu("explorer/context");
    const menuEntry = explorerMenu.find((entry) => entry.command === editConfig.command);
    assert.ok(
      menuEntry,
      `"${editConfig.command}" must be contributed to the explorer/context menu; got: ${
        explorerMenu.map((entry) => entry.command).join(", ") || "(no explorer/context menu)"
      }`,
    );

    // Scoped to pyproject.toml only — never on unrelated files.
    assert.ok(
      menuEntry.when?.includes("resourceFilename == pyproject.toml"),
      `Edit Config must target pyproject.toml via resourceFilename; got when: "${menuEntry.when}"`,
    );

    // Gated on editor support so it never renders a dead item when the
    // server lacks the configuration editor (same rule as the view-title gears).
    assert.ok(
      menuEntry.when?.includes("basilisk.configurationEditorSupported"),
      `Edit Config must be gated on basilisk.configurationEditorSupported; got when: "${menuEntry.when}"`,
    );

    // "Right at the top": the navigation group always renders first in
    // explorer/context, ahead of every numbered group.
    assert.match(
      menuEntry.group ?? "",
      /^navigation(@\d+)?$/,
      `Edit Config must sit in the top-most (navigation) group; got group: "${menuEntry.group}"`,
    );
  });
});
