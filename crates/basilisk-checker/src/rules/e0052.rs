//! BSK-E0052: Assignment to attribute of a frozen dataclass instance, or invalid
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

use crate::diagnostic::{Diagnostic, ErrorCode, error_diagnostic_owned};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0052",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0052",
};

/// Emits BSK-E0052 for:
/// - Attribute assignments on frozen dataclass instances at module level.
/// - Dataclass inheritance where frozen/non-frozen status is mixed.
pub(crate) struct FrozenDataclassAssignment;

impl Rule for FrozenDataclassAssignment {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let transform_classes = super::guards::collect_transform_classes(module);

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

fn check_inheritance(
    class_frozen: &HashMap<&str, (bool, bool)>,
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = &module.path;
    for cls in &module.classes {
        if !cls.is_dataclass {
            continue;
        }
        for base_name in &cls.bases {
            let Some(&(base_is_dc, base_is_frozen)) = class_frozen.get(base_name.as_str()) else {
                continue;
            };
            if !base_is_dc {
                continue;
            }
            if cls.is_dataclass_frozen && !base_is_frozen {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Frozen dataclass `{}` cannot inherit from non-frozen dataclass `{}`",
                        cls.name, base_name
                    ),
                    cls.def_span,
                    path,
                    Some(
                        "A frozen dataclass can only inherit from other frozen dataclasses"
                            .to_owned(),
                    ),
                    Some(
                        "PEP 557: mixing frozen and non-frozen dataclasses is not allowed"
                            .to_owned(),
                    ),
                ));
            } else if !cls.is_dataclass_frozen && base_is_frozen {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Non-frozen dataclass `{}` cannot inherit from frozen dataclass `{}`",
                        cls.name, base_name
                    ),
                    cls.def_span,
                    path,
                    Some(
                        "A non-frozen dataclass can only inherit from other non-frozen dataclasses"
                            .to_owned(),
                    ),
                    Some(
                        "PEP 557: mixing frozen and non-frozen dataclasses is not allowed"
                            .to_owned(),
                    ),
                ));
            }
        }
    }
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
            Some(
                "PEP 557: `@dataclass(frozen=True)` prohibits attribute assignment".to_owned(),
            ),
        ));
    }
}
