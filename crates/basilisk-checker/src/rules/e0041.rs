//! BSK-E0041: Too few arguments in a function call.
//!
//! When a function is called with fewer positional arguments than it has
//! required parameters (parameters without default values), Basilisk reports
//! a missing-argument error. Handles overloaded functions by checking all
//! overload signatures.
//!
//! Also validates constructor calls: when a class is instantiated and the
//! metaclass `__call__` passes through arguments (uses `*args, **kwargs`),
//! the `__new__` or `__init__` method signature is checked for missing
//! required arguments.
//!
//! ```python
//! def func1(a: int, b: str) -> None: ...
//!
//! func1()  # E: missing required arguments
//! func1(1)  # E: missing required argument `b`
//! ```

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, FunctionInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0041",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0041",
};

/// Emits BSK-E0041 for call sites with too few positional arguments.
pub(crate) struct TooFewArguments;

impl Rule for TooFewArguments {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        check_plain_function_calls(module, diagnostics);
        check_constructor_calls(module, diagnostics);
    }
}

/// Check plain (non-constructor) function calls for too few arguments.
fn check_plain_function_calls(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    // Group functions by name (module-level functions only).
    let mut func_groups: HashMap<&str, Vec<&FunctionInfo>> = HashMap::new();
    for func in &module.functions {
        if func.class_name.is_none() {
            func_groups
                .entry(func.name.as_str())
                .or_default()
                .push(func);
        }
    }

    for call in &module.calls {
        let Some(funcs) = func_groups.get(call.callee.as_str()) else {
            continue;
        };

        // Skip if there are keyword arguments (conservative approach)
        if !call.keywords.is_empty() {
            continue;
        }

        let provided_count = call.args.len();

        // Check if ANY overload matches the argument count
        let mut has_matching_overload = false;
        let mut min_required_args = usize::MAX;

        for func in funcs {
            // Skip functions with *args (they accept any number of positional args)
            if func.vararg.is_some() {
                has_matching_overload = true;
                break;
            }

            // Count required parameters (those without defaults)
            let required_count = func.parameters.iter().filter(|p| !p.has_default).count();

            min_required_args = min_required_args.min(required_count);

            // Check if this overload matches the argument count
            if provided_count >= required_count {
                has_matching_overload = true;
                break;
            }
        }

        // If no overload matches and we have overloads, emit an error
        if !has_matching_overload && funcs.len() > 1 {
            let func_name = &call.callee;
            let missing = min_required_args - provided_count;
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Call to `{func_name}` is missing {missing} required argument{} \
                     (no overload matches — expected at least {min_required_args}, got {provided_count})",
                    if missing == 1 { "" } else { "s" },
                ),
                span: call.span,
                path: module.path.clone(),
                help: None,
                note: None,
            });
        } else if !has_matching_overload {
            // Single function case (no overloads)
            let func = funcs[0];
            if func.vararg.is_some() {
                continue; // *args accepts any number
            }

            let required_count = func.parameters.iter().filter(|p| !p.has_default).count();
            if provided_count < required_count {
                let missing = required_count - provided_count;
                let func_name = &func.name;
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Call to `{func_name}` is missing {missing} required argument{} \
                         (expected {required_count}, got {provided_count})",
                        if missing == 1 { "" } else { "s" },
                    ),
                    span: call.span,
                    path: module.path.clone(),
                    help: None,
                    note: None,
                });
            }
        }
    }
}

/// Returns `true` when the metaclass `__call__` method passes through arguments
/// to the underlying `__new__`/`__init__` (i.e. its `__call__` uses `*args, **kwargs`
/// and does not have a fixed return type that overrides the constructor).
///
/// When the metaclass `__call__` has a concrete, non-generic return type that is
/// NOT the class being constructed (e.g. `-> NoReturn`, `-> int | Meta`), the
/// metaclass fully controls the constructor call and we should NOT validate
/// arguments against `__new__`/`__init__`.
fn metaclass_passes_through(
    metaclass_name: &str,
    classes: &[ClassInfo],
    functions: &[FunctionInfo],
) -> bool {
    // First check that the metaclass class exists
    if !classes.iter().any(|c| c.name == metaclass_name) {
        return false;
    }

    // Find the metaclass __call__ method
    let call_method = functions.iter().find(|f| {
        f.class_name.as_deref() == Some(metaclass_name) && f.name == "__call__"
    });

    let Some(call_fn) = call_method else {
        // No __call__ method on metaclass — default type.__call__ passes through
        return true;
    };

    // The metaclass __call__ must use *args and **kwargs to pass through
    call_fn.vararg.is_some() && call_fn.kwarg.is_some()
}

/// Check constructor calls (class instantiation) for too few arguments.
///
/// When a class is called as a constructor, we validate arguments against the
/// class's `__new__` or `__init__` method, but only if the metaclass `__call__`
/// passes arguments through (uses `*args, **kwargs`).
fn check_constructor_calls(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    // Build a map of class names for quick lookup.
    let class_map: HashMap<&str, &ClassInfo> = module
        .classes
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect();

    // Build a map of (class_name, method_name) → FunctionInfo for methods.
    let mut method_map: HashMap<(&str, &str), Vec<&FunctionInfo>> = HashMap::new();
    for func in &module.functions {
        if let Some(ref cls_name) = func.class_name {
            method_map
                .entry((cls_name.as_str(), func.name.as_str()))
                .or_default()
                .push(func);
        }
    }

    for call in &module.calls {
        // Only process constructor calls (callee matches a class name)
        let Some(class_info) = class_map.get(call.callee.as_str()) else {
            continue;
        };

        // Skip if there are keyword arguments (conservative approach)
        if !call.keywords.is_empty() {
            continue;
        }

        // Check metaclass: if the class has a metaclass that does NOT pass through
        // arguments, skip validation (the metaclass controls the call signature).
        if let Some(ref meta_name) = class_info.metaclass_name {
            if !metaclass_passes_through(meta_name, &module.classes, &module.functions) {
                continue;
            }
        }

        // Find the constructor method to validate against.
        // Priority: __new__ first, then __init__.
        let constructor_method = find_constructor_method(
            class_info,
            &method_map,
        );

        let Some(constructor) = constructor_method else {
            continue;
        };

        // For constructor methods, the first parameter (cls/self) is implicit.
        // Skip it when counting required parameters.
        let required_count = constructor
            .parameters
            .iter()
            .skip(1) // skip cls/self
            .filter(|p| !p.has_default)
            .count();

        // Constructor with *args accepts any number of positional args
        if constructor.vararg.is_some() {
            continue;
        }

        let provided_count = call.args.len();
        if provided_count < required_count {
            let missing = required_count - provided_count;
            let class_name = &class_info.name;
            let method_name = &constructor.name;
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Call to `{class_name}()` is missing {missing} required argument{} \
                     for `{method_name}` (expected {required_count}, got {provided_count})",
                    if missing == 1 { "" } else { "s" },
                ),
                span: call.span,
                path: module.path.clone(),
                help: None,
                note: None,
            });
        }
    }
}

/// Find the constructor method (`__new__` or `__init__`) to validate against.
///
/// Returns the `__new__` method if it exists and has a non-trivial signature
/// (not just `*args, **kwargs`), otherwise falls back to `__init__`.
fn find_constructor_method<'a>(
    class_info: &ClassInfo,
    method_map: &HashMap<(&str, &str), Vec<&'a FunctionInfo>>,
) -> Option<&'a FunctionInfo> {
    let class_name = class_info.name.as_str();

    // Try __new__ first
    if let Some(new_methods) = method_map.get(&(class_name, "__new__")) {
        // Use the first non-overload __new__, or the first one
        let new_fn = new_methods
            .iter()
            .find(|f| !f.decorators.iter().any(|d| d == "overload"))
            .or_else(|| new_methods.first());
        if let Some(func) = new_fn {
            return Some(func);
        }
    }

    // Fall back to __init__
    if let Some(init_methods) = method_map.get(&(class_name, "__init__")) {
        let init_fn = init_methods
            .iter()
            .find(|f| !f.decorators.iter().any(|d| d == "overload"))
            .or_else(|| init_methods.first());
        if let Some(func) = init_fn {
            return Some(func);
        }
    }

    None
}

