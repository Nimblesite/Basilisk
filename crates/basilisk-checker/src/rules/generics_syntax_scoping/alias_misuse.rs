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

use std::collections::HashSet;

use basilisk_resolver::{Pep695Scoping, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};

use super::CODE;

// ---------------------------------------------------------------------------
// Violation 7: misuse of a PEP 695 type alias
// ---------------------------------------------------------------------------

/// Aliases cannot be called, subclassed, used in `isinstance`/`issubclass`, or
/// have attributes accessed (except `__value__` / `__type_params__`).
pub(super) fn check_type_alias_misuse(
    module: &ResolvedModule,
    scoping: &Pep695Scoping,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;

    let alias_names: HashSet<&str> = module
        .type_statements
        .iter()
        .map(|ts| ts.name.as_str())
        .collect();
    if alias_names.is_empty() {
        return;
    }

    for call in &module.calls {
        if alias_names.contains(call.callee.as_str())
            && call.callee != "isinstance"
            && call.callee != "issubclass"
        {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Cannot call type alias `{}`: type aliases are not callable",
                    call.callee
                ),
                call.span,
                path,
                Some("Type aliases created with `type` cannot be instantiated".to_owned()),
                None,
            ));
        }

        if (call.callee == "isinstance" || call.callee == "issubclass") && call.args.len() >= 2 {
            if let Some((_, arg_span)) = call.args.get(1) {
                if let Some(arg_text) = crate::span_util::slice_span(source, *arg_span) {
                    let arg_trimmed = arg_text.trim();
                    if alias_names.contains(arg_trimmed) {
                        diagnostics.push(error_diagnostic_owned(
                            CODE.clone(),
                            format!("Cannot use type alias `{arg_trimmed}` in `{}`", call.callee),
                            *arg_span,
                            path,
                            Some(format!(
                                "Type aliases created with `type` cannot be used with `{}`",
                                call.callee
                            )),
                            None,
                        ));
                    }
                }
            }
        }
    }

    for class in &module.classes {
        for base in &class.bases {
            // ##############################################################
            // # DELETED — the base-name extraction. DO NOT RESTORE IT.     #
            // #                                                            #
            // # `base.split('[').next().unwrap_or(base).trim()` took a     #
            // # base class's identity from its SOURCE TEXT, then tested    #
            // # membership of that STRING in the alias-name set. So        #
            // # `type Alias = int` subclassed as `Alias [int]` was missed, #
            // # an alias reached under a second name was missed, and a     #
            // # user class sharing an alias's rendered name was falsely    #
            // # reported. Whether a base IS a `type` alias is a question   #
            // # about the binding it resolves to.                          #
            // #                                                            #
            // # Pinned by: tests/no_type_spelling_surgery_tests.rs         #
            // ##############################################################
            let _ = (base, &alias_names, class, path, &mut *diagnostics);
            panic!(
                "basilisk-checker: `alias_misuse`'s type-alias-as-base-class check was \
                 DELETED because it split a base's SOURCE TEXT at `[` and tested the \
                 resulting STRING for membership in the alias-name set. It panics \
                 because the real implementation — resolving the base expression to \
                 its binding and asking whether that binding is a `type` alias — DOES \
                 NOT EXIST YET. Do not restore the split and do not skip the check in \
                 its place."
            );
        }
    }

    check_alias_attribute_access(&alias_names, scoping, path, diagnostics);
}

/// Only `__value__` and `__type_params__` may be accessed on a type alias.
fn check_alias_attribute_access(
    alias_names: &HashSet<&str>,
    scoping: &Pep695Scoping,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for access in &scoping.attr_accesses {
        if !alias_names.contains(access.base.as_str()) {
            continue;
        }
        if access.attr == "__value__" || access.attr == "__type_params__" {
            continue;
        }
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Cannot access attribute `{}` on type alias `{}`",
                access.attr, access.base
            ),
            access.span,
            path,
            Some(
                "Type aliases only support `__value__` and `__type_params__` attributes".to_owned(),
            ),
            None,
        ));
    }
}

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
