//! BSK-E0025: Missing `@override` decorator.
//!
//! When a class overrides a method that is also defined in one of its base
//! classes (both defined within the same module), the overriding method must
//! carry the `@override` decorator (PEP 698 / `typing.override`).
//!
//! The check is limited to base classes that appear in the same source module,
//! because Basilisk cannot inspect the base class body without resolving
//! cross-module imports in Phase 1.
//!
//! Protocol implementations are exempt: when a class satisfies a `Protocol`
//! contract, it is expected to define the protocol methods without `@override`.

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::{guards::is_protocol_class, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0025",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0025",
};

/// Emits BSK-E0025 for methods that override a same-module base-class method
/// but are not decorated with `@override`.
pub(crate) struct MissingOverrideDecorator;

impl Rule for MissingOverrideDecorator {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build a raw class map first (name → ClassInfo).
        let raw_map: HashMap<&str, &ClassInfo> = module
            .classes
            .iter()
            .map(|cls| (cls.name.as_str(), cls))
            .collect();

        // Determine which classes are Protocol (transitively) — e.g.
        // `class MyProto(SomeBase)` where `SomeBase(Protocol)` is also Protocol.
        let class_map: HashMap<&str, (&ClassInfo, bool)> = module
            .classes
            .iter()
            .map(|cls| {
                (
                    cls.name.as_str(),
                    (cls, is_protocol_transitively(cls, &raw_map)),
                )
            })
            .collect();

        module.classes.iter().for_each(|child| {
            check_class(child, &class_map, &module.path, diagnostics);
        });
    }
}

/// Returns `true` when `cls` is a Protocol class directly or transitively
/// (i.e., any base class in `class_map` is itself a Protocol).
fn is_protocol_transitively<'a>(
    cls: &'a ClassInfo,
    class_map: &HashMap<&str, &'a ClassInfo>,
) -> bool {
    if is_protocol_class(cls) {
        return true;
    }
    cls.bases.iter().any(|base| {
        class_map
            .get(base.as_str())
            .is_some_and(|base_cls| is_protocol_transitively(base_cls, class_map))
    })
}

fn check_class(
    child: &ClassInfo,
    class_map: &HashMap<&str, (&ClassInfo, bool)>,
    path: &str,
    out: &mut Vec<Diagnostic>,
) {
    // Skip if this class itself is a Protocol.
    if is_protocol_class(child) {
        return;
    }

    // Collect base method names, skipping Protocol bases (Protocol methods
    // need implementation, not @override).
    let base_methods: Vec<&str> = child
        .bases
        .iter()
        .filter_map(|base_name| class_map.get(base_name.as_str()))
        .filter(|(_, is_proto)| !is_proto)
        .flat_map(|(cls, _)| cls.method_names.iter().map(String::as_str))
        .collect();

    if base_methods.is_empty() {
        return;
    }

    for method_name in &child.method_names {
        if !base_methods.contains(&method_name.as_str()) {
            continue;
        }

        if method_has_override_decorator(&child.method_decorators, method_name) {
            continue;
        }

        // Always report at the class name span so the diagnostic sorts before
        // per-method diagnostics (E0001/E0002) that share the same method position.
        out.push(make_diagnostic(child, method_name, child.name_span, path));
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

fn make_diagnostic(
    class: &ClassInfo,
    method_name: &str,
    span: basilisk_resolver::Span,
    path: &str,
) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Method `{}` in class `{}` overrides a base-class method but is missing `@override`",
            method_name, class.name
        ),
        span,
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
