// Tests for [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
/**
 * Contract tests for ./extension-manifest.
 *
 * Every other suite reads the manifest through that module, so a reader that
 * quietly returned `[]` would turn each of those suites green while asserting
 * against nothing. These tests hold the readers to the file itself: the manifest
 * is parsed straight off disk and compared with what the readers report, so a
 * narrowing step that drops a contribution fails here rather than hiding
 * everywhere else.
 */

import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import {
  manifestActivationEvents,
  manifestCommands,
  manifestConfigurationProperties,
  manifestContributes,
  manifestDebuggers,
  manifestDisplayName,
  manifestKeybindings,
  manifestMenu,
  manifestMenus,
  manifestViews,
  manifestViewsWelcome,
} from "./extension-manifest";
import { asRecord, rawField, recordField } from "../../unknown-shape";

/** The container that hosts every Basilisk view. */
const EXPLORER_CONTAINER = "basilisk-explorer";

/** The debugger type the extension contributes. */
const DEBUGGER_TYPE = "basilisk-debug";

/** Every command, menu and setting the extension owns is namespaced. */
const NAMESPACE = "basilisk.";

/** `package.json` as parsed straight off disk, bypassing the readers. */
function manifestOnDisk(): Record<string, unknown> {
  const manifestPath = path.resolve(__dirname, "../../../package.json");
  const parsed: unknown = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  return asRecord(parsed);
}

/** `contributes` as parsed straight off disk. */
function contributesOnDisk(): Record<string, unknown> {
  return recordField(manifestOnDisk(), "contributes") ?? {};
}

/** The raw entries of one on-disk contribution array. */
function onDisk(key: string): Record<string, unknown>[] {
  const raw = rawField(contributesOnDisk(), key);
  return Array.isArray(raw) ? raw.map(asRecord) : [];
}

suite("Extension manifest readers [VSIX]", () => {
  test("identity fields match the file", () => {
    const displayName = manifestDisplayName();
    assert.strictEqual(displayName, "Basilisk", "displayName must be Basilisk");
    assert.strictEqual(displayName, manifestOnDisk().displayName, "must match the file");
    assert.ok(displayName.length > 0, "displayName must never fall back to empty");
    assert.strictEqual(displayName.trim(), displayName, "displayName must not be padded");
  });

  test("activation events match the file exactly", () => {
    const events = manifestActivationEvents();
    const declared = onDisk("activationEvents");
    assert.ok(Array.isArray(events), "activationEvents must be an array");
    assert.ok(events.length > 0, "the extension must declare activation events");
    assert.ok(events.includes("onLanguage:python"), "must activate on Python");
    assert.ok(events.includes("onDebug"), "must activate for debugging");
    assert.ok(
      events.includes(`onDebugResolve:${DEBUGGER_TYPE}`),
      "must activate when resolving its own debug type",
    );
    assert.strictEqual(events.length, declared.length, "no activation event may be dropped");
    for (const event of events) {
      assert.strictEqual(typeof event, "string", `activation event ${event} must be a string`);
      assert.ok(event.length > 0, "an activation event must never be empty");
    }
  });

  test("every contributed command is read whole", () => {
    const commands = manifestCommands();
    assert.ok(commands.length > 0, "the extension must contribute commands");
    assert.strictEqual(
      commands.length,
      onDisk("commands").length,
      "no contributed command may be dropped by narrowing",
    );
    for (const command of commands) {
      assert.ok(command.command.length > 0, "a command id must never be empty");
      assert.ok(
        command.command.startsWith(NAMESPACE),
        `command "${command.command}" must be namespaced`,
      );
      assert.ok(command.title.length > 0, `command "${command.command}" must have a title`);
      assert.ok(
        command.category === undefined || command.category.length > 0,
        `command "${command.command}" must not declare an empty category`,
      );
      assert.ok(
        command.icon === undefined || typeof command.icon === "string" || typeof command.icon === "object",
        `command "${command.command}" icon must be a glyph or a light/dark pair`,
      );
    }
  });

  test("command ids are unique", () => {
    const ids = manifestCommands().map((command) => command.command);
    assert.strictEqual(new Set(ids).size, ids.length, `duplicate command ids: ${ids.join(", ")}`);
  });

  test("command titles and ids survive narrowing verbatim", () => {
    const read = new Map(manifestCommands().map((command) => [command.command, command.title]));
    for (const entry of onDisk("commands")) {
      const id = entry.command;
      assert.strictEqual(typeof id, "string", "every on-disk command needs an id");
      assert.ok(read.has(String(id)), `command "${String(id)}" must be reported`);
      assert.strictEqual(read.get(String(id)), entry.title, `title of "${String(id)}" must match`);
    }
  });
});

suite("Extension manifest menus and views [VSIX]", () => {
  test("keybindings bind declared commands", () => {
    const keybindings = manifestKeybindings();
    const ids = new Set(manifestCommands().map((command) => command.command));
    assert.ok(keybindings.length > 0, "the extension must contribute keybindings");
    assert.strictEqual(keybindings.length, onDisk("keybindings").length, "none may be dropped");
    for (const binding of keybindings) {
      assert.ok(binding.command.length > 0, "a keybinding must name a command");
      assert.ok(ids.has(binding.command), `keybinding "${binding.command}" must be a real command`);
      assert.ok(
        binding.key === undefined || binding.key.length > 0,
        `keybinding "${binding.command}" must not declare an empty key`,
      );
    }
  });

  test("every menu section is reported, and each entry names a real command", () => {
    const menus = manifestMenus();
    const sections = Object.keys(menus);
    const ids = new Set(manifestCommands().map((command) => command.command));
    assert.ok(sections.length > 0, "the extension must contribute menus");
    assert.ok(sections.includes("view/title"), "the panel toolbar must be contributed");
    assert.ok(sections.includes("view/item/context"), "row context menus must be contributed");
    for (const section of sections) {
      const entries = menus[section];
      assert.ok(Array.isArray(entries), `menu "${section}" must read as an array`);
      assert.deepStrictEqual(
        entries,
        manifestMenu(section),
        `manifestMenu("${section}") must agree with manifestMenus()`,
      );
      for (const entry of entries) {
        assert.ok(entry.command.length > 0, `an entry in "${section}" must name a command`);
        assert.ok(
          ids.has(entry.command),
          `menu "${section}" references undeclared command "${entry.command}"`,
        );
        assert.ok(
          entry.group === undefined || entry.group.length > 0,
          `entry "${entry.command}" must not declare an empty group`,
        );
      }
    }
  });

  test("an unknown menu id reads as empty, never as a throw", () => {
    assert.deepStrictEqual(manifestMenu("no/such/menu"), [], "unknown menus must read empty");
    assert.deepStrictEqual(manifestMenu(""), [], "an empty menu id must read empty");
  });

  test("views are contributed to the Basilisk container with unique ids", () => {
    const views = manifestViews();
    const explorer = views[EXPLORER_CONTAINER] ?? [];
    assert.ok(EXPLORER_CONTAINER in views, "views must live in the Basilisk container");
    assert.ok(explorer.length > 0, "the container must host at least one view");
    const ids = explorer.map((view) => view.id);
    assert.strictEqual(new Set(ids).size, ids.length, `duplicate view ids: ${ids.join(", ")}`);
    for (const view of explorer) {
      assert.ok(view.id.length > 0, "a view id must never be empty");
      assert.ok(view.id.startsWith(NAMESPACE), `view "${view.id}" must be namespaced`);
      assert.ok(view.name.length > 0, `view "${view.id}" must have a name`);
      assert.ok(
        view.when === undefined || view.when.length > 0,
        `view "${view.id}" must not declare an empty when clause`,
      );
    }
  });

  test("welcome content targets views that exist", () => {
    const welcome = manifestViewsWelcome();
    const ids = new Set((manifestViews()[EXPLORER_CONTAINER] ?? []).map((view) => view.id));
    assert.ok(welcome.length > 0, "the extension must contribute welcome content");
    assert.strictEqual(welcome.length, onDisk("viewsWelcome").length, "none may be dropped");
    for (const entry of welcome) {
      assert.ok(entry.view.length > 0, "welcome content must name a view");
      assert.ok(ids.has(entry.view), `welcome content targets unknown view "${entry.view}"`);
      assert.ok(entry.contents.length > 0, `welcome content for "${entry.view}" must not be empty`);
    }
  });
});

suite("Extension manifest debuggers and settings [VSIX]", () => {
  test("the Basilisk debugger is contributed with both config schemas", () => {
    const debuggers = manifestDebuggers();
    assert.ok(debuggers.length > 0, "the extension must contribute a debugger");
    const basilisk = debuggers.find((entry) => entry.type === DEBUGGER_TYPE);
    assert.ok(basilisk, `${DEBUGGER_TYPE} must be contributed`);
    assert.strictEqual(basilisk.label, "Python (Basilisk)", "the debugger must be labelled");
    assert.ok(basilisk.configurationAttributes.launch, "launch config must be declared");
    assert.ok(basilisk.configurationAttributes.attach, "attach config must be declared");
    const launch = basilisk.configurationAttributes.launch?.properties ?? {};
    assert.ok("program" in launch, "launch must accept a program");
    assert.ok("args" in launch, "launch must accept args");
    assert.ok("justMyCode" in launch, "launch must accept justMyCode");
    const attach = basilisk.configurationAttributes.attach?.properties ?? {};
    assert.ok(Object.keys(attach).length > 0, "attach must declare properties");
  });

  test("settings are namespaced and typed", () => {
    const properties = manifestConfigurationProperties();
    const keys = Object.keys(properties);
    assert.ok(keys.length > 0, "the extension must declare settings");
    for (const key of keys) {
      assert.ok(key.startsWith(NAMESPACE), `setting "${key}" must be namespaced`);
      const schema = properties[key];
      assert.strictEqual(typeof schema, "object", `setting "${key}" must have a schema object`);
      assert.ok(
        "type" in schema || "anyOf" in schema || "enum" in schema,
        `setting "${key}" must declare a type`,
      );
    }
  });

  test("every declared setting is reported", () => {
    const properties = manifestConfigurationProperties();
    const configuration = recordField(contributesOnDisk(), "configuration") ?? {};
    const declared = recordField(configuration, "properties") ?? {};
    assert.strictEqual(
      Object.keys(properties).length,
      Object.keys(declared).length,
      "no setting may be dropped by narrowing",
    );
    for (const key of Object.keys(declared)) {
      assert.ok(key in properties, `setting "${key}" must be reported`);
    }
  });

  test("the aggregate reader agrees with every granular reader", () => {
    const contributes = manifestContributes();
    assert.deepStrictEqual(contributes.commands, manifestCommands(), "commands must agree");
    assert.deepStrictEqual(contributes.keybindings, manifestKeybindings(), "keybindings must agree");
    assert.deepStrictEqual(contributes.menus, manifestMenus(), "menus must agree");
    assert.deepStrictEqual(contributes.views, manifestViews(), "views must agree");
    assert.deepStrictEqual(contributes.viewsWelcome, manifestViewsWelcome(), "welcome must agree");
    assert.deepStrictEqual(contributes.debuggers, manifestDebuggers(), "debuggers must agree");
    assert.deepStrictEqual(
      contributes.configurationProperties,
      manifestConfigurationProperties(),
      "settings must agree",
    );
  });

  test("readers are stable across calls", () => {
    assert.deepStrictEqual(manifestCommands(), manifestCommands(), "commands must be stable");
    assert.deepStrictEqual(manifestViews(), manifestViews(), "views must be stable");
    assert.strictEqual(manifestDisplayName(), manifestDisplayName(), "identity must be stable");
  });
});
