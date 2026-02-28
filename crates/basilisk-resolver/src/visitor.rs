//! AST visitor that collects function definitions and module-level information.

use ruff_python_ast::{
    Alias, Decorator, ElifElseClause, Expr, ExceptHandler, MatchCase, Parameter,
    ParameterWithDefault, Pattern, Stmt, StmtAnnAssign, StmtAssign, StmtClassDef, StmtFunctionDef,
    StmtImport, StmtImportFrom, StmtMatch, StmtReturn,
};
use ruff_text_size::{Ranged, TextRange};

use basilisk_parser::ParsedModule;

use crate::scope::{
    AttributeInfo, ClassInfo, FunctionInfo, ImportInfo, ImportKind, MatchStmtInfo, ParameterInfo,
    ResolvedModule, ReturnAnnotationKind, ReturnStmtInfo, RhsKind, Span, VariableInfo,
};

/// Collect all function definitions and module-level data from the parsed module.
pub(crate) fn collect(module: &ParsedModule) -> ResolvedModule {
    let mut functions = Vec::new();
    let mut classes = Vec::new();
    let mut module_vars = Vec::new();
    let mut imports = Vec::new();
    let mut match_stmts = Vec::new();

    collect_from_body(
        &module.ast.body,
        &mut functions,
        &mut classes,
        &mut module_vars,
        &mut imports,
        &mut match_stmts,
        true,
    );

    ResolvedModule {
        functions,
        classes,
        module_vars,
        imports,
        match_stmts,
        path: module.path.clone(),
        source: module.source.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_from_body(
    stmts: &[Stmt],
    functions: &mut Vec<FunctionInfo>,
    classes: &mut Vec<ClassInfo>,
    module_vars: &mut Vec<VariableInfo>,
    imports: &mut Vec<ImportInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
    is_module_level: bool,
) {
    for stmt in stmts {
        collect_from_stmt(
            stmt,
            functions,
            classes,
            module_vars,
            imports,
            match_stmts,
            is_module_level,
        );
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn collect_from_stmt(
    stmt: &Stmt,
    functions: &mut Vec<FunctionInfo>,
    classes: &mut Vec<ClassInfo>,
    module_vars: &mut Vec<VariableInfo>,
    imports: &mut Vec<ImportInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
    is_module_level: bool,
) {
    match stmt {
        Stmt::FunctionDef(func) => {
            functions.push(function_info_from(func));
            collect_from_body(
                &func.body,
                functions,
                &mut Vec::new(),
                &mut Vec::new(),
                &mut Vec::new(),
                match_stmts,
                false,
            );
        }
        Stmt::ClassDef(class) => {
            let class_info = class_info_from(class, functions, match_stmts);
            classes.push(class_info);
        }
        Stmt::If(node) => {
            collect_from_body(
                &node.body,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                is_module_level,
            );
            collect_from_elif_else(
                &node.elif_else_clauses,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                is_module_level,
            );
        }
        Stmt::For(node) => {
            collect_from_body(
                &node.body,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                false,
            );
            collect_from_body(
                &node.orelse,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                false,
            );
        }
        Stmt::While(node) => {
            collect_from_body(
                &node.body,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                false,
            );
            collect_from_body(
                &node.orelse,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                false,
            );
        }
        Stmt::With(node) => {
            collect_from_body(
                &node.body,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                false,
            );
        }
        Stmt::Try(node) => {
            collect_from_body(
                &node.body,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                false,
            );
            collect_from_handlers(
                &node.handlers,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
            );
            collect_from_body(
                &node.orelse,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                false,
            );
            collect_from_body(
                &node.finalbody,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                false,
            );
        }
        Stmt::Import(node) => {
            if is_module_level {
                imports.extend(import_infos_from(node));
            }
        }
        Stmt::ImportFrom(node) => {
            if is_module_level {
                imports.extend(import_from_infos_from(node));
            }
        }
        Stmt::Assign(node) => {
            if is_module_level {
                module_vars.extend(assign_infos_from(node));
            }
        }
        Stmt::AnnAssign(node) => {
            if is_module_level {
                if let Some(var) = ann_assign_info_from(node) {
                    module_vars.push(var);
                }
            }
        }
        Stmt::Match(node) => {
            match_stmts.push(match_stmt_info_from(node));
            for case in &node.cases {
                collect_from_body(
                    &case.body,
                    functions,
                    classes,
                    module_vars,
                    imports,
                    match_stmts,
                    false,
                );
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_from_elif_else(
    clauses: &[ElifElseClause],
    functions: &mut Vec<FunctionInfo>,
    class_defs: &mut Vec<ClassInfo>,
    module_vars: &mut Vec<VariableInfo>,
    imports: &mut Vec<ImportInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
    is_module_level: bool,
) {
    for clause in clauses {
        collect_from_body(
            &clause.body,
            functions,
            class_defs,
            module_vars,
            imports,
            match_stmts,
            is_module_level,
        );
    }
}

fn collect_from_handlers(
    handlers: &[ExceptHandler],
    functions: &mut Vec<FunctionInfo>,
    classes: &mut Vec<ClassInfo>,
    module_vars: &mut Vec<VariableInfo>,
    imports: &mut Vec<ImportInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
) {
    for handler in handlers {
        let ExceptHandler::ExceptHandler(h) = handler;
        collect_from_body(
            &h.body,
            functions,
            classes,
            module_vars,
            imports,
            match_stmts,
            false,
        );
    }
}

// ---------------------------------------------------------------------------
// Class info
// ---------------------------------------------------------------------------

fn class_info_from(
    class: &StmtClassDef,
    functions: &mut Vec<FunctionInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
) -> ClassInfo {
    let bases = class
        .arguments
        .as_ref()
        .map(|args| {
            args.args
                .iter()
                .filter_map(expr_simple_name)
                .collect()
        })
        .unwrap_or_default();

    let mut attributes = Vec::new();
    let mut method_names = Vec::new();
    let mut method_decorators: Vec<(String, Vec<String>)> = Vec::new();

    for stmt in &class.body {
        match stmt {
            Stmt::AnnAssign(ann) => {
                if let Some(name) = expr_simple_name(&ann.target) {
                    attributes.push(AttributeInfo {
                        name,
                        name_span: text_range_to_span(ann.target.range()),
                        has_annotation: true,
                    });
                }
            }
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    if let Some(name) = expr_simple_name(target) {
                        attributes.push(AttributeInfo {
                            name,
                            name_span: text_range_to_span(target.range()),
                            has_annotation: false,
                        });
                    }
                }
            }
            Stmt::FunctionDef(func) => {
                let func_info = function_info_from(func);
                let method_name = func_info.name.clone();
                let decs = func_info.decorators.clone();
                functions.push(func_info);
                collect_from_body(
                    &func.body,
                    functions,
                    &mut Vec::new(),
                    &mut Vec::new(),
                    &mut Vec::new(),
                    match_stmts,
                    false,
                );
                method_names.push(method_name.clone());
                method_decorators.push((method_name, decs));
            }
            _ => {}
        }
    }

    ClassInfo {
        name: class.name.to_string(),
        name_span: text_range_to_span(class.name.range),
        def_span: text_range_to_span(class.range),
        bases,
        attributes,
        method_names,
        method_decorators,
    }
}

// ---------------------------------------------------------------------------
// Function info
// ---------------------------------------------------------------------------

fn function_info_from(func: &StmtFunctionDef) -> FunctionInfo {
    let params = &func.parameters;

    let positional: Vec<ParameterInfo> = params
        .posonlyargs
        .iter()
        .chain(params.args.iter())
        .map(param_with_default_to_info)
        .collect();

    let kwonly: Vec<ParameterInfo> = params
        .kwonlyargs
        .iter()
        .map(param_with_default_to_info)
        .collect();

    let all_params: Vec<ParameterInfo> = positional.into_iter().chain(kwonly).collect();
    let vararg = params.vararg.as_deref().map(parameter_to_info);
    let kwarg = params.kwarg.as_deref().map(parameter_to_info);

    let return_annotation = func.returns
        .as_deref()
        .map_or(ReturnAnnotationKind::Missing, return_annotation_kind);

    let decorators = func
        .decorator_list
        .iter()
        .filter_map(decorator_name)
        .collect();

    let return_stmts = collect_return_stmts(&func.body);

    FunctionInfo {
        name: func.name.to_string(),
        parameters: all_params,
        vararg,
        kwarg,
        return_annotation,
        decorators,
        return_stmts,
        def_span: text_range_to_span(func.range),
        name_span: text_range_to_span(func.name.range),
    }
}

fn param_with_default_to_info(p: &ParameterWithDefault) -> ParameterInfo {
    parameter_to_info(&p.parameter)
}

fn parameter_to_info(p: &Parameter) -> ParameterInfo {
    let (annotation_is_any, annotation_is_numeric_literal) = p
        .annotation
        .as_deref()
        .map_or((false, false), |e| {
            let (is_any, _, is_num) = annotation_flags(e);
            (is_any, is_num)
        });

    ParameterInfo {
        name: p.name.to_string(),
        has_annotation: p.annotation.is_some(),
        annotation_is_any,
        annotation_is_numeric_literal,
        name_span: text_range_to_span(p.name.range),
    }
}

/// Collect `return` statements from a function body (not into nested functions).
fn collect_return_stmts(stmts: &[Stmt]) -> Vec<ReturnStmtInfo> {
    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::Return(ret) => out.push(return_stmt_info_from(ret)),
            Stmt::If(node) => {
                out.extend(collect_return_stmts(&node.body));
                for clause in &node.elif_else_clauses {
                    out.extend(collect_return_stmts(&clause.body));
                }
            }
            Stmt::For(node) => {
                out.extend(collect_return_stmts(&node.body));
                out.extend(collect_return_stmts(&node.orelse));
            }
            Stmt::While(node) => {
                out.extend(collect_return_stmts(&node.body));
                out.extend(collect_return_stmts(&node.orelse));
            }
            Stmt::With(node) => out.extend(collect_return_stmts(&node.body)),
            Stmt::Try(node) => {
                out.extend(collect_return_stmts(&node.body));
                for handler in &node.handlers {
                    let ExceptHandler::ExceptHandler(h) = handler;
                    out.extend(collect_return_stmts(&h.body));
                }
                out.extend(collect_return_stmts(&node.orelse));
                out.extend(collect_return_stmts(&node.finalbody));
            }
            // Do NOT recurse into nested FunctionDef — those have their own return stmts.
            _ => {}
        }
    }
    out
}

fn return_stmt_info_from(ret: &StmtReturn) -> ReturnStmtInfo {
    let has_value = ret.value.as_deref().is_some_and(|e| {
        !matches!(e, Expr::NoneLiteral(_))
    });
    ReturnStmtInfo {
        span: text_range_to_span(ret.range),
        has_value,
    }
}

// ---------------------------------------------------------------------------
// Annotation analysis helpers
// ---------------------------------------------------------------------------

/// Maps a return annotation expression to its [`ReturnAnnotationKind`].
fn return_annotation_kind(expr: &Expr) -> ReturnAnnotationKind {
    let (is_any, is_none, is_num) = annotation_flags(expr);
    if is_any {
        ReturnAnnotationKind::Any
    } else if is_none {
        ReturnAnnotationKind::NoneType
    } else if is_num {
        ReturnAnnotationKind::NumericLiteral
    } else {
        ReturnAnnotationKind::Other
    }
}

/// Returns `(is_any, is_none, is_numeric_literal)` for an annotation expression.
fn annotation_flags(expr: &Expr) -> (bool, bool, bool) {
    match expr {
        Expr::Name(name) => {
            let s = name.id.as_str();
            (s == "Any", s == "None", false)
        }
        Expr::Attribute(attr) => {
            let s = attr.attr.as_str();
            (s == "Any", s == "None", false)
        }
        Expr::NoneLiteral(_) => (false, true, false),
        Expr::NumberLiteral(_) | Expr::BooleanLiteral(_) => (false, false, true),
        _ => (false, false, false),
    }
}

// ---------------------------------------------------------------------------
// Import info
// ---------------------------------------------------------------------------

fn import_infos_from(node: &StmtImport) -> Vec<ImportInfo> {
    node.names
        .iter()
        .map(|alias| ImportInfo {
            module: alias.name.to_string(),
            names: Vec::new(),
            span: text_range_to_span(node.range),
            kind: ImportKind::Plain,
        })
        .collect()
}

fn import_from_infos_from(node: &StmtImportFrom) -> Vec<ImportInfo> {
    let module = node
        .module
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();

    let is_star = node.names.iter().any(|a| a.name.as_str() == "*");

    if is_star {
        return vec![ImportInfo {
            module,
            names: Vec::new(),
            span: text_range_to_span(node.range),
            kind: ImportKind::Star,
        }];
    }

    let names: Vec<String> = node.names.iter().map(alias_name).collect();
    vec![ImportInfo {
        module,
        names,
        span: text_range_to_span(node.range),
        kind: ImportKind::From,
    }]
}

fn alias_name(alias: &Alias) -> String {
    alias.name.to_string()
}

// ---------------------------------------------------------------------------
// Variable assignment info
// ---------------------------------------------------------------------------

fn assign_infos_from(node: &StmtAssign) -> Vec<VariableInfo> {
    let rhs_kind = classify_rhs(&node.value);
    node.targets
        .iter()
        .filter_map(|target| {
            expr_simple_name(target).map(|name| VariableInfo {
                name,
                name_span: text_range_to_span(target.range()),
                has_annotation: false,
                rhs_kind: rhs_kind.clone(),
            })
        })
        .collect()
}

fn ann_assign_info_from(node: &StmtAnnAssign) -> Option<VariableInfo> {
    let name = expr_simple_name(&node.target)?;
    let rhs_kind = node
        .value
        .as_deref()
        .map_or(RhsKind::Other, classify_rhs);
    Some(VariableInfo {
        name,
        name_span: text_range_to_span(node.target.range()),
        has_annotation: true,
        rhs_kind,
    })
}

fn classify_rhs(expr: &Expr) -> RhsKind {
    match expr {
        Expr::BooleanLiteral(_) => RhsKind::BoolLiteral,
        Expr::NumberLiteral(n) => {
            if matches!(n.value, ruff_python_ast::Number::Float(_)) {
                RhsKind::FloatLiteral
            } else {
                RhsKind::IntLiteral
            }
        }
        Expr::StringLiteral(_) | Expr::FString(_) => RhsKind::StrLiteral,
        Expr::BytesLiteral(_) => RhsKind::BytesLiteral,
        Expr::NoneLiteral(_) => RhsKind::NoneValue,
        Expr::List(list) if list.elts.is_empty() => RhsKind::EmptyList,
        Expr::Dict(dict) if dict.items.is_empty() => RhsKind::EmptyDict,
        Expr::Call(_) => RhsKind::CallExpr,
        _ => RhsKind::Other,
    }
}

// ---------------------------------------------------------------------------
// Match statement info
// ---------------------------------------------------------------------------

fn match_stmt_info_from(node: &StmtMatch) -> MatchStmtInfo {
    let has_wildcard = node.cases.iter().any(is_wildcard_case);
    MatchStmtInfo {
        span: text_range_to_span(node.range),
        has_wildcard,
    }
}

fn is_wildcard_case(case: &MatchCase) -> bool {
    is_wildcard_pattern(&case.pattern)
}

fn is_wildcard_pattern(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::MatchAs(ma) => ma.name.is_none() && ma.pattern.is_none(),
        Pattern::MatchOr(mo) => mo.patterns.iter().any(is_wildcard_pattern),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Decorator helpers
// ---------------------------------------------------------------------------

fn decorator_name(dec: &Decorator) -> Option<String> {
    match &dec.expression {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attr) => Some(attr.attr.to_string()),
        Expr::Call(call) => match call.func.as_ref() {
            Expr::Name(name) => Some(name.id.to_string()),
            Expr::Attribute(attr) => Some(attr.attr.to_string()),
            _ => None,
        },
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

fn expr_simple_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        _ => None,
    }
}

fn text_range_to_span(range: TextRange) -> Span {
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}
