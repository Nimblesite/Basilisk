// ############################################################################
// # BROKEN — THIS FILE DOES NOT COMPILE. DO NOT "FIX" IT BY RESTORING TEXT   #
// # MATCHING.                                                                #
// #                                                                          #
// # Deleted helper this file called:                                         #
// #   crate::subtyping (SubtypingContext::is_subtype / name_subtype)
// #                                                                          #
// # That helper decided types from the SPELLING of source text (lowercased   #
// # annotation strings, `"int"`/`"str"`/`"object"` literal matching, `|`     #
// # splitting, `starts_with("tuple[")`). It was deleted, not replaced.       #
// #                                                                          #
// # The call sites below are LEFT BROKEN ON PURPOSE. They are the map of     #
// # what must be rebuilt on the resolved AST — resolved bindings, canonical  #
// # `TypeNode`, and `assignable`/`equivalent` — or made to abstain.          #
// #                                                                          #
// # Restoring the deleted helper, vendoring a copy of it, or re-deriving a   #
// # type from source text anywhere below is FORBIDDEN.                       #
// #                                                                          #
// # Evidence and the failing tests that pin the real behaviour:              #
// #   docs/RULE-VALIDITY-REPORT.md                                           #
// #   crates/basilisk-checker/tests/legacy_annotation_text_parser_pin_tests.rs
// #   crates/basilisk-checker/tests/pep_spelling_invariance_pin_tests.rs     #
// ############################################################################

//! Implements [`generics_syntax_scoping`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! PEP 695 type-alias misuse (violation 7) and type-argument bound checks
//! (violation 8) for `generics_syntax_scoping`.
//!
//! Attribute accesses and alias type-parameter bounds are sourced from
//! `ruff_python_ast` nodes (via [`basilisk_resolver::Pep695Scoping`]), never
//! from raw line scanning.

use basilisk_resolver::{Pep695Scoping, ResolvedModule};

use crate::diagnostic::Diagnostic;

// ---------------------------------------------------------------------------
// Violation 7: misuse of a PEP 695 type alias
// ---------------------------------------------------------------------------

// ##########################################################################
// # DELETED BODY — `check_type_alias_misuse`. DO NOT RESTORE IT.
// #
// # Three separate spelling dependencies, all emitting LIVE diagnostics:
// #
// #   alias_names.contains(call.callee.as_str())   — alias identity by name
// #   call.callee != "isinstance"                  — BUILTIN BY SPELLING
// #   let arg_text = slice_span(source, *arg_span);
// #   alias_names.contains(arg_text.trim())        — RAW SOURCE TEXT
// #
// # The second argument of `isinstance(x, Alias)` was read back out of the
// # SOURCE, trimmed, and matched against a set of alias names. So
// # `isinstance(x, (Alias,))`, a line-broken argument, or an alias reached
// # under a second name each changed the verdict, and `isinstance` itself was
// # recognised by its five-character spelling rather than by resolution —
// # `from builtins import isinstance as check` was invisible, and a user
// # function named `isinstance` was treated as the builtin.
// #
// # `check_alias_attribute_access` carried the same defect
// # (`alias_names.contains(access.base.as_str())`) and is deleted with it.
// #
// # Whether a callee IS `builtins.isinstance`, and whether an argument IS a
// # `type` alias, are both questions about resolved bindings.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
pub(super) fn check_type_alias_misuse(
    _module: &ResolvedModule,
    _scoping: &Pep695Scoping,
    _diagnostics: &mut Vec<Diagnostic>,
) {
    panic!(
        "basilisk-checker: `check_type_alias_misuse` was DELETED because it recognised \
         `isinstance`/`issubclass` by their SPELLINGS, matched type aliases by rendered \
         name, and read the second argument back out of RAW SOURCE TEXT to compare it. \
         It panics because the real implementation — callee and argument resolved \
         through the binding table — DOES NOT EXIST YET. Do not restore the text read \
         and do not skip the check in its place."
    )
}

// ##########################################################################
// # DELETED AND GONE — `check_alias_attribute_access`. NO PANIC SHELL: its
// # only caller (`check_type_alias_misuse`) was deleted too, so there is no
// # call site left to keep visible. DO NOT RECREATE IT.
// #
// # `alias_names.contains(access.base.as_str())` decided whether an attribute
// # access targeted a `type` alias by matching the base's RENDERED NAME.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################

// ---------------------------------------------------------------------------
// Violation 8: a type argument violates a type parameter's bound
// ---------------------------------------------------------------------------

// ##########################################################################
// # DELETED — the ENTIRE PEP 695 alias bound-check path:                   #
// #   `TypeParamWithBound`, `TypeAliasWithBounds`, `collect_bounded_aliases`
// #   `check_type_alias_bound_violations`, `extract_annotation_for_var`,   #
// #   `check_annotation_bounds`, `split_top_level`.                        #
// #                                                                        #
// # NO PANIC SHELLS BELOW THE ENTRY POINT: the helpers had no callers left #
// # once the entry point was emptied, so there is nothing to keep visible. #
// # DO NOT RECREATE ANY OF THEM.                                           #
// #                                                                        #
// # This module's own header claims annotations are "sourced from          #
// # `ruff_python_ast` nodes … never from raw line scanning". They were     #
// # not. `extract_annotation_for_var` did exactly that:                    #
// #                                                                        #
// #   let line_start = source[..start].rfind('\n')…;                       #
// #   let colon_pos  = line[name_offset..].find(": ")?;                    #
// #   let end        = line[after_colon..].find('=')…;                     #
// #   let annotation = line[after_colon..end].trim();                      #
// #                                                                        #
// # It located an annotation by searching a LINE OF SOURCE for the two     #
// # characters `": "` and cutting at the next `=`. An annotation written   #
// # `x:int` (no space) was invisible; one split across lines was           #
// # truncated; a default value containing `=` or a dict display containing #
// # `: ` moved the boundary. `check_annotation_bounds` then hand-parsed    #
// # that fragment with `find('[')`/`rfind(']')`, matched the alias by      #
// # rendered name, split arguments with `split_top_level`, and settled     #
// # every bound through the string-keyed `SubtypingContext::is_subtype`.   #
// #                                                                        #
// # Not one step consulted a resolved symbol. The replacement reads the    #
// # annotation `Expr` the parser already produced, resolves the alias      #
// # through the binding table, and relates each type argument to its       #
// # declared bound on canonical types.                                     #
// #                                                                        #
// # Pinned by: tests/no_type_spelling_surgery_tests.rs                     #
// ##########################################################################

/// Check type-argument bounds where bounded PEP 695 aliases are used in
/// annotations.
///
/// DELETED — panics. See the banner above.
pub(super) fn check_type_alias_bound_violations(
    _module: &ResolvedModule,
    _scoping: &Pep695Scoping,
    _diagnostics: &mut Vec<Diagnostic>,
) {
    panic!(
        "basilisk-checker: `check_type_alias_bound_violations` was DELETED because it \
         located annotations by scanning a LINE OF SOURCE for `\": \"` and cutting at \
         the next `=`, hand-parsed the result with `find('[')`/`rfind(']')`, matched \
         aliases by rendered name, and settled bounds with the string-keyed \
         `SubtypingContext::is_subtype`. It panics because the real implementation — \
         resolving the annotation expression through the binding table and relating \
         each type argument to its declared bound — DOES NOT EXIST YET. Do not \
         restore any of it and do not return without checking in its place."
    )
}
