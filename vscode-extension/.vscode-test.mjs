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
        includeAll: true,
        // @vscode/test-cli sets report.exclude.relativePath = false, which
        // makes test-exclude match against absolute paths. Patterns must
        // start with **/ so minimatch can match any prefix.
        include: ['**/out/**/*.js'],
        exclude: ['**/out/test/**'],
        reporter: ['text', 'lcov'],
    },
});
