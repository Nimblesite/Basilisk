//! Implements [BSK-0025] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! BSK-0025: Missing `@override` decorator.
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
//!
//! Version gate (issue #171): `@override` (PEP 698 / `typing.override`) was
//! introduced in Python 3.12, so suggesting it on an older configured target is
//! a false positive — the decorator cannot be imported there. BSK-0025 is silent
//! when the configured `python_version` is below 3.12.

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, FunctionInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::{guards::is_protocol_class, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-0025",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-0025",
};

/// The first Python version with `typing.override` (PEP 698).
const OVERRIDE_MIN_VERSION: (u32, u32) = (3, 12);

/// Emits BSK-0025 for methods that override a same-module base-class method
/// but are not decorated with `@override`.
pub(crate) struct MissingOverrideDecorator;

impl Rule for MissingOverrideDecorator {
    fn opt_in_spec(&self) -> Option<crate::rule_tags::OptInSpec> {
        Some(crate::rule_tags::OptInSpec {
            code: CODE.code,
            tags: &["strictness"],
        })
    }

    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // `@override` (PEP 698) only exists from Python 3.12 — don't suggest it
        // on an older configured target (issue #171).
        let Some(target_version) = ctx.target_version else {
            return;
        };
        if target_version < OVERRIDE_MIN_VERSION {
            return;
        }

        // Build a raw class map first (name → ClassInfo).
        let raw_map = super::shared::class_name_map(&module.classes);

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

        // Build a map from (class_name, method_name) → FunctionInfo for span lookup.
        // For overloaded methods the implementation (last entry) is preferred.
        let func_map: HashMap<(&str, &str), &FunctionInfo> = module
            .functions
            .iter()
            .filter_map(|f| {
                f.class_name
                    .as_deref()
                    .map(|cls| ((cls, f.name.as_str()), f))
            })
            .collect();

        module.classes.iter().for_each(|child| {
            check_class(child, &class_map, &func_map, &module.path, diagnostics);
        });
    }
}

/// Returns `true` when `cls` is a Protocol class directly or transitively
/// (i.e., any base class in `class_map` is itself a Protocol). The shared
/// walk breaks base-name cycles (GitHub #278): `class Client(httpx.Client)`
/// records its base under the attribute name `Client`, which the by-name
/// class map resolves back to the class itself.
fn is_protocol_transitively<'a>(
    cls: &'a ClassInfo,
    class_map: &HashMap<&str, &'a ClassInfo>,
) -> bool {
    super::shared::class_or_base_matches(cls, &|name| class_map.get(name).copied(), &|candidate| {
        is_protocol_class(candidate)
    })
}

fn check_class(
    child: &ClassInfo,
    class_map: &HashMap<&str, (&ClassInfo, bool)>,
    func_map: &HashMap<(&str, &str), &FunctionInfo>,
    path: &str,
    out: &mut Vec<Diagnostic>,
) {
    // Skip if this class itself is a Protocol.
    if is_protocol_class(child) {
        return;
    }

    // Collect base method names, skipping Protocol bases (Protocol methods
    // need implementation, not @override). A base name that resolves to the
    // class itself is a name collision with an external base (`class
    // Client(httpx.Client)`), not inheritance — a class never overrides its
    // own methods (GitHub #278).
    let base_methods: Vec<&str> = child
        .bases
        .iter()
        .filter(|base_name| base_name.as_str() != child.name)
        .filter_map(|base_name| class_map.get(base_name.as_str()))
        .filter(|(_, is_proto)| !is_proto)
        .flat_map(|(cls, _)| cls.method_names.iter().map(String::as_str))
        .collect();

    if base_methods.is_empty() {
        return;
    }

    // Deduplicate method names: overloaded methods appear once per overload
    // variant.  We want exactly one diagnostic per unique method name.
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for method_name in &child.method_names {
        let name = method_name.as_str();
        if !seen.insert(name) {
            continue; // already processed this name
        }
        if !base_methods.contains(&name) {
            continue;
        }
        // Dunder methods (__init__, __post_init__, __eq__, etc.) are part of
        // Python's data model and are expected to be overridden without @override.
        if name.starts_with("__") && name.ends_with("__") {
            continue;
        }
        // Report at the method's own span; fall back to the class name span.
        let span = func_map
            .get(&(child.name.as_str(), name))
            .map_or(child.name_span, |f| f.name_span);
        out.push(make_diagnostic(child, name, span, path));
    }
}

fn make_diagnostic(
    class: &ClassInfo,
    method_name: &str,
    span: basilisk_resolver::Span,
    path: &str,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Method `{}` in class `{}` overrides a base-class method but is missing `@override`",
            method_name, class.name
        ),
        span,
        path,
        Some(format!(
            "Add `@override` above `def {method_name}(...)` to make the override explicit"
        )),
        Some(
            "`@override` (PEP 698) makes overrides explicit and lets the type checker \
             catch typos in method names"
                .to_owned(),
        ),
    )
}
