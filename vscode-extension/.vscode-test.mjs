import { defineConfig } from '@vscode/test-cli';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
    tests: [{
        files: 'out/test/suite/**/*.test.js',
        // Open the test-fixtures/workspace so the LSP server gets a rootUri.
        // This enables whole-module analysis tests that write Python files to the
        // workspace root without opening them in the editor.
        workspaceFolder: path.join(__dirname, 'test-fixtures', 'workspace'),
        launchArgs: ['--disable-extensions', '--user-data-dir', path.join(__dirname, '.vscode-test', 'user-data')],
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
        },
    }],
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
            '**/out/memory-decorations.js',
            '**/out/memory-profiler.js',
            '**/out/module-explorer.js',
            '**/out/profiler.js',
            '**/out/profiler-flamegraph-html.js',
            '**/out/test-explorer.js',
        ],
        reporter: ['text', 'lcov'],
    },
});
