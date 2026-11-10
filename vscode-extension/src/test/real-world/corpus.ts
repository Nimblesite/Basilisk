// Implements [VSIX-REALWORLD-CORPUS]. See docs/specs/VSIX-REAL-WORLD-SPEC.md#VSIX-REALWORLD-CORPUS
/**
 * Typed access to the real-world corpus manifest
 * (`test-fixtures/real-world-corpus.json`) — the single source of truth
 * shared with `scripts/fetch-real-world-repos.mjs` and `.vscode-test.mjs`.
 *
 * Probe positions are located by searching the PINNED file content for a
 * verified token (the corpus is pinned to exact commit SHAs, so tokens are
 * immutable). A missing token means the manifest and the fetched tree have
 * drifted — that fails loudly, never silently probes the wrong position.
 */

import * as assert from 'assert';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { locate } from '../suite/test-helpers';
import { recordArrayField } from '../../unknown-shape';

/** Env var each real-world test config sets to select its corpus entry. */
export const REPO_ENV_VAR = 'BSK_REAL_WORLD_REPO';

/** Marker file the fetch script stamps into a fully-extracted repo. */
export const FETCH_MARKER = '.bsk-real-world-ok';

export interface HoverProbe {
    readonly token: string;
    readonly at?: string;
    readonly expect: readonly string[];
}

export interface DefinitionProbe {
    readonly token: string;
    readonly at?: string;
    /** Repo-relative path (forward slashes) the definition must land in. */
    readonly expectFile: string;
}

export interface CompletionProbe {
    readonly token: string;
    /** Prefix inside `token` ending with the dot to complete after (e.g. `self.`). */
    readonly afterDot: string;
    readonly expect: readonly string[];
}

export interface ReferenceProbe {
    readonly token: string;
    readonly at?: string;
    readonly minLocations: number;
}

export interface FileJourney {
    readonly path: string;
    readonly minDocumentSymbols: number;
    readonly expectSymbols: readonly string[];
    readonly hovers: readonly HoverProbe[];
    readonly definitions: readonly DefinitionProbe[];
    readonly completions: readonly CompletionProbe[];
    readonly references: readonly ReferenceProbe[];
}

export interface WorkspaceSymbolProbe {
    readonly query: string;
    readonly expectName: string;
    readonly expectFile: string;
}

export interface EditChurnSpec {
    readonly path: string;
    readonly cycles: number;
}

export interface OpenBlitzSpec {
    readonly dir: string;
    readonly count: number;
}

export interface ResourceBudgetsSpec {
    readonly maxServerRssMb: number;
    readonly maxServerLeakMb: number;
    readonly maxExtHostRssMb: number;
    readonly maxIdleCpuPercent: number;
    readonly cpuSettleTimeoutMs: number;
}

export interface RepoSpec {
    readonly name: string;
    readonly org: string;
    readonly repo: string;
    readonly tag: string;
    readonly commit: string;
    readonly sentinel: string;
    /** Floor on `.py` files in the fetched tree — proves the full tree landed. */
    readonly minPythonFiles: number;
    readonly budgets: ResourceBudgetsSpec;
    readonly workspaceSymbols: readonly WorkspaceSymbolProbe[];
    readonly editChurn: EditChurnSpec;
    readonly openBlitz: OpenBlitzSpec;
    readonly files: readonly FileJourney[];
}


/** Absolute path to the extension root (…/vscode-extension). */
function extensionRoot(): string {
    // out/test/real-world → out/test → out → extension root
    return path.resolve(__dirname, '..', '..', '..');
}

/** Load the corpus manifest from test-fixtures. */
export function loadCorpus(): readonly RepoSpec[] {
    const manifest = path.join(extensionRoot(), 'test-fixtures', 'real-world-corpus.json');
    const parsed: unknown = JSON.parse(fs.readFileSync(manifest, 'utf8'));
    // Check `repos` is really a non-empty array of objects BEFORE trusting its
    // element type: reading `.length` off an unchecked cast throws a
    // TypeError on a malformed manifest instead of failing this assertion.
    const repos = recordArrayField(parsed, 'repos');
    assert.ok(repos.length > 0, `corpus manifest ${manifest} lists no repos`);
    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- RepoSpec is a 9-field committed fixture schema; scripts/fetch-real-world-repos.mjs validates it field-by-field before any test reads it
    return repos as unknown as readonly RepoSpec[];
}

/** The corpus entry selected by the active test config's env var. */
export function activeRepoSpec(): RepoSpec {
    const name = process.env[REPO_ENV_VAR];
    assert.ok(
        name !== undefined && name !== '',
        `${REPO_ENV_VAR} is not set — real-world suites must run via their ` +
        '.vscode-test.mjs configs (npm run test:real-world)',
    );
    const spec = loadCorpus().find((r) => r.name === name);
    assert.ok(spec !== undefined, `${REPO_ENV_VAR}=${name} does not match any corpus repo`);
    return spec;
}

/** Convert a 0-based character offset in `content` to a Position. */
function offsetToPosition(content: string, offset: number): vscode.Position {
    const before = content.slice(0, offset);
    const lines = before.split('\n');
    const line = lines.length - 1;
    return new vscode.Position(line, lines[line].length);
}

/**
 * Locate `token` (first occurrence) in `content` and return a Position in
 * the MIDDLE of `at` (a substring of `token`, defaulting to the whole
 * token) — where a user's cursor would sit when hovering or clicking.
 *
 * The `token` disambiguates WHICH occurrence of `at` is meant; the actual
 * cursor math is delegated to the shared {@link locate} helper.
 */
export function probePosition(content: string, token: string, at?: string): vscode.Position {
    const tokenIdx = content.indexOf(token);
    assert.notStrictEqual(tokenIdx, -1, `probe token ${JSON.stringify(token)} not found — corpus drifted from pinned tree`);
    const sub = at ?? token;
    const subIdx = token.indexOf(sub);
    assert.notStrictEqual(subIdx, -1, `probe 'at' ${JSON.stringify(sub)} not inside token ${JSON.stringify(token)}`);
    const occurrence = content.slice(0, tokenIdx + subIdx).split(sub).length - 1;
    return locate(content, sub, occurrence);
}

/**
 * Position immediately AFTER the dot of `afterDot` (e.g. `self.`) within
 * `token` — the position a user's caret has when member completion fires.
 */
export function completionPosition(content: string, probe: CompletionProbe): vscode.Position {
    const tokenIdx = content.indexOf(probe.token);
    assert.notStrictEqual(tokenIdx, -1, `completion token ${JSON.stringify(probe.token)} not found — corpus drifted from pinned tree`);
    const dotIdx = probe.token.indexOf(probe.afterDot);
    assert.notStrictEqual(dotIdx, -1, `afterDot ${JSON.stringify(probe.afterDot)} not inside token ${JSON.stringify(probe.token)}`);
    return offsetToPosition(content, tokenIdx + dotIdx + probe.afterDot.length);
}
