//! BSK-E0005: Missing class attribute type annotation.
//!
//! Every class attribute declared in the class body must have an explicit type
//! annotation.  Without one, Basilisk cannot verify assignments to the
//! attribute and cannot produce accurate stub types.
//!
//! Enum subclasses and Protocol subclasses are exempt: Enum members have
//! metaclass-synthesised `Literal[...]` types, and Protocol attributes are
//! interface specifications rather than concrete class variables.

use basilisk_resolver::{AttributeInfo, ClassInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::{
    guards::{is_enum_class, is_namedtuple_class, is_protocol_class},
    Rule,
};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0005",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0005",
};

/// Emits BSK-E0005 for every unannotated class attribute.
pub(crate) struct MissingAttributeAnnotation;

impl Rule for MissingAttributeAnnotation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Collect all TypeVar names (module-level and class-body) so we can
        // exempt unannotated TypeVar assignments like `T = TypeVar("T")` from E0005.
        let typevar_names: std::collections::HashSet<&str> = module
            .typevar_calls
            .iter()
            .map(|tv| tv.name.as_str())
            .collect();

        module
            .classes
            .iter()
            .filter(|class| {
                !is_enum_class(class) && !is_protocol_class(class) && !is_namedtuple_class(class)
            })
            .for_each(|class| check_class(class, &module.path, &typevar_names, diagnostics));
    }
}

fn check_class(
    class: &ClassInfo,
    path: &str,
    typevar_names: &std::collections::HashSet<&str>,
    out: &mut Vec<Diagnostic>,
) {
    class
        .attributes
        .iter()
        .filter(|attr| !attr.has_annotation && !typevar_names.contains(attr.name.as_str()))
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
