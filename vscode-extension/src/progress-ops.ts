// Implements [PROFILE-UX-PROGRESS]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-UX-PROGRESS
/**
 * Centralized user-visible progress for long-running profiler operations.
 *
 * Every multi-second profiling flow (CPU start/stop, memory operations,
 * panel refresh) runs through these wrappers so notification styling,
 * structured logging, and testability stay uniform. VS Code's progress UI is
 * not readable via the public API, so the wrappers also record a
 * begin/step/end **operation log** — the e2e seam tests assert against.
 */

import * as vscode from "vscode";
import { Logger } from "./logger";

/** Cap so a long session cannot grow the log unbounded. */
const OPERATION_LOG_CAP = 500;

/** Chronological `begin:`/`step:`/`end:`-prefixed operation entries. */
const operationLog: string[] = [];

function record(entry: string): void {
  operationLog.push(entry);
  if (operationLog.length > OPERATION_LOG_CAP) {
    operationLog.splice(0, operationLog.length - OPERATION_LOG_CAP);
  }
}

/** The operation log (e2e seam): `begin:<title>`, `step:<title>:<msg>`, `end:<title>`. */
export function recordedOperations(): readonly string[] {
  return operationLog;
}

/**
 * Run `task` under a user-facing progress notification. The task receives a
 * `report` callback for live stage messages ("Attaching…", "Collecting
 * results…"). The notification closes when the task settles — including on
 * throw, so a failed flow never leaves a zombie spinner.
 */
export async function withUserProgress<T>(
  title: string,
  task: (report: (message: string) => void) => Promise<T>,
): Promise<T> {
  record(`begin:${title}`);
  Logger.info(`[Progress] begin: ${title}`);
  try {
    return await vscode.window.withProgress(
      { location: vscode.ProgressLocation.Notification, title },
      async (progress) =>
        task((message) => {
          record(`step:${title}:${message}`);
          Logger.debug(`[Progress] ${title} — ${message}`);
          progress.report({ message });
        }),
    );
  } finally {
    record(`end:${title}`);
    Logger.info(`[Progress] end: ${title}`);
  }
}

/**
 * Run `task` under a tree view's built-in progress bar (the thin indeterminate
 * bar atop the view) — the right surface for an in-panel refresh, where a
 * notification would be heavyweight.
 */
export async function withViewProgress<T>(
  viewId: string,
  title: string,
  task: () => Promise<T>,
): Promise<T> {
  record(`begin:${title}`);
  try {
    return await vscode.window.withProgress({ location: { viewId } }, task);
  } finally {
    record(`end:${title}`);
  }
}
