// Stage every shipwright-declared runtime *binary* for a platform into the
// extension's `bin/<platform>/`, copying from a build directory.
//
// This is the SINGLE source of truth for which binaries the VSIX bundles — used
// by the e2e test harness (`_test_vsix`), the release packager (`_release_vsix`),
// AND the release.yml `vsix` job. Keeping one path is what stops the tests from
// validating a different bundle than what ships (issue #71). Asset components
// (e.g. debugpy) are vendored separately by `vendor-debugpy.mjs`.
// Implements [VSIX-PACKAGING-PARITY].
//
// Usage: node scripts/stage-runtime.mjs <build-dir> [platform]
import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const extensionRoot = resolve(scriptDir, "..");
const repoRoot = resolve(extensionRoot, "..");

/** Kinds that carry a `binaryName` (per the shipwright schema). */
const BINARY_KINDS = new Set(["cli", "lsp", "mcp", "sidecar", "dap", "tool"]);

function detectPlatform() {
  const arch = process.arch === "arm64" ? "arm64" : "x64";
  if (process.platform === "darwin") return `darwin-${arch}`;
  if (process.platform === "linux") return `linux-${arch}`;
  if (process.platform === "win32") return `win32-${arch}`;
  throw new Error(`Unsupported platform: ${process.platform}-${process.arch}`);
}

function supportsPlatform(component, platform) {
  return (
    component.platforms === undefined ||
    component.platforms.includes(platform) ||
    component.platforms.includes("all")
  );
}

const buildDirArg = process.argv[2];
if (!buildDirArg) {
  console.error("Usage: node scripts/stage-runtime.mjs <build-dir> [platform]");
  process.exit(2);
}
const buildDir = resolve(process.cwd(), buildDirArg);
const platform = process.argv[3] ?? detectPlatform();
const exe = platform.startsWith("win32-") ? ".exe" : "";

const manifest = JSON.parse(readFileSync(join(repoRoot, "shipwright.json"), "utf8"));

const binRoot = join(extensionRoot, "bin");
rmSync(binRoot, { recursive: true, force: true });
const platformDir = join(binRoot, platform);
mkdirSync(platformDir, { recursive: true });

const staged = [];
for (const component of manifest.components) {
  if (!component.bundled || !component.binaryName) continue;
  if (!BINARY_KINDS.has(component.kind)) continue;
  if (!supportsPlatform(component, platform)) continue;

  const file = `${component.binaryName}${exe}`;
  const source = join(buildDir, file);
  if (!existsSync(source)) {
    throw new Error(
      `stage-runtime: built binary for component '${component.id}' not found: ${source}`
    );
  }
  const dest = join(platformDir, file);
  copyFileSync(source, dest);
  if (process.platform !== "win32") {
    chmodSync(dest, 0o755);
  }
  staged.push(dest);
}

console.log(`Staged ${staged.length} runtime binary/binaries for ${platform}:`);
for (const path of staged) {
  console.log(`  ${path}`);
}
