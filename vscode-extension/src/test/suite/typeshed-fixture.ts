// Shared [LSPCFGED-TYPESHED] wire fixtures for the configuration-editor suites.
/** One source of truth for what the server sends, so no suite invents a shape. */

import type {
  TypeshedConfigurationState,
  TypeshedDownloadPolicy,
  TypeshedSource,
  TypeshedStatusState,
} from "../../configuration-editor-model";

export const ACTIVE_COMMIT = "83c2518a9e6abbda0c44592c3483de459198f887";
export const OTHER_COMMIT = "1f2e3d4c5b6a798877665544332211000ffeeddc";

export interface TypeshedFixtureOptions {
  readonly source?: TypeshedSource;
  readonly downloads?: TypeshedDownloadPolicy | undefined;
  readonly pinnableCommit?: string | undefined;
  readonly licenseAvailable?: boolean;
  readonly acquiring?: boolean;
  readonly warnings?: TypeshedStatusState["warnings"];
}

export const DEFAULT_DOWNLOADS: TypeshedDownloadPolicy = {
  reuseDownloads: true,
  verifyContent: true,
  archiveUrl: undefined,
  cacheFolder: undefined,
};

function defaultDownloads(
  source: TypeshedSource,
  options: TypeshedFixtureOptions,
): TypeshedDownloadPolicy | undefined {
  if ("downloads" in options) { return options.downloads; }
  return source.kind === "CustomFolder" ? undefined : DEFAULT_DOWNLOADS;
}

function defaultPin(
  source: TypeshedSource,
  acquiring: boolean,
  options: TypeshedFixtureOptions,
): string | undefined {
  if ("pinnableCommit" in options) { return options.pinnableCommit; }
  return !acquiring && source.kind === "Latest" ? ACTIVE_COMMIT : undefined;
}

function fixtureStatus(acquiring: boolean, warnings: TypeshedStatusState["warnings"]): TypeshedStatusState {
  return {
    lifecycle: { kind: acquiring ? "Acquiring" : "Ready" },
    blockedReason: undefined,
    activeSource: acquiring ? undefined : { kind: "Bundled" },
    commitIdentity: acquiring ? undefined : ACTIVE_COMMIT,
    transport: acquiring ? undefined : { kind: "EmbeddedZip" },
    licenseStatus: { kind: acquiring ? "Acquiring" : "Approved" },
    provenance: { kind: acquiring ? "Pending" : "BundleVetted" },
    signedRelease: false,
    warnings,
  };
}

/**
 * The server's projection for a settled root. Callers pass only what their
 * scenario changes; every other field stays the realistic default.
 */
export function typeshedFixture(options: TypeshedFixtureOptions = {}): TypeshedConfigurationState {
  const acquiring = options.acquiring === true;
  const source = options.source ?? { kind: "Latest" };
  return {
    source,
    downloads: defaultDownloads(source, options),
    pinnableCommit: defaultPin(source, acquiring, options),
    licenseAvailable: options.licenseAvailable ?? !acquiring,
    status: fixtureStatus(acquiring, options.warnings ?? []),
  };
}
