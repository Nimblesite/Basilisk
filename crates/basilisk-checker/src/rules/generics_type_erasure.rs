//! Implements [`generics_type_erasure`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_type_erasure`: Access to instance attribute on a class object.
//!
//! Instance attributes (annotations in the class body that lack a default
//! value) exist only on instances, not on the class object itself.  Assigning
//! such an attribute on the class is an error.
//!
//! ```python
//! from typing import Generic, TypeVar
//!
//! T = TypeVar("T")
//!
//! class Node(Generic[T]):
//!     label: T
//!
//! Node.label = 1       # E: instance attribute on class
//! ```
//!
//! # Reduced coverage
//!
//! Only assignments the resolver reports structurally are checked. The
//! subscripted (`Node[int].label`), bare-read (`Node.label`) and
//! `type(var).attr` forms were recognised by scanning `module.source` line by
//! line — trimming each line, matching the literal prefixes `type(`, `.`, `[`
//! and `=`, counting brackets by hand, and slicing assignment right-hand sides
//! out of the source to guess which class a variable held. That is scanning
//! Python source for language vocabulary, which the project's first standing
//! rule forbids: recognition is a question about the AST, never about the
//! characters at the use site. The scanner and its helpers have been deleted;
//! those three forms go unreported until they are recovered structurally.

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_type_erasure",
    docs_url: "https://www.basilisk-python.dev/errors/generics_type_erasure",
};

/// Emits `generics_type_erasure` for accessing instance-only attributes on class objects.
pub(crate) struct InstanceAttrOnClass;

impl Rule for InstanceAttrOnClass {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let class_instance_attrs = collect_instance_only_attributes(module);
        if class_instance_attrs.is_empty() {
            return;
        }

        for assign in &module.module_attr_assignments {
            let Some(attrs) = class_instance_attrs.get(assign.object_name.as_str()) else {
                continue;
            };
            if attrs.contains(assign.attr_name.as_str()) {
                diagnostics.push(make_diagnostic(
                    &assign.object_name,
                    &assign.attr_name,
                    assign.target_span,
                    &module.path,
                ));
            }
        }
    }
}

/// Map each generic class to the attributes that exist only on its instances:
/// those carrying an annotation but no default value.
fn collect_instance_only_attributes(module: &ResolvedModule) -> HashMap<&str, HashSet<&str>> {
    module
        .classes
        .iter()
        .filter(|cls| !cls.generic_params.is_empty())
        .filter_map(|cls| {
            let attrs: HashSet<&str> = cls
                .attributes
                .iter()
                .filter(|attr| attr.has_annotation && !attr.has_value)
                .map(|attr| attr.name.as_str())
                .collect();
            (!attrs.is_empty()).then(|| (cls.name.as_str(), attrs))
        })
        .collect()
}

/// Build a diagnostic for instance attribute access on a class object.
fn make_diagnostic(object_name: &str, attr_name: &str, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!("Cannot access instance attribute `{attr_name}` on class object `{object_name}`"),
        span,
        path,
        Some(format!(
            "`{attr_name}` is an instance attribute and can only be accessed on instances, \
             not on the class itself"
        )),
        Some(
            "Instance attributes exist only on instances. \
             Use an instance to access them, e.g. `Node[int]().label`"
                .to_owned(),
        ),
    )
}
