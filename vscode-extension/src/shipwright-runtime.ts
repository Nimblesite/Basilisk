// Implements [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { Logger } from "./logger";

const SHIPWRIGHT_PACKAGE = "@nimblesite/shipwright-vscode";
const BASILISK_COMPONENT_ID = "basilisk";

interface ShipwrightApi {
  activateShipwright?: ActivateRuntime;
  activateDeploymentToolkit?: ActivateRuntime;
  detectPlatform?: (platform: NodeJS.Platform, arch: string) => string;
  probeBinaryVersion?: (file: string) => Promise<{ name: string; version: string } | undefined>;
}

type ActivateRuntime = (
  context: vscode.ExtensionContext,
  options: {
    readonly vscode: typeof vscode;
    readonly manifestPath: string;
    readonly showMessages?: boolean;
  }
) => Promise<ActivationResult>;

interface ActivationResult {
  readonly diagnostics: readonly ActivationDiagnostic[];
  readonly ok: boolean;
}

interface ActivationDiagnostic {
  readonly blocking: boolean;
  readonly componentId: string;
  readonly message: string;
  readonly resolution: RuntimeResolution;
}

interface RuntimeResolution {
  readonly path?: string | null;
  readonly source: string | null;
  readonly version?: string | null;
}

export interface BasiliskRuntime {
  readonly componentId: string;
  readonly executablePath: string;
  readonly source: string;
  readonly version: string | undefined;
}

// Implements [VSIX-BINARY-RESOLUTION] / [VSIX-BINARY-DISTRIBUTION] — binary
// resolution is delegated to Shipwright (the resolution cascade is the single
// source of truth in LSP-ARCHITECTURE-SPEC.md#LSPARCH-BINRES), which selects the
// per-platform bundled `basilisk` binary from the VSIX (or an override) and
// returns its executable path.
export async function resolveBasiliskRuntime(context: vscode.ExtensionContext): Promise<BasiliskRuntime> {
  const api = await loadShipwrightApi();
  const activate = api.activateShipwright ?? api.activateDeploymentToolkit;
  if (activate === undefined) {
    throw new Error(`${SHIPWRIGHT_PACKAGE} does not export a VS Code activation function.`);
  }
  // showMessages: false — Shipwright AWAITS its error toast's action buttons,
  // which blocks activation forever in headless hosts (e2e tests) and stalls
  // real users behind a modal-ish prompt. Failures are surfaced by our own
  // non-blocking reportRuntimeFailure path instead.
  const result = await activate(context, {
    vscode,
    manifestPath: path.join(context.extensionPath, "shipwright.json"),
    showMessages: false,
  });
  const diagnostic = basiliskDiagnostic(result);
  if (!result.ok) {
    const fallback = await bundledFallback(api, context);
    if (fallback !== undefined) {
      return fallback;
    }
    throw new Error(formatActivationFailure(result));
  }
  if (diagnostic === undefined) {
    throw new Error("Shipwright did not return a basilisk runtime diagnostic.");
  }
  const executablePath = diagnostic.resolution.path;
  if (executablePath === null || executablePath === undefined || executablePath === "") {
    throw new Error(`Shipwright resolved ${BASILISK_COMPONENT_ID} without an executable path.`);
  }
  return {
    componentId: diagnostic.componentId,
    executablePath,
    source: diagnostic.resolution.source ?? "unknown",
    version: diagnostic.resolution.version ?? undefined,
  };
}

/**
 * Windows fallback for the bundled binary — works around an upstream
 * Shipwright defect (Nimblesite/Shipwright): `shipwright-core`'s
 * `joinBinary()` builds resolve candidates with `/` separators while
 * `shipwright-vscode`'s `candidatePaths()` keys its probe map via
 * `path.join()` (`\` on win32), so the probe lookup never matches and the
 * bundled source resolves as `no-source-resolved` on every Windows install.
 * Until that separator normalisation is fixed upstream, probe the bundled
 * path ourselves and use it when it is a genuine basilisk binary.
 * Remove once shipwright-vscode ships the fix.
 */
async function bundledFallback(
  api: ShipwrightApi,
  context: vscode.ExtensionContext
): Promise<BasiliskRuntime | undefined> {
  if (api.detectPlatform === undefined || api.probeBinaryVersion === undefined) {
    return undefined;
  }
  const platform = api.detectPlatform(process.platform, process.arch);
  const exe = process.platform === "win32" ? ".exe" : "";
  const candidate = path.join(context.extensionPath, "bin", platform, `basilisk${exe}`);
  if (!fs.existsSync(candidate)) {
    return undefined;
  }
  const probe = await api.probeBinaryVersion(candidate);
  if (probe?.name !== BASILISK_COMPONENT_ID) {
    return undefined;
  }
  Logger.warn(
    `Shipwright could not resolve the bundled basilisk binary; using direct bundled fallback at ${candidate} ` +
    "(upstream win32 path-separator defect — see shipwright-runtime.ts bundledFallback)"
  );
  return {
    componentId: BASILISK_COMPONENT_ID,
    executablePath: candidate,
    source: "bundled-fallback",
    version: probe.version,
  };
}

async function loadShipwrightApi(): Promise<ShipwrightApi> {
  // `new Function` is the only way to reach a native ESM `import()` from this
  // CommonJS build, and it is typed `Function` — no runtime check can recover a
  // call signature from that, so the shim's own type is the one thing here that
  // must be asserted. Nothing is assumed about what it *returns*: every entry
  // point read off the module is checked for `undefined` before it is called
  // (see `resolveBasiliskRuntime` and `bundledFallbackRuntime`).
  // eslint-disable-next-line @typescript-eslint/no-implied-eval, @typescript-eslint/no-unsafe-type-assertion -- see above.
  const importModule = new Function("specifier", "return import(specifier)") as (
    specifier: string
  ) => Promise<ShipwrightApi>;
  return importModule(SHIPWRIGHT_PACKAGE);
}

function basiliskDiagnostic(result: ActivationResult): ActivationDiagnostic | undefined {
  return result.diagnostics.find((diagnostic) => diagnostic.componentId === BASILISK_COMPONENT_ID);
}

function formatActivationFailure(result: ActivationResult): string {
  const message = result.diagnostics
    .filter((diagnostic) => diagnostic.blocking)
    .map((diagnostic) => diagnostic.message)
    .join("\n");
  return message === "" ? "Shipwright runtime activation failed." : message;
}

export function reportRuntimeFailure(error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  Logger.error(`Basilisk runtime resolution failed: ${message}`);
  void vscode.window.showErrorMessage(message, { modal: false });
}
