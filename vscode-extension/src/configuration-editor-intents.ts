// Implements [VSIX-CONFIGURATION-EDITOR-THIN-SHELL] runtime intent decoding.
/** Untrusted webview messages accepted by the configuration editor host. */

import type {
  EditorMutation,
  RuleOccurrencesRequest,
  RuleSelector,
  RuleSeverity,
  TypeshedAction,
  TypeshedSettingKey,
} from "./configuration-editor-model";

const MAX_MUTATIONS = 512;
const MAX_CODES = 2_048;
const MAX_TAGS = 128;
const MAX_TEXT_LENGTH = 8_192;
const MAX_OCCURRENCES = 500;

export type ConfigurationEditorIntent =
  | { readonly type: "ready" }
  | { readonly type: "refresh" }
  | { readonly type: "openRaw" }
  | { readonly type: "apply" }
  | { readonly type: "cancelPreview" }
  | { readonly type: "preview"; readonly mutations: EditorMutation[] }
  | { readonly type: "adopt"; readonly scope: "workspace" }
  | { readonly type: "fixSafe" }
  | { readonly type: "openConfigFile"; readonly uri: string }
  | { readonly type: "occurrences"; readonly request: Omit<RuleOccurrencesRequest, "rootUri"> }
  | { readonly type: "openDocs"; readonly uri: string }
  | { readonly type: "openOccurrence"; readonly uri: string; readonly line: number; readonly character: number }
  | { readonly type: "pickTypeshedFolder"; readonly key: "TypeshedPath" | "TypeshedStorePath" }
  | { readonly type: "typeshedAction"; readonly action: TypeshedAction };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function boundedString(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 && value.length <= MAX_TEXT_LENGTH
    ? value
    : undefined;
}

function stringList(value: unknown, maximum: number): string[] | undefined {
  if (!Array.isArray(value) || value.length === 0 || value.length > maximum) { return undefined; }
  const strings = value.map(boundedString);
  return strings.every((item): item is string => item !== undefined) ? strings : undefined;
}

/** Read-side occurrence selectors only — mutations never take selectors ([CONFIGEDITOR-MODEL]). */
function decodeSelector(value: unknown): RuleSelector | undefined {
  if (!isRecord(value) || typeof value.kind !== "string") { return undefined; }
  switch (value.kind) {
    case "All": return { kind: "All" };
    case "Codes": {
      const codes = stringList(value.codes, MAX_CODES);
      return codes === undefined ? undefined : { kind: "Codes", codes };
    }
    case "Tags": {
      const tags = stringList(value.tags, MAX_TAGS);
      return tags === undefined || typeof value.matchAll !== "boolean"
        ? undefined
        : { kind: "Tags", tags, matchAll: value.matchAll };
    }
    default: return undefined;
  }
}

function decodeSeverity(value: unknown): RuleSeverity | undefined {
  if (!isRecord(value) || typeof value.kind !== "string") { return undefined; }
  switch (value.kind) {
    case "Error": return { kind: "Error" };
    case "Warning": return { kind: "Warning" };
    case "Info": return { kind: "Info" };
    case "Disabled": return { kind: "Disabled" };
    default: return undefined;
  }
}

function decodeTypeshedKey(value: unknown): TypeshedSettingKey | undefined {
  if (!isRecord(value) || typeof value.kind !== "string") { return undefined; }
  switch (value.kind) {
    case "TypeshedPath": return { kind: "TypeshedPath" };
    case "TypeshedCommit": return { kind: "TypeshedCommit" };
    case "TypeshedStorePath": return { kind: "TypeshedStorePath" };
    default: return undefined;
  }
}

/**
 * The only four things the editor can request: set or remove one rule entry
 * or one tag entry ([CHKARCH-CONFIG-MODEL], [CONFIGEDITOR-OPERATIONS]).
 * Every surviving Typeshed key is text-valued ([LSPCFGED-TYPESHED]).
 */
function decodeTypeshedMutation(value: Record<string, unknown>): EditorMutation | undefined {
  if (value.kind === "SetTypeshedSetting") {
    const key = decodeTypeshedKey(value.key);
    const setting = boundedString(value.value);
    return key === undefined || setting === undefined
      ? undefined
      : { kind: "SetTypeshedSetting", key, value: setting };
  }
  if (value.kind === "RemoveTypeshedSetting") {
    const key = decodeTypeshedKey(value.key);
    return key === undefined ? undefined : { kind: "RemoveTypeshedSetting", key };
  }
  return undefined;
}

function decodeMutation(value: unknown): EditorMutation | undefined {
  if (!isRecord(value) || typeof value.kind !== "string") { return undefined; }
  if (value.kind === "SetTypeshedSetting" || value.kind === "RemoveTypeshedSetting") {
    return decodeTypeshedMutation(value);
  }
  switch (value.kind) {
    case "SetRule": {
      const code = boundedString(value.code);
      const severity = decodeSeverity(value.severity);
      return code === undefined || severity === undefined
        ? undefined
        : { kind: "SetRule", code, severity };
    }
    case "RemoveRule": {
      const code = boundedString(value.code);
      return code === undefined ? undefined : { kind: "RemoveRule", code };
    }
    case "SetTag": {
      const tag = boundedString(value.tag);
      const severity = decodeSeverity(value.severity);
      return tag === undefined || severity === undefined
        ? undefined
        : { kind: "SetTag", tag, severity };
    }
    case "RemoveTag": {
      const tag = boundedString(value.tag);
      return tag === undefined ? undefined : { kind: "RemoveTag", tag };
    }
    default: return undefined;
  }
}

function decodeTypeshedAction(value: unknown): TypeshedAction | undefined {
  switch (value) {
    case "DownloadLatest": return { kind: "DownloadLatest" };
    case "DownloadPinned": return { kind: "DownloadPinned" };
    case "ViewLicense": return { kind: "ViewLicense" };
    default: return undefined;
  }
}

function decodePreview(value: Record<string, unknown>): ConfigurationEditorIntent | undefined {
  if (!Array.isArray(value.mutations) || value.mutations.length === 0 || value.mutations.length > MAX_MUTATIONS) {
    return undefined;
  }
  const mutations = value.mutations.map(decodeMutation);
  if (!mutations.every((mutation): mutation is EditorMutation => mutation !== undefined)) {
    return undefined;
  }
  return { type: "preview", mutations };
}

function decodeOccurrences(value: Record<string, unknown>): ConfigurationEditorIntent | undefined {
  const selector = decodeSelector(value.selector);
  const cursor = value.cursor === undefined ? undefined : boundedString(value.cursor);
  const limit = value.limit;
  if (selector === undefined || (value.cursor !== undefined && cursor === undefined)) { return undefined; }
  if (!Number.isInteger(limit) || typeof limit !== "number" || limit < 1 || limit > MAX_OCCURRENCES) {
    return undefined;
  }
  return { type: "occurrences", request: { selector, cursor, limit } };
}

function decodeNavigationIntent(value: Record<string, unknown>): ConfigurationEditorIntent | undefined {
  if (value.type === "openDocs") {
    const uri = boundedString(value.uri);
    return uri === undefined ? undefined : { type: "openDocs", uri };
  }
  if (value.type !== "openOccurrence") { return undefined; }
  const uri = boundedString(value.uri);
  const line = value.line;
  const character = value.character;
  return uri !== undefined && typeof line === "number" && Number.isInteger(line) && line >= 0
    && typeof character === "number" && Number.isInteger(character) && character >= 0
    ? { type: "openOccurrence", uri, line, character }
    : undefined;
}

function decodeCoreIntent(value: Record<string, unknown>): ConfigurationEditorIntent | undefined {
  switch (value.type) {
    case "ready": return { type: "ready" };
    case "refresh": return { type: "refresh" };
    case "openRaw": return { type: "openRaw" };
    case "apply": return { type: "apply" };
    case "cancelPreview": return { type: "cancelPreview" };
    case "adopt": return value.scope === "workspace" ? { type: "adopt", scope: "workspace" } : undefined;
    case "fixSafe": return { type: "fixSafe" };
    default: return undefined;
  }
}

/** Decode one untrusted `webview.onDidReceiveMessage` payload. */
export function decodeConfigurationEditorIntent(value: unknown): ConfigurationEditorIntent | undefined {
  if (!isRecord(value) || typeof value.type !== "string") { return undefined; }
  if (value.type === "preview") { return decodePreview(value); }
  if (value.type === "occurrences") { return decodeOccurrences(value); }
  if (value.type === "openConfigFile") {
    const uri = boundedString(value.uri);
    return uri === undefined ? undefined : { type: "openConfigFile", uri };
  }
  if (value.type === "pickTypeshedFolder") {
    return value.key === "TypeshedPath" || value.key === "TypeshedStorePath"
      ? { type: "pickTypeshedFolder", key: value.key }
      : undefined;
  }
  if (value.type === "typeshedAction") {
    const action = decodeTypeshedAction(value.action);
    return action === undefined ? undefined : { type: "typeshedAction", action };
  }
  return decodeCoreIntent(value) ?? decodeNavigationIntent(value);
}
