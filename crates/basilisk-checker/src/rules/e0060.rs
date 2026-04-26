//! BSK-E0060: Cross-type ordering comparison of `order=True` dataclass instances.
//!
//! When `@dataclass(order=True)`, Python synthesizes `__lt__`, `__le__`, `__gt__`,
//! and `__ge__` methods.  These methods raise `TypeError` at runtime if the other
//! operand is not an instance of the **same** class.  Comparing two `order=True`
//! dataclass instances of different types with `<`, `<=`, `>`, or `>=` is therefore
//! a type error.
//!
//! ```python
//! from dataclasses import dataclass
//!
//! @dataclass(order=True)
//! class DC1:
//!     a: str
//!
//! @dataclass(order=True)
//! class DC2:
//!     a: str
//!
//! dc1 = DC1("x")
//! dc2 = DC2("y")
//!
//! if dc1 < dc2:   # E: incompatible types
//!     pass
//! ```

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0060",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0060",
};

/// Emits BSK-E0060 for ordering comparisons between instances of different `order=True` dataclasses.
pub(crate) struct CrossTypeDataclassOrderComparison;

impl Rule for CrossTypeDataclassOrderComparison {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Collect the set of order=True dataclass names.
        let order_classes: std::collections::HashSet<&str> = module
            .classes
            .iter()
            .filter(|cls| cls.is_dataclass && cls.is_dataclass_order)
            .map(|cls| cls.name.as_str())
            .collect();

        if order_classes.is_empty() {
            return;
        }

        // Build a map from variable name to the class it was instantiated from,
        // but only for order=True dataclass instances.
        let source = &module.source;
        let var_class: HashMap<&str, &str> = module
            .module_vars
            .iter()
            .filter_map(|var| {
                let rhs_span = var.rhs_span?;
                let rhs_text = source.get(rhs_span.start as usize..rhs_span.end as usize)?;
                // Extract callee name: text before '(' or '['
                let callee = rhs_text.split(['(', '[']).next()?.trim();
                // Strip any module prefix (e.g. `mod.ClassName` -> `ClassName`)
                let callee = callee.rsplit('.').next().unwrap_or(callee);
                if order_classes.contains(callee) {
                    Some((var.name.as_str(), callee))
                } else {
                    None
                }
            })
            .collect();

        if var_class.is_empty() {
            return;
        }

        for cmp in &module.module_order_comparisons {
            let Some(&left_class) = var_class.get(cmp.left_name.as_str()) else {
                continue;
            };
            let Some(&right_class) = var_class.get(cmp.right_name.as_str()) else {
                continue;
            };
            // Only flag when both operands are order=True dataclass instances
            // from *different* classes.
            if left_class == right_class {
                continue;
            }
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Cannot compare `{left_class}` and `{right_class}` with ordering operator: \
                     `@dataclass(order=True)` comparison methods only accept the same type"
                ),
                span: cmp.span,
                path: module.path.clone(),
                help: Some(
                    "Ordering comparisons (`<`, `<=`, `>`, `>=`) between different dataclass \
                     types are not supported"
                        .to_owned(),
                ),
                note: Some(
                    "PEP 557: the synthesized `__lt__` etc. return `NotImplemented` for \
                     instances of a different type"
                        .to_owned(),
                ),
            });
        }
    }
}
