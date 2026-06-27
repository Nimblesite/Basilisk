//! Implements [`classes_override_3`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-ownership
//! `classes_override_3`: `@override` on a method with no matching ancestor method.
//!
//! PEP 698 — a method decorated `@override` (or `typing.override`) must actually
//! override a method declared in a base class. When no ancestor declares a
//! method of that name, the decorator is a lie and the type checker should
//! report it.
//!
//! To stay free of false positives the check is deliberately conservative: it
//! only fires when the *entire* ancestor chain is resolvable within the current
//! module (no `Any` base and no imported base whose methods we cannot see), so a
//! method that legitimately overrides something in an unseen base is never
//! flagged.
//!
//! ```python
//! class Base:
//!     def existing(self) -> int: ...
//!
//! class Child(Base):
//!     @override
//!     def missing(self) -> int:  # E0159: nothing named `missing` in any base
//!         return 1
//! ```

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ClassInfo, FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "classes_override_3",
    docs_url: "https://www.basilisk-python.dev/errors/classes_override_3",
};

/// Bound on base-class recursion, guarding against malformed inheritance cycles.
const MAX_DEPTH: u32 = 32;

/// Returns `true` for the `override` / `typing.override` decorator.
fn is_override(decorator: &str) -> bool {
    decorator == "override" || decorator.ends_with(".override")
}

/// Emits `classes_override_3` for `@override` methods that override nothing.
pub(crate) struct OverrideWithoutBaseMethod;

impl Rule for OverrideWithoutBaseMethod {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let class_map = super::shared::class_name_map(&module.classes);

        // (class, method) -> span, preferring the `@override`-decorated entry so
        // an overloaded method's diagnostic lands on the implementation line.
        let mut func_map: HashMap<(&str, &str), &FunctionInfo> = HashMap::new();
        for func in &module.functions {
            let Some(cls) = func.class_name.as_deref() else {
                continue;
            };
            let key = (cls, func.name.as_str());
            let replace = match func_map.get(&key) {
                None => true,
                Some(prev) => {
                    func.decorators.iter().any(|d| is_override(d))
                        && !prev.decorators.iter().any(|d| is_override(d))
                }
            };
            if replace {
                let _ = func_map.insert(key, func);
            }
        }

        for child in &module.classes {
            check_class(child, &class_map, &func_map, &module.path, diagnostics);
        }
    }
}

fn check_class(
    child: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
    func_map: &HashMap<(&str, &str), &FunctionInfo>,
    path: &str,
    out: &mut Vec<Diagnostic>,
) {
    // Only a class whose every transitive base is visible here can be checked —
    // otherwise an override target might live in a base we cannot see.
    if child.bases.is_empty() || !bases_fully_resolvable(child, class_map, 0) {
        return;
    }

    let mut ancestor_methods: HashSet<&str> = HashSet::new();
    collect_ancestor_methods(child, class_map, 0, &mut ancestor_methods);

    let mut seen: HashSet<&str> = HashSet::new();
    for (method_name, decorators) in &child.method_decorators {
        if !decorators.iter().any(|d| is_override(d)) {
            continue;
        }
        let name = method_name.as_str();
        if !seen.insert(name) {
            continue; // one diagnostic per overloaded group
        }
        // Dunder methods belong to the data model and may be overridden freely.
        if name.starts_with("__") && name.ends_with("__") {
            continue;
        }
        if ancestor_methods.contains(name) {
            continue;
        }
        let span = func_map
            .get(&(child.name.as_str(), name))
            .map_or(child.name_span, |f| f.name_span);
        out.push(make_diagnostic(name, &child.name, span, path));
    }
}

/// `true` when every transitive base of `cls` is a same-module class (no `Any`
/// base and no base imported from outside this module).
fn bases_fully_resolvable(
    cls: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
    depth: u32,
) -> bool {
    if depth >= MAX_DEPTH {
        return false;
    }
    cls.bases.iter().all(|base| {
        base != "Any"
            && base != "typing.Any"
            && class_map
                .get(base.as_str())
                .is_some_and(|base_cls| bases_fully_resolvable(base_cls, class_map, depth + 1))
    })
}

/// Gather every method name declared on a transitive base of `cls`.
fn collect_ancestor_methods<'a>(
    cls: &'a ClassInfo,
    class_map: &HashMap<&'a str, &'a ClassInfo>,
    depth: u32,
    acc: &mut HashSet<&'a str>,
) {
    if depth >= MAX_DEPTH {
        return;
    }
    for base in &cls.bases {
        if let Some(base_cls) = class_map.get(base.as_str()) {
            for method in &base_cls.method_names {
                let _ = acc.insert(method.as_str());
            }
            collect_ancestor_methods(base_cls, class_map, depth + 1, acc);
        }
    }
}

fn make_diagnostic(method: &str, class_name: &str, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Method `{method}` in `{class_name}` is decorated with `@override` but no matching \
             method exists in any base class"
        ),
        span,
        path,
        Some(format!(
            "Remove the `@override` decorator from `{method}`, or add the method it should override \
             to a base class"
        )),
        Some("`@override` (PEP 698) requires a base-class method of the same name".to_owned()),
    )
}
