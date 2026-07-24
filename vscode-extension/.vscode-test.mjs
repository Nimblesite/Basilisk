import { defineConfig } from '@vscode/test-cli';
import crypto from 'crypto';
import fs from 'fs';
import os from 'os';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const defaultWorkspace = path.join(__dirname, 'test-fixtures', 'workspace');
const screenshotWorkspace = process.env.BASILISK_SCREENSHOT_WORKSPACE;
const workspaceFolder = process.env.BASILISK_SCREENSHOTS && screenshotWorkspace
    ? path.resolve(screenshotWorkspace)
    : defaultWorkspace;

// VS Code listens on a Unix socket inside the user-data dir; macOS caps
// AF_UNIX socket paths at 104 bytes ("IPC handle longer than 103 chars").
// Deep checkouts overflow that and the electron main process dies with
// `listen EINVAL`, so fall back to a short per-checkout dir under tmp.
const defaultUserDataDir = path.join(__dirname, '.vscode-test', 'user-data');
const userDataDir = defaultUserDataDir.length > 80
    ? path.join(
        os.tmpdir(),
        `bsk-vsct-${crypto.createHash('sha256').update(__dirname).digest('hex').slice(0, 8)}`,
    )
    : defaultUserDataDir;

// [VSIX-REALWORLD-WIRING]: one config per pinned real-world repo (see
// docs/specs/VSIX-REAL-WORLD-SPEC.md). Each opens the fetched tree (staged by
// scripts/fetch-real-world-repos.mjs — wired as `pretest`) as the workspace
// and selects its corpus entry via BSK_REAL_WORLD_REPO. Skipped in the
// screenshots flow, which drives vscode-test directly without the corpus.
const corpus = JSON.parse(
    fs.readFileSync(path.join(__dirname, 'test-fixtures', 'real-world-corpus.json'), 'utf8'),
);
const realWorldTests = process.env.BASILISK_SCREENSHOTS
    ? []
    : corpus.repos.map((repo) => ({
        label: `real-world-${repo.name}`,
        files: 'out/test/real-world/**/*.test.js',
        workspaceFolder: path.join(__dirname, '.real-world', repo.name),
        launchArgs: ['--disable-extensions', '--user-data-dir', userDataDir],
        env: { BSK_REAL_WORLD_REPO: repo.name },
        srcDir: __dirname,
        // Analysis of a whole real repo is the slowest thing the suite waits
        // on; per-test timeouts inside the suite scale off the corpus budgets,
        // so this outer timeout only needs to exceed the largest of them.
        mocha: {
            bail: true,
            reporter: 'list',
            timeout: 600_000,
        },
    }));

export default defineConfig({
    tests: [{
        label: 'workspace-suite',
        files: 'out/test/suite/**/*.test.js',
        // Open the test-fixtures/workspace so the LSP server gets a rootUri.
        // This enables whole-module analysis tests that write Python files to the
        // workspace root without opening them in the editor.
        workspaceFolder,
        launchArgs: [
            '--disable-extensions',
            '--user-data-dir', userDataDir,
            // [VSIX-EDITOR-SCREENSHOTS-PIPELINE]: when capturing website
            // screenshots, expose the headed VS Code window over CDP so the
            // watcher (screenshot-watcher.mjs) can grab it. No effect normally.
            ...(process.env.BASILISK_SCREENSHOTS
                ? [`--remote-debugging-port=${process.env.BASILISK_SCREENSHOT_CDP_PORT ?? '9229'}`]
                : []),
        ],
        // Coverage: tell c8 where compiled sources live. Without this,
        // @vscode/test-cli defaults to 'src' (TypeScript sources), so
        // include patterns like 'out/**/*.js' resolve against src/ and
        // find nothing.
        srcDir: __dirname,
        // Mocha config — @vscode/test-cli creates its own Mocha instance and
        // ignores src/test/suite/index.ts's Mocha config. This is the ONLY
        // place Mocha config is honoured when running `npm test`.
        // `require` runs once per test process — used to pre-warm the LSP.
        // Timeout sized for slow debug-integration tests that spawn debugpy
        // and step through real Python code on CI runners.
        mocha: {
            bail: true,
            reporter: 'list',
            timeout: 45_000,
            require: './out/test/suite/index.js',
            ...(process.env.BSK_TEST_GREP ? { grep: process.env.BSK_TEST_GREP } : {}),
        },
    }, ...realWorldTests],
    coverage: {
        includeAll: false,
        // @vscode/test-cli sets report.exclude.relativePath = false, which
        // makes test-exclude match against absolute paths. Patterns must
        // start with **/ so minimatch can match any prefix.
        include: ['**/out/**/*.js'],
        exclude: [
            '**/out/test/**',
            // Panel/webview command modules are validated by E2E contract tests,
            // but their callback-heavy UI branches are not a stable line
            // coverage signal under the VS Code extension host.
            '**/out/coverage-decorations.js',
            '**/out/info-panel.js',
            '**/out/memory-dashboard.js',
            '**/out/memory-decorations.js',
            '**/out/memory-profiler.js',
            '**/out/memory-ref-graph.js',
            '**/out/module-explorer.js',
            '**/out/profiler.js',
            '**/out/profiler-flamegraph-html.js',
            '**/out/test-explorer.js',
        ],
        reporter: ['text', 'lcov'],
    },
});
