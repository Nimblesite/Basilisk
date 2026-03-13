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
        launchArgs: ['--disable-extensions'],
        mocha: {
            timeout: 60000,
        },
    }],
    coverage: {
        includeAll: true,
        include: ['out/src/**/*.js'],
        exclude: ['out/src/test/**'],
        reporter: ['text', 'lcov'],
    },
});
