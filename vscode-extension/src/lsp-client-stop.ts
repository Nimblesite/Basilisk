// Implements [LSPARCH-CMDREG] client lifecycle — see docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CMDREG
/**
 * Shutting down a `LanguageClient` that may still be starting.
 *
 * `vscode-languageclient`'s `shutdown` rejects unless the state is exactly
 * `Running`, and `isRunning()` reports `false` for a client that is `Starting`
 * — one that has already spawned its server process. A shutdown path guarded
 * on `isRunning()` therefore has two failure modes on the same window:
 *
 *   - it calls `stop()` anyway and the rejection escapes (`deactivate()` threw
 *     "Client is not running and can't be stopped. It's current state is:
 *     starting"), or
 *   - it skips the client entirely and the server process it spawned outlives
 *     the client that owns it — the zombie publisher of GitHub #264, in the
 *     state where it is hardest to notice.
 *
 * `needsStop()` is the client's own name for "Starting or Running", and
 * `start()` returns the in-flight start promise rather than beginning a second
 * one, so a starting client can be settled and then shut down properly.
 *
 * The window is not theoretical: on win32 spawning the server binary is slow
 * enough that a deactivate/activate cycle routinely lands inside it, which is
 * what the Windows CI job reported ([VSIX-CI-PLATFORM-COVERAGE]).
 */

import type { LanguageClient } from "vscode-languageclient/node";
import { Logger } from "./logger";

/** How to tear the client down once its start has settled. */
export type StopMode = "stop" | "dispose";

/**
 * Shutdowns already in flight, keyed by client.
 *
 * `deactivate()` stops the client and then calls `store.reset()`, which also
 * wants it gone. Without this the second caller shuts down a client that the
 * first has already moved to `Stopping` and gets a rejection for its trouble.
 * A `WeakMap` keeps no client alive past its own lifetime.
 */
const inFlight = new WeakMap<LanguageClient, Promise<void>>();

/**
 * Stop a client that may still be starting, joining any shutdown already in
 * flight for it.
 *
 * Never rejects: a client that failed to start is already stopped, and a
 * shutdown error must not take down the deactivation around it.
 */
export async function stopClientSettled(
  client: LanguageClient,
  mode: StopMode = "stop",
): Promise<void> {
  // Everything up to the first await runs synchronously, so a second caller
  // in the same tick — store.reset() right behind deactivate() — always finds
  // the entry this call registers.
  const existing = inFlight.get(client);
  if (existing !== undefined) {
    return existing;
  }
  const shutdown = settleThenStop(client, mode).finally(() => {
    inFlight.delete(client);
  });
  inFlight.set(client, shutdown);
  return shutdown;
}

async function settleThenStop(client: LanguageClient, mode: StopMode): Promise<void> {
  if (!client.needsStop()) {
    return;
  }
  if (!client.isRunning() && !(await settleStart(client))) {
    return;
  }
  // The state can still have moved on while the start settled — an error
  // handler inside the client stops it on a failed handshake.
  if (!client.isRunning()) {
    return;
  }
  try {
    await (mode === "dispose" ? client.dispose() : client.stop());
  } catch (err: unknown) {
    Logger.warn(`Failed to ${mode} the LSP client: ${String(err)}`);
  }
}

/** Await an in-flight start. Returns false when it failed — nothing to stop. */
async function settleStart(client: LanguageClient): Promise<boolean> {
  try {
    await client.start();
    return true;
  } catch (err: unknown) {
    Logger.warn(`LSP client failed to start; nothing to shut down: ${String(err)}`);
    return false;
  }
}
