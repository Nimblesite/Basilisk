import { copyFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const extensionRoot = resolve(scriptDir, "..");
const repoRoot = resolve(extensionRoot, "..");
const source = join(repoRoot, "shipwright.json");
const target = join(extensionRoot, "shipwright.json");

if (!existsSync(source)) {
  throw new Error(`Missing Shipwright manifest: ${source}`);
}

copyFileSync(source, target);
