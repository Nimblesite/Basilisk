// Implements [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
/** Discriminated union modelling success/failure without throwing. */
export type Result<T, E = Error> =
  | { readonly ok: true; readonly value: T }
  | { readonly ok: false; readonly error: E };
