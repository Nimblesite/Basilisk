import * as path from 'path';
import Mocha from 'mocha';
import { glob } from 'glob';

export async function run(): Promise<void> {
    const timeout = parseInt(process.env.MOCHA_TIMEOUT ?? '60000', 10);
    const mocha = new Mocha({ ui: 'tdd', color: true, timeout });
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
