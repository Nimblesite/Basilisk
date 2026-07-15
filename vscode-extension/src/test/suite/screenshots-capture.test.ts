// Implements [VSIX-EDITOR-SCREENSHOTS-SET]: the captured set — one test per
// committed vscode-*.png (diagnostics, hover, quick fix, module explorer,
// configuration editor). Each
// drives a Basilisk feature until it is visible, then asks the CDP sidecar
// (scripts/screenshot-watcher.mjs, [VSIX-EDITOR-SCREENSHOTS-PIPELINE]) to grab
// the window.
//
// The whole suite is a NO-OP unless BASILISK_SCREENSHOTS=1 (it skips in
// suiteSetup), so normal `npm test` runs are unaffected and nothing is written
// into the repo. Drive it with `npm run screenshots:editor`, which builds +
// stages the binary, copies shipwright.json, launches the sidecar, and runs only
// this file. See docs/specs/VSIX-EDITOR-SCREENSHOTS-SPEC.md.

import * as vscode from 'vscode';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import {
    SUITE_SETUP_TIMEOUT_MS,
    closeAllEditors,
    findBasiliskBinary,
    openPythonFile,
    waitForDiagnostics,
    waitForLspReady,
} from './test-helpers';
import { takeWindowScreenshot } from './screenshot';
import { ConfigurationEditorController } from '../../configuration-editor';
import { getStore } from '../../extension';

async function sleep(ms: number): Promise<void> {
    await new Promise<void>((resolve) => {
        setTimeout(resolve, ms);
    });
}

// Strip transient chrome that clutters a marketing screenshot: the
// "--disable-extensions"/git toasts and the Chat auxiliary bar.
async function prepareWindow(): Promise<void> {
    await vscode.commands.executeCommand('notifications.clearAll');
    await vscode.commands.executeCommand('workbench.action.closeAuxiliaryBar');
    await sleep(400);
}

// A file that triggers several distinct Basilisk diagnostics, so the editor and
// Problems panel look representative.
const DEMO_SOURCE = `def process(data):
    return data.upper()


class User:
    def __init__(self, name, age):
        self.name = name
        self.age = age
`;

async function captureBookConfigurationPreview(
    controller: ConfigurationEditorController,
    store: NonNullable<ReturnType<typeof getStore>>,
): Promise<void> {
    // [CONFIGEDITOR-MODEL]: one typed SetRule mutation — the only write shape.
    await controller.receive({
        type: 'preview',
        mutations: [{ kind: 'SetRule', code: 'BSK-0002', severity: { kind: 'Warning' } }],
    });
    if (store.configurationEditor.value.preview === undefined) {
        throw new Error('configuration editor did not render the real LSP preview');
    }
    await prepareWindow();
    await sleep(800);
    await takeWindowScreenshot('09-configuration-preview-full.png');
}

suite('Editor screenshots', function () {
    let tmpDir: string;

    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        if (process.env.BASILISK_SCREENSHOTS === undefined) {
            this.skip();
        }
        if (findBasiliskBinary() === undefined) {
            throw new Error('Basilisk binary not found. Build with: cargo build -p basilisk-cli');
        }
        tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'basilisk-shots-'));
        await waitForLspReady();
        await closeAllEditors();
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        if (tmpDir !== undefined && tmpDir !== '' && fs.existsSync(tmpDir)) {
            fs.rmSync(tmpDir, { recursive: true, force: true });
        }
    });

    test('diagnostics + Problems panel', async function () {
        this.timeout(60_000);
        const { uri } = await openPythonFile(tmpDir, 'diagnostics.py', DEMO_SOURCE);
        await waitForDiagnostics(uri);
        // Surface the squiggles in the Problems panel for a complete picture.
        await vscode.commands.executeCommand('workbench.actions.view.problems');
        await sleep(1200);
        await prepareWindow();
        await takeWindowScreenshot('vscode-diagnostics.png');
        await vscode.commands.executeCommand('workbench.action.closePanel');
    });

    test('hover with type information', async function () {
        this.timeout(60_000);
        const { doc } = await openPythonFile(
            tmpDir,
            'hover.py',
            'def greet(name: str) -> str:\n    return f"Hello, {name}"\n',
        );
        const editor = await vscode.window.showTextDocument(doc, { preview: false });
        // Position on the `greet` function name and request the hover popup.
        const pos = new vscode.Position(0, 5);
        editor.selection = new vscode.Selection(pos, pos);
        editor.revealRange(new vscode.Range(pos, pos));
        await sleep(500);
        await vscode.commands.executeCommand('editor.action.showHover');
        await prepareWindow();
        await sleep(300);
        await vscode.commands.executeCommand('editor.action.showHover');
        await sleep(1200);
        await takeWindowScreenshot('vscode-hover.png');
    });

    test('quick fix code actions', async function () {
        this.timeout(60_000);
        const { uri, doc } = await openPythonFile(
            tmpDir,
            'quickfix.py',
            'def process(data):\n    return data\n',
        );
        await waitForDiagnostics(uri);
        const editor = await vscode.window.showTextDocument(doc, { preview: false });
        const pos = new vscode.Position(0, 12); // on the unannotated `data` parameter
        editor.selection = new vscode.Selection(pos, pos);
        editor.revealRange(new vscode.Range(pos, pos));
        await sleep(500);
        await vscode.commands.executeCommand('editor.action.quickFix');
        await prepareWindow();
        await sleep(300);
        await vscode.commands.executeCommand('editor.action.quickFix');
        await sleep(1200);
        await takeWindowScreenshot('vscode-quickfix.png');
        await vscode.commands.executeCommand('hideSuggestWidget');
    });

    test('module explorer activity panel', async function () {
        this.timeout(60_000);
        await openPythonFile(tmpDir, 'explorer.py', DEMO_SOURCE);
        await vscode.commands.executeCommand('workbench.view.extension.basilisk-explorer');
        await sleep(800);
        await vscode.commands.executeCommand('basilisk.refreshModuleExplorer');
        await sleep(1800);
        await prepareWindow();
        await takeWindowScreenshot('vscode-module-explorer.png');
    });

    test('configuration editor tag-first rules', async function () {
        this.timeout(60_000);
        await closeAllEditors();
        await vscode.commands.executeCommand('workbench.action.closeSidebar');
        const root = vscode.workspace.workspaceFolders?.[0];
        const store = getStore();
        if (root === undefined || store === undefined) {
            throw new Error('real workspace and extension store are required for configuration capture');
        }
        const controller = new ConfigurationEditorController(store);
        try {
            controller.open(root.uri.toString());
            const deadline = Date.now() + 10_000;
            while (store.configurationEditor.value.phase !== 'ready' && Date.now() < deadline) {
                await sleep(50);
            }
            if (store.configurationEditor.value.phase !== 'ready') {
                throw new Error('configuration editor did not receive a snapshot from the real LSP');
            }
            await prepareWindow();
            await sleep(1_200);
            const bookCapture = process.env.BASILISK_BOOK_SCREENSHOTS !== undefined;
            await takeWindowScreenshot(
                bookCapture ? '09-configuration-editor-full.png' : 'vscode-configuration-editor.png',
            );
            if (bookCapture) {
                await captureBookConfigurationPreview(controller, store);
            }
        } finally {
            controller.dispose();
        }
    });
});
