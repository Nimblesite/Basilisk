// Implements [VSIX-ARCHITECTURE]. See docs/specs/VSIX-SPEC.md#VSIX-ARCHITECTURE
/**
 * Runtime narrowing for values the extension does not own.
 *
 * DAP messages, LSP `experimental` capabilities, webview posts and
 * `JSON.parse` results all arrive as `unknown`. Writing `payload as { id?:
 * number }` at each read site *asserts* a shape the compiler then trusts
 * forever — one protocol change and every downstream read is silently wrong
 * with no error anywhere. These accessors check the shape at the moment of
 * reading and return `undefined` when it does not hold, so a protocol drift
 * surfaces as a missing value rather than as a lie the type system repeats.
 *
 * Read one field at a time. There is deliberately no `as SomeInterface`
 * shortcut here — that is the very construct this module exists to replace.
 */

/** Whether `value` is a non-null, non-array object with string keys. */
export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * `value` as a keyed record, or an empty one when it is not an object.
 *
 * The empty fallback keeps callers free of null checks: every field read on it
 * yields `undefined`, which is what an absent payload means anyway.
 */
export function asRecord(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {};
}

/** The `key` field of `value` when it is a string, else `undefined`. */
export function stringField(value: unknown, key: string): string | undefined {
  const field = asRecord(value)[key];
  return typeof field === "string" ? field : undefined;
}

/** The `key` field of `value` when it is a finite number, else `undefined`. */
export function numberField(value: unknown, key: string): number | undefined {
  const field = asRecord(value)[key];
  return typeof field === "number" && Number.isFinite(field) ? field : undefined;
}

/** The `key` field of `value` when it is a boolean, else `undefined`. */
export function booleanField(value: unknown, key: string): boolean | undefined {
  const field = asRecord(value)[key];
  return typeof field === "boolean" ? field : undefined;
}

/** The `key` field of `value` when it is itself a record, else `undefined`. */
export function recordField(
  value: unknown,
  key: string,
): Record<string, unknown> | undefined {
  const field = asRecord(value)[key];
  return isRecord(field) ? field : undefined;
}

/** The `key` field of `value` as an array of unknowns; `[]` when absent. */
export function arrayField(value: unknown, key: string): unknown[] {
  const field = asRecord(value)[key];
  return Array.isArray(field) ? field : [];
}

/**
 * The `key` field of `value` as an array of records.
 *
 * Non-object elements are dropped rather than passed on as holes, so callers
 * can read fields off every element without re-checking.
 */
export function recordArrayField(
  value: unknown,
  key: string,
): Record<string, unknown>[] {
  return arrayField(value, key).filter(isRecord);
}

/** The `key` field of `value` as an array of finite numbers; `[]` when absent. */
export function numberArrayField(value: unknown, key: string): number[] {
  return arrayField(value, key).filter(
    (item): item is number => typeof item === "number" && Number.isFinite(item),
  );
}

/** The `key` field of `value` as an array of strings; `[]` when absent. */
export function stringArrayField(value: unknown, key: string): string[] {
  return arrayField(value, key).filter(
    (item): item is string => typeof item === "string",
  );
}

/**
 * Walk a chain of record keys, stopping at the first link that is not a record.
 *
 * `nested(message, "body", "source")` replaces `(message as { body?: { source?:
 * X } }).body?.source` without asserting either level.
 */
export function nested(
  value: unknown,
  ...keys: string[]
): Record<string, unknown> | undefined {
  return keys.reduce<Record<string, unknown> | undefined>(
    (current, key) => (current === undefined ? undefined : recordField(current, key)),
    asRecord(value),
  );
}
