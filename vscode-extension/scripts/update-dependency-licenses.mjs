// Generate the complete production npm license carrier shipped in the VSIX.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const extensionRoot = resolve(scriptDir, "..");
const repoRoot = resolve(extensionRoot, "..");
const outputPath = join(repoRoot, "VSCODE-DEPENDENCY-LICENSES");
const manifestPath = join(repoRoot, "vscode-license-manifest.json");
const packageLock = JSON.parse(readFileSync(join(extensionRoot, "package-lock.json"), "utf8"));

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function productionPackageDirs() {
  // On Windows npm is npm.cmd, which Node can only spawn through a shell
  // (spawning it directly fails ENOENT/EINVAL). The arguments are static, so
  // the shell path takes no untrusted input.
  const windows = process.platform === "win32";
  return execFileSync(windows ? "npm.cmd" : "npm", ["ls", "--omit=dev", "--parseable", "--all"], {
    cwd: extensionRoot,
    encoding: "utf8",
    shell: windows,
  })
    .split("\n")
    .map((entry) => entry.trim())
    .filter((entry) => entry && resolve(entry) !== extensionRoot)
    .sort();
}

function repositoryUrl(repository) {
  if (typeof repository === "string") return repository;
  return repository?.url ?? "unknown";
}

function legalSources(packageDir, packageName) {
  const names = readdirSync(packageDir)
    .filter((name) => /^(licen[cs]e|copying|notice|third[-_]?party[-_]?notices?)(?:[._-].*)?$/iu.test(name))
    .sort();
  if (names.length > 0) {
    return names.map((name) => ({ label: name, path: join(packageDir, name) }));
  }
  // Both Shipwright packages come from one repository and use its root MIT
  // license; shipwright-core 0.10.0 omitted the duplicate file from its tarball.
  if (packageName === "@nimblesite/shipwright-core") {
    const shared = join(extensionRoot, "node_modules", "@nimblesite", "shipwright-vscode", "LICENSE");
    if (existsSync(shared)) {
      return [{ label: "shared Shipwright repository LICENSE", path: shared }];
    }
  }
  throw new Error(`${packageName} has no packaged license or notice file`);
}

function componentFor(packageDir) {
  const metadata = JSON.parse(readFileSync(join(packageDir, "package.json"), "utf8"));
  if (!metadata.name || !metadata.version || !metadata.license) {
    throw new Error(`${packageDir}/package.json lacks name, version, or license`);
  }
  const lockKey = relative(extensionRoot, packageDir).replaceAll("\\", "/");
  const locked = packageLock.packages?.[lockKey];
  if (!locked || locked.version !== metadata.version || locked.license !== metadata.license) {
    throw new Error(`${metadata.name} installed metadata differs from package-lock.json`);
  }
  const files = legalSources(packageDir, metadata.name).map(({ label, path }) => {
    const bytes = readFileSync(path);
    return {
      label,
      source: relative(extensionRoot, path).replaceAll("\\", "/"),
      sha256: sha256(bytes),
      text: bytes.toString("utf8").trimEnd(),
    };
  });
  return {
    name: metadata.name,
    version: metadata.version,
    license: metadata.license,
    repository: repositoryUrl(metadata.repository),
    resolved: locked.resolved,
    integrity: locked.integrity,
    files,
  };
}

function generate() {
  const components = productionPackageDirs()
    .map(componentFor)
    .sort((left, right) => left.name.localeCompare(right.name) || left.version.localeCompare(right.version));
  const graph = components.map(({ name, version, license, resolved, integrity }) => ({
    name,
    version,
    license,
    resolved,
    integrity,
  }));
  const graphSha = sha256(`${JSON.stringify(graph)}\n`);
  const lines = [
    "Basilisk VS Code Production Dependency Licenses",
    "=================================================",
    "",
    "Generated from the exact npm production graph selected by package-lock.json.",
    `Production graph SHA-256: ${graphSha}`,
    "Regenerate with: npm run licenses:update",
    "",
  ];
  for (const component of components) {
    lines.push(
      "===============================================================================",
      `${component.name} ${component.version}`,
      `License: ${component.license}`,
      `Repository: ${component.repository}`,
    );
    for (const file of component.files) {
      lines.push(`Source: ${file.label}`, `SHA-256: ${file.sha256}`, "", file.text, "");
    }
  }
  const carrier = Buffer.from(`${lines.join("\n")}\n`);
  const manifest = Buffer.from(
    `${JSON.stringify(
      {
        carrier_sha256: sha256(carrier),
        dependencies: components.map(({ files, ...component }) => ({
          ...component,
          legal_files: files.map(({ text: _text, ...file }) => file),
        })),
        production_graph_sha256: graphSha,
      },
      null,
      2,
    )}\n`,
  );
  return { carrier, manifest };
}

function requireExact(path, expected) {
  if (!existsSync(path) || !readFileSync(path).equals(expected)) {
    throw new Error(`${relative(repoRoot, path)} is stale; run npm run licenses:update`);
  }
}

const generated = generate();
if (process.argv.includes("--check")) {
  requireExact(outputPath, generated.carrier);
  requireExact(manifestPath, generated.manifest);
  console.log("VS Code production dependency licenses are exact");
} else {
  writeFileSync(outputPath, generated.carrier);
  writeFileSync(manifestPath, generated.manifest);
  console.log(`Wrote ${relative(repoRoot, outputPath)} and ${relative(repoRoot, manifestPath)}`);
}
