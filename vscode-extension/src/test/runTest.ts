import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { runTests } from '@vscode/test-electron';
import { execSync } from 'child_process';

const EXECUTABLE_MODE = 0o755;

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

function detectShipwrightPlatform(): string {
    if (process.platform === 'darwin' && process.arch === 'arm64') { return 'darwin-arm64'; }
    if (process.platform === 'linux' && process.arch === 'arm64') { return 'linux-arm64'; }
    if (process.platform === 'linux') { return 'linux-x64'; }
    if (process.platform === 'win32' && process.arch === 'arm64') { return 'win32-arm64'; }
    if (process.platform === 'win32') { return 'win32-x64'; }
    throw new Error(`Unsupported VSIX test platform: ${process.platform}-${process.arch}`);
}

function syncShipwrightManifest(extensionDevelopmentPath: string): void {
    const repoRoot = path.resolve(extensionDevelopmentPath, '..');
    const source = path.join(repoRoot, 'shipwright.json');
    const target = path.join(extensionDevelopmentPath, 'shipwright.json');
    if (!fs.existsSync(source)) {
        throw new Error(`Missing Shipwright manifest: ${source}`);
    }
    fs.copyFileSync(source, target);
}

function stageBundledBinary(extensionDevelopmentPath: string, binary: string): void {
    const binRoot = path.join(extensionDevelopmentPath, 'bin');
    const targetDir = path.join(binRoot, detectShipwrightPlatform());
    fs.rmSync(binRoot, { recursive: true, force: true });
    fs.mkdirSync(targetDir, { recursive: true });
    const target = path.join(targetDir, path.basename(binary));
    fs.copyFileSync(binary, target);
    if (process.platform !== 'win32') {
        fs.chmodSync(target, EXECUTABLE_MODE);
    }
}

async function main(): Promise<void> {
    try {
        const extensionDevelopmentPath = path.resolve(__dirname, '../../');
        const extensionTestsPath = path.resolve(__dirname, './suite/index');

        const systemElectron = findSystemVSCodeElectron();

        const debugBinary = process.env.BASILISK_EXECUTABLE_PATH ?? findBinary();
        if (debugBinary === undefined || debugBinary === '') {
            throw new Error('Basilisk binary not found. Build with: cargo build -p basilisk-cli');
        }
        syncShipwrightManifest(extensionDevelopmentPath);
        stageBundledBinary(extensionDevelopmentPath, debugBinary);
        delete process.env.BASILISK_EXECUTABLE_PATH;
        delete process.env.BASILISK_BINARY_DIR;

        // Create a temp workspace with empty binary settings so activation must
        // prove the bundled VSIX path instead of a developer-machine override.
        const tmpWorkspace = fs.mkdtempSync(path.join(os.tmpdir(), 'basilisk-test-ws-'));
        const vscodeDir = path.join(tmpWorkspace, '.vscode');
        fs.mkdirSync(vscodeDir, { recursive: true });

        const settings: Record<string, unknown> = {};
        fs.writeFileSync(
            path.join(vscodeDir, 'settings.json'),
            JSON.stringify(settings, null, 2),
            'utf8'
        );

        await runTests({
            extensionDevelopmentPath,
            extensionTestsPath,
            ...(systemElectron !== undefined ? { vscodeExecutablePath: systemElectron } : {}),
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
