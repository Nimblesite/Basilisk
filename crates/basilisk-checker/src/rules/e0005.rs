//! BSK-E0005: Missing class attribute type annotation.
//!
//! Every class attribute declared in the class body must have an explicit type
//! annotation.  Without one, Basilisk cannot verify assignments to the
//! attribute and cannot produce accurate stub types.

use basilisk_resolver::{AttributeInfo, ClassInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0005",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0005",
};

/// Emits BSK-E0005 for every unannotated class attribute.
pub(crate) struct MissingAttributeAnnotation;

impl Rule for MissingAttributeAnnotation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module
            .classes
            .iter()
            .for_each(|class| check_class(class, &module.path, diagnostics));
    }
}

fn check_class(class: &ClassInfo, path: &str, out: &mut Vec<Diagnostic>) {
    class
        .attributes
        .iter()
        .filter(|attr| !attr.has_annotation)
        .for_each(|attr| out.push(make_diagnostic(attr, &class.name, path)));
}

fn make_diagnostic(attr: &AttributeInfo, class_name: &str, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Missing type annotation for attribute `{}` in class `{}`",
            attr.name, class_name
        ),
        span: attr.name_span,
        path: path.to_owned(),
        help: Some(format!("Add a type annotation: `{}: <type>`", attr.name)),
        note: Some(
            "In Basilisk, all class attributes require explicit type annotations".to_owned(),
        ),
    }
}
