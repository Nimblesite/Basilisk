//! Implements [`assignment_compatibility`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! Dataclass attribute assignment checking for `assignment_compatibility`.
//!
//! Validates module-level attribute assignments (`instance.field = value`)
//! against the declared field types of `dataclass`/`dataclass_transform`
//! classes, catching obvious literal kind mismatches.

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, RhsKind, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::span_util::slice_span;

use super::CODE;

// ##########################################################################
// # DELETED BODY — `annotation_rhs_mismatch_simple`. DO NOT RESTORE IT.    #
// # DO NOT SUBSTITUTE A PLACEHOLDER THAT RETURNS `None`.                   #
// #                                                                        #
// # It took the annotation as TEXT, split it at `[`, trimmed it,           #
// # LOWER-CASED it, and matched the result against the literal strings     #
// # `"int"`, `"bool"`, `"float"`, `"bytes"`, `"str"`. Everything about     #
// # that is spelling:                                                      #
// #                                                                        #
// #   * lower-casing means a user class `Int` was judged as builtin `int`; #
// #   * the whitelist means `from builtins import int as Whole` was never  #
// #     recognised, and a module that rebinds `int` still was;             #
// #   * splitting at `[` means `dict [str, int]` and `dict[str, int]`      #
// #     produced different bases.                                          #
// #                                                                        #
// # "Does this RHS fit this declared field type?" is the ordinary          #
// # assignability question, asked of the RESOLVED annotation — not a       #
// # lookup table of builtin name spellings.                                #
// #                                                                        #
// # Pinned by: tests/no_type_spelling_surgery_tests.rs                     #
// ##########################################################################

/// DELETED — panics. The signature survives only so its caller stays visible
/// as the rebuild map; see the banner above.
pub(super) fn annotation_rhs_mismatch_simple(
    _annotation: &str,
    _rhs: &RhsKind,
) -> Option<&'static str> {
    panic!(
        "basilisk-checker: `annotation_rhs_mismatch_simple` was DELETED because it \
         lower-cased the annotation TEXT, split it at `[`, and matched the result \
         against a whitelist of builtin name spellings. It panics because the real \
         implementation — resolving the field's annotation through the binding table \
         and asking the ordinary assignability question — DOES NOT EXIST YET. Do not \
         restore the table and do not return `None` in its place: `None` reports the \
         rule as implemented while it checks nothing."
    )
}

/// Checks module-level attribute assignments (`instance.field = value`) against
/// the declared field types of `dataclass`/`dataclass_transform` classes.
pub(super) fn check_dataclass_attr_assignments(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if module.module_attr_assignments.is_empty() {
        return;
    }

    let transform_classes = super::super::guards::collect_transform_classes(module);

    // Build a map: class_name -> { field_name -> annotation_text }
    let mut class_field_types: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
    for cls in &module.classes {
        let is_dc_like = cls.is_dataclass || transform_classes.contains_key(&cls.name_span);
        if !is_dc_like {
            continue;
        }
        let mut fields = HashMap::new();
        for attr in &cls.attributes {
            if let Some(ann_span) = attr.annotation_span {
                if let Some(ann_text) = slice_span(&module.source, ann_span) {
                    let _ = fields.insert(attr.name.as_str(), ann_text.trim());
                }
            }
        }
        let _ = class_field_types.insert(cls.name.as_str(), fields);
    }

    if class_field_types.is_empty() {
        return;
    }

    // Build a map: variable_name -> class_name (for instances of DC-like classes)
    let source = &module.source;
    let instance_class: HashMap<&str, &str> = module
        .module_vars
        .iter()
        .filter_map(|var| {
            let rhs_span = var.rhs_span?;
            let rhs_text = slice_span(source, rhs_span)?;
            // Call site of the DELETED trailing-word callee reduction — see the
            // banner on `constructed_class_name` below.
            let callee = constructed_class_name(rhs_text);
            if class_field_types.contains_key(callee) {
                Some((var.name.as_str(), callee))
            } else {
                None
            }
        })
        .collect();

    if instance_class.is_empty() {
        return;
    }

    for assign in &module.module_attr_assignments {
        let Some(&class_name) = instance_class.get(assign.object_name.as_str()) else {
            continue;
        };
        let Some(fields) = class_field_types.get(class_name) else {
            continue;
        };
        let Some(&field_type) = fields.get(assign.attr_name.as_str()) else {
            continue;
        };

        // Extract the RHS literal kind from the source line
        let rhs_kind = extract_rhs_kind_from_assign(source, assign.target_span);
        if let Some(kind) = rhs_kind {
            if let Some(rhs_description) = annotation_rhs_mismatch_simple(field_type, &kind) {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Type mismatch: `{}.{}` is typed `{field_type}` but assigned {rhs_description}",
                        assign.object_name, assign.attr_name
                    ),
                    assign.target_span,
                    &module.path,
                    Some(format!(
                        "Field `{}` of `{class_name}` expects `{field_type}`",
                        assign.attr_name
                    )),
                    Some(
                        "Basilisk requires attribute assignments to be compatible with the declared field type"
                            .to_owned(),
                    ),
                ));
            }
        }
    }
}

/// Extracts the RHS literal kind from a module-level attribute assignment line.
///
/// Given the target span of `obj.attr` in `obj.attr = value`, finds the `= value`
/// portion and determines the literal kind.
// ##########################################################################
// # DELETED BODY — the constructed-class reduction. DO NOT RESTORE IT AND  #
// # DO NOT RETURN `""` IN ITS PLACE.                                       #
// #                                                                        #
// #   let callee = rhs_text.split(['(', '[']).next()?.trim();              #
// #   let callee = callee.rsplit('.').next().unwrap_or(callee);            #
// #                                                                        #
// # The same defect as in `dataclasses_frozen`, vendored here: the         #
// # constructed class read out of RAW SOURCE by cutting at the first `(`   #
// # or `[`, then reduced to its trailing word so that every same-named     #
// # class in the program collapsed into one entry of a by-name map.        #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
fn constructed_class_name(_rhs_text: &str) -> &str {
    panic!(
        "basilisk-checker: the constructed-class reduction in `dataclass_check` was \
         DELETED because it read the class out of RAW SOURCE by cutting at the first \
         `(` or `[` and then discarded the module qualifier with `rsplit('.')`. It \
         panics because the real implementation — resolving the callee through the \
         binding table — DOES NOT EXIST YET. Do not restore the splitting and do not \
         return `\"\"` in its place."
    )
}

// ##########################################################################
// # DELETED BODY — `extract_rhs_kind_from_assign`, AND `classify_literal`  #
// # WITH IT. DO NOT RESTORE EITHER AND DO NOT RETURN `None`.               #
// #                                                                        #
// # `extract_rhs_kind_from_assign` located the assigned value by taking    #
// # the rest of the target's LINE and cutting it at the first `=`:         #
// #                                                                        #
// #   let eq_pos = after_target.find('=')?;                                #
// #   let rhs = after_target.get(eq_pos + 1..)?.trim();                    #
// #                                                                        #
// # so `obj.attr == other` (a comparison, not an assignment) yielded the   #
// # "value" `= other`, every augmented assignment (`+=`, `//=`, `**=`)     #
// # yielded a value one character short, and a value continued onto the    #
// # next line yielded nothing.                                             #
// #                                                                        #
// # `classify_literal` then decided the KIND from leading characters:      #
// #                                                                        #
// #   if text.bytes().next()?.is_ascii_digit() {                           #
// #       if text.contains('.') { FloatLiteral } else { IntLiteral }       #
// #   }                                                                    #
// #   if text.starts_with("b\"") … { BytesLiteral }                        #
// #   if text.starts_with("None")  { NoneValue }                           #
// #                                                                        #
// # `1e3` and `1E3` are floats with no `.` in them and were called ints.   #
// # `1.method()` — sorry, `x.attr = 1 .bit_length()` — contains a `.` and  #
// # was called a float. `starts_with("None")` matched the NAME `NoneType`, #
// # and any identifier beginning with those four letters. `r"x"`, `rb"x"`, #
// # `u"x"`, and `"""x"""` matched nothing at all.                          #
// #                                                                        #
// # The resolver already reports `RhsKind` for ordinary assignments from   #
// # the AST; the missing case is `AnnAssign`, and the fix is to record it  #
// # there — not to re-read the file.                                       #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
fn extract_rhs_kind_from_assign(_source: &str, _target_span: Span) -> Option<RhsKind> {
    panic!(
        "basilisk-checker: `extract_rhs_kind_from_assign` and `classify_literal` were \
         DELETED because they located an assigned value by cutting a SOURCE LINE at \
         the first `=` and then classified it from its leading characters, calling \
         `1e3` an int and the name `NoneType` a `None`. They panic because the real \
         implementation — recording `RhsKind` for `AnnAssign` in the resolver, off the \
         literal `Expr` node — DOES NOT EXIST YET. Do not restore the character tests \
         and do not return `None` in their place."
    )
}
