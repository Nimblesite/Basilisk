//! BSK-E0142: `dataclass_transform` violations when the transform is applied via a base class.
//!
//! When a class is decorated with `@dataclass_transform(...)`, subclasses that inherit
//! from it behave like dataclasses with the transform's default settings overridable by
//! keyword arguments on the class definition.
//!
//! This rule detects:
//! 1. A non-frozen subclass inheriting from a frozen transform-class (line 51).
//! 2. Attribute assignment on a frozen transform-class instance (lines 63, 122).
//! 3. Positional arguments to a `kw_only` transform-class constructor (lines 66, 82).
//! 4. Comparison operators on transform-class instances that lack `order=True` (line 72).
//!
//! ```python
//! from typing import dataclass_transform
//!
//! @dataclass_transform(kw_only_default=True)
//! class ModelBase: ...
//!
//! class Customer(ModelBase, frozen=True):
//!     id: int
//!
//! c = Customer(3)           # E — kw_only requires keyword args
//! c.id = 4                  # E — frozen instance is immutable
//! ```

mod helpers;

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;
use crate::span_util::slice_span;

use super::Rule;

use helpers::{
    check_frozen_inheritance, check_frozen_instance_assignment, check_kw_only_positional_args,
    check_no_order_comparison, collect_transform_base_classes, collect_transform_subclasses,
    resolve_inherited_settings, TransformClassSettings,
};

/// Emits BSK-E0142 for `dataclass_transform` violations via class-based transform.
pub(crate) struct DataclassTransformClassViolation;

impl Rule for DataclassTransformClassViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let transform_bases = collect_transform_base_classes(module);
        if transform_bases.is_empty() {
            return;
        }

        let direct_settings = collect_transform_subclasses(module, &transform_bases);
        if direct_settings.is_empty() {
            return;
        }

        let source = &module.source;
        let path = &module.path;

        // Build a map of all transform-class instances at module level:
        // variable_name -> (class_name, settings).
        let mut instance_map: HashMap<&str, (&str, TransformClassSettings)> = HashMap::new();
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

            if let Some(settings) = resolve_inherited_settings(callee, module, &direct_settings) {
                let _ = instance_map.insert(var.name.as_str(), (callee, settings));
            }
        }

        // --- Check 1: Non-frozen subclass inheriting from a frozen transform class ---
        check_frozen_inheritance(module, &direct_settings, path, diagnostics);

        // --- Check 2: Attribute assignment on frozen transform-class instances ---
        check_frozen_instance_assignment(module, &instance_map, path, diagnostics);

        // --- Check 3: Positional args to kw_only transform-class constructor ---
        check_kw_only_positional_args(module, &direct_settings, source, path, diagnostics);

        // --- Check 4: Comparison operator on instance without order=True ---
        check_no_order_comparison(module, &instance_map, source, path, diagnostics);
    }
}
