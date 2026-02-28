//! AST visitor that collects function definitions and module-level information.

const ENUM_BASES: &[&str] = &["Enum", "IntEnum", "StrEnum", "Flag", "IntFlag", "ReprEnum"];

use ruff_python_ast::{
    Alias, Decorator, ElifElseClause, ExceptHandler, Expr, MatchCase, Parameter, ParameterWithDefault,
    Pattern, Stmt, StmtAnnAssign, StmtAssign, StmtClassDef, StmtFunctionDef, StmtImport,
    StmtImportFrom, StmtMatch, StmtReturn, TypeParam,
};
use ruff_text_size::{Ranged, TextRange};

use basilisk_parser::ParsedModule;

use crate::scope::{
    AttributeInfo, CallSite, ClassInfo, FunctionInfo, GenericParamInfo, ImportInfo, ImportKind,
    MatchStmtInfo, ParameterInfo, ResolvedModule, ReturnAnnotationKind, ReturnStmtInfo,
    RevealTypeCallInfo, RhsKind, Span, TypedDictCallInfo, TypedDictSecondArgKind, TypeVarCallInfo,
    UnhashableKeyRef, VariableInfo,
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

    let calls = collect_module_level_calls(&module.ast.body);
    let typevar_calls = collect_typevar_calls(&module.ast.body);
    let reveal_type_calls = collect_reveal_type_calls(&module.ast.body);
    let assert_type_calls = collect_special_calls(&module.ast.body, "assert_type");
    let typeddict_calls = collect_typeddict_calls(&module.ast.body);

    ResolvedModule {
        functions,
        classes,
        module_vars,
        imports,
        match_stmts,
        calls,
        typevar_calls,
        reveal_type_calls,
        assert_type_calls,
        typeddict_calls,
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
            functions.push(function_info_from(func, None));
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

#[allow(clippy::type_complexity)]
fn collect_class_body(
    class: &StmtClassDef,
    functions: &mut Vec<FunctionInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
) -> (Vec<AttributeInfo>, Vec<String>, Vec<(String, Vec<String>)>) {
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
                        annotation_span: Some(text_range_to_span(ann.annotation.range())),
                        has_value: ann.value.is_some(),
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
                            annotation_span: None,
                            has_value: true,
                        });
                    }
                }
            }
            Stmt::FunctionDef(func) => {
                let func_info = function_info_from(func, Some(class.name.to_string()));
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
            Stmt::ClassDef(inner_class) => {
                // Recurse into nested classes so their methods are checked
                // by E0001/E0002.  The inner ClassInfo is not added to the
                // module's class list (Phase 1 limitation), but all its
                // method FunctionInfos land in `functions`.
                let _inner_info = class_info_from(inner_class, functions, match_stmts);
            }
            _ => {}
        }
    }

    (attributes, method_names, method_decorators)
}

fn class_info_from(
    class: &StmtClassDef,
    functions: &mut Vec<FunctionInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
) -> ClassInfo {
    let bases: Vec<String> = class
        .arguments
        .as_ref()
        .map(|args| args.args.iter().filter_map(expr_simple_name).collect())
        .unwrap_or_default();

    let (attributes, method_names, method_decorators) =
        collect_class_body(class, functions, match_stmts);

    let (generic_params, generic_non_typevar_args) = extract_generic_params(class);
    let is_typed_dict = bases.iter().any(|b| b == "TypedDict");

    let class_keywords: Vec<String> = class
        .arguments
        .as_ref()
        .map(|args| {
            args.keywords
                .iter()
                .filter_map(|kw| kw.arg.as_ref().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();

    let class_decorators: Vec<String> = class
        .decorator_list
        .iter()
        .filter_map(decorator_name)
        .collect();

    let is_dataclass = class_decorators
        .iter()
        .any(|d| d == "dataclass" || d.ends_with(".dataclass"));

    let is_final = class_decorators
        .iter()
        .any(|d| d == "final" || d.rsplit('.').next() == Some("final"));

    let is_enum = bases.iter().any(|b| ENUM_BASES.contains(&b.as_str()));

    let has_pep695_type_params = class.type_params.is_some();
    let pep695_type_param_names: Vec<String> = class
        .type_params
        .as_deref()
        .map(|tp| tp.type_params.iter().map(type_param_name).collect())
        .unwrap_or_default();
    let base_expression_names: Vec<String> = class
        .arguments
        .as_ref()
        .map(|args| {
            let mut names = Vec::new();
            for expr in &args.args {
                collect_name_refs_from_expr(expr, &mut names);
            }
            names
        })
        .unwrap_or_default();

    ClassInfo {
        name: class.name.to_string(),
        name_span: text_range_to_span(class.name.range),
        def_span: text_range_to_span(class.range),
        bases,
        attributes,
        method_names,
        method_decorators,
        generic_params,
        is_typed_dict,
        class_keywords,
        is_dataclass,
        is_final,
        is_enum,
        has_pep695_type_params,
        pep695_type_param_names,
        base_expression_names,
        generic_non_typevar_args,
    }
}

// ---------------------------------------------------------------------------
// Function info
// ---------------------------------------------------------------------------

fn function_info_from(func: &StmtFunctionDef, class_name: Option<String>) -> FunctionInfo {
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

    let return_annotation = func
        .returns
        .as_deref()
        .map_or(ReturnAnnotationKind::Missing, return_annotation_kind);

    let return_annotation_span = func
        .returns
        .as_deref()
        .map(|e| text_range_to_span(e.range()));

    let decorators = func
        .decorator_list
        .iter()
        .filter_map(decorator_name)
        .collect();

    let return_stmts = collect_return_stmts(&func.body);
    let all_local_assigns = collect_all_assigns(&func.body);
    let unconditional_assigns = collect_unconditional_assigns(&func.body);
    let return_name_refs = collect_return_name_refs(&func.body);
    let top_level_return_name_refs = collect_top_level_return_name_refs(&func.body);
    let unhashable_keys = collect_unhashable_keys_from_stmts(&func.body);
    let is_stub_body = body_is_stub(&func.body);
    let has_pep695_type_params = func.type_params.is_some();
    let pep695_type_param_names: Vec<String> = func
        .type_params
        .as_deref()
        .map(|tp| tp.type_params.iter().map(type_param_name).collect())
        .unwrap_or_default();

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
        return_annotation_span,
        class_name,
        all_local_assigns,
        unconditional_assigns,
        return_name_refs,
        top_level_return_name_refs,
        unhashable_keys,
        is_stub_body,
        has_pep695_type_params,
        pep695_type_param_names,
    }
}

/// Returns `true` when a function body is a pure ellipsis stub (`...`).
///
/// Only `...` — optionally preceded by a docstring — is treated as a stub.
/// `pass` is valid in real function bodies and must not suppress diagnostics.
///
/// These stubs appear in `@overload` signatures, Protocol method declarations,
/// and abstract method placeholders where annotation enforcement should not apply.
fn body_is_stub(stmts: &[Stmt]) -> bool {
    let non_docstring: Vec<&Stmt> = stmts
        .iter()
        .skip_while(|s| matches!(s, Stmt::Expr(e) if matches!(e.value.as_ref(), Expr::StringLiteral(_))))
        .collect();

    non_docstring.iter().all(|s| {
        matches!(s, Stmt::Expr(e) if matches!(e.value.as_ref(), Expr::EllipsisLiteral(_)))
    })
}

fn param_with_default_to_info(p: &ParameterWithDefault) -> ParameterInfo {
    let mut info = parameter_to_info(&p.parameter);
    info.has_default = p.default.is_some();
    info
}

fn parameter_to_info(p: &Parameter) -> ParameterInfo {
    let (annotation_is_any, annotation_is_numeric_literal) =
        p.annotation.as_deref().map_or((false, false), |e| {
            let (is_any, _, is_num) = annotation_flags(e);
            (is_any, is_num)
        });

    ParameterInfo {
        name: p.name.to_string(),
        has_annotation: p.annotation.is_some(),
        annotation_is_any,
        annotation_is_numeric_literal,
        has_default: false,
        name_span: text_range_to_span(p.name.range),
        annotation_span: p
            .annotation
            .as_deref()
            .map(|e| text_range_to_span(e.range())),
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
    let value_expr = ret.value.as_deref();
    let has_value = value_expr.is_some_and(|e| !matches!(e, Expr::NoneLiteral(_)));
    let value_is_call = value_expr.is_some_and(|e| matches!(e, Expr::Call(_)));
    ReturnStmtInfo {
        span: text_range_to_span(ret.range),
        has_value,
        value_is_call,
    }
}

// ---------------------------------------------------------------------------
// Assign name collection helpers
// ---------------------------------------------------------------------------

/// Extract all simple names from an assignment target expression.
/// Handles single names, tuples, and nested tuples (e.g. `for (x, y) in ...`).
fn extract_target_names(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::Name(name) => vec![name.id.to_string()],
        Expr::Tuple(tuple) => tuple.elts.iter().flat_map(extract_target_names).collect(),
        Expr::List(list) => list.elts.iter().flat_map(extract_target_names).collect(),
        _ => Vec::new(),
    }
}

/// Collect all names assigned anywhere in the function body (not in nested functions).
fn collect_all_assigns(stmts: &[Stmt]) -> Vec<String> {
    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::Assign(node) => {
                out.extend(node.targets.iter().flat_map(extract_target_names));
            }
            Stmt::AnnAssign(node) => {
                if let Some(name) = expr_simple_name(&node.target) {
                    out.push(name);
                }
            }
            Stmt::For(node) => {
                out.extend(extract_target_names(&node.target));
                out.extend(collect_all_assigns(&node.body));
                out.extend(collect_all_assigns(&node.orelse));
            }
            Stmt::FunctionDef(func) => {
                // Nested function name is defined in enclosing scope.
                out.push(func.name.to_string());
                // Do NOT recurse into nested function body.
            }
            Stmt::If(node) => {
                out.extend(collect_all_assigns(&node.body));
                for clause in &node.elif_else_clauses {
                    out.extend(collect_all_assigns(&clause.body));
                }
            }
            Stmt::While(node) => {
                out.extend(collect_all_assigns(&node.body));
                out.extend(collect_all_assigns(&node.orelse));
            }
            Stmt::With(node) => {
                for item in &node.items {
                    if let Some(var) = item.optional_vars.as_deref() {
                        out.extend(extract_target_names(var));
                    }
                }
                out.extend(collect_all_assigns(&node.body));
            }
            Stmt::Try(node) => {
                out.extend(collect_all_assigns(&node.body));
                for handler in &node.handlers {
                    let ExceptHandler::ExceptHandler(h) = handler;
                    if let Some(exc_name) = &h.name {
                        out.push(exc_name.to_string());
                    }
                    out.extend(collect_all_assigns(&h.body));
                }
                out.extend(collect_all_assigns(&node.orelse));
                out.extend(collect_all_assigns(&node.finalbody));
            }
            _ => {}
        }
    }
    out
}

/// Collect names assigned at the top level of a function body (unconditionally).
fn collect_unconditional_assigns(stmts: &[Stmt]) -> Vec<String> {
    stmts
        .iter()
        .flat_map(|stmt| match stmt {
            Stmt::Assign(node) => node
                .targets
                .iter()
                .flat_map(extract_target_names)
                .collect::<Vec<_>>(),
            Stmt::AnnAssign(node) => expr_simple_name(&node.target).into_iter().collect(),
            Stmt::For(node) => {
                // The for-loop variable(s) are bound whenever the loop body runs.
                extract_target_names(&node.target)
            }
            Stmt::FunctionDef(func) => vec![func.name.to_string()],
            _ => Vec::new(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Return name ref collection
// ---------------------------------------------------------------------------

/// Collect `(name, span)` pairs from `return <name>` stmts in a function body.
/// Does not recurse into nested function definitions.
fn collect_return_name_refs(stmts: &[Stmt]) -> Vec<(String, Span)> {
    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::Return(ret) => {
                if let Some(Expr::Name(name)) = ret.value.as_deref() {
                    out.push((name.id.to_string(), text_range_to_span(name.range)));
                }
            }
            Stmt::If(node) => {
                out.extend(collect_return_name_refs(&node.body));
                for clause in &node.elif_else_clauses {
                    out.extend(collect_return_name_refs(&clause.body));
                }
            }
            Stmt::For(node) => {
                out.extend(collect_return_name_refs(&node.body));
                out.extend(collect_return_name_refs(&node.orelse));
            }
            Stmt::While(node) => {
                out.extend(collect_return_name_refs(&node.body));
                out.extend(collect_return_name_refs(&node.orelse));
            }
            Stmt::With(node) => {
                out.extend(collect_return_name_refs(&node.body));
            }
            Stmt::Try(node) => {
                out.extend(collect_return_name_refs(&node.body));
                for handler in &node.handlers {
                    let ExceptHandler::ExceptHandler(h) = handler;
                    out.extend(collect_return_name_refs(&h.body));
                }
                out.extend(collect_return_name_refs(&node.orelse));
                out.extend(collect_return_name_refs(&node.finalbody));
            }
            // Do NOT recurse into nested FunctionDef.
            _ => {}
        }
    }
    out
}

/// Collects `return <name>` references from the TOP LEVEL of a function body only.
///
/// Unlike [`collect_return_name_refs`], this does NOT recurse into `if`/`for`/
/// `while`/`try`/`with` blocks.  A `return name` inside a conditional branch will
/// only execute when that branch is taken, so `name` is always bound at that point
/// if it was assigned earlier in the same branch.  Recursing would produce false
/// positives; this conservative variant is used by E0019.
fn collect_top_level_return_name_refs(stmts: &[Stmt]) -> Vec<(String, Span)> {
    stmts
        .iter()
        .filter_map(|stmt| {
            if let Stmt::Return(ret) = stmt {
                if let Some(Expr::Name(name)) = ret.value.as_deref() {
                    return Some((name.id.to_string(), text_range_to_span(name.range)));
                }
            }
            None
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Unhashable key collection
// ---------------------------------------------------------------------------

/// Walk all statements in a function body looking for dict literals with unhashable keys.
fn collect_unhashable_keys_from_stmts(stmts: &[Stmt]) -> Vec<UnhashableKeyRef> {
    let mut out = Vec::new();
    for stmt in stmts {
        collect_unhashable_keys_from_stmt(stmt, &mut out);
    }
    out
}

#[allow(clippy::too_many_lines)]
fn collect_unhashable_keys_from_stmt(stmt: &Stmt, out: &mut Vec<UnhashableKeyRef>) {
    match stmt {
        Stmt::Assign(node) => collect_unhashable_keys_from_expr(&node.value, out),
        Stmt::AnnAssign(node) => {
            if let Some(val) = node.value.as_deref() {
                collect_unhashable_keys_from_expr(val, out);
            }
        }
        Stmt::Return(node) => {
            if let Some(val) = node.value.as_deref() {
                collect_unhashable_keys_from_expr(val, out);
            }
        }
        Stmt::Expr(node) => collect_unhashable_keys_from_expr(&node.value, out),
        Stmt::If(node) => {
            collect_unhashable_keys_from_expr(&node.test, out);
            for s in &node.body {
                collect_unhashable_keys_from_stmt(s, out);
            }
            for clause in &node.elif_else_clauses {
                if let Some(test) = &clause.test {
                    collect_unhashable_keys_from_expr(test, out);
                }
                for s in &clause.body {
                    collect_unhashable_keys_from_stmt(s, out);
                }
            }
        }
        Stmt::For(node) => {
            collect_unhashable_keys_from_expr(&node.iter, out);
            for s in &node.body {
                collect_unhashable_keys_from_stmt(s, out);
            }
        }
        Stmt::While(node) => {
            collect_unhashable_keys_from_expr(&node.test, out);
            for s in &node.body {
                collect_unhashable_keys_from_stmt(s, out);
            }
        }
        Stmt::With(node) => {
            for s in &node.body {
                collect_unhashable_keys_from_stmt(s, out);
            }
        }
        Stmt::Try(node) => {
            for s in &node.body {
                collect_unhashable_keys_from_stmt(s, out);
            }
            for handler in &node.handlers {
                let ExceptHandler::ExceptHandler(h) = handler;
                for s in &h.body {
                    collect_unhashable_keys_from_stmt(s, out);
                }
            }
        }
        // Do NOT recurse into nested FunctionDef.
        _ => {}
    }
}

fn collect_unhashable_keys_from_expr(expr: &Expr, out: &mut Vec<UnhashableKeyRef>) {
    match expr {
        Expr::Dict(dict) => {
            for item in &dict.items {
                if let Some(key) = item.key.as_ref() {
                    let key_type_opt = match key {
                        Expr::List(_) => Some("list"),
                        Expr::Set(_) => Some("set"),
                        Expr::Dict(_) => Some("dict"),
                        _ => None,
                    };
                    if let Some(key_type) = key_type_opt {
                        out.push(UnhashableKeyRef {
                            span: text_range_to_span(key.range()),
                            key_type,
                        });
                    }
                    collect_unhashable_keys_from_expr(key, out);
                }
                collect_unhashable_keys_from_expr(&item.value, out);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                collect_unhashable_keys_from_expr(elt, out);
            }
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                collect_unhashable_keys_from_expr(elt, out);
            }
        }
        Expr::Call(call) => {
            collect_unhashable_keys_from_expr(&call.func, out);
            for arg in &call.arguments.args {
                collect_unhashable_keys_from_expr(arg, out);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Module-level call site collection
// ---------------------------------------------------------------------------

/// Collect call sites from module-level statements.
fn collect_module_level_calls(stmts: &[Stmt]) -> Vec<CallSite> {
    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::AnnAssign(node) => {
                if let Some(val) = node.value.as_deref() {
                    if let Some(site) = call_site_from_expr(val) {
                        out.push(site);
                    }
                }
            }
            Stmt::Assign(node) => {
                if let Some(site) = call_site_from_expr(&node.value) {
                    out.push(site);
                }
            }
            Stmt::Expr(node) => {
                if let Some(site) = call_site_from_expr(&node.value) {
                    out.push(site);
                }
            }
            _ => {}
        }
    }
    out
}

/// Collect module-level `TypeVar(...)` assignments.
/// Returns `true` if an expression is a `TypeVar(...)` or `typing.TypeVar(...)` call.
fn is_typevar_call(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else { return false };
    (expr_simple_name(&call.func)
        .as_deref() == Some("TypeVar"))
        || matches!(call.func.as_ref(), Expr::Attribute(a) if a.attr.as_str() == "TypeVar")
}

/// Builds a `TypeVarCallInfo` from a `TypeVar(...)` call expression and a bound name.
fn typevar_call_info_from(name: String, call: &ruff_python_ast::ExprCall) -> TypeVarCallInfo {
    use ruff_text_size::Ranged as _;
    let positional_args = call.arguments.args.len();
    let constraint_count = positional_args.saturating_sub(1);
    let has_default = call
        .arguments
        .keywords
        .iter()
        .any(|kw| kw.arg.as_ref().is_some_and(|a| a.as_str() == "default"));
    let has_bound = call
        .arguments
        .keywords
        .iter()
        .any(|kw| kw.arg.as_ref().is_some_and(|a| a.as_str() == "bound"));
    let has_parameterized_bound = call
        .arguments
        .keywords
        .iter()
        .find(|kw| kw.arg.as_ref().is_some_and(|a| a.as_str() == "bound"))
        .is_some_and(|kw| expr_is_parameterized(&kw.value));
    let has_parameterized_constraint = call
        .arguments
        .args
        .iter()
        .skip(1)
        .any(expr_is_parameterized);
    TypeVarCallInfo {
        name,
        constraint_count,
        has_default,
        has_bound,
        has_parameterized_bound,
        has_parameterized_constraint,
        span: text_range_to_span(call.range()),
    }
}

fn collect_typevar_calls(stmts: &[Stmt]) -> Vec<TypeVarCallInfo> {
    let mut out = Vec::new();
    collect_typevar_calls_from_stmts(stmts, &mut out);
    out
}

fn collect_typevar_calls_from_stmts(stmts: &[Stmt], out: &mut Vec<TypeVarCallInfo>) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign(node) => {
                let Expr::Call(call) = node.value.as_ref() else {
                    continue;
                };
                if !is_typevar_call(node.value.as_ref()) {
                    continue;
                }
                let Some(name) = node.targets.first().and_then(expr_simple_name) else {
                    continue;
                };
                out.push(typevar_call_info_from(name, call));
            }
            Stmt::AnnAssign(node) => {
                let Some(val) = node.value.as_deref() else {
                    continue;
                };
                let Expr::Call(call) = val else { continue };
                if !is_typevar_call(val) {
                    continue;
                }
                let Some(name) = expr_simple_name(&node.target) else {
                    continue;
                };
                out.push(typevar_call_info_from(name, call));
            }
            // Also search inside class bodies (TypeVars declared as class attributes).
            Stmt::ClassDef(cls) => {
                collect_typevar_calls_from_stmts(&cls.body, out);
            }
            _ => {}
        }
    }
}

/// Collect all `reveal_type(...)` calls found anywhere in the module body.
fn collect_reveal_type_calls(stmts: &[Stmt]) -> Vec<RevealTypeCallInfo> {
    let mut out = Vec::new();
    collect_reveal_type_calls_from_stmts(stmts, &mut out);
    out
}

fn collect_reveal_type_calls_from_stmts(stmts: &[Stmt], out: &mut Vec<RevealTypeCallInfo>) {
    for stmt in stmts {
        collect_reveal_type_calls_from_stmt(stmt, out);
    }
}

fn collect_reveal_type_calls_from_stmt(stmt: &Stmt, out: &mut Vec<RevealTypeCallInfo>) {
    match stmt {
        Stmt::Expr(node) => {
            if let Expr::Call(call) = node.value.as_ref() {
                let is_reveal_type = expr_simple_name(&call.func)
                    .is_some_and(|n| n == "reveal_type");
                if is_reveal_type {
                    out.push(RevealTypeCallInfo {
                        arg_count: call.arguments.args.len(),
                        span: text_range_to_span(call.range()),
                    });
                }
            }
        }
        Stmt::FunctionDef(func) => {
            collect_reveal_type_calls_from_stmts(&func.body, out);
        }
        Stmt::ClassDef(cls) => {
            collect_reveal_type_calls_from_stmts(&cls.body, out);
        }
        Stmt::If(node) => {
            collect_reveal_type_calls_from_stmts(&node.body, out);
            for elif_else in &node.elif_else_clauses {
                collect_reveal_type_calls_from_stmts(&elif_else.body, out);
            }
        }
        Stmt::For(node) => {
            collect_reveal_type_calls_from_stmts(&node.body, out);
            collect_reveal_type_calls_from_stmts(&node.orelse, out);
        }
        Stmt::While(node) => {
            collect_reveal_type_calls_from_stmts(&node.body, out);
            collect_reveal_type_calls_from_stmts(&node.orelse, out);
        }
        Stmt::With(node) => {
            collect_reveal_type_calls_from_stmts(&node.body, out);
        }
        Stmt::Try(node) => {
            collect_reveal_type_calls_from_stmts(&node.body, out);
            for handler in &node.handlers {
                let ExceptHandler::ExceptHandler(h) = handler;
                collect_reveal_type_calls_from_stmts(&h.body, out);
            }
            collect_reveal_type_calls_from_stmts(&node.orelse, out);
            collect_reveal_type_calls_from_stmts(&node.finalbody, out);
        }
        Stmt::Match(node) => {
            for case in &node.cases {
                collect_reveal_type_calls_from_stmts(&case.body, out);
            }
        }
        _ => {}
    }
}

/// Collect all calls to a specific function name found anywhere in the module body.
///
/// Reuses `RevealTypeCallInfo` to record the argument count and span.
fn collect_special_calls(stmts: &[Stmt], func_name: &str) -> Vec<RevealTypeCallInfo> {
    let mut out = Vec::new();
    collect_special_calls_from_stmts(stmts, func_name, &mut out);
    out
}

fn collect_special_calls_from_stmts(stmts: &[Stmt], func_name: &str, out: &mut Vec<RevealTypeCallInfo>) {
    for stmt in stmts {
        collect_special_calls_from_stmt(stmt, func_name, out);
    }
}

fn collect_special_calls_from_stmt(stmt: &Stmt, func_name: &str, out: &mut Vec<RevealTypeCallInfo>) {
    match stmt {
        Stmt::Expr(node) => {
            if let Expr::Call(call) = node.value.as_ref() {
                let is_target = expr_simple_name(&call.func)
                    .is_some_and(|n| n == func_name);
                if is_target {
                    out.push(RevealTypeCallInfo {
                        arg_count: call.arguments.args.len(),
                        span: text_range_to_span(call.range()),
                    });
                }
            }
        }
        Stmt::FunctionDef(func) => {
            collect_special_calls_from_stmts(&func.body, func_name, out);
        }
        Stmt::ClassDef(cls) => {
            collect_special_calls_from_stmts(&cls.body, func_name, out);
        }
        Stmt::If(node) => {
            collect_special_calls_from_stmts(&node.body, func_name, out);
            for elif_else in &node.elif_else_clauses {
                collect_special_calls_from_stmts(&elif_else.body, func_name, out);
            }
        }
        Stmt::For(node) => {
            collect_special_calls_from_stmts(&node.body, func_name, out);
            collect_special_calls_from_stmts(&node.orelse, func_name, out);
        }
        Stmt::While(node) => {
            collect_special_calls_from_stmts(&node.body, func_name, out);
            collect_special_calls_from_stmts(&node.orelse, func_name, out);
        }
        Stmt::With(node) => {
            collect_special_calls_from_stmts(&node.body, func_name, out);
        }
        Stmt::Try(node) => {
            collect_special_calls_from_stmts(&node.body, func_name, out);
            for handler in &node.handlers {
                let ExceptHandler::ExceptHandler(h) = handler;
                collect_special_calls_from_stmts(&h.body, func_name, out);
            }
            collect_special_calls_from_stmts(&node.orelse, func_name, out);
            collect_special_calls_from_stmts(&node.finalbody, func_name, out);
        }
        Stmt::Match(node) => {
            for case in &node.cases {
                collect_special_calls_from_stmts(&case.body, func_name, out);
            }
        }
        _ => {}
    }
}

/// Extract `Generic[T, ...]` or `Protocol[T, ...]` type parameter names and
/// any non-TypeVar (non-simple-name) argument spans from a class definition.
///
/// Returns `(type_params, non_typevar_arg_spans)`.
fn extract_generic_params(class: &StmtClassDef) -> (Vec<GenericParamInfo>, Vec<Span>) {
    let args = match class.arguments.as_ref() {
        Some(a) => &a.args,
        None => return (Vec::new(), Vec::new()),
    };
    for base in args {
        let Expr::Subscript(sub) = base else { continue };
        let is_generic_or_protocol =
            matches!(sub.value.as_ref(), Expr::Name(n) if n.id.as_str() == "Generic" || n.id.as_str() == "Protocol")
            || matches!(sub.value.as_ref(), Expr::Attribute(a) if a.attr.as_str() == "Generic" || a.attr.as_str() == "Protocol");
        if !is_generic_or_protocol {
            continue;
        }
        let elts: &[Expr] = match sub.slice.as_ref() {
            Expr::Tuple(tuple) => &tuple.elts,
            other => std::slice::from_ref(other),
        };
        let mut params = Vec::new();
        let mut non_typevar = Vec::new();
        for e in elts {
            // A starred expression like `*Ts` is a valid TypeVarTuple unpack.
            let name_opt = expr_simple_name(e).or_else(|| {
                if let Expr::Starred(starred) = e {
                    expr_simple_name(&starred.value)
                } else {
                    None
                }
            });
            match name_opt {
                Some(name) => params.push(GenericParamInfo {
                    span: text_range_to_span(e.range()),
                    name,
                }),
                None => non_typevar.push(text_range_to_span(e.range())),
            }
        }
        return (params, non_typevar);
    }
    (Vec::new(), Vec::new())
}

fn call_site_from_expr(expr: &Expr) -> Option<CallSite> {
    let Expr::Call(call) = expr else { return None };
    let callee = expr_simple_name(&call.func)?;
    let args: Vec<(RhsKind, Span)> = call
        .arguments
        .args
        .iter()
        .map(|arg| (classify_rhs(arg), text_range_to_span(arg.range())))
        .collect();
    let keyword_count = call.arguments.keywords.len();
    Some(CallSite {
        callee,
        args,
        keyword_count,
        span: text_range_to_span(call.range()),
    })
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
    let rhs_span = Some(text_range_to_span(node.value.range()));
    node.targets
        .iter()
        .filter_map(|target| {
            expr_simple_name(target).map(|name| VariableInfo {
                name,
                name_span: text_range_to_span(target.range()),
                has_annotation: false,
                rhs_kind: rhs_kind.clone(),
                annotation_span: None,
                rhs_span,
            })
        })
        .collect()
}

fn ann_assign_info_from(node: &StmtAnnAssign) -> Option<VariableInfo> {
    let name = expr_simple_name(&node.target)?;
    let rhs_kind = node.value.as_deref().map_or(RhsKind::Other, classify_rhs);
    let annotation_span = Some(text_range_to_span(node.annotation.range()));
    let rhs_span = node.value.as_deref().map(|v| text_range_to_span(v.range()));
    Some(VariableInfo {
        name,
        name_span: text_range_to_span(node.target.range()),
        has_annotation: true,
        rhs_kind,
        annotation_span,
        rhs_span,
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
// TypedDict functional-syntax call collection
// ---------------------------------------------------------------------------

/// Collect module-level `TypedDict(...)` functional-syntax call sites.
///
/// Matches assignments of the form `Name = TypedDict("Name", {...}, ...)`.
fn collect_typeddict_calls(stmts: &[Stmt]) -> Vec<TypedDictCallInfo> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::Assign(node) = stmt else { continue };
        let Expr::Call(call) = node.value.as_ref() else { continue };
        // Callee must be `TypedDict` or `typing.TypedDict`.
        let is_typeddict = if let Some(name) = expr_simple_name(&call.func) {
            name == "TypedDict"
        } else if let Expr::Attribute(attr) = call.func.as_ref() {
            attr.attr.as_str() == "TypedDict"
        } else {
            false
        };
        if !is_typeddict {
            continue;
        }
        // Determine the LHS name.
        let Some(lhs_name) = node.targets.first().and_then(expr_simple_name) else {
            continue;
        };
        // First positional arg: the declared name (must be a string literal).
        let declared_name = call.arguments.args.first().and_then(|arg| {
            if let Expr::StringLiteral(s) = arg {
                Some(s.value.to_string())
            } else {
                None
            }
        });
        // Second positional arg: expected to be a dict literal.
        let has_positional_dict;
        let (second_arg_kind, has_non_string_key) =
            if let Some(second_arg) = call.arguments.args.get(1) {
                has_positional_dict = true;
                if let Expr::Dict(dict) = second_arg {
                    // Check if every key is a string literal.
                    let non_string = dict.items.iter().any(|item| {
                        item.key.as_ref().is_some_and(|k| {
                            !matches!(k, Expr::StringLiteral(_))
                        })
                    });
                    (TypedDictSecondArgKind::DictLiteral, non_string)
                } else {
                    (TypedDictSecondArgKind::NotDictLiteral, false)
                }
            } else {
                // No second arg — keyword syntax or zero args; treat as dict literal
                // variant since we don't flag keyword-only syntax here.
                has_positional_dict = false;
                (TypedDictSecondArgKind::DictLiteral, false)
            };
        let keyword_names: Vec<String> = call
            .arguments
            .keywords
            .iter()
            .filter_map(|kw| kw.arg.as_ref().map(std::string::ToString::to_string))
            .collect();
        out.push(TypedDictCallInfo {
            lhs_name,
            declared_name,
            second_arg_kind,
            has_non_string_key,
            has_positional_dict,
            keyword_names,
            span: text_range_to_span(call.range()),
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

/// Names of well-known typing forms that are NOT parameterized by TypeVars even
/// when subscripted.  `Literal["x"]`, `Optional[int]`, etc. are valid TypeVar
/// bounds and constraints, so we must not flag them as "parameterized by TypeVar".
const TYPING_FORMS: &[&str] = &[
    "Literal", "Optional", "Union", "Final", "ClassVar", "Annotated",
    "Required", "NotRequired", "ReadOnly", "TypeAlias",
];

/// Returns `true` when an expression is a subscript parameterized by a potential
/// TypeVar — i.e. it is `list[T]` or similar, NOT a typing form like `Literal[...]`.
///
/// Used to detect cases like `TypeVar("T", bound=list[T])` where the bound is
/// parameterized by a free TypeVar rather than being a valid concrete generic.
fn expr_is_parameterized(expr: &Expr) -> bool {
    match expr {
        Expr::Subscript(sub) => {
            // Skip well-known typing forms: Literal["x"], Optional[T], etc.
            let base_name = expr_simple_name(&sub.value);
            if base_name.as_deref().is_some_and(|n| TYPING_FORMS.contains(&n)) {
                return false;
            }
            true
        }
        Expr::BinOp(bin) => {
            expr_is_parameterized(&bin.left) || expr_is_parameterized(&bin.right)
        }
        Expr::Tuple(tup) => tup.elts.iter().any(expr_is_parameterized),
        _ => false,
    }
}

/// Extracts the name from a PEP 695 `TypeParam` (`TypeVar`, `TypeVarTuple`, or `ParamSpec`).
fn type_param_name(tp: &TypeParam) -> String {
    match tp {
        TypeParam::TypeVar(tv) => tv.name.to_string(),
        TypeParam::TypeVarTuple(tvt) => tvt.name.to_string(),
        TypeParam::ParamSpec(ps) => ps.name.to_string(),
    }
}

fn expr_simple_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        _ => None,
    }
}

/// Recursively collect all `Name` references from an expression tree.
///
/// Used to find all identifier names referenced within base class expressions,
/// including those nested inside subscripts, tuples, and other compound forms.
fn collect_name_refs_from_expr(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Name(name) => out.push(name.id.to_string()),
        Expr::Subscript(sub) => {
            collect_name_refs_from_expr(&sub.value, out);
            collect_name_refs_from_expr(&sub.slice, out);
        }
        Expr::Attribute(attr) => collect_name_refs_from_expr(&attr.value, out),
        Expr::Tuple(tup) => {
            for elt in &tup.elts {
                collect_name_refs_from_expr(elt, out);
            }
        }
        Expr::BinOp(bin) => {
            collect_name_refs_from_expr(&bin.left, out);
            collect_name_refs_from_expr(&bin.right, out);
        }
        Expr::Call(call) => {
            collect_name_refs_from_expr(&call.func, out);
            for arg in &call.arguments.args {
                collect_name_refs_from_expr(arg, out);
            }
        }
        _ => {}
    }
}

fn text_range_to_span(range: TextRange) -> Span {
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}

