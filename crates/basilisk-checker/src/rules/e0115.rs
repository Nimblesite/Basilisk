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

use ruff_python_ast::{Expr, Operator, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0115",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0115",
};

/// Emits BSK-E0115 for usage of `@deprecated` decorated entities.
pub(crate) struct DeprecatedUsage;

impl Rule for DeprecatedUsage {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
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
        // Track deprecated from-imports with their spans (for import-site diagnostics).
        let mut from_import_deprecated: Vec<(String, Span)> = Vec::new();
        collect_imported_deprecated(
            &parsed.ast.body,
            &module.path,
            &mut imported_deprecated,
            &mut module_aliases,
            &mut from_import_deprecated,
        );

        // Emit diagnostics for deprecated from-imports (e.g. `from X import Ham`).
        // PEP 702 requires a diagnostic at the import site when a deprecated name is imported.
        for (local_name, span) in &from_import_deprecated {
            if let Some(info) = imported_deprecated.get(local_name.as_str()) {
                diagnostics.push(make_diagnostic(
                    *span,
                    &info.kind,
                    local_name,
                    info.message.as_deref(),
                    &module.path,
                ));
            }
        }

        // Merge all deprecated names.
        let mut all_deprecated = local_deprecated;
        for (name, info) in imported_deprecated {
            let _ = all_deprecated.insert(name, info);
        }

        if all_deprecated.is_empty() && module_aliases.is_empty() {
            return;
        }

        // Collect deprecated method/attribute info from imported module classes.
        let deprecated_members =
            collect_imported_deprecated_members(&parsed.ast.body, &module.path);

        // Build a variable-to-type map from simple assignments, e.g.:
        //   spam = library.Spam()   -> spam -> VarType { module_alias: "library", class_name: "Spam" }
        //   invocable = Invocable() -> invocable -> VarType { module_alias: "", class_name: "Invocable" }
        let var_types = collect_var_types(&parsed.ast.body);

        // Walk the AST to find usages of deprecated names.
        let def_spans: HashSet<u32> = all_deprecated
            .values()
            .map(|info| info.def_span.start)
            .collect();
        let ctx = DeprecatedUsageContext {
            deprecated: &all_deprecated,
            module_aliases: &module_aliases,
            deprecated_members: &deprecated_members,
            var_types: &var_types,
            path: &module.path,
            _def_spans: &def_spans,
        };
        for stmt in &parsed.ast.body {
            visit_stmt_for_usage(stmt, &ctx, diagnostics);
        }
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

/// Inferred type of a variable assigned to a class constructor call.
#[derive(Debug, Clone)]
struct VarType {
    /// Module alias used to access the class (e.g. "library"), or "" for a local class.
    module_alias: String,
    /// The class name (e.g. "Spam" or "Invocable").
    class_name: String,
}

fn text_range_to_span(range: ruff_text_size::TextRange) -> Span {
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}

/// Check if a decorator expression is `@deprecated(...)`.
///
/// Returns `None` if not a deprecated decorator, `Some(None)` if deprecated
/// without a message, and `Some(Some(msg))` if deprecated with a message.
#[expect(
    clippy::option_option,
    reason = "None=not deprecated, Some(None)=deprecated without message, Some(Some(msg))=deprecated with message"
)]
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
                        } else if class_name.is_some() {
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

                        let _ = out.insert(
                            name,
                            DeprecatedInfo {
                                kind,
                                message,
                                def_span: text_range_to_span(func.range()),
                            },
                        );
                        break;
                    }
                }

                // Recurse into method bodies for nested definitions.
                collect_deprecated_definitions(&func.body, out, class_name);
            }
            Stmt::ClassDef(cls) => {
                for dec in &cls.decorator_list {
                    if let Some(message) = is_deprecated_decorator(&dec.expression) {
                        let _ = out.insert(
                            cls.name.to_string(),
                            DeprecatedInfo {
                                kind: "class".to_owned(),
                                message,
                                def_span: text_range_to_span(cls.range()),
                            },
                        );
                        break;
                    }
                }
                // Recurse into class body for methods.
                collect_deprecated_definitions(&cls.body, out, Some(cls.name.as_str()));
            }
            _ => {}
        }
    }
}

/// Collect deprecated names imported from sibling modules.
///
/// Also populates `from_import_deprecated` with `(local_name, import_span)` pairs so
/// that a diagnostic can be emitted at the import site itself (PEP 702 requirement).
fn collect_imported_deprecated(
    stmts: &[Stmt],
    module_path: &str,
    out: &mut HashMap<String, DeprecatedInfo>,
    module_aliases: &mut HashMap<String, String>,
    from_import_deprecated: &mut Vec<(String, Span)>,
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
                collect_deprecated_definitions(&sibling.ast.body, &mut sibling_deprecated, None);

                for alias in &import_from.names {
                    let name = alias.name.as_str();
                    if let Some(info) = sibling_deprecated.get(name) {
                        let local_name = alias
                            .asname
                            .as_ref()
                            .map_or_else(|| name.to_owned(), std::string::ToString::to_string);
                        // Record the import site span so we can emit a diagnostic there.
                        let import_span = text_range_to_span(import_from.range());
                        from_import_deprecated.push((local_name.clone(), import_span));
                        let _ = out.insert(local_name, info.clone());
                    }
                }
            }
            Stmt::Import(import_stmt) => {
                for alias in &import_stmt.names {
                    let module_str = alias.name.to_string();
                    if let Some(asname) = alias.asname.as_ref() {
                        let _ = module_aliases.insert(asname.to_string(), module_str);
                    } else {
                        let _ = module_aliases.insert(module_str.clone(), module_str);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Collect deprecated members from imported module classes.
///
/// Returns a map: `module_alias` -`member_key`ey -> `DeprecatedInfo`.
/// Member keys look like "`norwegian_blue`" (top-level) or "Spam.__add__" (class member).
fn collect_imported_deprecated_members(
    stmts: &[Stmt],
    module_path: &str,
) -> HashMap<String, HashMap<String, DeprecatedInfo>> {
    let mut result: HashMap<String, HashMap<String, DeprecatedInfo>> = HashMap::new();
    let Some(module_dir) = std::path::Path::new(module_path).parent() else {
        return result;
    };

    for stmt in stmts {
        if let Stmt::Import(import_stmt) = stmt {
            for alias in &import_stmt.names {
                let module_str = alias.name.to_string();
                if module_str.contains('.') {
                    continue;
                }
                let alias_name = alias
                    .asname
                    .as_ref()
                    .map_or_else(|| module_str.clone(), std::string::ToString::to_string);
                let sibling_path = module_dir.join(format!("{module_str}.py"));
                let Some(sibling_path_str) = sibling_path.to_str() else {
                    continue;
                };
                let Ok(sibling) = basilisk_parser::parse_file(sibling_path_str) else {
                    continue;
                };
                let mut sibling_deprecated: HashMap<String, DeprecatedInfo> = HashMap::new();
                collect_deprecated_definitions(&sibling.ast.body, &mut sibling_deprecated, None);
                if !sibling_deprecated.is_empty() {
                    let _ = result.insert(alias_name, sibling_deprecated);
                }
            }
        }
    }
    result
}

/// Build a map from variable name to inferred class type by scanning simple assignments.
///
/// Handles:
/// - `spam = library.Spam()` -> spam: `VarType` `module_alias`as: "librar`class_name`name: "Spam" }
/// - `invocable = Invocable()` -> invocable: `VarType` `module_alias`as: `class_name`name: "Invocable" }
fn collect_var_types(stmts: &[Stmt]) -> HashMap<String, VarType> {
    let mut var_types: HashMap<String, VarType> = HashMap::new();
    collect_var_types_from_stmts(stmts, &mut var_types);
    var_types
}

fn collect_var_types_from_stmts(stmts: &[Stmt], var_types: &mut HashMap<String, VarType>) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign(assign) => {
                if let Some(var_type) = infer_call_type(&assign.value) {
                    for target in &assign.targets {
                        if let Expr::Name(name) = target {
                            let _ = var_types.insert(name.id.to_string(), var_type.clone());
                        }
                    }
                }
            }
            Stmt::FunctionDef(func) => {
                collect_var_types_from_stmts(&func.body, var_types);
            }
            Stmt::ClassDef(cls) => {
                collect_var_types_from_stmts(&cls.body, var_types);
            }
            _ => {}
        }
    }
}

/// Infer the class type from a constructor call expression.
fn infer_call_type(expr: &Expr) -> Option<VarType> {
    if let Expr::Call(call) = expr {
        match call.func.as_ref() {
            Expr::Name(name) => {
                return Some(VarType {
                    module_alias: String::new(),
                    class_name: name.id.to_string(),
                });
            }
            Expr::Attribute(attr) => {
                if let Expr::Name(obj) = attr.value.as_ref() {
                    return Some(VarType {
                        module_alias: obj.id.to_string(),
                        class_name: attr.attr.to_string(),
                    });
                }
            }
            _ => {}
        }
    }
    None
}

/// Map a binary/augmented operator to its dunder method name.
fn op_to_dunder(op: Operator) -> &'static str {
    match op {
        Operator::Add => "__add__",
        Operator::Sub => "__sub__",
        Operator::Mult => "__mul__",
        Operator::Div => "__truediv__",
        Operator::Mod => "__mod__",
        Operator::Pow => "__pow__",
        Operator::LShift => "__lshift__",
        Operator::RShift => "__rshift__",
        Operator::BitOr => "__or__",
        Operator::BitXor => "__xor__",
        Operator::BitAnd => "__and__",
        Operator::FloorDiv => "__floordiv__",
        Operator::MatMult => "__matmul__",
    }
}

/// Check if a dunder method on a given inferred type is deprecated; emit a diagnostic if so.
fn check_dunder_deprecated_on_type(
    var_type: &VarType,
    dunder: &str,
    deprecated: &HashMap<String, DeprecatedInfo>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    span: Span,
) {
    let class_member_key = format!("{}.{}", var_type.class_name, dunder);

    // Check deprecated members from an imported module.
    if !var_type.module_alias.is_empty() {
        if let Some(members) = deprecated_members.get(&var_type.module_alias) {
            if let Some(info) = members.get(&class_member_key) {
                diagnostics.push(make_diagnostic(
                    span,
                    &info.kind,
                    &class_member_key,
                    info.message.as_deref(),
                    path,
                ));
                return;
            }
        }
    }

    // Check locally-defined deprecated members.
    if let Some(info) = deprecated.get(&class_member_key) {
        diagnostics.push(make_diagnostic(
            span,
            &info.kind,
            &class_member_key,
            info.message.as_deref(),
            path,
        ));
    }
}

/// Check if a property setter on a given inferred type is deprecated; emit a diagnostic if so.
fn check_setter_deprecated_on_type(
    var_type: &VarType,
    member_name: &str,
    deprecated: &HashMap<String, DeprecatedInfo>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    span: Span,
) {
    let class_member_key = format!("{}.{}", var_type.class_name, member_name);

    // Check deprecated members from an imported module.
    if !var_type.module_alias.is_empty() {
        if let Some(members) = deprecated_members.get(&var_type.module_alias) {
            if let Some(info) = members.get(&class_member_key) {
                if info.kind == "property setter" {
                    diagnostics.push(make_diagnostic(
                        span,
                        &info.kind,
                        &class_member_key,
                        info.message.as_deref(),
                        path,
                    ));
                    return;
                }
            }
        }
    }

    // Check locally-defined deprecated members.
    if let Some(info) = deprecated.get(&class_member_key) {
        if info.kind == "property setter" {
            diagnostics.push(make_diagnostic(
                span,
                &info.kind,
                &class_member_key,
                info.message.as_deref(),
                path,
            ));
        }
    }
}

/// Contextual data for visiting statements and detecting deprecated usages.
struct DeprecatedUsageContext<'a> {
    deprecated: &'a HashMap<String, DeprecatedInfo>,
    module_aliases: &'a HashMap<String, String>,
    deprecated_members: &'a HashMap<String, HashMap<String, DeprecatedInfo>>,
    var_types: &'a HashMap<String, VarType>,
    path: &'a str,
    _def_spans: &'a HashSet<u32>,
}

/// Visit a statement looking for deprecated name usages.
#[expect(
    clippy::too_many_lines,
    reason = "statement visitor covers all statement variants"
)]
fn visit_stmt_for_usage(
    stmt: &Stmt,
    ctx: &DeprecatedUsageContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::Expr(expr_stmt) => {
            visit_expr_for_usage(
                &expr_stmt.value,
                ctx.deprecated,
                ctx.module_aliases,
                ctx.deprecated_members,
                ctx.var_types,
                ctx.path,
                diagnostics,
            );
        }
        Stmt::Assign(assign) => {
            visit_expr_for_usage(
                &assign.value,
                ctx.deprecated,
                ctx.module_aliases,
                ctx.deprecated_members,
                ctx.var_types,
                ctx.path,
                diagnostics,
            );
            for target in &assign.targets {
                // Check for deprecated property setter via assignment target (e.g. `spam.shape = ...`).
                check_assignment_target_deprecated(
                    target,
                    ctx.deprecated,
                    ctx.deprecated_members,
                    ctx.var_types,
                    ctx.path,
                    diagnostics,
                );
            }
        }
        Stmt::AugAssign(aug) => {
            // `spam += 1` triggers __add__; `spam.shape += "cube"` triggers property setter.
            visit_expr_for_usage(
                &aug.value,
                ctx.deprecated,
                ctx.module_aliases,
                ctx.deprecated_members,
                ctx.var_types,
                ctx.path,
                diagnostics,
            );
            check_aug_assign_deprecated(
                &aug.target,
                aug.op,
                ctx.deprecated,
                ctx.deprecated_members,
                ctx.var_types,
                ctx.path,
                diagnostics,
                text_range_to_span(aug.range()),
            );
        }
        Stmt::AnnAssign(ann) => {
            if let Some(value) = &ann.value {
                visit_expr_for_usage(
                    value,
                    ctx.deprecated,
                    ctx.module_aliases,
                    ctx.deprecated_members,
                    ctx.var_types,
                    ctx.path,
                    diagnostics,
                );
            }
        }
        Stmt::Return(ret) => {
            if let Some(value) = &ret.value {
                visit_expr_for_usage(
                    value,
                    ctx.deprecated,
                    ctx.module_aliases,
                    ctx.deprecated_members,
                    ctx.var_types,
                    ctx.path,
                    diagnostics,
                );
            }
        }
        Stmt::FunctionDef(func) => {
            for body_stmt in &func.body {
                visit_stmt_for_usage(body_stmt, ctx, diagnostics);
            }
        }
        Stmt::ClassDef(cls) => {
            for body_stmt in &cls.body {
                visit_stmt_for_usage(body_stmt, ctx, diagnostics);
            }
        }
        Stmt::If(if_stmt) => {
            visit_expr_for_usage(
                &if_stmt.test,
                ctx.deprecated,
                ctx.module_aliases,
                ctx.deprecated_members,
                ctx.var_types,
                ctx.path,
                diagnostics,
            );
            for body_stmt in &if_stmt.body {
                visit_stmt_for_usage(body_stmt, ctx, diagnostics);
            }
            for elif in &if_stmt.elif_else_clauses {
                for body_stmt in &elif.body {
                    visit_stmt_for_usage(body_stmt, ctx, diagnostics);
                }
            }
        }
        Stmt::For(for_stmt) => {
            visit_expr_for_usage(
                &for_stmt.iter,
                ctx.deprecated,
                ctx.module_aliases,
                ctx.deprecated_members,
                ctx.var_types,
                ctx.path,
                diagnostics,
            );
            for body_stmt in &for_stmt.body {
                visit_stmt_for_usage(body_stmt, ctx, diagnostics);
            }
        }
        Stmt::While(while_stmt) => {
            visit_expr_for_usage(
                &while_stmt.test,
                ctx.deprecated,
                ctx.module_aliases,
                ctx.deprecated_members,
                ctx.var_types,
                ctx.path,
                diagnostics,
            );
            for body_stmt in &while_stmt.body {
                visit_stmt_for_usage(body_stmt, ctx, diagnostics);
            }
        }
        _ => {}
    }
}

/// Check if an assignment target accesses a deprecated property setter.
///
/// Handles `spam.shape = "cube"` where `spam` has been inferred as type `Spam`.
fn check_assignment_target_deprecated(
    target: &Expr,
    deprecated: &HashMap<String, DeprecatedInfo>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    var_types: &HashMap<String, VarType>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Expr::Attribute(attr) = target {
        let member_name = attr.attr.as_str();
        if let Expr::Name(obj_name) = attr.value.as_ref() {
            let var_name = obj_name.id.as_str();
            if let Some(var_type) = var_types.get(var_name) {
                let span = text_range_to_span(target.range());
                check_setter_deprecated_on_type(
                    var_type,
                    member_name,
                    deprecated,
                    deprecated_members,
                    path,
                    diagnostics,
                    span,
                );
            }
        }
    }
}

/// Check augmented assignment for deprecated usage.
///
/// - `spam += 1` triggers the deprecated `__add__` method on `spam`'s type.
/// - `spam.shape += "cube"` triggers the deprecated property setter.
#[expect(
    clippy::too_many_arguments,
    reason = "deprecated usage check requires full context"
)]
fn check_aug_assign_deprecated(
    target: &Expr,
    op: Operator,
    deprecated: &HashMap<String, DeprecatedInfo>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    var_types: &HashMap<String, VarType>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    span: Span,
) {
    match target {
        Expr::Name(name) => {
            let var_name = name.id.as_str();
            if let Some(var_type) = var_types.get(var_name) {
                let dunder = op_to_dunder(op);
                check_dunder_deprecated_on_type(
                    var_type,
                    dunder,
                    deprecated,
                    deprecated_members,
                    path,
                    diagnostics,
                    span,
                );
            }
        }
        Expr::Attribute(attr) => {
            let member_name = attr.attr.as_str();
            if let Expr::Name(obj_name) = attr.value.as_ref() {
                let var_name = obj_name.id.as_str();
                if let Some(var_type) = var_types.get(var_name) {
                    check_setter_deprecated_on_type(
                        var_type,
                        member_name,
                        deprecated,
                        deprecated_members,
                        path,
                        diagnostics,
                        span,
                    );
                }
            }
        }
        _ => {}
    }
}

/// Visit an expression to find deprecated name usages.
#[expect(
    clippy::too_many_lines,
    reason = "expression visitor covers all expression variants"
)]
fn visit_expr_for_usage(
    expr: &Expr,
    deprecated: &HashMap<String, DeprecatedInfo>,
    module_aliases: &HashMap<String, String>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    var_types: &HashMap<String, VarType>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        // Direct name reference: `lorem` or as the callee of `lorem()`
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
        // Function/method call
        Expr::Call(call) => {
            match call.func.as_ref() {
                Expr::Name(name) => {
                    if let Some(info) = deprecated.get(name.id.as_str()) {
                        // Deprecated function/class called directly.
                        diagnostics.push(make_diagnostic(
                            text_range_to_span(call.range()),
                            &info.kind,
                            name.id.as_str(),
                            info.message.as_deref(),
                            path,
                        ));
                    } else {
                        // Check if calling an instance whose class has a deprecated __call__.
                        let var_name = name.id.as_str();
                        if let Some(var_type) = var_types.get(var_name) {
                            check_dunder_deprecated_on_type(
                                var_type,
                                "__call__",
                                deprecated,
                                deprecated_members,
                                path,
                                diagnostics,
                                text_range_to_span(call.range()),
                            );
                        }
                    }
                }
                Expr::Attribute(attr) => {
                    // Attribute-style call: `library.func()`, `spam.method()`, `f.foo()`
                    let mut handled = false;
                    if let Expr::Name(obj_name) = attr.value.as_ref() {
                        let var_name = obj_name.id.as_str();
                        let member_name = attr.attr.as_str();
                        if let Some(var_type) = var_types.get(var_name) {
                            // Instance method call: look up ClassName.method.
                            let key = format!("{}.{}", var_type.class_name, member_name);
                            if !var_type.module_alias.is_empty() {
                                if let Some(members) =
                                    deprecated_members.get(&var_type.module_alias)
                                {
                                    if let Some(info) = members.get(&key) {
                                        diagnostics.push(make_diagnostic(
                                            text_range_to_span(call.range()),
                                            &info.kind,
                                            member_name,
                                            info.message.as_deref(),
                                            path,
                                        ));
                                        handled = true;
                                    }
                                }
                            }
                            if !handled {
                                if let Some(info) = deprecated.get(&key) {
                                    diagnostics.push(make_diagnostic(
                                        text_range_to_span(call.range()),
                                        &info.kind,
                                        member_name,
                                        info.message.as_deref(),
                                        path,
                                    ));
                                    handled = true;
                                }
                            }
                        }
                    }
                    if !handled {
                        check_attribute_deprecated(
                            attr,
                            deprecated,
                            module_aliases,
                            deprecated_members,
                            var_types,
                            path,
                            diagnostics,
                            Some(text_range_to_span(call.range())),
                        );
                    }
                }
                _ => {
                    visit_expr_for_usage(
                        call.func.as_ref(),
                        deprecated,
                        module_aliases,
                        deprecated_members,
                        var_types,
                        path,
                        diagnostics,
                    );
                }
            }
            // Visit call arguments.
            for arg in &call.arguments.args {
                visit_expr_for_usage(
                    arg,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    var_types,
                    path,
                    diagnostics,
                );
            }
        }
        // Attribute access: `spam.greasy`, `library.norwegian_blue`
        Expr::Attribute(attr) => {
            // Check for deprecated property/method access on an inferred-type variable.
            let mut handled = false;
            if let Expr::Name(obj_name) = attr.value.as_ref() {
                let var_name = obj_name.id.as_str();
                let member_name = attr.attr.as_str();
                if let Some(var_type) = var_types.get(var_name) {
                    let key = format!("{}.{}", var_type.class_name, member_name);
                    // Only flag property getters or methods here; setters are handled on assignment.
                    if !var_type.module_alias.is_empty() {
                        if let Some(members) = deprecated_members.get(&var_type.module_alias) {
                            if let Some(info) = members.get(&key) {
                                if info.kind == "property" || info.kind == "method" {
                                    diagnostics.push(make_diagnostic(
                                        text_range_to_span(attr.range()),
                                        &info.kind,
                                        &key,
                                        info.message.as_deref(),
                                        path,
                                    ));
                                    handled = true;
                                }
                            }
                        }
                    }
                    if !handled {
                        if let Some(info) = deprecated.get(&key) {
                            if info.kind == "property" || info.kind == "method" {
                                diagnostics.push(make_diagnostic(
                                    text_range_to_span(attr.range()),
                                    &info.kind,
                                    &key,
                                    info.message.as_deref(),
                                    path,
                                ));
                                handled = true;
                            }
                        }
                    }
                }
            }
            if !handled {
                check_attribute_deprecated(
                    attr,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    var_types,
                    path,
                    diagnostics,
                    None,
                );
            }
        }
        // Binary operations: `spam + 1` triggers `__add__`
        Expr::BinOp(binop) => {
            check_binop_deprecated(
                &binop.left,
                binop.op,
                deprecated,
                deprecated_members,
                var_types,
                path,
                diagnostics,
                text_range_to_span(binop.range()),
            );
            visit_expr_for_usage(
                &binop.left,
                deprecated,
                module_aliases,
                deprecated_members,
                var_types,
                path,
                diagnostics,
            );
            visit_expr_for_usage(
                &binop.right,
                deprecated,
                module_aliases,
                deprecated_members,
                var_types,
                path,
                diagnostics,
            );
        }
        Expr::Tuple(t) => {
            for elt in &t.elts {
                visit_expr_for_usage(
                    elt,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    var_types,
                    path,
                    diagnostics,
                );
            }
        }
        Expr::List(l) => {
            for elt in &l.elts {
                visit_expr_for_usage(
                    elt,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    var_types,
                    path,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

/// Check if an attribute access refers to a deprecated member (module-level or qualified).
#[expect(
    clippy::too_many_arguments,
    reason = "attribute deprecation check requires full context"
)]
fn check_attribute_deprecated(
    attr: &ruff_python_ast::ExprAttribute,
    deprecated: &HashMap<String, DeprecatedInfo>,
    module_aliases: &HashMap<String, String>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    var_types: &HashMap<String, VarType>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    call_span: Option<Span>,
) {
    let member_name = attr.attr.as_str();
    let span = call_span.unwrap_or_else(|| text_range_to_span(attr.range()));

    if let Expr::Name(value_name) = attr.value.as_ref() {
        let alias = value_name.id.as_str();

        // Case 1: `library.func_name` where `library` is a module alias with deprecated members.
        if let Some(members) = deprecated_members.get(alias) {
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

        // Case 2: local qualified name like `ClassName.method` matches a deprecated key directly.
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

        // Case 3: `alias` is a typed variable; look up its class's deprecated members.
        if let Some(var_type) = var_types.get(alias) {
            let key = format!("{}.{}", var_type.class_name, member_name);
            if !var_type.module_alias.is_empty() {
                if let Some(members) = deprecated_members.get(&var_type.module_alias) {
                    if let Some(info) = members.get(&key) {
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
            }
            if let Some(info) = deprecated.get(&key) {
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
    }

    // Recurse into the value expression for chained access.
    visit_expr_for_usage(
        attr.value.as_ref(),
        deprecated,
        module_aliases,
        deprecated_members,
        var_types,
        path,
        diagnostics,
    );
}

/// Check if a binary operation triggers a deprecated dunder method on the left operand.
#[expect(
    clippy::too_many_arguments,
    reason = "binary op deprecation check requires full context"
)]
fn check_binop_deprecated(
    left: &Expr,
    op: Operator,
    deprecated: &HashMap<String, DeprecatedInfo>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    var_types: &HashMap<String, VarType>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    span: Span,
) {
    let dunder = op_to_dunder(op);
    if let Expr::Name(name) = left {
        let var_name = name.id.as_str();
        if let Some(var_type) = var_types.get(var_name) {
            check_dunder_deprecated_on_type(
                var_type,
                dunder,
                deprecated,
                deprecated_members,
                path,
                diagnostics,
                span,
            );
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
