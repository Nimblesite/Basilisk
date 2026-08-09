//! Implements [`dataclasses_frozen`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
//! `dataclasses_frozen`: Assignment to attribute of a frozen dataclass instance, or invalid
//! frozen/non-frozen dataclass inheritance.
//!
//! `@dataclass(frozen=True)` instances are immutable — their attributes cannot
//! be reassigned after construction.  Additionally, a frozen dataclass cannot
//! inherit from a non-frozen one, and vice versa.
//!
//! ```python
//! @dataclass(frozen=True)
//! class Point:
//!     x: float
//!
//! p = Point(1.0)
//! p.x = 2.0  # E: dataclass is frozen
//!
//! @dataclass          # E: non-frozen cannot inherit from frozen
//! class Sub(Point):
//!     pass
//! ```

use std::collections::{HashMap, HashSet};

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "dataclasses_frozen",
    docs_url: "https://www.basilisk-python.dev/errors/dataclasses_frozen",
};

/// Emits `dataclasses_frozen` for:
/// - Attribute assignments on frozen dataclass instances at module level.
/// - Dataclass inheritance where frozen/non-frozen status is mixed.
pub(crate) struct FrozenDataclassAssignment;

impl Rule for FrozenDataclassAssignment {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let transform_classes = super::guards::collect_transform_classes(module);

        // This map is keyed on `ClassInfo::name` — a RENDERED SPELLING. It is
        // built here only so the deleted `check_inheritance` call site stays
        // visible as the rebuild map; the callee panics before reading it.
        let class_frozen: HashMap<&str, (bool, bool)> = module
            .classes
            .iter()
            .map(|c| {
                let is_dc = c.is_dataclass || transform_classes.contains_key(c.name.as_str());
                let is_frozen = c.is_dataclass_frozen
                    || transform_classes
                        .get(c.name.as_str())
                        .is_some_and(|info| info.frozen);
                (c.name.as_str(), (is_dc, is_frozen))
            })
            .collect();

        check_inheritance(&class_frozen, module, diagnostics);
        check_frozen_instance_assigns(module, diagnostics);
    }
}

// ##########################################################################
// # DELETED BODY — `dataclasses_frozen::check_inheritance`. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
// #
// # `class_frozen.get(base_name.as_str())` decided frozen/non-frozen dataclass inheritance by rendered base name.
// #
// # `ClassInfo::bases` is a `Vec<String>` the resolver fills with "simple
// # names only; complex expressions ignored", and the lookup map is keyed on
// # `ClassInfo::name`. So a base reached through an alias MISSED, a dotted
// # base collided with any local class sharing its trailing word, and two
// # classes with one rendered name were a single entry.
// #
// # The replacement resolves each base EXPRESSION through the binding table
// # and keys the hierarchy on definition site. That needs base SPANS on
// # `ClassInfo`, which the resolver does not record yet.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
fn check_inheritance(
    _class_frozen: &HashMap<&str, (bool, bool)>,
    _module: &ResolvedModule,
    _diagnostics: &mut Vec<Diagnostic>,
) {
    panic!(
        "basilisk-checker: `dataclasses_frozen::check_inheritance` was DELETED because it identified base classes by \
         their RENDERED NAMES, so an aliased base missed and a dotted base collided with \
         any local class sharing its trailing word. It panics because the real \
         implementation — base expressions resolved through the binding table — DOES NOT \
         EXIST YET. Do not restore the name lookup and do not substitute a default \
         answer in its place."
    )
}

fn check_frozen_instance_assigns(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let source = &module.source;
    let path = &module.path;

    let transform_classes = super::guards::collect_transform_classes(module);

    let mut frozen_classes: HashSet<&str> =
        basilisk_resolver::collect_name_set_where(&module.classes, |c| c.is_dataclass_frozen);

    // Also include dataclass_transform classes that are frozen
    for (name, info) in &transform_classes {
        if info.frozen {
            let _ = frozen_classes.insert(name.as_str());
        }
    }

    if frozen_classes.is_empty() {
        return;
    }

    let mut instance_class: HashMap<&str, &str> = HashMap::new();
    for var in &module.module_vars {
        let Some(rhs_span) = var.rhs_span else {
            continue;
        };
        let Some(rhs_text) = slice_span(source, rhs_span) else {
            continue;
        };
        let callee = rhs_text.split(['(', '[']).next().unwrap_or("").trim();
        if callee.is_empty() {
            continue;
        }
        let callee = callee.rsplit('.').next().unwrap_or(callee);
        if frozen_classes.contains(callee) {
            let _ = instance_class.insert(var.name.as_str(), callee);
        }
    }

    for assign in &module.module_attr_assignments {
        let Some(&class_name) = instance_class.get(assign.object_name.as_str()) else {
            continue;
        };
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Cannot assign to attribute `{}` of frozen dataclass `{}` instance `{}`",
                assign.attr_name, class_name, assign.object_name
            ),
            assign.target_span,
            path,
            Some("Frozen dataclass instances are immutable after construction".to_owned()),
            Some("PEP 557: `@dataclass(frozen=True)` prohibits attribute assignment".to_owned()),
        ));
    }
}
