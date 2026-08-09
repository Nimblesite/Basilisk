//! Implements [`classes_override_2`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `classes_override_2`: Incompatible class attribute override.
//!
//! When a child class declares an attribute that also exists in a same-module
//! base class but with a different type annotation, Basilisk reports an
//! incompatible override.
//!
//! ```python
//! class Base:
//!     count: int = 0
//!
//! class Child(Base):
//!     count: str = "zero"   # annotation changed from int to str → E0017
//! ```

use std::collections::HashMap;

use basilisk_resolver::{AttributeInfo, ClassGraph, ClassInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

// ######################################################################
// # `is_typed_dict_hierarchy` IS GONE, NOT REBUILT IN PLACE.            #
// #                                                                    #
// # Its body was:                                                      #
// #                                                                    #
// #   class_map.get(base.as_str()).is_some_and(|b| b.is_typed_dict)    #
// #                                                                    #
// # It took a base class's identity from its RENDERED NAME and looked  #
// # that string up in a name-keyed map. A base reached through an      #
// # alias missed; a base sharing a rendered name with an unrelated     #
// # local class matched. This gate decides whether the whole rule runs #
// # at all, so a wrong answer here silently disabled E0017 or applied  #
// # it to a TypedDict where it produces only false positives.          #
// #                                                                    #
// # `ClassGraph::is_typed_dict` answers exactly this question from     #
// # resolved bases keyed on definition site, so the wrapper had no     #
// # work left to do. The call site below asks the graph directly.      #
// #                                                                    #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs         #
// ######################################################################

const CODE: ErrorCode = ErrorCode {
    code: "classes_override_2",
    docs_url: "https://www.basilisk-python.dev/errors/classes_override_2",
};

/// Emits `classes_override_2` for class attributes that override a base-class attribute
/// with a different type annotation.
pub(crate) struct IncompatibleVariableOverride;

impl Rule for IncompatibleVariableOverride {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let graph = ClassGraph::new(&module.classes);

        // Build map: (class DEFINITION SITE, attr_name) → &AttributeInfo, so
        // an attribute is attached to the class that declares it rather than
        // to every class sharing its rendered name.
        let attr_map: HashMap<(Span, &str), &AttributeInfo> = module
            .classes
            .iter()
            .flat_map(|cls| {
                cls.attributes
                    .iter()
                    .map(move |attr| ((cls.name_span, attr.name.as_str()), attr))
            })
            .collect();

        module.classes.iter().for_each(|child| {
            // TypedDict hierarchies have their own subtyping rules — skip.
            if graph.is_typed_dict(child) {
                return;
            }
            check_class(
                &graph,
                child,
                &attr_map,
                &module.source,
                &module.path,
                diagnostics,
            );
        });
    }
}

// ##########################################################################
// # DELETED BODY — `check_class`. DO NOT RESTORE IT OR RETURN EARLY.        #
// #                                                                         #
// # The class walk and attribute lookup had been rebuilt on definition      #
// # sites, but the verdict at the end still read both annotation SPANS back #
// # out of the source and compared their characters:                        #
// #                                                                         #
// #   let child_ann = annotation_text(source, attr.annotation_span);        #
// #   let base_ann = annotation_text(source, base_attr.annotation_span);    #
// #   child_ann != base_ann                                                  #
// #                                                                         #
// # That makes formatting, aliases, qualification, and forward-reference    #
// # quotes part of type compatibility. `int` and `builtins.int` conflict,   #
// # while two unrelated classes rendered with one spelling agree. The sound #
// # definition-site walk around that comparison cannot make the comparison  #
// # lawful, so the ENTIRE verdict body is deleted.                           #
// #                                                                         #
// # The rebuild must retrieve both original annotation `Expr` nodes by      #
// # their spans, lower them through the binding table, and report only a     #
// # definite semantic incompatibility. An unresolvable side abstains.        #
// #                                                                         #
// # Pinned by: tests/pep_spelling_invariance_pin_tests.rs                    #
// ##########################################################################

/// DELETED — panics; see the banner above.
fn check_class(
    _graph: &ClassGraph<'_>,
    _child: &ClassInfo,
    _attr_map: &HashMap<(Span, &str), &AttributeInfo>,
    _source: &str,
    _path: &str,
    _out: &mut Vec<Diagnostic>,
) {
    panic!(
        "basilisk-checker: `classes_override_2::check_class` was DELETED because it \
         decided override compatibility by comparing the SOURCE SPELLINGS of the child \
         and base annotations. It panics because the real implementation — resolving \
         both original annotation `Expr` nodes and relating their binding-backed types — \
         DOES NOT EXIST YET. Do not restore the text comparison and do not return early \
         or emit no diagnostic in its place."
    )
}

// ##########################################################################
// # DELETED BODY — `annotation_text`. DO NOT RESTORE IT.                   #
// #                                                                         #
// #   slice_span(source, span?)                                            #
// #                                                                         #
// # Innocuous on its own; the defect is what the caller does with it. The  #
// # override check compares a child attribute's annotation to the base's   #
// # AS WRITTEN and reports a mismatch when the characters differ, so this  #
// # rule's whole verdict is a spelling comparison:                         #
// #                                                                         #
// #   * `x: int` overriding `x: builtins.int` reads as a type mismatch;    #
// #   * `Alias = int` used in one of the two reads as a mismatch;          #
// #   * `list[int]` vs `list[ int ]` — pure formatting — reads as one too; #
// #   * two genuinely different classes SPELLED alike read as a match, so  #
// #     the real error goes unreported.                                     #
// #                                                                         #
// # The lawful replacement resolves both annotation spans to their `Expr`  #
// # nodes through `rules::shared::ExprIndex` and relates the two lowered   #
// # types, reporting only on a proven incompatibility and abstaining when  #
// # either side is unresolvable ([CHKARCH-CONFORMANCE-MODE]).              #
// ##########################################################################

/// DELETED — panics; see the banner above.
#[expect(
    dead_code,
    reason = "caller deleted with the source-spelling verdict; retained as part of the rebuild map"
)]
fn annotation_text(_source: &str, _span: Option<Span>) -> Option<&str> {
    panic!(
        "basilisk-checker: `annotation_text` was DELETED because `classes_override_2` \
         decided override compatibility by comparing the child's and base's annotation \
         SOURCE CHARACTERS. It panics because the real implementation — both annotations \
         resolved to type expressions and related semantically — DOES NOT EXIST YET. Do \
         not restore the slice and do not return `None` in its place."
    )
}

#[expect(
    dead_code,
    clippy::too_many_arguments,
    reason = "orphaned by the deleted verdict body; retained with its diagnostic contract"
)]
fn make_diagnostic(
    attr: &AttributeInfo,
    attr_name: &str,
    child_class: &str,
    base_class: &str,
    child_ann: Option<&str>,
    base_ann: Option<&str>,
    path: &str,
) -> Diagnostic {
    let child_ann_str = child_ann.unwrap_or("unknown");
    let base_ann_str = base_ann.unwrap_or("unknown");
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Attribute `{attr_name}` in `{child_class}` has type `{child_ann_str}` but \
             base class `{base_class}` declares it as `{base_ann_str}`"
        ),
        attr.name_span,
        path,
        Some(format!(
            "Change the annotation of `{attr_name}` in `{child_class}` to `{base_ann_str}` \
             to match the base class, or restructure the class hierarchy"
        )),
        Some(
            "In Basilisk, child class attributes must have type-compatible annotations \
             with any same-name attributes in base classes"
                .to_owned(),
        ),
    )
}
