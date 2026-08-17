// Implements [VSIX-ARCHITECTURE]. See docs/specs/VSIX-SPEC.md#VSIX-ARCHITECTURE
/**
 * LSP ready-handle machinery for the store: a per-start-cycle promise that
 * resolves when the client reaches Running, plus the awaited/polled path
 * `ensureLspReadyPromise` rides. Pure functions over the store's own signals
 * (the profiler-state.ts pattern) — the signals themselves stay owned by
 * store.ts, the single global-state file.
 */

import type { Signal } from "@preact/signals-core";
import type { LanguageClient } from "vscode-languageclient/node";
import type { Result } from "./result";
import { POLL_INTERVAL_MS } from "./timeouts";

/** LSP lifecycle states exposed to consumers. */
export type LspState = "idle" | "starting" | "running" | "stopped";

/** Lifecycle promise handle for LSP client ready signaling. */
export interface ReadyHandle {
  promise: Promise<void>;
  resolve: () => void;
}

/** The slice of the store's signals the ready machinery operates on. */
export interface ReadySignals {
  client: Signal<LanguageClient | undefined>;
  lspState: Signal<LspState>;
  readyHandle: Signal<ReadyHandle | undefined>;
}

/** Resolve the ready handle and clear it.
 *  Resolution MUST be async (next tick) so callers' .then() handlers
 *  are attached before the promise settles. */
export function resolveLspReady(signals: ReadySignals): void {
  const handle = signals.readyHandle.value;
  if (handle !== undefined) {
    signals.readyHandle.value = undefined;
    setTimeout(handle.resolve, 0);
  }
}

/** Placeholder resolver, replaced synchronously by the `Promise` executor. */
function unresolved(): void {
  // Intentionally empty — see `createReadyHandle`.
}

/** Create a fresh ready handle for this start cycle. */
export function createReadyHandle(signals: ReadySignals): ReadyHandle {
  // Seeded with a no-op so the binding is a `() => void` without asserting one.
  // The executor runs synchronously inside `new Promise`, so the real resolver
  // is always in place by the time the handle is built on the next line.
  let resolve: () => void = unresolved;
  const promise = new Promise<void>((settle) => { resolve = settle; });
  const handle: ReadyHandle = { promise, resolve };
  signals.readyHandle.value = handle;
  return handle;
}

/** Wait for the LSP ready handle with a timeout, returning Result. */
export async function awaitLspReady(
  signals: ReadySignals,
  timeoutMs: number,
): Promise<Result<LanguageClient>> {
  // Fast path: client already running.
  const client = signals.client.value;
  if (client?.isRunning() === true) {
    return { ok: true, value: client };
  }
  // Also check our own state signal (catches post-restart where isRunning()
  // lags behind the onDidChangeState callback that set lspState = "running").
  if (signals.lspState.value === "running" && client !== undefined) {
    return { ok: true, value: client };
  }

  const existing = signals.readyHandle.value;
  const ready = existing !== undefined ? existing.promise : createReadyHandle(signals).promise;

  // Poll for the client becoming ready via both isRunning() and our own
  // lspState signal. The double check catches cases where the readyHandle
  // was resolved before this function was called (e.g. after a deactivate/
  // activate cycle where the state listener already fired).
  const poll = new Promise<"poll">((resolve) => {
    const interval = setInterval(() => {
      const c = signals.client.value;
      if (c?.isRunning() === true || (signals.lspState.value === "running" && c !== undefined)) {
        clearInterval(interval);
        resolve("poll");
      }
    }, POLL_INTERVAL_MS);
    setTimeout(() => { clearInterval(interval); }, timeoutMs);
  });

  const timeout = new Promise<"timeout">((resolve) => {
    setTimeout(() => { resolve("timeout"); }, timeoutMs);
  });
  const outcome = await Promise.race([ready.then(() => "ready" as const), poll, timeout]);
  if (outcome === "timeout") {
    return { ok: false, error: new Error(`LSP client did not reach Running state within ${timeoutMs}ms`) };
  }
  const resolved = signals.client.value;
  if (resolved === undefined) {
    return { ok: false, error: new Error("LSP client resolved but is undefined") };
  }
  return { ok: true, value: resolved };
}
