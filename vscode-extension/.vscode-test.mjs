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
        mocha: {
            timeout: 60000,
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
