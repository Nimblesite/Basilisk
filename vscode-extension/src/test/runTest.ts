import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { runTests } from '@vscode/test-electron';
import { execSync } from 'child_process';

/**
 * Find the system VS Code Electron binary on macOS.
 * Returns the path to the Electron binary inside the .app bundle,
 * or undefined if not found.
 */
function findSystemVSCodeElectron(): string | undefined {
    // Check common macOS install locations.
    const appPaths = [
        '/Applications/Visual Studio Code.app',
        path.join(process.env.HOME ?? '', 'Applications/Visual Studio Code.app'),
    ];
    for (const appPath of appPaths) {
        const electron = path.join(appPath, 'Contents/MacOS/Electron');
        if (fs.existsSync(electron)) {
            return electron;
        }
    }

    // Try resolving from the `code` CLI shim.
    try {
        const codePath = execSync('which code', { encoding: 'utf8' }).trim();
        const realPath = execSync(`realpath "${codePath}"`, { encoding: 'utf8' }).trim();
        // realPath is like /Applications/Visual Studio Code.app/Contents/Resources/app/bin/code
        const appRoot = realPath.replace(/\/Contents\/Resources\/app\/bin\/code$/, '');
        const electron = path.join(appRoot, 'Contents/MacOS/Electron');
        if (fs.existsSync(electron)) {
            return electron;
        }
    } catch {
        // Ignore
    }

    return undefined;
}

/**
 * Find the debug-built basilisk binary.
 */
function findBinary(): string | undefined {
    const workspaceRoot = path.resolve(__dirname, '../../..');
    for (const profile of ['release', 'debug']) {
        const binary = path.join(workspaceRoot, 'target', profile, 'basilisk');
        if (fs.existsSync(binary)) {
            return binary;
        }
    }
    return undefined;
}

async function main(): Promise<void> {
    try {
        const extensionDevelopmentPath = path.resolve(__dirname, '../../');
        const extensionTestsPath = path.resolve(__dirname, './suite/index');

        const systemElectron = findSystemVSCodeElectron();

        // Create a temp workspace with settings pointing to the debug binary
        // so the extension uses the latest build, not a stale installed version.
        const tmpWorkspace = fs.mkdtempSync(path.join(os.tmpdir(), 'basilisk-test-ws-'));
        const vscodeDir = path.join(tmpWorkspace, '.vscode');
        fs.mkdirSync(vscodeDir, { recursive: true });

        const debugBinary = process.env.BASILISK_EXECUTABLE_PATH ?? findBinary();
        const settings: Record<string, unknown> = {};
        if (debugBinary) {
            settings['basilisk.executablePath'] = debugBinary;
        }
        fs.writeFileSync(
            path.join(vscodeDir, 'settings.json'),
            JSON.stringify(settings, null, 2),
            'utf8'
        );

        await runTests({
            extensionDevelopmentPath,
            extensionTestsPath,
            ...(systemElectron ? { vscodeExecutablePath: systemElectron } : {}),
            launchArgs: ['--disable-extensions', tmpWorkspace],
        });

        // Clean up temp workspace.
        fs.rmSync(tmpWorkspace, { recursive: true, force: true });
    } catch (err) {
        // eslint-disable-next-line no-console
        console.error('Failed to run tests', err);
        process.exit(1);
    }
}

void main();
