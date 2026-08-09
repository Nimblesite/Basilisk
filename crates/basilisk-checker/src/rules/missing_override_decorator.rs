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

use super::Rule;

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
        candidate.is_protocol
    })
}

// ##########################################################################
// # DELETED BODY — `missing_override_decorator::check_class`. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
// #
// # `.filter(|base_name| base_name.as_str() != child.name)` — a SECOND string comparison papering over the first: it dropped any base sharing the child's rendered name to dodge the `class Client(httpx.Client)` self-ancestor bug (GitHub #278), which also drops legitimate inheritance from a same-named base.
// #
// # A base class's identity came from its RENDERED NAME, looked up in a map
// # keyed on `ClassInfo::name`. `ClassInfo::bases` is a `Vec<String>` the
// # resolver fills with "simple names only; complex expressions ignored", so:
// #   * a base reached through an alias  ->  MISSED
// #   * a dotted base (`httpx.Client`)   ->  collides with any local class
// #                                          sharing its trailing word
// #   * two classes with one rendered name -> a single map entry
// #
// # The replacement resolves each base EXPRESSION through the binding table
// # and keys the hierarchy on definition site. That needs base SPANS on
// # `ClassInfo`, which the resolver does not record yet.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
fn check_class(
    _child: &ClassInfo,
    _class_map: &HashMap<&str, (&ClassInfo, bool)>,
    _func_map: &HashMap<(&str, &str), &FunctionInfo>,
    _path: &str,
    _out: &mut Vec<Diagnostic>,
) {
    panic!(
        "basilisk-checker: `missing_override_decorator::check_class` was DELETED because it identified base classes by \
         their RENDERED NAMES in a name-keyed map, so an aliased base missed and a \
         dotted base collided with any local class sharing its trailing word. It panics \
         because the real implementation — base expressions resolved through the binding \
         table — DOES NOT EXIST YET. Do not restore the name lookup and do not \
         substitute a default answer in its place."
    )
}

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; this diagnostic constructor is \
              correct and is retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
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
