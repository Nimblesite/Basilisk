import { chmodSync, copyFileSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const extensionRoot = resolve(scriptDir, "..");
const repoRoot = resolve(extensionRoot, "..");
const executableBit = 0o755;

function usage() {
  console.error("Usage: node scripts/stage-bundled-binary.mjs <binary-path> [platform]");
}

function detectPlatform() {
  if (process.platform === "darwin" && process.arch === "arm64") return "darwin-arm64";
  if (process.platform === "darwin") return "darwin-x64";
  if (process.platform === "linux" && process.arch === "arm64") return "linux-arm64";
  if (process.platform === "linux") return "linux-x64";
  if (process.platform === "win32" && process.arch === "arm64") return "win32-arm64";
  if (process.platform === "win32") return "win32-x64";
  throw new Error(`Unsupported platform: ${process.platform}-${process.arch}`);
}

function stage(binaryPath, platform) {
  const binRoot = join(extensionRoot, "bin");
  const platformDir = join(binRoot, platform);
  rmSync(binRoot, { recursive: true, force: true });
  mkdirSync(platformDir, { recursive: true });
  const targetPath = join(platformDir, basename(binaryPath));
  copyFileSync(binaryPath, targetPath);
  if (process.platform !== "win32") {
    chmodSync(targetPath, executableBit);
  }
  return targetPath;
}

const binaryArg = process.argv[2];
if (!binaryArg) {
  usage();
  process.exit(2);
}

const binaryPath = resolve(repoRoot, binaryArg);
if (!existsSync(binaryPath)) {
  throw new Error(`Binary does not exist: ${binaryPath}`);
}

const staged = stage(binaryPath, process.argv[3] ?? detectPlatform());
console.log(staged);
