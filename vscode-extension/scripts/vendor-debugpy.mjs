// Vendor debugpy into the VSIX bundle so debugging works without the user
// installing debugpy into their interpreter. The version is the single source
// of truth in shipwright.json (the `debugpy` asset's `pip:debugpy==X.Y.Z`),
// so this never drifts from what verification expects.
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const extensionRoot = resolve(scriptDir, "..");
const repoRoot = resolve(extensionRoot, "..");

const manifest = JSON.parse(readFileSync(join(repoRoot, "shipwright.json"), "utf8"));
const component = manifest.components.find((entry) => entry.id === "debugpy");
if (!component) {
  throw new Error("shipwright.json has no 'debugpy' component to vendor");
}

const source = component.asset?.source ?? "";
const match = /^pip:(.+)$/.exec(source);
if (!match) {
  throw new Error(`debugpy asset.source must be 'pip:<spec>'; got: ${JSON.stringify(source)}`);
}
const spec = match[1];
const target = join(extensionRoot, component.bundled.bundlePath);
const defaultPython = process.platform === "win32" ? "python" : "python3";
const python = process.env.BASILISK_PYTHON || process.env.PYTHON || defaultPython;

console.log(`Vendoring ${spec} -> ${target} (using ${python})`);
rmSync(target, { recursive: true, force: true });
mkdirSync(target, { recursive: true });

execFileSync(
  python,
  ["-m", "pip", "install", "--no-compile", "--no-input", "--target", target, spec],
  { stdio: "inherit" }
);

if (!existsSync(join(target, "debugpy")) || readdirSync(target).length === 0) {
  throw new Error(`debugpy vendoring produced no files in ${target}`);
}
console.log("debugpy vendored.");
