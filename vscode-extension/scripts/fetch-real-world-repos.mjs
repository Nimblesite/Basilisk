// Implements [VSIX-REALWORLD-CORPUS]. See docs/specs/VSIX-REAL-WORLD-SPEC.md#VSIX-REALWORLD-CORPUS
//
// Fetches the pinned real-world Python repositories the [VSIX-REALWORLD]
// e2e suites open as VS Code workspaces. Every repo is pinned to an exact
// commit SHA (immutable content), downloaded as a GitHub tarball (no git
// dependency), extracted under `.real-world/<name>/`, and stamped with a
// marker file so repeat runs are a no-op. Runs as `pretest`, so the corpus
// is always present before `vscode-test` launches.
//
// Honesty rule: a fetch that cannot produce the pinned tree FAILS the run.
// There is no offline skip — a missing corpus would silently disarm the
// real-world suites, which is forbidden (CLAUDE.md, Testing).

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const EXTENSION_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const CORPUS_PATH = path.join(EXTENSION_ROOT, "test-fixtures", "real-world-corpus.json");
const REPOS_ROOT = path.join(EXTENSION_ROOT, ".real-world");
const MARKER_NAME = ".bsk-real-world-ok";
// VS Code hot-exit backups from a previous (possibly aborted mid-churn) test
// session. All test configs share this persistent user-data dir, so a dirty
// buffer backed up by an interrupted edit-churn run would be silently
// restored into the NEXT run's identical workspace, poisoning its baseline.
const USER_DATA_BACKUPS = path.join(EXTENSION_ROOT, ".vscode-test", "user-data", "Backups");

const DOWNLOAD_ATTEMPTS = 3;
const RETRY_BASE_DELAY_MS = 2_000;

/** @returns {{repos: Array<{name: string, org: string, repo: string, tag: string, commit: string, sentinel: string, minPythonFiles: number}>}} */
function loadCorpus() {
    return JSON.parse(fs.readFileSync(CORPUS_PATH, "utf8"));
}

function markerPath(dest) {
    return path.join(dest, MARKER_NAME);
}

/** Repo already extracted at the pinned commit? */
function isFresh(dest, commit) {
    try {
        return fs.readFileSync(markerPath(dest), "utf8").trim() === commit;
    } catch {
        return false;
    }
}

async function sleep(ms) {
    await new Promise((resolve) => setTimeout(resolve, ms));
}

/** Download `url` to `file`, retrying transient failures with backoff. */
async function download(url, file) {
    let lastError;
    for (let attempt = 1; attempt <= DOWNLOAD_ATTEMPTS; attempt++) {
        try {
            const response = await fetch(url, { redirect: "follow" });
            if (!response.ok) {
                throw new Error(`HTTP ${response.status} for ${url}`);
            }
            const bytes = Buffer.from(await response.arrayBuffer());
            if (bytes.length === 0) {
                throw new Error(`Empty tarball from ${url}`);
            }
            fs.writeFileSync(file, bytes);
            return;
        } catch (error) {
            lastError = error;
            console.warn(`  attempt ${attempt}/${DOWNLOAD_ATTEMPTS} failed: ${error.message ?? error}`);
            if (attempt < DOWNLOAD_ATTEMPTS) {
                await sleep(RETRY_BASE_DELAY_MS * attempt);
            }
        }
    }
    throw new Error(`Download failed after ${DOWNLOAD_ATTEMPTS} attempts: ${lastError?.message ?? lastError}`);
}

/**
 * Fetch one pinned repo into `.real-world/<name>` and stamp the marker.
 * Extraction uses the system `tar` (bsdtar on Windows 10+, GNU tar on
 * Linux, bsdtar on macOS) — present on every platform CI and devs use.
 */
async function fetchRepo(entry) {
    const dest = path.join(REPOS_ROOT, entry.name);
    if (isFresh(dest, entry.commit)) {
        console.log(`✓ ${entry.name} already pinned at ${entry.commit.slice(0, 12)} (${entry.tag})`);
        return;
    }

    console.log(`▶ Fetching ${entry.org}/${entry.repo} @ ${entry.tag} (${entry.commit.slice(0, 12)})`);
    // Stale or partial tree: rebuild the cache dir from scratch.
    fs.rmSync(dest, { recursive: true, force: true });
    fs.mkdirSync(dest, { recursive: true });

    const tarballName = `${entry.name}-${entry.commit.slice(0, 12)}.tar.gz`;
    const tarball = path.join(REPOS_ROOT, tarballName);
    const url = `https://codeload.github.com/${entry.org}/${entry.repo}/tar.gz/${entry.commit}`;
    try {
        await download(url, tarball);
        // Relative paths + cwd, NOT absolute Windows paths: GNU tar (e.g. the
        // MSYS tar on a Git Bash PATH) parses `C:\...` as a remote host spec.
        execFileSync("tar", ["-xzf", tarballName, "--strip-components=1", "-C", entry.name], {
            cwd: REPOS_ROOT,
            stdio: "inherit",
        });
    } finally {
        fs.rmSync(tarball, { force: true });
    }

    const sentinel = path.join(dest, entry.sentinel);
    if (!fs.existsSync(sentinel)) {
        throw new Error(
            `${entry.name}: sentinel ${entry.sentinel} missing after extraction — ` +
            "tarball layout changed or extraction failed"
        );
    }
    const pyFiles = countPythonFiles(dest);
    if (pyFiles < entry.minPythonFiles) {
        throw new Error(
            `${entry.name}: extracted tree holds ${pyFiles} .py files — expected at ` +
            `least ${entry.minPythonFiles}; the tree is truncated`
        );
    }

    fs.writeFileSync(markerPath(dest), `${entry.commit}\n`);
    console.log(`✓ ${entry.name} ready at ${path.relative(EXTENSION_ROOT, dest)} (${pyFiles} .py files)`);
}

/** Recursively count `.py` files under `dir` (skipping dot-directories). */
function countPythonFiles(dir) {
    let count = 0;
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        if (entry.name.startsWith(".")) { continue; }
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) {
            count += countPythonFiles(full);
        } else if (entry.name.endsWith(".py")) {
            count += 1;
        }
    }
    return count;
}

async function main() {
    const corpus = loadCorpus();
    fs.mkdirSync(REPOS_ROOT, { recursive: true });
    // A dirty buffer from an aborted previous session must never be hot-exit
    // restored into this run — see USER_DATA_BACKUPS.
    fs.rmSync(USER_DATA_BACKUPS, { recursive: true, force: true });
    for (const entry of corpus.repos) {
        await fetchRepo(entry);
    }
}

main().catch((error) => {
    console.error(`fetch-real-world-repos failed: ${error.message ?? error}`);
    process.exit(1);
});
