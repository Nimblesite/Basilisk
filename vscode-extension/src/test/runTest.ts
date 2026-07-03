// Tests for [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import * as crypto from 'crypto';
import { runTests } from '@vscode/test-electron';
import { execSync } from 'child_process';

/**
 * VS Code listens on a Unix socket inside the user-data dir; macOS caps
 * AF_UNIX socket paths at 104 bytes ("IPC handle longer than 103 chars").
 * Deep checkouts (e.g. git worktrees) overflow that and the electron main
 * process dies with `listen EINVAL`, so fall back to a short per-checkout
 * dir under tmp — the same policy as .vscode-test.mjs.
 */
function resolveUserDataDir(extensionDevelopmentPath: string): string {
    const defaultDir = path.join(extensionDevelopmentPath, '.vscode-test', 'user-data');
    if (defaultDir.length <= 80) {
        return defaultDir;
    }
    const checkoutHash = crypto
        .createHash('sha256')
        .update(extensionDevelopmentPath)
        .digest('hex')
        .slice(0, 8);
    return path.join(os.tmpdir(), `bsk-vsct-${checkoutHash}`);
}

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

function syncShipwrightManifest(extensionDevelopmentPath: string): void {
    const repoRoot = path.resolve(extensionDevelopmentPath, '..');
    const source = path.join(repoRoot, 'shipwright.json');
    const target = path.join(extensionDevelopmentPath, 'shipwright.json');
    if (!fs.existsSync(source)) {
        throw new Error(`Missing Shipwright manifest: ${source}`);
    }
    fs.copyFileSync(source, target);
}

/**
 * Stage the runtime binaries through the ONE canonical staging script
 * (`scripts/stage-runtime.mjs`) — the same path `_test_vsix` and the release
 * packager use, so this debug runner can never validate a different bundle
 * shape than what ships ([VSIX-PACKAGING-PARITY], #71). Also vendors the
 * debugpy asset when it is not already present, so bundle-dependent journeys
 * (debugging, memory profiling) run against the real layout.
 */
function stageBundledRuntime(extensionDevelopmentPath: string, binaryDir: string): void {
    execSync(`node scripts/stage-runtime.mjs ${JSON.stringify(binaryDir)}`, {
        cwd: extensionDevelopmentPath,
        stdio: 'inherit',
    });
    const debugpyDir = path.join(extensionDevelopmentPath, 'bundled', 'debugpy');
    const vendored = fs.existsSync(debugpyDir) && fs.readdirSync(debugpyDir).length > 0;
    if (!vendored) {
        execSync('node scripts/vendor-debugpy.mjs', {
            cwd: extensionDevelopmentPath,
            stdio: 'inherit',
        });
    }
}

async function main(): Promise<void> {
    try {
        const extensionDevelopmentPath = path.resolve(__dirname, '../../');
        const extensionTestsPath = path.resolve(__dirname, './suite/index');

        const systemElectron = findSystemVSCodeElectron();

        const debugBinary = process.env.BASILISK_EXECUTABLE_PATH ?? findBinary();
        if (debugBinary === undefined || debugBinary === '') {
            throw new Error(
                'Basilisk binary not found. Build with: cargo build -p basilisk-cli -p basilisk-profiler-helper'
            );
        }
        syncShipwrightManifest(extensionDevelopmentPath);
        stageBundledRuntime(extensionDevelopmentPath, path.dirname(debugBinary));
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
            launchArgs: [
                '--disable-extensions',
                '--user-data-dir', resolveUserDataDir(extensionDevelopmentPath),
                tmpWorkspace,
            ],
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
