// Implements [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
/**
 * Screenshot capture for Basilisk VS Code extension E2E tests.
 *
 * The integration suite runs in a *headed* Electron VS Code instance on the
 * host (`@vscode/test-cli` / `@vscode/test-electron`). That lets us grab a
 * picture of the editor — Python file open, Basilisk diagnostics squiggled,
 * Problems panel — purely for *local* debugging.
 *
 * Hard rules (see CLAUDE.md → [GITHUB-NO-ARTIFACTS]):
 *   - Screenshots are written to a gitignored local folder ONLY.
 *   - They are NEVER committed and NEVER uploaded as CI artifacts.
 *
 * Capture is best-effort: a failure to screenshot must never fail a test, so
 * every error is swallowed (and logged). On platforms without a screenshot
 * tool, or in a headless CI run, capture is silently skipped.
 */

import { delay } from '../../timeouts';
import { execFile } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

/**
 * Directory for the *committed* website editor screenshots. Resolves to
 * `website/src/assets/images/` (`__dirname` is `out/test/suite`, four levels up
 * is the repo root).
 */
function websiteImageDir(): string {
    const configured = process.env.BASILISK_SCREENSHOT_OUTPUT_DIR;
    if (configured !== undefined && configured.trim() !== '') {
        return path.resolve(configured);
    }
    return path.resolve(__dirname, '..', '..', '..', '..', 'website', 'src', 'assets', 'images');
}

/**
 * Capture the real VS Code window for the website via the CDP sidecar
 * ([VSIX-EDITOR-SCREENSHOTS-PIPELINE], screenshot-watcher.mjs). Writes a `.signal` file
 * into the website image dir and waits for the sidecar to produce the PNG, then
 * renames it into place.
 *
 * No-op unless `BASILISK_SCREENSHOTS=1` — so normal test runs are unaffected and
 * never write into the repo. Call after assertions prove the feature is visible.
 *
 * @param filename final PNG name, e.g. `vscode-diagnostics.png`.
 */
export async function takeWindowScreenshot(filename: string): Promise<void> {
    if (process.env.BASILISK_SCREENSHOTS === undefined) {
        return;
    }
    const dir = websiteImageDir();
    fs.mkdirSync(dir, { recursive: true });
    const tempFilename = `${filename}.tmp-${process.pid.toString()}.png`;
    const tempPath = path.join(dir, tempFilename);
    const signalPath = path.join(dir, `${tempFilename}.signal`);
    const outPath = path.join(dir, filename);
    if (fs.existsSync(tempPath)) {
        fs.rmSync(tempPath, { force: true });
    }
    fs.writeFileSync(signalPath, filename, 'utf8');

    const deadline = Date.now() + 20_000;
    while (Date.now() < deadline) {
        if (fs.existsSync(tempPath)) {
            fs.renameSync(tempPath, outPath);
            // eslint-disable-next-line no-console
            console.log(`[screenshot] wrote ${filename}`);
            return;
        }
        await delay(100);
    }
    throw new Error(`screenshot sidecar did not produce ${filename} within 20s`);
}

/**
 * Directory where screenshots are written. Resolves to
 * `vscode-extension/.screenshots/` (gitignored). `__dirname` is
 * `out/test/suite`, so three levels up reaches the extension root.
 */
function screenshotDir(): string {
    return path.resolve(__dirname, '..', '..', '..', '.screenshots');
}

/** Sanitise a label into a filesystem-safe filename stem. */
function safeStem(label: string): string {
    const cleaned = label.replace(/[^a-zA-Z0-9._-]+/g, '-').replace(/^-+|-+$/g, '');
    return cleaned.length > 0 ? cleaned : 'screenshot';
}

/**
 * Capture a screenshot of the current screen to the gitignored
 * `.screenshots/` folder. Best-effort: never throws, never fails a test.
 *
 * Only macOS is wired up (the dev/CI host for the VSIX suite). `screencapture`
 * grabs the main display; the headed VS Code test window is frontmost.
 *
 * @param label human-readable stem for the file (timestamped to avoid clobber).
 * @returns the written file path, or `undefined` if capture was skipped/failed.
 */
export async function captureScreenshot(label: string): Promise<string | undefined> {
    if (process.platform !== 'darwin') {
        return undefined;
    }

    const dir = screenshotDir();
    try {
        fs.mkdirSync(dir, { recursive: true });
    } catch {
        return undefined;
    }

    const stamp = new Date().toISOString().replace(/[:.]/g, '-');
    const outPath = path.join(dir, `${safeStem(label)}-${stamp}.png`);

    return new Promise<string | undefined>((resolve) => {
        // -x: no capture sound. -o: omit window shadow. Full main-display grab
        // is the most reliable mode for a headed test (no window-id lookup).
        execFile('screencapture', ['-x', '-o', outPath], (error) => {
            if (error !== null) {
                // eslint-disable-next-line no-console
                console.warn(`[screenshot] capture failed for "${label}": ${error.message}`);
                resolve(undefined);
                return;
            }
            // eslint-disable-next-line no-console
            console.log(`[screenshot] wrote ${outPath}`);
            resolve(outPath);
        });
    });
}
