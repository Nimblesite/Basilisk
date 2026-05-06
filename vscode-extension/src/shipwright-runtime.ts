import * as vscode from "vscode";
import { Logger } from "./logger";

const SHIPWRIGHT_PACKAGE = "@nimblesite/shipwright-vscode";
const BASILISK_COMPONENT_ID = "basilisk";

interface ShipwrightApi {
  activateShipwright(
    context: vscode.ExtensionContext,
    options: { vscode: typeof vscode }
  ): Promise<ActivationResult>;
}

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
  const result = await api.activateShipwright(context, { vscode });
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
