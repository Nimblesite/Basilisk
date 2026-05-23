// Implements [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
import * as vscode from "vscode";
import * as path from "path";
import { Logger } from "./logger";

const SHIPWRIGHT_PACKAGE = "@nimblesite/shipwright-vscode";
const BASILISK_COMPONENT_ID = "basilisk";

interface ShipwrightApi {
  activateShipwright?: ActivateRuntime;
  activateDeploymentToolkit?: ActivateRuntime;
}

type ActivateRuntime = (
  context: vscode.ExtensionContext,
  options: { readonly vscode: typeof vscode; readonly manifestPath: string }
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

export async function resolveBasiliskRuntime(context: vscode.ExtensionContext): Promise<BasiliskRuntime> {
  const api = await loadShipwrightApi();
  const activate = api.activateShipwright ?? api.activateDeploymentToolkit;
  if (activate === undefined) {
    throw new Error(`${SHIPWRIGHT_PACKAGE} does not export a VS Code activation function.`);
  }
  const result = await activate(context, {
    vscode,
    manifestPath: path.join(context.extensionPath, "shipwright.json"),
  });
  const diagnostic = basiliskDiagnostic(result);
  if (!result.ok) {
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

async function loadShipwrightApi(): Promise<ShipwrightApi> {
  // eslint-disable-next-line @typescript-eslint/no-implied-eval -- preserves native ESM import from this CommonJS extension build.
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
