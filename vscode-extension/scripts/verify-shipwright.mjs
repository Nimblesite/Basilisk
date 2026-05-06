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
  return execFileSync("unzip", ["-l", vsix], { encoding: "utf8" });
}

function verifyVsix(vsix, platform) {
  if (!existsSync(vsix)) {
    throw new Error(`VSIX does not exist: ${vsix}`);
  }
  const listing = listVsix(vsix);
  const exe = platform.startsWith("win32-") ? ".exe" : "";
  const required = `extension/bin/${platform}/basilisk${exe}`;
  if (!listing.includes(required)) {
    throw new Error(`${vsix} is missing ${required}`);
  }
  if (!listing.includes("extension/shipwright.json")) {
    throw new Error(`${vsix} is missing extension/shipwright.json`);
  }
  console.log(`${vsix}: package contents valid for ${platform}`);
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
