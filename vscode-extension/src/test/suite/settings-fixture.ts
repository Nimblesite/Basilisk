// Shared [LSPCFGED-TYPESHED] wire fixtures for the configuration-editor suites.
/** One source of truth for what the server sends, so no suite invents a shape. */

import type {
  TypeshedConfigurationState,
  TypeshedSource,
  TypeshedStatusState,
} from "../../configuration-editor-model";

export const ACTIVE_COMMIT = "83c2518a9e6abbda0c44592c3483de459198f887";
export const OTHER_COMMIT = "1f2e3d4c5b6a798877665544332211000ffeeddc";
/** What resolving python/typeshed@main yields for a Download latest run. */
export const LATEST_COMMIT = "aaaabbbbccccddddeeeeffff0000111122223333";

export interface TypeshedFixtureOptions {
  readonly source?: TypeshedSource;
  readonly storeFolder?: string | undefined;
  readonly licenseAvailable?: boolean;
  readonly downloading?: boolean;
  readonly noSourceReason?: string;
  readonly warnings?: TypeshedStatusState["warnings"];
}

function fixtureLifecycle(options: TypeshedFixtureOptions): TypeshedStatusState["lifecycle"] {
  if (options.downloading === true) { return { kind: "Downloading" }; }
  return options.noSourceReason === undefined ? { kind: "Ready" } : { kind: "NoSource" };
}

function fixtureStatus(options: TypeshedFixtureOptions): TypeshedStatusState {
  const lifecycle = fixtureLifecycle(options);
  const ready = lifecycle.kind === "Ready";
  return {
    lifecycle,
    noSourceReason: lifecycle.kind === "NoSource" ? options.noSourceReason : undefined,
    activeSource: ready ? { kind: "Bundled" } : undefined,
    commitIdentity: ready ? ACTIVE_COMMIT : undefined,
    licenseStatus: { kind: ready ? "Approved" : "Unavailable" },
    warnings: options.warnings ?? [],
  };
}

/**
 * The server's projection for one root. Callers pass only what their scenario
 * changes; every other field stays the realistic default: the pinned-commit
 * source (the only default source — there is no "Latest") with no store
 * folder configured.
 */
export function typeshedFixture(options: TypeshedFixtureOptions = {}): TypeshedConfigurationState {
  const source = options.source ?? { kind: "ExactCommit", commit: ACTIVE_COMMIT };
  return {
    source,
    // A custom folder downloads nothing, so it has no store folder at all.
    storeFolder: source.kind === "CustomFolder" ? undefined : options.storeFolder,
    licenseAvailable: options.licenseAvailable ?? source.kind !== "CustomFolder",
    status: fixtureStatus(options),
  };
}
