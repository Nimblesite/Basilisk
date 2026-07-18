import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, readFileSync } from "node:fs";
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
const attributionFiles = ["LICENSE", "NOTICES", "THIRD-PARTY-LICENSES"];

function stageAttribution() {
  for (const file of attributionFiles) {
    copyFileSync(join(repoRoot, file), join(extensionRoot, file));
  }
  console.log("Exact root attribution staged for VSIX packaging");
}

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
  // Line 1 is the Shipwright contract: `<component-id> <semver>`, exactly.
  // Later lines list embedded engines (e.g. `Ruff formatter: 0.15.17`,
  // [LSPFMT-PROVENANCE]) and must keep the strict `<label>: <value>` shape —
  // the machine contract stays single-line-parseable.
  const [contractLine, ...engineLines] = plain.split("\n");
  if (contractLine !== `${expectedName} ${expectedVersion}`) {
    throw new Error(`${binary} plain version mismatch: ${plain}`);
  }
  for (const line of engineLines) {
    if (!/^[A-Za-z][A-Za-z0-9 _-]*: \S+$/.test(line)) {
      throw new Error(`${binary} malformed engine line in --version output: ${line}`);
    }
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
  verifyAttribution(vsix, entries);
  // Every component declared `bundled` for this platform MUST be present in the
  // VSIX — including optional (`required: false`) ones such as the profiler
  // helper. `required` governs runtime fallback, NOT whether we ship the
  // artifact: a declared-but-missing binary used to pass verification, letting a
  // broken bundle ship green.
  const expected = bundledEntries(manifest, platform, false);
  for (const entry of expected) {
    if (!entries.includes(entry)) {
      throw new Error(`${vsix} is missing bundled component: ${entry}`);
    }
  }
  // Asset components (e.g. the bundled debugpy) ship a directory tree, not a
  // single binary — assert at least one packaged file lives under each.
  for (const prefix of bundledAssetPrefixes(manifest, platform)) {
    if (!entries.some((entry) => entry.startsWith(prefix))) {
      throw new Error(`${vsix} is missing bundled asset: ${prefix}`);
    }
  }
  rejectOtherPlatformBins(vsix, entries, platform);
  rejectUnmanifestedTargetBins(vsix, entries, manifest, platform);
  console.log(`${vsix}: package contents valid for ${platform}`);
}

// [STUBRES-TYPESHED-LICENSE] A package that embeds Typeshed content must ship
// the exact Basilisk and third-party legal files used to build it. Presence is
// insufficient: stale attribution must fail the release gate too.
function verifyAttribution(vsix, entries) {
  // `vsce` deliberately normalizes the extension's root LICENSE to
  // `LICENSE.txt`. Keep the source-to-package mapping explicit so release
  // verification checks the path that actually ships as well as exact bytes.
  for (const sourceFile of attributionFiles) {
    const packagedFile = sourceFile === "LICENSE" ? "LICENSE.txt" : sourceFile;
    const entry = `extension/${packagedFile}`;
    if (!entries.includes(entry)) {
      throw new Error(`${vsix} is missing attribution file: ${entry}`);
    }
    const packaged = execFileSync("unzip", ["-p", vsix, entry]);
    const source = readFileSync(join(repoRoot, sourceFile));
    if (!packaged.equals(source)) {
      throw new Error(`${vsix} contains a stale or modified attribution file: ${entry}`);
    }
  }
}

/// Packaged-path prefixes for `asset` components bundled on this platform.
function bundledAssetPrefixes(manifest, platform) {
  return manifest.components
    .filter((component) => component.kind === "asset" && component.bundled?.bundlePath)
    .filter((component) => supportsPlatform(component, platform))
    .map(
      (component) =>
        `extension/${component.bundled.bundlePath.replaceAll("${platform}", platform)}/`
    );
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
} else if (command === "stage-attribution") {
  stageAttribution();
} else if (command === "versions") {
  verifyVersions(...process.argv.slice(3));
} else if (command === "vsix") {
  verifyVsix(resolve(process.argv[3] ?? ""), process.argv[4] ?? "");
} else {
  console.error("Usage: node scripts/verify-shipwright.mjs manifest|stage-attribution|versions <binaries...>|vsix <file> <platform>");
  process.exit(2);
}
