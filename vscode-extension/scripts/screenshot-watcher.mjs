// Implements [VSIX-EDITOR-SCREENSHOTS-PIPELINE]: dependency-free sidecar that
// captures the real VS Code window for the website. Runs with the VSIX suite when
// BASILISK_SCREENSHOTS=1. The harness launches VS Code with
// --remote-debugging-port; this watcher speaks the Chrome DevTools Protocol over
// Node's built-in WebSocket (no Playwright / browser download), watches for
// `.signal` files written by takeWindowScreenshot() in screenshot.ts, captures
// the workbench page, and writes the PNG. See docs/specs/VSIX-EDITOR-SCREENSHOTS-SPEC.md.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// vscode-extension/scripts -> repo root is two levels up.
const repoRoot = path.resolve(__dirname, "../../");
const outputDir = path.resolve(repoRoot, "website/src/assets/images");
const CDP_PORT = Number.parseInt(process.env.BASILISK_SCREENSHOT_CDP_PORT ?? "9229", 10);
const POLL_MS = 200;
const TIMEOUT_MS = 600_000;

fs.mkdirSync(outputDir, { recursive: true });
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// Find the VS Code workbench page target and return its debugger WebSocket URL.
async function findWorkbenchSocket(deadline) {
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`http://127.0.0.1:${CDP_PORT}/json/list`);
      if (res.ok) {
        const targets = await res.json();
        const page = targets.find(
          (t) => t.type === "page" && /workbench\.(esm\.)?html|workbench\.html/.test(t.url ?? ""),
        ) ?? targets.find((t) => t.type === "page");
        if (page?.webSocketDebuggerUrl) return page.webSocketDebuggerUrl;
      }
    } catch {
      /* endpoint not up yet */
    }
    await sleep(500);
  }
  throw new Error(`VS Code workbench page not found on CDP port ${CDP_PORT}`);
}

// Minimal CDP client over the built-in WebSocket: correlates responses by id.
function connect(wsUrl) {
  const ws = new WebSocket(wsUrl);
  const pending = new Map();
  let nextId = 1;
  ws.addEventListener("message", (event) => {
    const msg = JSON.parse(typeof event.data === "string" ? event.data : event.data.toString());
    if (msg.id !== undefined && pending.has(msg.id)) {
      const { resolve, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result);
    }
  });
  const ready = new Promise((resolve, reject) => {
    ws.addEventListener("open", () => resolve());
    ws.addEventListener("error", () => reject(new Error("CDP socket error")));
  });
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = nextId++;
      pending.set(id, { resolve, reject });
      ws.send(JSON.stringify({ id, method, params }));
    });
  return { ws, ready, send };
}

async function main() {
  console.log(`[screenshots] waiting for VS Code CDP on port ${CDP_PORT}...`);
  const wsUrl = await findWorkbenchSocket(Date.now() + 120_000);
  const { ws, ready, send } = connect(wsUrl);
  await ready;
  await send("Page.enable");
  // Force a uniform, Retina-crisp viewport so every screenshot is the same size
  // regardless of the headed window's actual dimensions.
  await send("Emulation.setDeviceMetricsOverride", {
    width: 1440,
    height: 900,
    deviceScaleFactor: 2,
    mobile: false,
  });
  await sleep(800);
  console.log("[screenshots] connected; watching for signal files...");

  const start = Date.now();
  while (Date.now() - start < TIMEOUT_MS) {
    for (const signal of fs.readdirSync(outputDir).filter((f) => f.endsWith(".signal"))) {
      const signalPath = path.join(outputDir, signal);
      const requested = fs.readFileSync(signalPath, "utf8").trim();
      const tempPath = path.join(outputDir, signal.replace(/\.signal$/, ""));
      const { data } = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
      fs.writeFileSync(tempPath, Buffer.from(data, "base64"));
      fs.unlinkSync(signalPath);
      console.log(`[screenshots] ${requested} (${Math.round(fs.statSync(tempPath).size / 1024)}KB)`);
    }
    await sleep(POLL_MS);
  }
  ws.close();
}

main().catch((error) => {
  console.error("[screenshots] failed:", error.message);
  process.exit(1);
});
