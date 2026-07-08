// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
/**
 * Shared constants and type helpers for profiler test suites.
 *
 * Extracted to avoid duplication across profiler.test.ts,
 * profiler-decorations.test.ts, and profiler-memory-integration.test.ts.
 */

import * as assert from 'assert';
import * as vscode from 'vscode';
import { EXTENSION_ID } from './test-helpers';

/** Profiler client-side commands (registered in profiler.ts). */
export const PROFILER_CLIENT_COMMANDS = [
    'basilisk.profileStart',
    'basilisk.profileStop',
    'basilisk.profileSnapshot',
    'basilisk.profileAttachToDebug',
    'basilisk.profileShowResults',
] as const;

/** Memory profiler client-side commands (registered in memory-profiler.ts). */
export const MEMORY_CLIENT_COMMANDS = [
    'basilisk.memoryStart',
    'basilisk.memorySnapshot',
    'basilisk.memoryStop',
    'basilisk.memoryReferences',
] as const;

/** Profiler server-side commands (advertised by LSP). */
export const PROFILER_SERVER_COMMANDS = [
    'basilisk.profiler.start',
    'basilisk.profiler.stop',
    'basilisk.profiler.snapshot',
    'basilisk.profiler.list',
    'basilisk.profiler.cooperativeScript',
    'basilisk.profiler.cooperativeAttach',
] as const;

/** Profiler configuration keys. */
export const PROFILER_SETTINGS = [
    'basilisk.profiler.sampleRate',
    'basilisk.profiler.includeNative',
    'basilisk.profiler.lineThreshold',
    'basilisk.profiler.functionThreshold',
    'basilisk.profiler.maxDiagnosticsPerFile',
    'basilisk.profiler.showInlineHeatMap',
] as const;

/** Additional profiler configuration keys. */
export const PROFILER_EXTRA_SETTINGS = [
    'basilisk.profiler.profileOnLaunch',
    'basilisk.profiler.preset',
] as const;

/** Shape of a command entry in package.json contributes.commands. */
export interface PackageJsonCommandEntry {
    command: string;
    title?: string;
    category?: string;
    icon?: string;
}

/** Shape of a menu contribution entry in package.json contributes.menus. */
export interface PackageJsonMenuEntry {
    command: string;
    when?: string;
    group?: string;
}

/** Shape of a viewsWelcome entry in package.json contributes.viewsWelcome. */
export interface PackageJsonViewsWelcomeEntry {
    view: string;
    contents: string;
    when?: string;
}

/** Shape of a keybinding entry in package.json contributes.keybindings. */
export interface PackageJsonKeybinding {
    command: string;
    key?: string;
    when?: string;
}

/** Shape of package.json contributes.configuration.properties. */
export type PackageJsonProperties = Record<string, Record<string, unknown>>;

/** Shape of the packageJSON object exposed by the VS Code extension API. */
interface PackageJson {
    contributes?: {
        configuration?: {
            properties?: PackageJsonProperties;
        };
        commands?: PackageJsonCommandEntry[];
        keybindings?: PackageJsonKeybinding[];
        menus?: Record<string, PackageJsonMenuEntry[]>;
        viewsWelcome?: PackageJsonViewsWelcomeEntry[];
    };
}

/**
 * Retrieve the configuration properties from the extension's package.json.
 * Asserts the extension is found and returns the properties map.
 */
export function getPackageJsonProperties(): PackageJsonProperties {
    const extension = vscode.extensions.getExtension(EXTENSION_ID);
    assert.ok(extension, 'Extension should be found');
    const packageJson = extension.packageJSON as PackageJson;
    return packageJson.contributes?.configuration?.properties ?? {};
}

/**
 * Retrieve the command entries from the extension's package.json.
 * Asserts the extension is found and returns the commands array.
 */
export function getPackageJsonCommands(): PackageJsonCommandEntry[] {
    const extension = vscode.extensions.getExtension(EXTENSION_ID);
    assert.ok(extension, 'Extension should be found');
    const packageJson = extension.packageJSON as PackageJson;
    return packageJson.contributes?.commands ?? [];
}

/**
 * Retrieve a menu's contribution entries from the extension's package.json
 * (e.g. `view/title`, `view/item/context`).
 */
export function getPackageJsonMenu(menuId: string): PackageJsonMenuEntry[] {
    const extension = vscode.extensions.getExtension(EXTENSION_ID);
    assert.ok(extension, 'Extension should be found');
    const packageJson = extension.packageJSON as PackageJson;
    return packageJson.contributes?.menus?.[menuId] ?? [];
}

/**
 * Retrieve the viewsWelcome entries from the extension's package.json.
 */
export function getPackageJsonViewsWelcome(): PackageJsonViewsWelcomeEntry[] {
    const extension = vscode.extensions.getExtension(EXTENSION_ID);
    assert.ok(extension, 'Extension should be found');
    const packageJson = extension.packageJSON as PackageJson;
    return packageJson.contributes?.viewsWelcome ?? [];
}
