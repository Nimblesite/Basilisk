/**
 * Book-only driver for the Chapter 10 terminal captures.
 *
 * This file is copied into a temporary checkout of the pinned release's VS
 * Code test harness.  It drives an actual integrated terminal and the
 * checksum-verified 0.39.0 binary; it does not replace or redraw product UI.
 */

import { delay } from '../../timeouts';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

import { closeAllEditors, SUITE_SETUP_TIMEOUT_MS } from './test-helpers';
import { takeWindowScreenshot } from './screenshot';

function requiredEnvironment(name: string): string {
    const value = process.env[name];
    if (value === undefined || value.trim() === '') {
        throw new Error(`Missing required Chapter 10 capture environment: ${name}`);
    }
    return value;
}

async function waitForText(filename: string, text: string, timeoutMs = 10_000): Promise<void> {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
        if (fs.existsSync(filename) && fs.readFileSync(filename, 'utf8').includes(text)) {
            return;
        }
        await delay(50);
    }
    throw new Error(`${path.basename(filename)} never contained ${JSON.stringify(text)}`);
}

async function prepareWindow(): Promise<void> {
    await vscode.commands.executeCommand('notifications.clearAll');
    await vscode.commands.executeCommand('workbench.action.closeSidebar');
    await vscode.commands.executeCommand('workbench.action.closeAuxiliaryBar');
    await closeAllEditors();
    await delay(400);
}

suite('Chapter 10 book capture', function () {
    test('real 0.39.0 fix and adoption terminal', async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        if (process.env.BASILISK_BOOK_CH10_SCREENSHOTS === undefined) {
            this.skip();
        }

        const workspace = requiredEnvironment('BASILISK_CH10_WORKSPACE');
        const binaryDirectory = requiredEnvironment('BASILISK_CH10_BINARY_DIR');
        const decoder = path.join(workspace, 'src', 'signal_box', 'legacy', 'decoder.py');
        const reviewedDecoder = path.join(workspace, 'stages', 'decoder.reviewed');
        const baselineConfiguration = path.join(workspace, 'stages', 'pyproject.before');
        const configuration = path.join(workspace, 'pyproject.toml');

        await prepareWindow();
        await vscode.commands.executeCommand('workbench.action.terminal.killAll');
        const terminal = vscode.window.createTerminal({
            name: 'Basilisk 0.39.0 — Signal Box adoption',
            cwd: vscode.Uri.file(workspace),
            shellPath: '/bin/zsh',
            shellArgs: ['-f'],
            env: {
                PATH: `${binaryDirectory}:${process.env.PATH ?? ''}`,
                PROMPT: 'signal-box $ ',
                PS1: 'signal-box $ ',
            },
        });

        try {
            terminal.show(false);
            await delay(800);
            await vscode.commands.executeCommand('workbench.action.toggleMaximizedPanel');
            terminal.sendText("export PROMPT='signal-box $ '; clear");
            await delay(500);

            terminal.sendText('basilisk --version');
            await delay(500);
            terminal.sendText('basilisk fix src/signal_box/legacy');
            await waitForText(decoder, 'raw: Any');
            terminal.sendText('diff -u stages/decoder.before src/signal_box/legacy/decoder.py || true');
            await delay(1_200);
            await takeWindowScreenshot('10-cli-fix-full.png');

            fs.copyFileSync(reviewedDecoder, decoder);
            fs.copyFileSync(baselineConfiguration, configuration);
            terminal.sendText('clear');
            await delay(400);
            terminal.sendText('basilisk adopt src/signal_box/legacy');
            await waitForText(configuration, 'calls_argument_type = "warning"');
            terminal.sendText('basilisk adopt --status .');
            await delay(800);
            terminal.sendText('basilisk check --color never src/signal_box/legacy');
            await delay(1_500);
            await takeWindowScreenshot('10-adopt-status-full.png');
        } finally {
            terminal.dispose();
        }
    });
});
