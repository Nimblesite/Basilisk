// Tests for [WITHDRAWAL-SURFACES]. See
// docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-SURFACES
/**
 * The extension's whole contract: it says the approved statement, and it
 * contains no type checker. The second half matters more than the first — a
 * setting, a command, or a bundled binary creeping back would put the checker
 * that produced incorrect results in front of users again.
 */

import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import {
    ANNOUNCED_KEY,
    ANNOUNCEMENT,
    NOTICE_URI,
    SHOW_STATEMENT_COMMAND,
    STATEMENT_URL,
    announce,
    extensionVersion,
    shouldAnnounce,
    statementText,
    type AnnouncementState,
} from "../../extension";

const EXTENSION_ID = "Nimblesite.basilisk";

function extension(): vscode.Extension<unknown> {
    const found = vscode.extensions.getExtension(EXTENSION_ID);
    assert.ok(found, `${EXTENSION_ID} must be installed in the test host`);
    return found;
}

function isRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === "object" && value !== null;
}

/** The manifest as shipped, read from disk rather than from the `any` API. */
function manifest(): Record<string, unknown> {
    const file = path.join(extension().extensionPath, "package.json");
    const parsed: unknown = JSON.parse(fs.readFileSync(file, "utf8"));
    assert.ok(isRecord(parsed), "package.json must parse to an object");
    return parsed;
}

function contributes(): Record<string, unknown> {
    const value = manifest().contributes;
    assert.ok(isRecord(value), "the manifest must have a contributes block");
    return value;
}

/** A memento standing in for `globalState`. */
function state(initial: string | undefined): AnnouncementState {
    let stored = initial;
    return {
        get: (): string | undefined => stored,
        update: async (_key: string, value: string): Promise<void> => {
            stored = value;
        },
    };
}

suite("Basilisk is a notice", () => {
    test("activates", async () => {
        await extension().activate();
        assert.strictEqual(extension().isActive, true);
    });

    test("the statement is the approved notice plus a pointer to the full one", () => {
        const text = statementText();
        assert.ok(text.startsWith("Basilisk is unlisted."), text);
        assert.ok(text.includes("checks nothing"), text);
        assert.ok(text.includes("https://github.com/python/typing/pull/2330"), text);
        assert.ok(text.includes("basilisk-conformance-apology"), text);
        assert.ok(text.includes(STATEMENT_URL), text);
    });

    test("the announcement names the fault and asks for removal", () => {
        assert.ok(ANNOUNCEMENT.includes("incorrect results"), ANNOUNCEMENT);
        assert.ok(ANNOUNCEMENT.includes("Uninstall"), ANNOUNCEMENT);
    });

    test("showStatement opens the statement as a read-only document", async () => {
        await extension().activate();
        await vscode.commands.executeCommand(SHOW_STATEMENT_COMMAND);
        const opened = vscode.window.visibleTextEditors.find(
            (editor) => editor.document.uri.scheme === NOTICE_URI.scheme,
        );
        assert.ok(opened, "the statement must be visible in an editor");
        assert.strictEqual(opened.document.getText(), statementText());
    });
});

suite("The announcement fires once per version", () => {
    test("shouldAnnounce is true only for an unseen version", () => {
        assert.strictEqual(shouldAnnounce(undefined, "1.0.0"), true);
        assert.strictEqual(shouldAnnounce("0.9.0", "1.0.0"), true);
        assert.strictEqual(shouldAnnounce("1.0.0", "1.0.0"), false);
    });

    test("extensionVersion falls back rather than throwing", () => {
        assert.strictEqual(extensionVersion(undefined), "unknown");
        assert.strictEqual(extensionVersion({}), "unknown");
        assert.strictEqual(extensionVersion({ version: 7 }), "unknown");
        assert.strictEqual(extensionVersion({ version: "2.3.4" }), "2.3.4");
    });

    test("choosing the action opens the statement, and the next activation is silent", async () => {
        const seen: string[] = [];
        const memento = state(undefined);
        await announce(memento, "9.9.9", async (message, action) => {
            seen.push(message);
            return action;
        });
        assert.deepStrictEqual(seen, [ANNOUNCEMENT]);
        assert.strictEqual(memento.get(ANNOUNCED_KEY), "9.9.9");

        await announce(memento, "9.9.9", async (message) => {
            seen.push(message);
            return undefined;
        });
        assert.strictEqual(seen.length, 1, "an already-announced version must stay silent");

        const opened = vscode.window.visibleTextEditors.find(
            (editor) => editor.document.uri.scheme === NOTICE_URI.scheme,
        );
        assert.ok(opened, "choosing the action must open the statement");
    });

    test("dismissing the notification does not open the statement", async () => {
        let prompted = 0;
        await announce(state(undefined), "9.9.9", async () => {
            prompted += 1;
            return undefined;
        });
        assert.strictEqual(prompted, 1);
    });

    test("a fresh version announces again", async () => {
        let prompted = 0;
        await announce(state("1.0.0"), "1.0.1", async () => {
            prompted += 1;
            return undefined;
        });
        assert.strictEqual(prompted, 1);
    });
});

suite("No type checker ships in the VSIX", () => {
    test("the manifest contributes nothing but the statement command", () => {
        assert.deepStrictEqual(Object.keys(contributes()), ["commands"]);
        const commands = contributes().commands;
        assert.ok(Array.isArray(commands), "commands must be an array");
        const names = commands.map((entry) => (isRecord(entry) ? entry.command : undefined));
        assert.deepStrictEqual(names, [SHOW_STATEMENT_COMMAND]);
    });

    test("no setting, view, debugger, keybinding or walkthrough survives", () => {
        for (const key of [
            "configuration",
            "views",
            "viewsContainers",
            "viewsWelcome",
            "debuggers",
            "breakpoints",
            "keybindings",
            "menus",
            "walkthroughs",
        ]) {
            assert.strictEqual(contributes()[key], undefined, `contributes.${key} must be gone`);
        }
    });

    test("the package carries no runtime dependency and no bundled binary", () => {
        assert.strictEqual(manifest().dependencies, undefined, "the notice needs no dependency");
        for (const directory of ["bin", "bundled"]) {
            assert.strictEqual(
                fs.existsSync(path.join(extension().extensionPath, directory)),
                false,
                `${directory}/ must not be packaged — the type checker must not ship`,
            );
        }
    });
});
