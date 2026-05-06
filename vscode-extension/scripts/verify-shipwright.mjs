import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const extensionRoot = resolve(scriptDir, "..");
const repoRoot = resolve(extensionRoot, "..");
const manifestPath = join(repoRoot, "shipwright.json");
const manifestSchemaPath = join(repoRoot, "schemas", "shipwright.schema.json");
const versionSchemaPath = join(repoRoot, "schemas", "version-manifest.schema.json");

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}

function validateJson(schemaPath, dataPath, label) {
  const ajv = new Ajv2020({ allErrors: true, strict: false });
  addFormats(ajv);
  const validate = ajv.compile(readJson(schemaPath));
  const data = readJson(dataPath);
  if (!validate(data)) {
    const errors = validate.errors?.map((error) => `${error.instancePath || "/"} ${error.message}`).join("\n");
    throw new Error(`${label} failed validation:\n${errors}`);
  }
  console.log(`${label}: valid`);
}

function verifyManifest() {
  validateJson(manifestSchemaPath, manifestPath, "shipwright.json");
}

function parseVersionJson(binary) {
  const stdout = execFileSync(binary, ["--version", "--json"], { encoding: "utf8", timeout: 5000 });
  return JSON.parse(stdout);
}

function verifyBinary(binary, expectedName, expectedVersion) {
  const plain = execFileSync(binary, ["--version"], { encoding: "utf8", timeout: 5000 }).trim();
  if (plain !== `${expectedName} ${expectedVersion}`) {
    throw new Error(`${binary} plain version mismatch: ${plain}`);
  }

  const data = parseVersionJson(binary);
  const ajv = new Ajv2020({ allErrors: true, strict: false });
  addFormats(ajv);
  const validate = ajv.compile(readJson(versionSchemaPath));
  if (!validate(data)) {
    const errors = validate.errors?.map((error) => `${error.instancePath || "/"} ${error.message}`).join("\n");
    throw new Error(`${binary} JSON version failed validation:\n${errors}`);
  }
  if (data.name !== expectedName || data.version !== expectedVersion) {
    throw new Error(`${binary} JSON version mismatch: ${JSON.stringify(data)}`);
  }
  console.log(`${binary}: version contract valid`);
}

function verifyVersions(...binaries) {
  const manifest = readJson(manifestPath);
  const expectedVersion = manifest.product.version;
  for (const binary of binaries) {
    const name = binary.endsWith("basilisk-profiler-helper") ? "basilisk-profiler-helper" : "basilisk";
    verifyBinary(binary, name, expectedVersion);
  }
}

function listVsix(vsix) {
  return execFileSync("unzip", ["-Z1", vsix], { encoding: "utf8" })
    .split("\n")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

function verifyVsix(vsix, platform) {
  if (!existsSync(vsix)) {
    throw new Error(`VSIX does not exist: ${vsix}`);
  }
  const entries = listVsix(vsix);
  const packagedManifestEntry = "extension/shipwright.json";
  if (!entries.includes(packagedManifestEntry)) {
    throw new Error(`${vsix} is missing ${packagedManifestEntry}`);
  }
  const packagedManifest = JSON.parse(
    execFileSync("unzip", ["-p", vsix, packagedManifestEntry], { encoding: "utf8" })
  );
  const manifest = readJson(manifestPath);
  if (JSON.stringify(packagedManifest) !== JSON.stringify(manifest)) {
    throw new Error(`${vsix} contains a shipwright.json that differs from the repo manifest`);
  }
  const required = bundledEntries(manifest, platform, true);
  for (const entry of required) {
    if (!entries.includes(entry)) {
      throw new Error(`${vsix} is missing ${entry}`);
    }
  }
  rejectOtherPlatformBins(vsix, entries, platform);
  rejectUnmanifestedTargetBins(vsix, entries, manifest, platform);
  console.log(`${vsix}: package contents valid for ${platform}`);
}

function bundledEntries(manifest, platform, requiredOnly) {
  return manifest.components
    .filter((component) => component.bundled && component.binaryName)
    .filter((component) => supportsPlatform(component, platform))
    .filter((component) => !requiredOnly || component.required !== false)
    .map((component) => bundledEntry(component, platform));
}

function bundledEntry(component, platform) {
  const exe = platform.startsWith("win32-") ? ".exe" : "";
  const relative = component.bundled.bundlePath
    .replaceAll("${platform}", platform)
    .replaceAll("${binaryName}", component.binaryName)
    .replaceAll("${exe}", exe);
  return `extension/${relative}`;
}

function supportsPlatform(component, platform) {
  return (
    component.platforms === undefined ||
    component.platforms.includes(platform) ||
    component.platforms.includes("all")
  );
}

function rejectOtherPlatformBins(vsix, entries, platform) {
  const targetPrefix = `extension/bin/${platform}/`;
  const anyBinPrefix = "extension/bin/";
  for (const entry of entries) {
    if (entry.startsWith(anyBinPrefix) && !entry.startsWith(targetPrefix)) {
      throw new Error(`${vsix} includes another platform runtime path: ${entry}`);
    }
  }
}

function rejectUnmanifestedTargetBins(vsix, entries, manifest, platform) {
  const targetPrefix = `extension/bin/${platform}/`;
  const allowed = new Set(bundledEntries(manifest, platform, false));
  for (const entry of entries) {
    if (entry.startsWith(targetPrefix) && !allowed.has(entry)) {
      throw new Error(`${vsix} includes unmanifested runtime binary: ${entry}`);
    }
  }
}

const command = process.argv[2];
if (command === "manifest") {
  verifyManifest();
} else if (command === "versions") {
  verifyVersions(...process.argv.slice(3));
} else if (command === "vsix") {
  verifyVsix(resolve(process.argv[3] ?? ""), process.argv[4] ?? "");
} else {
  console.error("Usage: node scripts/verify-shipwright.mjs manifest|versions <binaries...>|vsix <file> <platform>");
  process.exit(2);
}
