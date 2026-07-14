// Implements [VSIX-CONFIGURATION-EDITOR-THIN-SHELL] runtime intent decoding.
/** Untrusted webview messages accepted by the configuration editor host. */

import type {
  ConfigurationMutation,
  MutationScope,
  RuleOccurrencesRequest,
  RuleSelector,
  RuleSetting,
} from "./configuration-editor-model";

const MAX_MUTATIONS = 512;
const MAX_CODES = 2_048;
const MAX_TAGS = 128;
const MAX_TEXT_LENGTH = 8_192;
const MAX_OCCURRENCES = 500;

export type ConfigurationEditorIntent =
  | { readonly type: "ready" }
  | { readonly type: "refresh" }
  | { readonly type: "fixSafe" }
  | { readonly type: "openRaw" }
  | { readonly type: "apply" }
  | { readonly type: "preview"; readonly mutations: ConfigurationMutation[] }
  | { readonly type: "occurrences"; readonly request: Omit<RuleOccurrencesRequest, "rootUri"> }
  | { readonly type: "openDocs"; readonly uri: string }
  | { readonly type: "openOccurrence"; readonly uri: string; readonly line: number; readonly character: number };

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

function decodeSelector(value: unknown): RuleSelector | undefined {
  if (!isRecord(value) || typeof value.kind !== "string") { return undefined; }
  switch (value.kind) {
    case "All": return { kind: "All" };
    case "CurrentViolations": return { kind: "CurrentViolations" };
    case "SafeFixable": return { kind: "SafeFixable" };
    case "WithoutSafeFix": return { kind: "WithoutSafeFix" };
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

function decodeSetting(value: unknown): RuleSetting | undefined {
  if (!isRecord(value) || typeof value.kind !== "string") { return undefined; }
  switch (value.kind) {
    case "Inherit": return { kind: "Inherit" };
    case "Native": return { kind: "Native" };
    case "Error": return { kind: "Error" };
    case "Warning": return { kind: "Warning" };
    case "Info": return { kind: "Info" };
    case "Disabled": return { kind: "Disabled" };
    default: return undefined;
  }
}

function decodeScope(value: unknown): MutationScope | undefined {
  if (!isRecord(value) || typeof value.kind !== "string") { return undefined; }
  if (value.kind === "Project") { return { kind: "Project" }; }
  if (value.kind !== "Path") { return undefined; }
  const pattern = boundedString(value.pattern);
  return pattern === undefined ? undefined : { kind: "Path", pattern };
}

function decodeMutation(value: unknown): ConfigurationMutation | undefined {
  if (!isRecord(value)) { return undefined; }
  const selector = decodeSelector(value.selector);
  const setting = decodeSetting(value.setting);
  const scope = decodeScope(value.scope);
  return selector === undefined || setting === undefined || scope === undefined
    ? undefined
    : { selector, setting, scope };
}

function decodePreview(value: Record<string, unknown>): ConfigurationEditorIntent | undefined {
  if (!Array.isArray(value.mutations) || value.mutations.length === 0 || value.mutations.length > MAX_MUTATIONS) {
    return undefined;
  }
  const mutations = value.mutations.map(decodeMutation);
  if (!mutations.every((mutation): mutation is ConfigurationMutation => mutation !== undefined)) {
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

/** Decode one untrusted `webview.onDidReceiveMessage` payload. */
export function decodeConfigurationEditorIntent(value: unknown): ConfigurationEditorIntent | undefined {
  if (!isRecord(value) || typeof value.type !== "string") { return undefined; }
  switch (value.type) {
    case "ready": return { type: "ready" };
    case "refresh": return { type: "refresh" };
    case "fixSafe": return { type: "fixSafe" };
    case "openRaw": return { type: "openRaw" };
    case "apply": return { type: "apply" };
    case "preview": return decodePreview(value);
    case "occurrences": return decodeOccurrences(value);
    default: return decodeNavigationIntent(value);
  }
}
