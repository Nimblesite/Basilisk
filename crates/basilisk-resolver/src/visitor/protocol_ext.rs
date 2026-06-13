//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Protocol Ext visitor functions.

use ruff_python_ast::{Expr, Stmt};

use crate::scope::{
    ClassInfo, ProtocolInstantiationViolation, ProtocolRtcViolation, ProtocolRtcViolationKind, Span,
};

use super::annotations::is_classvar_annotation;
use super::class_info_ext::expr_simple_name;
use super::core::text_range_to_span;

pub(super) fn collect_transitive_required_members(
    proto: &ClassInfo,
    protocol_names: &std::collections::HashSet<&str>,
    class_map: &std::collections::HashMap<&str, &ClassInfo>,
    protocol_required_methods: &std::collections::HashMap<
        String,
        std::collections::HashSet<String>,
    >,
    protocol_required_attrs: &std::collections::HashMap<String, std::collections::HashSet<String>>,
    required_methods: &mut std::collections::HashSet<String>,
    required_attrs: &mut std::collections::HashSet<String>,
) {
    for base_name in &proto.bases {
        if base_name == "Protocol" {
            continue;
        }
        if !protocol_names.contains(base_name.as_str()) {
            continue;
        }
        if let Some(methods) = protocol_required_methods.get(base_name) {
            required_methods.extend(methods.iter().cloned());
        }
        if let Some(attrs) = protocol_required_attrs.get(base_name) {
            required_attrs.extend(attrs.iter().cloned());
        }
        if let Some(parent_proto) = class_map.get(base_name.as_str()) {
            collect_transitive_required_members(
                parent_proto,
                protocol_names,
                class_map,
                protocol_required_methods,
                protocol_required_attrs,
                required_methods,
                required_attrs,
            );
        }
    }
}

/// Collect methods and attributes provided by a class.
pub(super) fn collect_provided_members<'a>(
    cls: &'a ClassInfo,
    provided_methods: &mut std::collections::HashSet<&'a str>,
    provided_attrs: &mut std::collections::HashSet<&'a str>,
) {
    for method_name in &cls.method_names {
        let _ = provided_methods.insert(method_name.as_str());
    }
    for attr in &cls.attributes {
        let _ = provided_attrs.insert(attr.name.as_str());
    }
}

/// Collect required methods for each Protocol class by walking the AST.
pub(super) fn collect_protocol_required_methods(
    stmts: &[Stmt],
    protocol_names: &std::collections::HashSet<&str>,
) -> std::collections::HashMap<String, std::collections::HashSet<String>> {
    let mut result: std::collections::HashMap<String, std::collections::HashSet<String>> =
        std::collections::HashMap::new();
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else { continue };
        if !protocol_names.contains(cls.name.id.as_str()) {
            continue;
        }
        let mut required = std::collections::HashSet::new();
        for body_stmt in &cls.body {
            let Stmt::FunctionDef(func) = body_stmt else {
                continue;
            };
            let name = func.name.id.as_str();
            if name.starts_with("__") && name.ends_with("__") {
                continue;
            }
            let is_abstract = func.decorator_list.iter().any(
                |d| matches!(&d.expression, Expr::Name(n) if n.id.as_str() == "abstractmethod"),
            );
            let is_stub = is_function_body_stub(&func.body);
            if is_abstract || is_stub {
                let _ = required.insert(name.to_string());
            }
        }
        let _ = result.insert(cls.name.id.to_string(), required);
    }
    result
}

/// Check if a function body consists only of `...` (Ellipsis) or `pass`.
pub(super) fn is_function_body_stub(body: &[Stmt]) -> bool {
    if body.len() != 1 {
        return false;
    }
    let Some(first) = body.first() else {
        return false;
    };
    match first {
        Stmt::Pass(_) => true,
        Stmt::Expr(expr_stmt) => matches!(&*expr_stmt.value, Expr::EllipsisLiteral(_)),
        _ => false,
    }
}

/// Collect required `ClassVar` attributes for each Protocol class.
pub(super) fn collect_protocol_required_attrs(
    stmts: &[Stmt],
    protocol_names: &std::collections::HashSet<&str>,
) -> std::collections::HashMap<String, std::collections::HashSet<String>> {
    let mut result: std::collections::HashMap<String, std::collections::HashSet<String>> =
        std::collections::HashMap::new();
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else { continue };
        if !protocol_names.contains(cls.name.id.as_str()) {
            continue;
        }
        let mut required = std::collections::HashSet::new();
        for body_stmt in &cls.body {
            let Stmt::AnnAssign(ann) = body_stmt else {
                continue;
            };
            if ann.value.is_some() {
                continue;
            }
            if is_classvar_annotation(&ann.annotation) {
                if let Some(name) = expr_simple_name(&ann.target) {
                    let _ = required.insert(name);
                }
            }
        }
        let _ = result.insert(cls.name.id.to_string(), required);
    }
    result
}

/// Check if an annotation expression is `ClassVar` or `ClassVar[...]`.
pub(super) fn find_protocol_instantiations(
    stmts: &[Stmt],
    protocol_names: &std::collections::HashSet<&str>,
    abstract_names: &std::collections::HashSet<&str>,
    out: &mut Vec<ProtocolInstantiationViolation>,
) {
    crate::walk_all_stmts(stmts, &mut |stmt| match stmt {
        Stmt::Assign(node) => {
            check_expr_for_protocol_call(&node.value, protocol_names, abstract_names, out);
        }
        Stmt::AnnAssign(node) => {
            if let Some(value) = &node.value {
                check_expr_for_protocol_call(value, protocol_names, abstract_names, out);
            }
        }
        Stmt::Expr(node) => {
            check_expr_for_protocol_call(&node.value, protocol_names, abstract_names, out);
        }
        _ => {}
    });
}

/// Check if an expression is a call to a Protocol or abstract class,
/// or if a Protocol/abstract class is passed as an argument.
pub(super) fn check_expr_for_protocol_call(
    expr: &Expr,
    protocol_names: &std::collections::HashSet<&str>,
    abstract_names: &std::collections::HashSet<&str>,
    out: &mut Vec<ProtocolInstantiationViolation>,
) {
    if let Expr::Call(call) = expr {
        if let Some(name) = expr_simple_name(&call.func) {
            let name_str = name.as_str();
            if protocol_names.contains(name_str) {
                out.push(ProtocolInstantiationViolation {
                    class_name: name,
                    span: text_range_to_span(call.range),
                    is_abstract: false,
                });
            } else if abstract_names.contains(name_str) {
                out.push(ProtocolInstantiationViolation {
                    class_name: name,
                    span: text_range_to_span(call.range),
                    is_abstract: true,
                });
            }
        }
        if let Expr::Subscript(sub) = call.func.as_ref() {
            if let Some(name) = expr_simple_name(&sub.value) {
                let name_str = name.as_str();
                if protocol_names.contains(name_str) {
                    out.push(ProtocolInstantiationViolation {
                        class_name: name,
                        span: text_range_to_span(call.range),
                        is_abstract: false,
                    });
                } else if abstract_names.contains(name_str) {
                    out.push(ProtocolInstantiationViolation {
                        class_name: name,
                        span: text_range_to_span(call.range),
                        is_abstract: true,
                    });
                }
            }
        }
        // Check if a Protocol/abstract class is passed as an argument.
        // Type-checking utilities (assert_type, reveal_type, cast, ...) reference
        // their type arguments nominally and never instantiate them, so passing a
        // Protocol/abstract class to them is valid — skip their argument lists.
        let callee_is_type_utility = expr_simple_name(&call.func)
            .is_some_and(|name| is_type_checking_utility(name.as_str()));
        if !callee_is_type_utility {
            for arg in &call.arguments.args {
                if let Some(arg_name) = expr_simple_name(arg) {
                    let arg_str = arg_name.as_str();
                    if protocol_names.contains(arg_str) {
                        out.push(ProtocolInstantiationViolation {
                            class_name: arg_name,
                            span: text_range_to_span(call.range),
                            is_abstract: false,
                        });
                    } else if abstract_names.contains(arg_str) {
                        out.push(ProtocolInstantiationViolation {
                            class_name: arg_name,
                            span: text_range_to_span(call.range),
                            is_abstract: true,
                        });
                    }
                }
            }
        }
    }
}

/// Returns `true` for type-checking utility functions whose arguments are type
/// expressions referenced nominally, not values to be instantiated.
fn is_type_checking_utility(name: &str) -> bool {
    matches!(name, "assert_type" | "reveal_type" | "cast")
}

// ---------------------------------------------------------------------------
// Protocol runtime_checkable violation detection
// ---------------------------------------------------------------------------

/// Information about a protocol class relevant to runtime-checkable checks.
pub(super) struct ProtocolInfo<'a> {
    /// Whether the protocol is decorated with `@runtime_checkable`.
    pub(super) is_runtime_checkable: bool,
    /// Whether the protocol has data attributes (non-method members).
    pub(super) is_data_protocol: bool,
    /// The class name.
    pub(super) name: &'a str,
}

/// Build a map of protocol class names to their `ProtocolInfo`.
pub(super) fn build_protocol_map(
    classes: &[ClassInfo],
) -> std::collections::HashMap<&str, ProtocolInfo<'_>> {
    let mut map = std::collections::HashMap::new();
    for cls in classes {
        let is_protocol = cls.bases.iter().any(|b| b == "Protocol");
        if !is_protocol {
            continue;
        }
        let is_runtime_checkable = cls
            .decorator_spans
            .iter()
            .any(|(name, _)| name == "runtime_checkable");
        let is_data_protocol = !cls.attributes.is_empty();
        let _ = map.insert(
            cls.name.as_str(),
            ProtocolInfo {
                is_runtime_checkable,
                is_data_protocol,
                name: cls.name.as_str(),
            },
        );
    }
    map
}

/// Collect protocol `isinstance`/`issubclass` runtime-checkable violations.
pub(super) fn collect_protocol_rtc_violations(
    stmts: &[Stmt],
    classes: &[ClassInfo],
) -> Vec<ProtocolRtcViolation> {
    let protocol_map = build_protocol_map(classes);
    if protocol_map.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    collect_protocol_rtc_in_stmts(stmts, &protocol_map, &mut out);
    out
}

pub(super) fn collect_protocol_rtc_in_stmts(
    stmts: &[Stmt],
    protocol_map: &std::collections::HashMap<&str, ProtocolInfo<'_>>,
    out: &mut Vec<ProtocolRtcViolation>,
) {
    crate::walk_all_stmts(stmts, &mut |stmt| match stmt {
        Stmt::If(node) => {
            collect_protocol_rtc_in_expr(&node.test, protocol_map, out);
            for clause in &node.elif_else_clauses {
                if let Some(test) = &clause.test {
                    collect_protocol_rtc_in_expr(test, protocol_map, out);
                }
            }
        }
        Stmt::Expr(node) => collect_protocol_rtc_in_expr(&node.value, protocol_map, out),
        Stmt::Assign(node) => collect_protocol_rtc_in_expr(&node.value, protocol_map, out),
        Stmt::AnnAssign(node) => {
            if let Some(val) = &node.value {
                collect_protocol_rtc_in_expr(val, protocol_map, out);
            }
        }
        Stmt::While(node) => collect_protocol_rtc_in_expr(&node.test, protocol_map, out),
        _ => {}
    });
}

/// Check a single `isinstance`/`issubclass` call for protocol violations.
pub(super) fn check_protocol_arg(
    call_name: &str,
    arg_name: &str,
    call_span: Span,
    protocol_map: &std::collections::HashMap<&str, ProtocolInfo<'_>>,
    out: &mut Vec<ProtocolRtcViolation>,
) {
    let Some(info) = protocol_map.get(arg_name) else {
        return;
    };

    // Not runtime_checkable: error for both isinstance and issubclass.
    if !info.is_runtime_checkable {
        out.push(ProtocolRtcViolation {
            span: call_span,
            kind: ProtocolRtcViolationKind::NotRuntimeCheckable {
                protocol_name: info.name.to_owned(),
                call_name: call_name.to_owned(),
            },
        });
        return;
    }

    // issubclass with a data protocol: error.
    if call_name == "issubclass" && info.is_data_protocol {
        out.push(ProtocolRtcViolation {
            span: call_span,
            kind: ProtocolRtcViolationKind::IssubclassDataProtocol {
                protocol_name: info.name.to_owned(),
            },
        });
    }
}

pub(super) fn collect_protocol_rtc_in_expr(
    expr: &Expr,
    protocol_map: &std::collections::HashMap<&str, ProtocolInfo<'_>>,
    out: &mut Vec<ProtocolRtcViolation>,
) {
    use ruff_text_size::Ranged as _;
    let Expr::Call(call) = expr else { return };

    // Determine if callee is isinstance or issubclass.
    let call_name = match call.func.as_ref() {
        Expr::Name(n) if n.id == "isinstance" => "isinstance",
        Expr::Name(n) if n.id == "issubclass" => "issubclass",
        _ => return,
    };

    let Some(second_arg) = call.arguments.args.get(1) else {
        return;
    };

    let call_span = text_range_to_span(call.range());

    match second_arg {
        // Single name: isinstance(x, Proto)
        Expr::Name(name) => {
            check_protocol_arg(call_name, name.id.as_str(), call_span, protocol_map, out);
        }
        // Tuple: isinstance(x, (Proto1, Proto2))
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                if let Expr::Name(name) = elt {
                    check_protocol_arg(call_name, name.id.as_str(), call_span, protocol_map, out);
                }
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Generator violation collection
// ---------------------------------------------------------------------------

/// Valid return type names for synchronous generator functions.
pub(super) const SYNC_GENERATOR_TYPES: &[&str] = &["Generator", "Iterator", "Iterable"];

/// Valid return type names for asynchronous generator functions.
pub(super) const ASYNC_GENERATOR_TYPES: &[&str] =
    &["AsyncGenerator", "AsyncIterator", "AsyncIterable"];

/// Extract the base type name from an annotation string.
pub(super) fn base_type_name(annotation: &str) -> &str {
    annotation
        .find('[')
        .map_or(annotation, |idx| &annotation[..idx])
        .trim()
}

/// Strip string-annotation quotes and dotted module prefixes:
/// `"collections.abc.AsyncIterator` → `AsyncIterator` (issue #36).
pub(super) fn unqualified_base(base: &str) -> &str {
    let trimmed = base
        .trim_matches(|quote: char| quote == '"' || quote == '\'')
        .trim();
    trimmed.rsplit('.').next().unwrap_or(trimmed)
}
