// Implements [WEBSITE-SCREENSHOTS] / [WEBSITE-SCREENSHOTS-PURPOSE]: the single,
// fully automated, reproducible command that produces the site's CLI screenshots —
// real `basilisk check --color always` output, PII-free, with a built-in guard
// that every snippet still triggers the diagnostic it documents.
// Implements [WEBSITE-SCREENSHOTS-GENERATE]: regenerate every CLI screenshot on
// the site from the real `basilisk` binary, with no manual Terminal/screencapture
// step. See docs/specs/WEBSITE-SCREENSHOTS-SPEC.md.
//
// For each shot it writes the snippet to a throwaway, neutrally-named directory
// (so diagnostic paths read `e0001.py:1:13` with no PII), runs
// `basilisk check --color always <file>` there, asserts the documented code
// actually fires, renders the output inside a macOS Terminal window via Playwright,
// and writes website/src/assets/images/<name>.png at 2× for crisp Retina display.
//
// Usage:  node screenshots/generate.mjs            (regenerate all)
//         node screenshots/generate.mjs e0001 e0012 (regenerate a subset)
//         BASILISK_BIN=../target/release/basilisk node screenshots/generate.mjs

import { chromium } from "@playwright/test";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { SHOTS } from "./shots.mjs";
import { buildTerminalHtml, WINDOW_SELECTOR } from "./terminal.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const OUTPUT_DIR = path.resolve(here, "../src/assets/images");
const BIN = process.env.BASILISK_BIN ?? "basilisk";
const SCALE = 2;

const stripAnsi = (text) => text.replace(/\x1b\[[0-9;]*m/g, "");

// Run `basilisk check` in `cwd`. A non-zero exit is expected whenever diagnostics
// are reported, so we read the captured stdout off the thrown error too.
const runChecker = (file, cwd) => {
  try {
    return execFileSync(BIN, ["check", "--color", "always", file], {
      cwd,
      encoding: "utf8",
      maxBuffer: 8 * 1024 * 1024,
    });
  } catch (error) {
    if (typeof error.stdout === "string" && error.stdout.length > 0) return error.stdout;
    throw new Error(`basilisk failed for ${file}: ${error.stderr || error.message}`);
  }
};

const captureShot = async (page, shot, workDir) => {
  fs.writeFileSync(path.join(workDir, shot.file), shot.code);
  const output = runChecker(shot.file, workDir);

  if (!stripAnsi(output).includes(shot.expect)) {
    throw new Error(
      `${shot.name}: expected "${shot.expect}" in output but it was absent — ` +
        `the snippet no longer triggers the documented diagnostic.\n${stripAnsi(output)}`,
    );
  }

  await page.setContent(
    buildTerminalHtml({ command: `basilisk check ${shot.file}`, ansiOutput: output }),
    { waitUntil: "load" },
  );
  const target = OUTPUT_DIR + path.sep + `${shot.name}.png`;
  await page.locator(WINDOW_SELECTOR).screenshot({ path: target });
  const kb = Math.round(fs.statSync(target).size / 1024);
  console.log(`  ✓ ${shot.name}.png  (${kb} KB)  [${shot.expect}]`);
};

const main = async () => {
  const requested = new Set(process.argv.slice(2));
  const shots = requested.size === 0 ? SHOTS : SHOTS.filter((s) => requested.has(s.name));
  if (shots.length === 0) throw new Error(`no shots matched: ${[...requested].join(", ")}`);

  fs.mkdirSync(OUTPUT_DIR, { recursive: true });
  const workDir = fs.mkdtempSync(path.join(os.tmpdir(), "basilisk-demo-"));
  console.log(`Generating ${shots.length} screenshot(s) → src/assets/images/`);

  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1200, height: 2400 }, deviceScaleFactor: SCALE });
  try {
    for (const shot of shots) await captureShot(page, shot, workDir);
  } finally {
    await browser.close();
    fs.rmSync(workDir, { recursive: true, force: true });
  }
  console.log("Done.");
};

main().catch((error) => {
  console.error(`screenshots: ${error.message}`);
  process.exit(1);
});
