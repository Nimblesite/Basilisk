//! BSK-E0025: Missing `@override` decorator.
//!
//! When a class overrides a method that is also defined in one of its base
//! classes (both defined within the same module), the overriding method must
//! carry the `@override` decorator (PEP 698 / `typing.override`).
//!
//! The check is limited to base classes that appear in the same source module,
//! because Basilisk cannot inspect the base class body without resolving
//! cross-module imports in Phase 1.

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0025",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0025",
};

/// Emits BSK-E0025 for methods that override a same-module base-class method
/// but are not decorated with `@override`.
pub(crate) struct MissingOverrideDecorator;

impl Rule for MissingOverrideDecorator {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build a map from class name → method names for fast lookup.
        let method_map: HashMap<&str, &[String]> = module
            .classes
            .iter()
            .map(|cls| (cls.name.as_str(), cls.method_names.as_slice()))
            .collect();

        module
            .classes
            .iter()
            .for_each(|child| check_class(child, &method_map, &module.path, diagnostics));
    }
}

fn check_class(
    child: &ClassInfo,
    method_map: &HashMap<&str, &[String]>,
    path: &str,
    out: &mut Vec<Diagnostic>,
) {
    // Only consider classes that inherit from at least one same-module class.
    let base_methods: Vec<&str> = child
        .bases
        .iter()
        .filter_map(|base_name| method_map.get(base_name.as_str()))
        .flat_map(|methods| methods.iter().map(String::as_str))
        .collect();

    if base_methods.is_empty() {
        return;
    }

    for method_name in &child.method_names {
        if !base_methods.contains(&method_name.as_str()) {
            // Method is not in any resolved base class — not an override.
            continue;
        }

        if method_has_override_decorator(&child.method_decorators, method_name) {
            continue;
        }

        // Find the name span via the class def_span as a fallback anchor.
        // We use the class def_span as we don't have individual method spans.
        out.push(make_diagnostic(child, method_name, path));
    }
}

/// Returns `true` when `method_name` has an `@override` (or `typing.override`)
/// decorator recorded in `method_decorators`.
fn method_has_override_decorator(
    method_decorators: &[(String, Vec<String>)],
    method_name: &str,
) -> bool {
    method_decorators
        .iter()
        .filter(|(name, _)| name == method_name)
        .flat_map(|(_, decorators)| decorators.iter())
        .any(|d| d == "override" || d.ends_with(".override"))
}

fn make_diagnostic(class: &ClassInfo, method_name: &str, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Method `{}` in class `{}` overrides a base-class method but is missing `@override`",
            method_name, class.name
        ),
        // Use the class name span as the anchor; method-level spans are not
        // available in Phase 1 resolver output.
        span: class.name_span,
        path: path.to_owned(),
        help: Some(format!(
            "Add `@override` above `def {method_name}(...)` to make the override explicit"
        )),
        note: Some(
            "`@override` (PEP 698) makes overrides explicit and lets the type checker \
             catch typos in method names"
                .to_owned(),
        ),
    }
}
