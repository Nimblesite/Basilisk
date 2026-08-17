// Tests for [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
//
// Regression for issue #71: the e2e suite must run against the REAL release
// bundle. Every component shipwright declares `bundled` for this platform must
// be present in the extension-under-test. If the packaging process omits one
// (e.g. debugpy, or the profiler helper), this test fails — so a broken bundle
// can no longer pass tests while shipping a debugger-less VSIX.
import * as assert from 'assert';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

import { EXTENSION_ID } from './test-helpers';
import {
    recordArrayField,
    recordField,
    stringArrayField,
    stringField,
} from '../../unknown-shape';

// shipwright.json is bytes read off disk, so every field below is narrowed
// rather than asserted into a `Manifest` shape nothing has checked: a manifest
// that drifts must fail this test's assertion, not type-error past it.

/** One shipwright component, narrowed from the parsed manifest. */
interface Component {
    readonly id: string;
    readonly kind: string | undefined;
    readonly binaryName: string | undefined;
    readonly platforms: readonly string[] | undefined;
    readonly bundlePath: string | undefined;
}

/** Read the manifest's `components`, keeping only what this test reads. */
function componentsOf(manifest: unknown): Component[] {
    return recordArrayField(manifest, 'components').map((component) => ({
        id: stringField(component, 'id') ?? '',
        kind: stringField(component, 'kind'),
        binaryName: stringField(component, 'binaryName'),
        platforms:
            'platforms' in component ? stringArrayField(component, 'platforms') : undefined,
        bundlePath: stringField(recordField(component, 'bundled'), 'bundlePath'),
    }));
}

/** The release target triple for the host, matching shipwright `${platform}`. */
function currentTarget(): string {
    const arch = process.arch === 'arm64' ? 'arm64' : 'x64';
    if (process.platform === 'darwin') {
        return `darwin-${arch}`;
    }
    if (process.platform === 'linux') {
        return `linux-${arch}`;
    }
    if (process.platform === 'win32') {
        return `win32-${arch}`;
    }
    throw new Error(`unsupported platform: ${process.platform}`);
}

function supportsPlatform(component: Component, target: string): boolean {
    return (
        component.platforms === undefined ||
        component.platforms.includes(target) ||
        component.platforms.includes('all')
    );
}

/** Substitute shipwright `${...}` placeholders without relying on replaceAll. */
function fill(template: string, vars: Record<string, string>): string {
    return Object.entries(vars).reduce(
        (acc, [key, value]) => acc.split(`\${${key}}`).join(value),
        template
    );
}

// Tests [VSIX-BINARY-DISTRIBUTION] / [VSIX-PACKAGING-PARITY]: the VSIX under test
// bundles the per-platform `basilisk` binary (and every other shipwright-declared
// component, e.g. debugpy) the manifest requires for this platform.
suite('Bundle integrity (#71)', () => {
    test('extension-under-test bundles every shipwright-declared component for this platform', () => {
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, `extension ${EXTENSION_ID} not found`);
        const root = ext.extensionPath;

        const manifestPath = path.join(root, 'shipwright.json');
        assert.ok(fs.existsSync(manifestPath), `shipwright.json missing at ${manifestPath}`);
        const manifest: unknown = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));

        const target = currentTarget();
        const exe = target.startsWith('win32-') ? '.exe' : '';

        const missing: string[] = [];
        for (const component of componentsOf(manifest)) {
            if (component.bundlePath === undefined || !supportsPlatform(component, target)) {
                continue;
            }
            const rel = fill(component.bundlePath, {
                platform: target,
                binaryName: component.binaryName ?? '',
                exe,
            });
            const abs = path.join(root, rel);
            if (component.binaryName !== undefined && component.binaryName !== '') {
                if (!fs.existsSync(abs)) {
                    missing.push(`binary ${component.id} (${rel})`);
                }
            } else if (component.kind === 'asset') {
                const present = fs.existsSync(abs) && fs.readdirSync(abs).length > 0;
                if (!present) {
                    missing.push(`asset ${component.id} (${rel}/)`);
                }
            }
        }

        assert.deepStrictEqual(
            missing,
            [],
            `e2e tests are running against an incomplete bundle — these release ` +
                `components are missing (run the suite against a bundle built by the ` +
                `real packaging process): ${missing.join(', ')}`
        );
    });
});
