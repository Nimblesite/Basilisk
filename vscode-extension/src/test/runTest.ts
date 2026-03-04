import * as path from 'path';
import * as fs from 'fs';
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

async function main(): Promise<void> {
    try {
        const extensionDevelopmentPath = path.resolve(__dirname, '../../');
        const extensionTestsPath = path.resolve(__dirname, './suite/index');

        const systemElectron = findSystemVSCodeElectron();

        await runTests({
            extensionDevelopmentPath,
            extensionTestsPath,
            ...(systemElectron ? { vscodeExecutablePath: systemElectron } : {}),
            launchArgs: ['--disable-extensions'],
        });
    } catch (err) {
        console.error('Failed to run tests', err);
        process.exit(1);
    }
}

main();
