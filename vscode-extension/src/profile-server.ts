// Implements [PROFILE-VIEWER-DELIVERY]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-VIEWER-DELIVERY
/**
 * Loopback HTTP server that makes exported profiles readable by speedscope.app.
 *
 * speedscope.app is served over https and can never read `file://` URLs, so a
 * `#profileURL=file://…` deep link always dead-ends on "Something went wrong".
 * Browsers do, however, treat loopback as a potentially-trustworthy origin an
 * https page may fetch from — so this module serves each exported profile over
 * `http://127.0.0.1:<port>/<token>/profile.json` and the speedscope deep link
 * loads it automatically instead of demanding a manual drag-and-drop.
 *
 * Containment: bound to 127.0.0.1 only, one unguessable token per registered
 * file, GET only, CORS open (speedscope's importer fetches cross-origin),
 * `no-store`, registrations expire so a forgotten server never keeps a stale
 * profile reachable, and the whole server tears down with the extension.
 */

import { randomBytes } from "node:crypto";
import { readFile } from "node:fs/promises";
import * as http from "node:http";
import { basename } from "node:path";
import { Logger } from "./logger";

/** How long a registered profile stays fetchable (10 minutes). */
const REGISTRATION_TTL_MS = 600_000;

/** Entropy (bytes) for the per-profile URL token. */
const TOKEN_BYTES = 16;

/** HTTP status codes used by the loopback server. */
const HTTP_OK = 200;
const HTTP_NOT_FOUND = 404;

/** A profile registered for one-token retrieval. */
interface Registration {
  readonly filePath: string;
  readonly expiresAt: number;
}

let server: http.Server | undefined;
let serverPort: number | undefined;
const registrations = new Map<string, Registration>();

/**
 * Register `filePath` with the loopback server and return the `http://127.0.0.1`
 * URL speedscope can fetch it from. Starts the server on first use.
 *
 * `ttlMs` is a test seam — production callers use the default.
 */
export async function serveProfileForBrowser(
  filePath: string,
  ttlMs: number = REGISTRATION_TTL_MS,
): Promise<string> {
  const port = await ensureServer();
  pruneExpired();
  const token = randomBytes(TOKEN_BYTES).toString("hex");
  registrations.set(token, { filePath, expiresAt: Date.now() + ttlMs });
  // The URL carries the real basename — speedscope falls back to
  // extension-based format detection (`.heapprofile` vs speedscope JSON).
  return `http://127.0.0.1:${port}/${token}/${encodeURIComponent(basename(filePath))}`;
}

/** Stop the server and forget every registration (extension teardown). */
export function disposeProfileServer(): void {
  server?.close();
  server = undefined;
  serverPort = undefined;
  registrations.clear();
}

/** Start the loopback server if needed and resolve its port. */
async function ensureServer(): Promise<number> {
  if (server !== undefined && serverPort !== undefined) {
    return serverPort;
  }
  return new Promise<number>((resolve, reject) => {
    const created = http.createServer((req, res) => {
      void handleRequest(req, res);
    });
    created.on("error", (err: Error) => {
      Logger.warn(`Profile loopback server error: ${err.message}`);
      reject(err);
    });
    // Port 0 = ephemeral; loopback only — never reachable off-machine.
    created.listen(0, "127.0.0.1", () => {
      const address = created.address();
      if (address === null || typeof address === "string") {
        created.close();
        reject(new Error("profile loopback server bound without a port"));
        return;
      }
      server = created;
      serverPort = address.port;
      resolve(address.port);
    });
  });
}

/** Serve a registered profile by token; anything else is a 404. */
async function handleRequest(req: http.IncomingMessage, res: http.ServerResponse): Promise<void> {
  const registration = req.method === "GET" ? lookupRegistration(req.url ?? "") : undefined;
  if (registration === undefined) {
    res.writeHead(HTTP_NOT_FOUND).end();
    return;
  }
  try {
    const body = await readFile(registration.filePath);
    res.writeHead(HTTP_OK, {
      "Content-Type": "application/json",
      // speedscope.app fetches cross-origin; the token is the access control.
      "Access-Control-Allow-Origin": "*",
      "Cache-Control": "no-store",
    });
    res.end(body);
  } catch (err: unknown) {
    Logger.warn(
      `Profile loopback server could not read ${registration.filePath}: ${err instanceof Error ? err.message : String(err)}`,
    );
    res.writeHead(HTTP_NOT_FOUND).end();
  }
}

/** Resolve a request path to its live registration, dropping expired ones. */
function lookupRegistration(url: string): Registration | undefined {
  const token = url.split("/").find((segment) => segment.length === TOKEN_BYTES * 2);
  if (token === undefined) {
    return undefined;
  }
  const registration = registrations.get(token);
  if (registration === undefined) {
    return undefined;
  }
  if (registration.expiresAt <= Date.now()) {
    registrations.delete(token);
    return undefined;
  }
  return registration;
}

/** Drop expired registrations so the map never grows unboundedly. */
function pruneExpired(): void {
  const now = Date.now();
  for (const [token, registration] of registrations) {
    if (registration.expiresAt <= now) {
      registrations.delete(token);
    }
  }
}
