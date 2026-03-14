//! Protocol visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtFunctionDef};
use ruff_text_size::Ranged;

use crate::scope::{
    ClassInfo, FunctionInfo, ProtocolInstantiationViolation, ProtocolSelfViolation,
};

use super::class_info_ext::expr_simple_name;
use super::core::{source_slice_range, source_slice_span, text_range_to_span};
use super::protocol_ext::{
    collect_protocol_required_attrs, collect_protocol_required_methods, collect_provided_members,
    collect_transitive_required_members, find_protocol_instantiations,
};

pub(super) fn collect_protocol_self_violations(
    stmts: &[Stmt],
    classes: &[ClassInfo],
    functions: &[FunctionInfo],
    source: &str,
) -> Vec<ProtocolSelfViolation> {
    // Build map of protocol class name -> list of method names that return `Self`.
    let protocol_self_methods: std::collections::HashMap<&str, Vec<&str>> = classes
        .iter()
        .filter(|cls| cls.bases.iter().any(|b| b == "Protocol"))
        .filter_map(|cls| {
            let self_methods: Vec<&str> = functions
                .iter()
                .filter(|f| f.class_name.as_deref() == Some(cls.name.as_str()))
                .filter(|f| {
                    f.return_annotation_span.is_some_and(|span| {
                        source_slice_span(source, span).map(str::trim) == Some("Self")
                    })
                })
                .map(|f| f.name.as_str())
                .collect();
            if self_methods.is_empty() {
                None
            } else {
                Some((cls.name.as_str(), self_methods))
            }
        })
        .collect();

    if protocol_self_methods.is_empty() {
        return Vec::new();
    }

    // Build map of free function name -> parameter annotations (name, annotation text).
    let func_param_types: std::collections::HashMap<&str, Vec<(&str, &str)>> = functions
        .iter()
        .filter(|f| f.class_name.is_none())
        .map(|f| {
            let param_types: Vec<(&str, &str)> = f
                .parameters
                .iter()
                .filter_map(|p| {
                    p.annotation_span.and_then(|span| {
                        source_slice_span(source, span)
                            .map(|ann_text| (p.name.as_str(), ann_text.trim()))
                    })
                })
                .collect();
            (f.name.as_str(), param_types)
        })
        .collect();

    // Build map of class name -> method name -> return annotation text.
    let class_method_returns: std::collections::HashMap<
        &str,
        std::collections::HashMap<&str, &str>,
    > = classes
        .iter()
        .map(|cls| {
            let method_returns: std::collections::HashMap<&str, &str> = functions
                .iter()
                .filter(|f| f.class_name.as_deref() == Some(cls.name.as_str()))
                .filter_map(|f| {
                    f.return_annotation_span.and_then(|span| {
                        source_slice_span(source, span)
                            .map(|ret_text| (f.name.as_str(), ret_text.trim()))
                    })
                })
                .collect();
            (cls.name.as_str(), method_returns)
        })
        .collect();

    let mut out = Vec::new();
    collect_protocol_violations_from_stmts(
        stmts,
        &protocol_self_methods,
        &func_param_types,
        &class_method_returns,
        source,
        &mut out,
    );
    out
}

/// Walk statements recursively to find function bodies with protocol violations.
pub(super) fn collect_protocol_violations_from_stmts(
    stmts: &[Stmt],
    protocol_self_methods: &std::collections::HashMap<&str, Vec<&str>>,
    func_param_types: &std::collections::HashMap<&str, Vec<(&str, &str)>>,
    class_method_returns: &std::collections::HashMap<&str, std::collections::HashMap<&str, &str>>,
    source: &str,
    out: &mut Vec<ProtocolSelfViolation>,
) {
    for stmt in stmts {
        if let Stmt::FunctionDef(func) = stmt {
            check_protocol_violations_in_function(
                func,
                protocol_self_methods,
                func_param_types,
                class_method_returns,
                source,
                out,
            );
            // Recurse into nested functions.
            collect_protocol_violations_from_stmts(
                &func.body,
                protocol_self_methods,
                func_param_types,
                class_method_returns,
                source,
                out,
            );
        }
    }
}

/// Check a single function body for calls that violate protocol `Self` conformance.
pub(super) fn check_protocol_violations_in_function(
    func: &StmtFunctionDef,
    protocol_self_methods: &std::collections::HashMap<&str, Vec<&str>>,
    func_param_types: &std::collections::HashMap<&str, Vec<(&str, &str)>>,
    class_method_returns: &std::collections::HashMap<&str, std::collections::HashMap<&str, &str>>,
    source: &str,
    out: &mut Vec<ProtocolSelfViolation>,
) {
    // Build a map from this function's parameter names to their annotation text.
    let enclosing_param_types: std::collections::HashMap<&str, &str> = func
        .parameters
        .posonlyargs
        .iter()
        .chain(func.parameters.args.iter())
        .chain(func.parameters.kwonlyargs.iter())
        .filter_map(|p| {
            p.parameter.annotation.as_deref().and_then(|ann| {
                let range = ann.range();
                source_slice_range(source, range)
                    .map(|text| (p.parameter.name.as_str(), text.trim()))
            })
        })
        .collect();

    if enclosing_param_types.is_empty() {
        return;
    }

    // Walk the function body looking for call expressions.
    for stmt in &func.body {
        let call_expr = match stmt {
            Stmt::Expr(expr_stmt) => {
                if let Expr::Call(call) = expr_stmt.value.as_ref() {
                    Some(call)
                } else {
                    None
                }
            }
            Stmt::Assign(assign) => {
                if let Expr::Call(call) = assign.value.as_ref() {
                    Some(call)
                } else {
                    None
                }
            }
            Stmt::AnnAssign(ann_assign) => ann_assign.value.as_deref().and_then(|val| {
                if let Expr::Call(call) = val {
                    Some(call)
                } else {
                    None
                }
            }),
            _ => None,
        };

        let Some(call) = call_expr else { continue };

        // Get the callee name (simple function call only).
        let Some(callee_name) = expr_simple_name(&call.func) else {
            continue;
        };

        // Check if the callee function has protocol-typed parameters.
        let Some(callee_params) = func_param_types.get(callee_name.as_str()) else {
            continue;
        };

        // Check each positional argument.
        for (arg_idx, arg) in call.arguments.args.iter().enumerate() {
            let Some((_param_name, param_type)) = callee_params.get(arg_idx) else {
                continue;
            };

            // Is this parameter typed as a protocol with Self-returning methods?
            let Some(required_methods) = protocol_self_methods.get(param_type) else {
                continue;
            };

            // The argument must be a simple name referencing an enclosing parameter.
            let Some(arg_name) = expr_simple_name(arg) else {
                continue;
            };

            // Resolve the argument's type via the enclosing function's parameters.
            let Some(arg_class_name) = enclosing_param_types.get(arg_name.as_str()) else {
                continue;
            };

            // Look up the argument class's methods.
            let Some(arg_methods) = class_method_returns.get(arg_class_name) else {
                continue;
            };

            // Check each required Self-returning method.
            for method_name in required_methods {
                let Some(actual_return) = arg_methods.get(method_name) else {
                    // Method missing entirely: different violation, skip here.
                    continue;
                };

                // The return type is acceptable if it is:
                // - `Self` (generic self-type)
                // - The class name itself (concrete self-type)
                // - A quoted version of the class name (forward reference)
                let is_self = *actual_return == "Self";
                let is_own_class = *actual_return == *arg_class_name;
                let is_quoted_own_class = actual_return.trim_matches('"') == *arg_class_name;

                if !is_self && !is_own_class && !is_quoted_own_class {
                    out.push(ProtocolSelfViolation {
                        class_name: (*arg_class_name).to_owned(),
                        protocol_name: (*param_type).to_owned(),
                        method_name: (*method_name).to_owned(),
                        actual_return_type: (*actual_return).to_owned(),
                        span: text_range_to_span(arg.range()),
                    });
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// isinstance() with TypedDict class detection
// ---------------------------------------------------------------------------

/// Collect spans of `isinstance(x, T)` calls where `T` is a `TypedDict` class.
///
/// PEP 589: `TypedDict` type objects cannot be used in `isinstance()` tests.
pub(super) fn class_is_protocol(cls: &ClassInfo) -> bool {
    cls.bases.iter().any(|b| b == "Protocol")
}

/// Detect direct instantiation of Protocol classes (e.g. `Proto()`) and
/// concrete subclasses that fail to implement all required protocol members.
pub(super) fn collect_protocol_instantiation_violations(
    stmts: &[Stmt],
    classes: &[ClassInfo],
) -> Vec<ProtocolInstantiationViolation> {
    let protocol_names: std::collections::HashSet<&str> = classes
        .iter()
        .filter(|cls| class_is_protocol(cls))
        .map(|cls| cls.name.as_str())
        .collect();

    let class_map: std::collections::HashMap<&str, &ClassInfo> =
        classes.iter().map(|cls| (cls.name.as_str(), cls)).collect();

    let mut abstract_names: std::collections::HashSet<&str> = classes
        .iter()
        .filter(|cls| !class_is_protocol(cls) && class_has_abstract_methods(cls))
        .map(|cls| cls.name.as_str())
        .collect();

    let protocol_required_methods = collect_protocol_required_methods(stmts, &protocol_names);
    let protocol_required_attrs = collect_protocol_required_attrs(stmts, &protocol_names);

    for cls in classes {
        if class_is_protocol(cls) || abstract_names.contains(cls.name.as_str()) {
            continue;
        }
        if class_missing_protocol_members(
            cls,
            &protocol_names,
            &class_map,
            &protocol_required_methods,
            &protocol_required_attrs,
        ) {
            let _ = abstract_names.insert(cls.name.as_str());
        }
    }

    if protocol_names.is_empty() && abstract_names.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    find_protocol_instantiations(stmts, &protocol_names, &abstract_names, &mut out);
    out
}

/// Check if a class has any methods decorated with `@abstractmethod`.
pub(super) fn class_has_abstract_methods(cls: &ClassInfo) -> bool {
    cls.method_decorators
        .iter()
        .any(|(_method_name, decorators)| decorators.iter().any(|d| d == "abstractmethod"))
}

/// Check if a non-Protocol class missing required protocol members.
pub(super) fn class_missing_protocol_members(
    cls: &ClassInfo,
    protocol_names: &std::collections::HashSet<&str>,
    class_map: &std::collections::HashMap<&str, &ClassInfo>,
    protocol_required_methods: &std::collections::HashMap<
        String,
        std::collections::HashSet<String>,
    >,
    protocol_required_attrs: &std::collections::HashMap<String, std::collections::HashSet<String>>,
) -> bool {
    let mut required_methods: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut required_attrs: std::collections::HashSet<String> = std::collections::HashSet::new();

    for base_name in &cls.bases {
        if !protocol_names.contains(base_name.as_str()) {
            continue;
        }
        if let Some(methods) = protocol_required_methods.get(base_name) {
            required_methods.extend(methods.iter().cloned());
        }
        if let Some(attrs) = protocol_required_attrs.get(base_name) {
            required_attrs.extend(attrs.iter().cloned());
        }
        if let Some(proto) = class_map.get(base_name.as_str()) {
            collect_transitive_required_members(
                proto,
                protocol_names,
                class_map,
                protocol_required_methods,
                protocol_required_attrs,
                &mut required_methods,
                &mut required_attrs,
            );
        }
    }

    if required_methods.is_empty() && required_attrs.is_empty() {
        return false;
    }

    let mut provided_methods: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut provided_attrs: std::collections::HashSet<&str> = std::collections::HashSet::new();

    collect_provided_members(cls, &mut provided_methods, &mut provided_attrs);

    for base_name in &cls.bases {
        if protocol_names.contains(base_name.as_str()) {
            continue;
        }
        if let Some(base_cls) = class_map.get(base_name.as_str()) {
            collect_provided_members(base_cls, &mut provided_methods, &mut provided_attrs);
        }
    }

    for method in &required_methods {
        if !provided_methods.contains(method.as_str()) {
            return true;
        }
    }
    for attr in &required_attrs {
        if !provided_attrs.contains(attr.as_str()) {
            return true;
        }
    }
    false
}
