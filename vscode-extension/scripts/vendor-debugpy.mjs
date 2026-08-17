// Vendor debugpy into the VSIX bundle so debugging works without the user
// installing debugpy into their interpreter. The version is the single source
// of truth in shipwright.json (the `debugpy` asset's `pip:debugpy==X.Y.Z`),
// so this never drifts from what verification expects.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
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
const expectedSha256 = component.asset?.sha256 ?? "";
if (component.asset?.contentHash !== true || !/^[0-9a-f]{64}$/.test(expectedSha256)) {
  throw new Error("debugpy asset must enable contentHash and declare a SHA-256");
}
const target = join(extensionRoot, component.bundled.bundlePath);
const defaultPython = process.platform === "win32" ? "python" : "python3";
const python = process.env.BASILISK_PYTHON || process.env.PYTHON || defaultPython;

console.log(`Vendoring pinned universal ${spec} -> ${target} (using ${python})`);
const download = mkdtempSync(join(tmpdir(), "basilisk-debugpy-"));
try {
  execFileSync(
    python,
    [
      "-m", "pip", "download", "--disable-pip-version-check", "--no-deps",
      "--only-binary=:all:", "--platform", "any", "--implementation", "py",
      "--python-version", "38", "--abi", "none", "--dest", download, spec,
    ],
    { stdio: "inherit" },
  );
  const wheels = readdirSync(download).filter((name) => name.endsWith("-none-any.whl"));
  if (wheels.length !== 1) {
    throw new Error(`expected one universal debugpy wheel, found ${wheels.length}`);
  }
  const wheel = join(download, wheels[0]);
  const actualSha256 = createHash("sha256").update(readFileSync(wheel)).digest("hex");
  if (actualSha256 !== expectedSha256) {
    throw new Error(`debugpy wheel SHA-256 mismatch: ${actualSha256}`);
  }
  rmSync(target, { recursive: true, force: true });
  mkdirSync(target, { recursive: true });
  execFileSync(
    python,
    ["-m", "pip", "install", "--no-compile", "--no-deps", "--no-index", "--target", target, wheel],
    { stdio: "inherit" },
  );
} finally {
  rmSync(download, { recursive: true, force: true });
}

if (!existsSync(join(target, "debugpy")) || readdirSync(target).length === 0) {
  throw new Error(`debugpy vendoring produced no files in ${target}`);
}
console.log("debugpy vendored.");
