// Implements [LSPCFGED-TYPESHED] / [LSPCFGED-CACHE] native folder pickers and
// the direct-write rule the Typeshed and Caching panels share.

import * as vscode from "vscode";
import type { ConfigurationSnapshot } from "./configuration-editor-model";
import type { ConfigurationEditorIntent } from "./configuration-editor-intents";

type PreviewIntent = Extract<ConfigurationEditorIntent, { type: "preview" }>;

/**
 * Every directory-typed setting in the editor renders with a native
 * folder-picker rather than free text. A cancelled picker writes nothing at
 * all, so the controls stay on the configuration that still holds.
 */
async function pickFolder(
  snapshot: ConfigurationSnapshot,
  openLabel: string,
  title: string,
): Promise<vscode.Uri | undefined> {
  const selected = await vscode.window.showOpenDialog({
    canSelectFiles: false, canSelectFolders: true, canSelectMany: false,
    defaultUri: vscode.Uri.parse(snapshot.rootUri, true),
    openLabel,
    title,
  });
  return selected?.[0];
}

/**
 * Choosing the Typeshed source folder is one atomic transition — it also
 * clears the commit pin AND the package pin, so no combination of source
 * values can be written ([LSPCFGED-TYPESHED], [STUBRES-TYPESHED-PYPI]).
 * Cancelling the picker returns `undefined` and writes nothing, so the
 * current source survives a backed-out switch.
 */
export async function pickTypeshedFolder(
  snapshot: ConfigurationSnapshot,
  key: "TypeshedPath" | "TypeshedStorePath",
): Promise<PreviewIntent | undefined> {
  const isSource = key === "TypeshedPath";
  const folder = await pickFolder(
    snapshot,
    isSource ? "Use Typeshed folder" : "Use store folder",
    isSource ? "Choose a Typeshed tree containing stdlib/" : "Choose the Typeshed store folder",
  );
  if (folder === undefined) { return undefined; }
  const mutations: PreviewIntent["mutations"] = [{
    kind: "SetTypeshedSetting", key: { kind: key }, value: folder.fsPath,
  }];
  if (isSource) {
    mutations.push({ kind: "RemoveTypeshedSetting", key: { kind: "TypeshedCommit" } });
    mutations.push({ kind: "RemoveTypeshedSetting", key: { kind: "TypeshedPackage" } });
  }
  return { type: "preview", mutations };
}

/**
 * The persistent result cache's folder ([LSPCFGED-CACHE]). Relocating it is a
 * single key write; it does not imply enabling the cache, because where
 * entries would live and whether they are written are separate decisions.
 */
export async function pickCacheFolder(
  snapshot: ConfigurationSnapshot,
): Promise<PreviewIntent | undefined> {
  const folder = await pickFolder(
    snapshot,
    "Use cache folder",
    "Choose where Basilisk stores cached check results",
  );
  return folder === undefined
    ? undefined
    : {
      type: "preview",
      mutations: [{ kind: "SetCacheSetting", key: { kind: "CacheDir" }, value: folder.fsPath }],
    };
}

/**
 * A Typeshed or cache edit is a direct setting switch, not a rule-severity
 * trade-off: there is no impact to weigh, so it applies as soon as it is made
 * ([LSPCFGED-TYPESHED], [LSPCFGED-CACHE]). A control that needed a second
 * confirmation could show a value the configuration does not hold.
 */
export function isDirectSettingOnly(intent: PreviewIntent): boolean {
  return intent.mutations.every((mutation) =>
    mutation.kind === "SetTypeshedSetting" || mutation.kind === "RemoveTypeshedSetting"
    || mutation.kind === "SetCacheSetting" || mutation.kind === "RemoveCacheSetting");
}
