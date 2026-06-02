// Tests for [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
import * as path from 'path';
import Mocha from 'mocha';
import { glob } from 'glob';
import {
    SUITE_SETUP_TIMEOUT_MS,
    waitForLspReady,
} from './test-helpers';

export async function run(): Promise<void> {
    const timeout = parseInt(process.env.MOCHA_TIMEOUT ?? '60000', 10);
    const mocha = new Mocha({
        ui: 'tdd',
        color: true,
        timeout,
        // Optional focus filter for invoking the runner directly to debug a
        // single test/suite (see CLAUDE.md). Unset in CI, where the full suite runs.
        ...(process.env.BSK_TEST_GREP !== undefined ? { grep: process.env.BSK_TEST_GREP } : {}),
        rootHooks: {
            beforeAll(this: Mocha.Context, done: Mocha.Done) {
                this.timeout(SUITE_SETUP_TIMEOUT_MS);
                waitForLspReady().then(() => done(), done);
            },
        },
    });
    const testsRoot = path.resolve(__dirname);
    const files = await glob('**/**.test.js', { cwd: testsRoot });
    files.forEach(f => mocha.addFile(path.resolve(testsRoot, f)));
    return new Promise<void>((resolve, reject) => {
        mocha.run(failures => {
            if (failures > 0) {
                reject(new Error(`${failures} tests failed.`));
            } else {
                resolve();
            }
        });
    });
}
