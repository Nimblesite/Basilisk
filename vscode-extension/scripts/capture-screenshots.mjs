// Implements [VSIX-EDITOR-SCREENSHOTS]: one command to regenerate the website's
// VS Code editor screenshots. Stages the built binary into the dev extension,
// copies the shipwright manifest, launches the CDP screenshot sidecar, and runs
// the (otherwise-skipped) "Editor screenshots" suite headed with
// BASILISK_SCREENSHOTS=1 so the sidecar captures each feature.
//
// Prerequisite: the binaries must be built —
//   cargo build -p basilisk-cli -p basilisk-profiler-helper
//
// Usage (from vscode-extension/):  npm run screenshots:editor
// See docs/specs/VSIX-EDITOR-SCREENSHOTS-SPEC.md.

import { spawn, spawnSync } from "node:child_process";
import { copyFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const extensionRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(extensionRoot, "..");
const CDP_PORT = process.env.BASILISK_SCREENSHOT_CDP_PORT ?? "9229";

const run = (cmd, args, opts = {}) => {
  const result = spawnSync(cmd, args, { stdio: "inherit", cwd: repoRoot, ...opts });
  if (result.status !== 0) {
    throw new Error(`${cmd} ${args.join(" ")} exited with ${result.status ?? result.signal}`);
  }
};

// 1. Stage the runtime binaries into the dev extension's bin/<platform>/ (the
//    same path the packaged VSIX and the extension's shipwright resolver use).
if (!existsSync(join(repoRoot, "target", "debug", "basilisk"))) {
  throw new Error("missing target/debug/basilisk — run: cargo build -p basilisk-cli -p basilisk-profiler-helper");
}
run("node", [join(extensionRoot, "scripts", "stage-runtime.mjs"), "target/debug"]);

// 2. The extension reads shipwright.json from its own root; at dev time it only
//    lives at the repo root (packaging copies it in). Mirror it (gitignored).
copyFileSync(join(repoRoot, "shipwright.json"), join(extensionRoot, "shipwright.json"));

// 3. Compile the extension + tests.
run("npm", ["run", "compile"], { cwd: extensionRoot });

// 4. Launch the CDP screenshot sidecar.
const env = { ...process.env, BASILISK_SCREENSHOTS: "1", BASILISK_SCREENSHOT_CDP_PORT: CDP_PORT };
const watcher = spawn("node", [join(extensionRoot, "src", "test", "suite", "screenshot-watcher.mjs")], {
  stdio: "inherit",
  cwd: extensionRoot,
  env,
});

// 5. Run only the screenshot suite, headed, with the sidecar attached.
const test = spawnSync("npx", ["vscode-test", "--grep", "Editor screenshots"], {
  stdio: "inherit",
  cwd: extensionRoot,
  env,
});

watcher.kill();
process.exit(test.status ?? 1);
