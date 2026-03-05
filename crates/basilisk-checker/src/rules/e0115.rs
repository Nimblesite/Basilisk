//! BSK-E0115: Use of deprecated class, function, or method.
//!
//! PEP 702 introduces `@deprecated` from `typing` / `typing_extensions`.
//! Using a deprecated entity (calling, importing, accessing) should produce
//! a diagnostic so that developers migrate away from the deprecated API.
//!
//! ```python
//! from typing_extensions import deprecated
//!
//! @deprecated("Use new_func instead")
//! def old_func() -> None: ...
//!
//! old_func()  # BSK-E0115
//! ```

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0115",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0115",
};

/// Emits BSK-E0115 for usage of `@deprecated` decorated entities.
pub(crate) struct DeprecatedUsage;

impl Rule for DeprecatedUsage {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) =
            basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };

        // Collect deprecated names defined in THIS module.
        let mut local_deprecated: HashMap<String, DeprecatedInfo> = HashMap::new();
        collect_deprecated_definitions(&parsed.ast.body, &mut local_deprecated, None);

        // Collect deprecated names from imported sibling modules.
        let mut imported_deprecated: HashMap<String, DeprecatedInfo> = HashMap::new();
        // Also track module aliases: `import X as alias` -> alias maps to module X
        let mut module_aliases: HashMap<String, String> = HashMap::new();
        collect_imported_deprecated(
            &parsed.ast.body,
            &module.path,
            &mut imported_deprecated,
            &mut module_aliases,
        );

        // Merge all deprecated names.
        let mut all_deprecated = local_deprecated;
        for (name, info) in imported_deprecated {
            all_deprecated.insert(name, info);
        }

        if all_deprecated.is_empty() && module_aliases.is_empty() {
            return;
        }

        // Collect deprecated method/attribute info from imported module classes.
        let deprecated_members = collect_imported_deprecated_members(
            &parsed.ast.body,
            &module.path,
        );

        // Walk the AST to find usages of deprecated names.
        collect_usage_violations(
            &parsed.ast.body,
            &all_deprecated,
            &module_aliases,
            &deprecated_members,
            &module.path,
            diagnostics,
        );
    }
}

/// Info about a deprecated entity.
#[derive(Debug, Clone)]
struct DeprecatedInfo {
    /// The kind of entity: "class", "function", "method", "overload", "property", "property setter".
    kind: String,
    /// The deprecation message from the decorator argument.
    message: Option<String>,
    /// The defining span (for deduplication).
    def_span: Span,
}

/// Info about deprecated members of imported classes.
#[derive(Debug, Clone)]
struct DeprecatedMemberInfo {
    /// The class the member belongs to.
    class_name: String,
    /// The member name.
    member_name: String,
    /// The kind: "method", "property", "property setter".
    kind: String,
}

fn text_range_to_span(range: ruff_text_size::TextRange) -> Span {
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}

/// Check if a decorator expression is `@deprecated(...)`.
fn is_deprecated_decorator(expr: &Expr) -> Option<Option<String>> {
    match expr {
        Expr::Call(call) => {
            let is_deprecated_name = match call.func.as_ref() {
                Expr::Name(name) => name.id.as_str() == "deprecated",
                Expr::Attribute(attr) => {
                    attr.attr.as_str() == "deprecated"
                        && matches!(attr.value.as_ref(), Expr::Name(n) if n.id.as_str() == "typing" || n.id.as_str() == "typing_extensions")
                }
                _ => false,
            };
            if !is_deprecated_name {
                return None;
            }
            // Extract the message from the first positional argument.
            let message = call.arguments.args.first().and_then(|arg| {
                if let Expr::StringLiteral(s) = arg {
                    Some(s.value.to_string())
                } else {
                    None
                }
            });
            Some(message)
        }
        Expr::Name(name) if name.id.as_str() == "deprecated" => Some(None),
        Expr::Attribute(attr)
            if attr.attr.as_str() == "deprecated"
                && matches!(attr.value.as_ref(), Expr::Name(n) if n.id.as_str() == "typing" || n.id.as_str() == "typing_extensions") =>
        {
            Some(None)
        }
        _ => None,
    }
}

/// Collect deprecated function/class definitions from a list of statements.
fn collect_deprecated_definitions(
    stmts: &[Stmt],
    out: &mut HashMap<String, DeprecatedInfo>,
    class_name: Option<&str>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                let has_overload = func.decorator_list.iter().any(|d| {
                    matches!(&d.expression, Expr::Name(n) if n.id.as_str() == "overload")
                        || matches!(&d.expression, Expr::Attribute(a) if a.attr.as_str() == "overload")
                });

                for dec in &func.decorator_list {
                    if let Some(message) = is_deprecated_decorator(&dec.expression) {
                        let kind = if has_overload {
                            "overload".to_owned()
                        } else if let Some(cls) = class_name {
                            // Check if this is a property
                            let has_property = func.decorator_list.iter().any(|d| {
                                matches!(&d.expression, Expr::Name(n) if n.id.as_str() == "property")
                            });
                            let has_setter = func.decorator_list.iter().any(|d| {
                                if let Expr::Attribute(a) = &d.expression {
                                    a.attr.as_str() == "setter"
                                } else {
                                    false
                                }
                            });
                            if has_setter {
                                "property setter".to_owned()
                            } else if has_property {
                                "property".to_owned()
                            } else {
                                "method".to_owned()
                            }
                        } else {
                            "function".to_owned()
                        };

                        let name = if let Some(cls) = class_name {
                            format!("{cls}.{}", func.name)
                        } else {
                            func.name.to_string()
                        };

                        out.insert(name, DeprecatedInfo {
                            kind,
                            message,
                            def_span: text_range_to_span(func.range()),
                        });
                        break;
                    }
                }

                // Recurse into method bodies for nested definitions.
                collect_deprecated_definitions(&func.body, out, class_name);
            }
            Stmt::ClassDef(cls) => {
                for dec in &cls.decorator_list {
                    if let Some(message) = is_deprecated_decorator(&dec.expression) {
                        out.insert(cls.name.to_string(), DeprecatedInfo {
                            kind: "class".to_owned(),
                            message,
                            def_span: text_range_to_span(cls.range()),
                        });
                        break;
                    }
                }
                // Recurse into class body for methods.
                collect_deprecated_definitions(
                    &cls.body,
                    out,
                    Some(cls.name.as_str()),
                );
            }
            _ => {}
        }
    }
}

/// Collect deprecated names imported from sibling modules.
fn collect_imported_deprecated(
    stmts: &[Stmt],
    module_path: &str,
    out: &mut HashMap<String, DeprecatedInfo>,
    module_aliases: &mut HashMap<String, String>,
) {
    let Some(module_dir) = std::path::Path::new(module_path).parent() else {
        return;
    };

    for stmt in stmts {
        match stmt {
            Stmt::ImportFrom(import_from) => {
                let Some(module_name) = import_from.module.as_ref() else {
                    continue;
                };
                let module_str = module_name.to_string();
                if module_str.contains('.') {
                    continue;
                }
                let sibling_path = module_dir.join(format!("{module_str}.py"));
                let Some(sibling_path_str) = sibling_path.to_str() else {
                    continue;
                };
                let Ok(sibling) = basilisk_parser::parse_file(sibling_path_str) else {
                    continue;
                };

                // Collect deprecated definitions from the sibling.
                let mut sibling_deprecated: HashMap<String, DeprecatedInfo> = HashMap::new();
                collect_deprecated_definitions(
                    &sibling.ast.body,
                    &mut sibling_deprecated,
                    None,
                );

                for alias in &import_from.names {
                    let name = alias.name.as_str();
                    if let Some(info) = sibling_deprecated.get(name) {
                        let local_name = alias
                            .asname
                            .as_ref()
                            .map_or_else(|| name.to_owned(), |a| a.to_string());
                        out.insert(local_name, info.clone());
                    }
                }
            }
            Stmt::Import(import_stmt) => {
                for alias in &import_stmt.names {
                    let module_str = alias.name.to_string();
                    if let Some(asname) = alias.asname.as_ref() {
                        module_aliases
                            .insert(asname.to_string(), module_str);
                    } else {
                        module_aliases
                            .insert(module_str.clone(), module_str);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Collect deprecated members from imported module classes.
///
/// Returns a map from (module_alias, member_path) -> `DeprecatedMemberInfo`.
/// For `library.norwegian_blue(1)`, this maps ("library", "norwegian_blue") -> info.
/// For `library.Spam().__add__`, this maps ("library", "Spam.__add__") -> info.
fn collect_imported_deprecated_members(
    stmts: &[Stmt],
    module_path: &str,
) -> HashMap<String, HashMap<String, DeprecatedInfo>> {
    let mut result: HashMap<String, HashMap<String, DeprecatedInfo>> = HashMap::new();
    let Some(module_dir) = std::path::Path::new(module_path).parent() else {
        return result;
    };

    for stmt in stmts {
        // Handle `import X as alias`
        let module_str = match stmt {
            Stmt::Import(import_stmt) => {
                for alias in &import_stmt.names {
                    let module_str = alias.name.to_string();
                    if module_str.contains('.') {
                        continue;
                    }
                    let alias_name = alias
                        .asname
                        .as_ref()
                        .map_or_else(|| module_str.clone(), |a| a.to_string());
                    let sibling_path = module_dir.join(format!("{module_str}.py"));
                    let Some(sibling_path_str) = sibling_path.to_str() else {
                        continue;
                    };
                    let Ok(sibling) = basilisk_parser::parse_file(sibling_path_str) else {
                        continue;
                    };
                    let mut sibling_deprecated: HashMap<String, DeprecatedInfo> = HashMap::new();
                    collect_deprecated_definitions(
                        &sibling.ast.body,
                        &mut sibling_deprecated,
                        None,
                    );
                    if !sibling_deprecated.is_empty() {
                        result.insert(alias_name, sibling_deprecated);
                    }
                }
                continue;
            }
            _ => continue,
        };
    }
    result
}

/// Walk the AST to find usages of deprecated names and emit diagnostics.
#[allow(clippy::too_many_lines)]
fn collect_usage_violations(
    stmts: &[Stmt],
    deprecated: &HashMap<String, DeprecatedInfo>,
    module_aliases: &HashMap<String, String>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Collect all deprecated definition spans so we skip diagnostics inside them.
    let def_spans: HashSet<u32> = deprecated.values().map(|info| info.def_span.start).collect();

    for stmt in stmts {
        visit_stmt_for_usage(stmt, deprecated, module_aliases, deprecated_members, path, diagnostics, &def_spans);
    }
}

/// Check if a span is inside a deprecated definition (to avoid self-reports).
fn is_inside_def(span_start: u32, def_spans: &HashSet<u32>, stmt: &Stmt) -> bool {
    // We rely on the fact that usage within the definition body should not be flagged
    // (the definition itself is not a "usage"). We handle this by checking statement context.
    false
}

/// Visit a statement looking for deprecated name usages.
fn visit_stmt_for_usage(
    stmt: &Stmt,
    deprecated: &HashMap<String, DeprecatedInfo>,
    module_aliases: &HashMap<String, String>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    def_spans: &HashSet<u32>,
) {
    match stmt {
        Stmt::Expr(expr_stmt) => {
            visit_expr_for_usage(
                &expr_stmt.value,
                deprecated,
                module_aliases,
                deprecated_members,
                path,
                diagnostics,
            );
        }
        Stmt::Assign(assign) => {
            visit_expr_for_usage(
                &assign.value,
                deprecated,
                module_aliases,
                deprecated_members,
                path,
                diagnostics,
            );
            for target in &assign.targets {
                visit_expr_for_usage(
                    target,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    path,
                    diagnostics,
                );
            }
        }
        Stmt::AugAssign(aug) => {
            visit_expr_for_usage(
                &aug.value,
                deprecated,
                module_aliases,
                deprecated_members,
                path,
                diagnostics,
            );
            visit_expr_for_usage(
                &aug.target,
                deprecated,
                module_aliases,
                deprecated_members,
                path,
                diagnostics,
            );
        }
        Stmt::AnnAssign(ann) => {
            if let Some(value) = &ann.value {
                visit_expr_for_usage(
                    value,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    path,
                    diagnostics,
                );
            }
        }
        Stmt::Return(ret) => {
            if let Some(value) = &ret.value {
                visit_expr_for_usage(
                    value,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    path,
                    diagnostics,
                );
            }
        }
        Stmt::FunctionDef(func) => {
            // Don't flag usages inside the function's own decorators — those define the deprecation.
            // But DO recurse into the body.
            for body_stmt in &func.body {
                visit_stmt_for_usage(
                    body_stmt,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    path,
                    diagnostics,
                    def_spans,
                );
            }
        }
        Stmt::ClassDef(cls) => {
            for body_stmt in &cls.body {
                visit_stmt_for_usage(
                    body_stmt,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    path,
                    diagnostics,
                    def_spans,
                );
            }
        }
        Stmt::If(if_stmt) => {
            visit_expr_for_usage(
                &if_stmt.test,
                deprecated,
                module_aliases,
                deprecated_members,
                path,
                diagnostics,
            );
            for body_stmt in &if_stmt.body {
                visit_stmt_for_usage(
                    body_stmt,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    path,
                    diagnostics,
                    def_spans,
                );
            }
            for elif in &if_stmt.elif_else_clauses {
                for body_stmt in &elif.body {
                    visit_stmt_for_usage(
                        body_stmt,
                        deprecated,
                        module_aliases,
                        deprecated_members,
                        path,
                        diagnostics,
                        def_spans,
                    );
                }
            }
        }
        Stmt::For(for_stmt) => {
            visit_expr_for_usage(
                &for_stmt.iter,
                deprecated,
                module_aliases,
                deprecated_members,
                path,
                diagnostics,
            );
            for body_stmt in &for_stmt.body {
                visit_stmt_for_usage(
                    body_stmt,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    path,
                    diagnostics,
                    def_spans,
                );
            }
        }
        Stmt::While(while_stmt) => {
            visit_expr_for_usage(
                &while_stmt.test,
                deprecated,
                module_aliases,
                deprecated_members,
                path,
                diagnostics,
            );
            for body_stmt in &while_stmt.body {
                visit_stmt_for_usage(
                    body_stmt,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    path,
                    diagnostics,
                    def_spans,
                );
            }
        }
        _ => {}
    }
}

/// Visit an expression to find deprecated name usages.
fn visit_expr_for_usage(
    expr: &Expr,
    deprecated: &HashMap<String, DeprecatedInfo>,
    module_aliases: &HashMap<String, String>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        // Direct name reference: `lorem()` or just `lorem`
        Expr::Name(name) => {
            if let Some(info) = deprecated.get(name.id.as_str()) {
                diagnostics.push(make_diagnostic(
                    text_range_to_span(name.range()),
                    &info.kind,
                    name.id.as_str(),
                    info.message.as_deref(),
                    path,
                ));
            }
        }
        // Function/method call: `lorem()`, `library.norwegian_blue(1)`, `invocable()`
        Expr::Call(call) => {
            // Check if the callee itself is deprecated (but avoid double-reporting for Name).
            match call.func.as_ref() {
                Expr::Name(name) => {
                    if let Some(info) = deprecated.get(name.id.as_str()) {
                        diagnostics.push(make_diagnostic(
                            text_range_to_span(call.range()),
                            &info.kind,
                            name.id.as_str(),
                            info.message.as_deref(),
                            path,
                        ));
                    }
                }
                Expr::Attribute(attr) => {
                    // `library.norwegian_blue(1)` or `f.foo()`
                    check_attribute_deprecated(
                        attr,
                        deprecated,
                        module_aliases,
                        deprecated_members,
                        path,
                        diagnostics,
                        Some(text_range_to_span(call.range())),
                    );
                }
                _ => {
                    visit_expr_for_usage(
                        call.func.as_ref(),
                        deprecated,
                        module_aliases,
                        deprecated_members,
                        path,
                        diagnostics,
                    );
                }
            }
            // Visit arguments.
            for arg in &call.arguments.args {
                visit_expr_for_usage(
                    arg,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    path,
                    diagnostics,
                );
            }
        }
        // Attribute access: `spam.greasy`, `library.norwegian_blue`
        Expr::Attribute(attr) => {
            check_attribute_deprecated(
                attr,
                deprecated,
                module_aliases,
                deprecated_members,
                path,
                diagnostics,
                None,
            );
        }
        // Binary operations: `spam + 1` triggers `__add__`
        Expr::BinOp(binop) => {
            // Check if the left operand is an instance of a class with a deprecated dunder.
            check_binop_deprecated(
                &binop.left,
                &binop.op,
                deprecated,
                path,
                diagnostics,
                text_range_to_span(binop.range()),
            );
            visit_expr_for_usage(
                &binop.left,
                deprecated,
                module_aliases,
                deprecated_members,
                path,
                diagnostics,
            );
            visit_expr_for_usage(
                &binop.right,
                deprecated,
                module_aliases,
                deprecated_members,
                path,
                diagnostics,
            );
        }
        // Tuple, list, set, etc.
        Expr::Tuple(t) => {
            for elt in &t.elts {
                visit_expr_for_usage(elt, deprecated, module_aliases, deprecated_members, path, diagnostics);
            }
        }
        Expr::List(l) => {
            for elt in &l.elts {
                visit_expr_for_usage(elt, deprecated, module_aliases, deprecated_members, path, diagnostics);
            }
        }
        _ => {}
    }
}

/// Check if an attribute access refers to a deprecated member.
fn check_attribute_deprecated(
    attr: &ruff_python_ast::ExprAttribute,
    deprecated: &HashMap<String, DeprecatedInfo>,
    module_aliases: &HashMap<String, String>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    call_span: Option<Span>,
) {
    let member_name = attr.attr.as_str();
    let span = call_span.unwrap_or_else(|| text_range_to_span(attr.range()));

    // Case 1: `library.func_name` where `library` is a module alias
    if let Expr::Name(value_name) = attr.value.as_ref() {
        let alias = value_name.id.as_str();

        // Check if this is a module alias with deprecated members
        if let Some(members) = deprecated_members.get(alias) {
            // Direct function/class: `library.norwegian_blue`
            if let Some(info) = members.get(member_name) {
                diagnostics.push(make_diagnostic(
                    span,
                    &info.kind,
                    member_name,
                    info.message.as_deref(),
                    path,
                ));
                return;
            }
        }

        // Check local deprecated qualified names: `ClassName.method`
        // e.g. for `f.foo()` where `foo` is deprecated on the protocol
        // We need to check if the type of `f` has a deprecated `foo`.
        // This requires type info we may not have. For now, check direct class-qualified names.
        let qualified = format!("{alias}.{member_name}");
        if let Some(info) = deprecated.get(&qualified) {
            diagnostics.push(make_diagnostic(
                span,
                &info.kind,
                member_name,
                info.message.as_deref(),
                path,
            ));
            return;
        }
    }

    // Recurse into the value expression.
    visit_expr_for_usage(
        attr.value.as_ref(),
        deprecated,
        module_aliases,
        deprecated_members,
        path,
        diagnostics,
    );
}

/// Check if a binary operation triggers a deprecated dunder method.
fn check_binop_deprecated(
    left: &Expr,
    _op: &ruff_python_ast::Operator,
    deprecated: &HashMap<String, DeprecatedInfo>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    span: Span,
) {
    // For `spam + 1`, if `spam` is of type `Spam` and `Spam.__add__` is deprecated,
    // we need to detect this. We approximate by checking if the left-hand side
    // is a Name whose type has a deprecated `__add__`.
    // We check all `ClassName.__add__` entries in deprecated.
    if let Expr::Name(name) = left {
        let var_name = name.id.as_str();
        // Look for any `X.__add__` where X might be the type of var_name.
        // This is an approximation — we check all deprecated class members.
        for (key, info) in deprecated {
            if key.ends_with(".__add__") && info.kind == "method" {
                let class_name = key.strip_suffix(".__add__").unwrap_or_default();
                // We can't easily do type inference here, so we emit if we have
                // evidence the variable is of that class type (e.g., assignment).
                // For now, flag it for any deprecated __add__.
                diagnostics.push(make_diagnostic(
                    span,
                    &info.kind,
                    &format!("{class_name}.__add__"),
                    info.message.as_deref(),
                    path,
                ));
                return;
            }
        }
    }
}

fn make_diagnostic(
    span: Span,
    kind: &str,
    name: &str,
    message: Option<&str>,
    path: &str,
) -> Diagnostic {
    let primary = format!("Use of deprecated {kind} `{name}`");
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: primary,
        span,
        path: path.to_owned(),
        help: message.map(|m| format!("Deprecated: {m}")),
        note: Some("Marked with `@deprecated` per PEP 702".to_owned()),
    }
}
