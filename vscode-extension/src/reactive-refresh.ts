// Implements [EXTACT-REACTIVE-STATE]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-REACTIVE-STATE
/**
 * Shared reactive-refresh wiring for sidebar panels.
 *
 * Every panel is a pure projection of centralized store state: it subscribes to
 * a monotonic revision signal and re-renders on each bump, never owning a timer
 * or refreshing itself directly. The Modules panel keys off `analysisRevision`;
 * the Python Processes panel keys off `processesRevision` (whose visibility-gated
 * poll bumps the signal, since the OS has no push event for process changes,
 * issue #148). Both share the one subscription primitive below.
 */

import { effect, type ReadonlySignal } from "@preact/signals-core";
import type * as vscode from "vscode";

/** A panel that can re-render on demand and tracks its own disposables. */
export interface RefreshablePanel {
  refresh(): void;
  readonly disposables: vscode.Disposable[];
}

/**
 * Subscribe `panel` to a centralized revision signal, refreshing on each real
 * bump. The effect runs once on subscription; the captured baseline suppresses
 * that initial no-op so only genuine bumps trigger a refresh. The effect's
 * disposer is tracked on the panel so it is torn down with the panel.
 */
export function subscribeRevision(
  revision: ReadonlySignal<number>,
  panel: RefreshablePanel,
): void {
  let lastRevision = revision.value;
  const dispose = effect(() => {
    const next = revision.value;
    if (next !== lastRevision) {
      lastRevision = next;
      panel.refresh();
    }
  });
  panel.disposables.push({ dispose });
}
