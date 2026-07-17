// Implements [EXTACT-MODULES]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-MODULES
/**
 * Module Explorer wire types and row rendering.
 *
 * The [EXTACT-DATA-MODEL] client mirrors plus the pure description / tooltip /
 * icon-colour helpers for module and folder/package rows, split out of
 * `module-explorer.ts` so the provider file stays focused on tree mechanics.
 */

import * as vscode from "vscode";
import { type DiagnosticNode } from "./module-explorer-diagnostics";

// ── LSP response types ───────────────────────────────────────────────────
//
// Implements the client mirror of [EXTACT-DATA-MODEL] — the shared
// WorkspaceModulesResponse / ModuleNode / SymbolNode / DiagnosticNode /
// HealthStats wire shapes returned by basilisk.workspaceModules.

export interface SymbolNode {
  readonly name: string;
  readonly kind: "class" | "function" | "variable" | "constant" | "typeAlias";
  readonly line: number;
  readonly annotated: boolean;
  readonly exported: boolean;
  readonly children?: readonly SymbolNode[];
}

export interface ModuleNode {
  readonly name: string;
  readonly path: string;
  readonly kind: "package" | "module";
  readonly symbols: readonly SymbolNode[];
  // Every diagnostic for this module, rendered as the first drill-down rows so
  // the errors/warnings tallies below are navigable, never dead
  // ([EXTACT-MODULES-DIAGNOSTICS], #235). Empty while Type Checking is
  // disabled; optional only to stay crash-safe against a pre-#235 server
  // binary supplied via basilisk.executablePath.
  readonly diagnostics?: readonly DiagnosticNode[];
  // Health rollup folded into each module by basilisk.workspaceModules
  // [EXTACT-MODULES] — coverage %, diagnostic counts, and adoption state, so the
  // merged panel needs no separate basilisk.typeHealth round-trip. ABSENT while
  // Type Checking is disabled ([ANALYSIS-ENABLED], #119): the server omits all
  // grading, so there is nothing to render as "% typed" or a red tint.
  readonly coveragePercent?: number;
  // Raw symbol counts behind coveragePercent — the weights for the client-side
  // folder/package coverage rollup ([EXTACT-MODULES-TREE-STRUCTURE]), so folder
  // percentages aggregate exactly like the workspace header's "% typed".
  readonly totalSymbols?: number;
  readonly annotatedSymbols?: number;
  readonly errors?: number;
  readonly warnings?: number;
  readonly adopted?: boolean;
}

/** Workspace-wide health rollup carried alongside the module list. */
export interface HealthStats {
  // The Type Checking toggle state stamped by the server ([ANALYSIS-ENABLED],
  // #119). `false` means the grading fields below are absent by construction.
  readonly typeCheckingEnabled?: boolean;
  readonly totalSymbols?: number;
  readonly annotatedSymbols?: number;
  readonly coveragePercent?: number;
  readonly errors?: number;
  readonly warnings?: number;
  readonly adoptedFiles?: number;
  readonly totalFiles: number;
  // Whether the server's initial workspace scan has finished. A zero-file
  // rollup only means "empty workspace" when this is true; before that it
  // means "not scanned yet" ([EXTACT-MODULES-HEADER-LOADING], #144).
  readonly scanComplete?: boolean;
}

export interface WorkspaceModulesResponse {
  readonly modules: readonly ModuleNode[];
  readonly workspace: HealthStats;
}

/**
 * A node in the client-reconstructed package/folder tree
 * [EXTACT-MODULES-TREE-STRUCTURE] (#149). The LSP returns a *flat* list of
 * modules keyed by dotted name (e.g. `pkg.sub.mod`); the nested tree is rebuilt
 * here by splitting each name into path segments. Intermediate folders that are
 * not themselves Python packages are synthesised as container nodes with no
 * `module`, so the panel renders `pkg/ → sub/ → mod` instead of a flat list.
 */
export interface PackageTreeNode {
  /** Last path segment — the row's display label (e.g. `auth`). */
  readonly segment: string;
  /** Fully-qualified dotted prefix up to and including this node. */
  readonly fullName: string;
  /** The module/package file mapping exactly here, if one exists. */
  module?: ModuleNode;
  /** Child packages and modules, keyed by their segment. */
  readonly children: Map<string, PackageTreeNode>;
  // Diagnostics rolled up across this node's whole subtree (self module +
  // every descendant). Surfaced on the folder/package row so errors are
  // visible without drilling into the hierarchy (#149). Set by `rollup`.
  errors: number;
  warnings: number;
  // Symbol counts rolled up across the subtree — the weights for the folder's
  // coverage %, aggregated exactly like the workspace header. Set by `rollup`.
  totalSymbols: number;
  annotatedSymbols: number;
  /** Whether any module in the subtree carried grading (Type Checking on, #119). */
  graded: boolean;
}

// ── Coverage rendering [EXTACT-MODULES] ──────────────────────────────────

/** Width of the Unicode coverage bar in characters. */
const COVERAGE_BAR_WIDTH = 10;
/** Coverage threshold for "good" (green). */
const COVERAGE_GOOD_THRESHOLD = 90;
/** Coverage threshold for "warning" (yellow); below it is red. */
const COVERAGE_WARN_THRESHOLD = 50;
/** Neutral coverage for ungraded rows (Type Checking disabled, #119). */
export const FULL_COVERAGE_PERCENT = 100;
/** Scale factor from a 0–1 ratio to a percentage. */
const PERCENT_SCALE = 100;

/** Render a coverage progress bar using Unicode block characters. */
function coverageBar(percent: number): string {
  const filled = Math.round(percent / COVERAGE_BAR_WIDTH);
  return "█".repeat(filled) + "░".repeat(COVERAGE_BAR_WIDTH - filled);
}

/** Theme color for a coverage percentage: green >=90%, yellow >=50%, else red. */
export function coverageColor(percent: number): vscode.ThemeColor {
  if (percent >= COVERAGE_GOOD_THRESHOLD) { return new vscode.ThemeColor("testing.iconPassed"); }
  if (percent >= COVERAGE_WARN_THRESHOLD) { return new vscode.ThemeColor("list.warningForeground"); }
  return new vscode.ThemeColor("list.errorForeground");
}

// [EXTACT-MODULES-COUNT-STYLE] is the diagnostic-tally surface for module rows.
/** Module row description: coverage bar + % + error/warning counts + adopted badge. */
export function moduleDescription(module: ModuleNode): string {
  // Type Checking disabled (#119): the server serves no grading, so the row is
  // a plain navigation entry — no bar, no percentage, no tallies.
  if (module.coveragePercent === undefined) { return ""; }
  const issueTally = diagnosticTally(module.errors ?? 0, module.warnings ?? 0);
  const issueStr = issueTally === "" ? "" : ` — ${issueTally}`;
  const badge = module.adopted === true ? " [adopted]" : "";
  return `${coverageBar(module.coveragePercent)} ${module.coveragePercent}%${issueStr}${badge}`;
}

/** Module row tooltip: name + path + coverage + diagnostics + adoption. */
export function moduleTooltip(module: ModuleNode): string {
  return [
    module.name,
    module.path,
    module.coveragePercent !== undefined ? `Coverage: ${module.coveragePercent}%` : "",
    module.errors !== undefined ? `Errors: ${module.errors}` : "",
    module.warnings !== undefined ? `Warnings: ${module.warnings}` : "",
    module.adopted === true ? "Status: Adopted (errors demoted to warnings)" : "",
  ].filter(Boolean).join("\n");
}

/** Implements [EXTACT-MODULES-COUNT-STYLE]: coloured glyphs `🔴 n` (errors) /
 *  `🟠 n` (warnings) — never `nE nW`; a zero severity is omitted, or "" when clean. */
export function diagnosticTally(errors: number, warnings: number): string {
  const issues: string[] = [];
  if (errors > 0) { issues.push(`🔴 ${errors}`); }
  if (warnings > 0) { issues.push(`🟠 ${warnings}`); }
  return issues.join(" ");
}

/**
 * Subtree type coverage for a folder/package row: symbol-weighted across every
 * module beneath the node, exactly like the workspace header's "% typed"
 * ([EXTACT-MODULES-TREE-STRUCTURE]) — never an average of pre-divided child
 * percentages. `undefined` while ungraded (Type Checking disabled, #119) so no
 * folder renders a vacuous 100% conjured from zero data.
 */
function subtreeCoverage(node: PackageTreeNode): number | undefined {
  if (!node.graded) { return undefined; }
  if (node.totalSymbols === 0) { return FULL_COVERAGE_PERCENT; }
  return Math.round((node.annotatedSymbols / node.totalSymbols) * PERCENT_SCALE);
}

/**
 * Folder/package icon tint: red if the subtree holds any error, else yellow if
 * any warning, else the subtree's rolled-up coverage colour (an ungraded folder
 * stays untinted). Lets a folder with hidden errors read red without expanding (#149).
 */
export function packageIconColor(node: PackageTreeNode): vscode.ThemeColor | undefined {
  if (node.errors > 0) { return new vscode.ThemeColor("list.errorForeground"); }
  if (node.warnings > 0) { return new vscode.ThemeColor("list.warningForeground"); }
  // No coverage served (Type Checking disabled, #119) → untinted, like a folder.
  const coverage = subtreeCoverage(node);
  return coverage !== undefined ? coverageColor(coverage) : undefined;
}

/**
 * Folder/package row description: the subtree's rolled-up coverage bar + % and
 * count-style tally ([EXTACT-MODULES-COUNT-STYLE]) so type health and problems
 * are both visible without drilling in (#149).
 */
export function packageDescription(node: PackageTreeNode): string {
  const coverage = subtreeCoverage(node);
  const bar = coverage !== undefined ? `${coverageBar(coverage)} ${coverage}%` : "";
  return [bar, diagnosticTally(node.errors, node.warnings)].filter(Boolean).join(" — ");
}

/** Folder/package row tooltip: name + (package path) + subtree coverage/diagnostics. */
export function packageTooltip(node: PackageTreeNode): string {
  const errs = `${node.errors} error${node.errors === 1 ? "" : "s"}`;
  const warns = `${node.warnings} warning${node.warnings === 1 ? "" : "s"}`;
  const coverage = subtreeCoverage(node);
  return [
    node.fullName,
    node.module?.path,
    coverage !== undefined ? `Coverage: ${coverage}% (subtree)` : "",
    `Subtree: ${errs}, ${warns}`,
  ].filter(Boolean).join("\n");
}
