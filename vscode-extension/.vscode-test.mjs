import { defineConfig } from '@vscode/test-cli';
import crypto from 'crypto';
import os from 'os';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

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

// One suite. The extension is a notice ([WITHDRAWAL-SURFACES]): there is no
// language server to pre-warm, no workspace to analyse, and no real-world
// corpus to run against, so the whole configuration is the suite itself.
export default defineConfig({
    tests: [{
        label: 'workspace-suite',
        files: 'out/test/suite/**/*.test.js',
        workspaceFolder: path.join(__dirname, 'test-fixtures', 'workspace'),
        launchArgs: [
            '--disable-extensions',
            '--user-data-dir', userDataDir,
        ],
        // Coverage: tell c8 where compiled sources live. Without this,
        // @vscode/test-cli defaults to 'src' (TypeScript sources), so
        // include patterns like 'out/**/*.js' resolve against src/ and
        // find nothing.
        srcDir: __dirname,
        mocha: {
            ui: 'tdd',
            // Fail fast by DEFAULT: a local run or CI should stop at the first
            // failure rather than spend time on a verdict already decided.
            // `BSK_TEST_BAIL=0` opts out.
            bail: process.env.BSK_TEST_BAIL !== '0',
            reporter: 'list',
            timeout: 45_000,
            ...(process.env.BSK_TEST_GREP ? { grep: process.env.BSK_TEST_GREP } : {}),
        },
    }],
    coverage: {
        includeAll: false,
        // @vscode/test-cli sets report.exclude.relativePath = false, which
        // makes test-exclude match against absolute paths. Patterns must
        // start with **/ so minimatch can match any prefix.
        include: ['**/out/**/*.js'],
        exclude: ['**/out/test/**'],
        reporter: ['text', 'lcov'],
    },
});
