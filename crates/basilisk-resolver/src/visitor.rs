//! AST visitor that collects function definitions and module-level information.

const ENUM_BASES: &[&str] = &["Enum", "IntEnum", "StrEnum", "Flag", "IntFlag", "ReprEnum"];

use ruff_python_ast::{
    Alias, Decorator, ElifElseClause, ExceptHandler, Expr, MatchCase, Parameter,
    ParameterWithDefault, Pattern, Stmt, StmtAnnAssign, StmtAssign, StmtClassDef, StmtFunctionDef,
    StmtImport, StmtImportFrom, StmtMatch, StmtReturn, TypeParam,
};
use ruff_text_size::{Ranged, TextRange};

use basilisk_parser::ParsedModule;

use crate::scope::{
    AssertTypeCallInfo, AttributeInfo, CallSite, ClassInfo, FloatParamIntAttrAccess, FunctionInfo,
    GenericParamInfo, ImportInfo, ImportKind, LiteralStringEnumMismatch, MatchStmtInfo,
    NewTypeCallInfo, ParameterInfo, ReadOnlyViolationInfo, ReadOnlyViolationKind, ResolvedModule,
    ReturnAnnotationKind, ReturnStmtInfo, RevealTypeCallInfo, RhsKind, Span, TypeVarCallInfo,
    TypedDictCallInfo, TypedDictSecondArgKind, UnhashableKeyRef, VariableInfo,
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
    let assert_type_calls =
        collect_assert_type_calls_from_stmts(&module.ast.body, &[], &module.source);
    let typeddict_calls = collect_typeddict_calls(&module.ast.body);
    let newtype_calls = collect_newtype_calls(&module.ast.body);
    let multiple_unbounded_tuple_spans = collect_multiple_unbounded_tuple_spans(&module.ast.body);

    let module_bare_assignments = collect_module_bare_assignments(&module.ast.body);
    let module_attr_assignments = collect_module_attr_assignments(&module.ast.body);
    let final_violations = collect_final_violations(&module.ast.body, &classes, &module.source);
    let float_param_int_attr_accesses =
        collect_float_param_int_attr_accesses(&module.ast.body, &module.source);
    let literal_string_enum_mismatches =
        collect_literal_string_enum_mismatches(&module.ast.body, &module.source);
    let readonly_violations = collect_readonly_violations(&module.ast.body, &classes);
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
        newtype_calls,
        multiple_unbounded_tuple_spans,
        final_violations,
        module_bare_assignments,
        module_attr_assignments,
        module_attr_accesses: collect_module_attr_accesses(&module.ast.body),
        module_order_comparisons: collect_module_order_comparisons(&module.ast.body),
        readonly_violations,
        annotated_direct_call_spans: collect_annotated_direct_calls(&module.ast.body),
        imported_final_names: collect_imported_final_names(&module.ast.body, &module.path),
        type_alias_type_calls: Vec::new(),
        type_statements: Vec::new(),
        annotated_too_few_args: Vec::new(),
        namedtuple_defs: Vec::new(),
        float_param_int_attr_accesses,
        literal_string_enum_mismatches,
        enum_value_type_violations: collect_enum_value_type_violations(&module.ast.body, &module.source),
        local_classvar_violations: Vec::new(),
        pep695_bound_violations: Vec::new(),
        historical_positional_violations: Vec::new(),
        invalid_string_annotations: Vec::new(),
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
    class_kw_only: bool,
) -> (Vec<AttributeInfo>, Vec<String>, Vec<(String, Vec<String>)>) {
    let mut attributes = Vec::new();
    let mut method_names = Vec::new();
    let mut method_decorators: Vec<(String, Vec<String>)> = Vec::new();
    // Track whether we have passed the `_: KW_ONLY` sentinel.
    let mut after_kw_only_sentinel = false;

    for stmt in &class.body {
        match stmt {
            Stmt::AnnAssign(ann) => {
                if let Some(name) = expr_simple_name(&ann.target) {
                    // Detect `_: KW_ONLY` sentinel — skip it as a real attribute.
                    if name == "_" && annotation_is_kw_only(&ann.annotation) {
                        after_kw_only_sentinel = true;
                        continue;
                    }
                    let is_readonly = annotation_contains_readonly_expr(&ann.annotation);
                    let field_kw_only = ann.value.as_deref().and_then(field_kw_only_override);
                    // Determine kw_only: explicit field() override wins; then sentinel; then class default.
                    let is_kw_only =
                        field_kw_only.unwrap_or(after_kw_only_sentinel || class_kw_only);
                    attributes.push(AttributeInfo {
                        name,
                        name_span: text_range_to_span(ann.target.range()),
                        has_annotation: true,
                        annotation_span: Some(text_range_to_span(ann.annotation.range())),
                        has_value: ann.value.is_some(),
                        rhs_kind: RhsKind::Other,
                        rhs_span: ann.value.as_ref().map(|v| text_range_to_span(v.range())),
                        rhs_is_nonmember_call: false,
                        rhs_is_lambda: false,
                        rhs_is_descriptor_call: false,
                        is_readonly,
                        is_kw_only,
                    });
                }
            }
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    if let Some(name) = expr_simple_name(target) {
                        let rhs_is_nonmember_call = matches!(
                            &*assign.value,
                            Expr::Call(c) if matches!(c.func.as_ref(), Expr::Name(n) if n.id == "nonmember")
                        );
                        let rhs_is_lambda = matches!(&*assign.value, Expr::Lambda(_));
                        let rhs_is_descriptor_call = matches!(
                            &*assign.value,
                            Expr::Call(c) if matches!(
                                c.func.as_ref(),
                                Expr::Name(n) if n.id == "staticmethod" || n.id == "classmethod"
                            )
                        );
                        attributes.push(AttributeInfo {
                            name,
                            name_span: text_range_to_span(target.range()),
                            has_annotation: false,
                            annotation_span: None,
                            has_value: true,
                            rhs_kind: RhsKind::Other,
                            rhs_span: Some(text_range_to_span(assign.value.range())),
                            rhs_is_nonmember_call,
                            rhs_is_lambda,
                            rhs_is_descriptor_call,
                            is_readonly: false,
                            is_kw_only: false,
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

    // Pre-compute is_dataclass and is_dataclass_kw_only so we can pass kw_only
    // into collect_class_body for per-attribute kw_only resolution.
    let pre_is_dataclass = class.decorator_list.iter().any(
        |d| matches!(decorator_name(d), Some(n) if n == "dataclass" || n.ends_with(".dataclass")),
    );
    let pre_is_dataclass_kw_only = pre_is_dataclass && dataclass_flag(class, "kw_only");

    let (attributes, method_names, method_decorators) =
        collect_class_body(class, functions, match_stmts, pre_is_dataclass_kw_only);

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

    let is_dataclass_frozen = is_dataclass && dataclass_flag(class, "frozen");
    let is_dataclass_kw_only = pre_is_dataclass_kw_only;
    let is_dataclass_match_args_false =
        is_dataclass && dataclass_bool_flag_is_false(class, "match_args");
    let is_dataclass_order = is_dataclass && dataclass_flag(class, "order");
    let is_dataclass_unsafe_hash = is_dataclass && dataclass_flag(class, "unsafe_hash");
    let is_dataclass_eq_false = is_dataclass && dataclass_bool_flag_is_false(class, "eq");

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
        is_dataclass_frozen,
        is_dataclass_kw_only,
        is_dataclass_match_args_false,
        is_dataclass_order,
        is_dataclass_unsafe_hash,
        is_dataclass_eq_false,
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
        body_last_stmt_terminates: func.body.last().is_some_and(|s| match s {
            Stmt::Raise(_) => true,
            Stmt::Expr(e) => matches!(e.value.as_ref(), Expr::Call(_)),
            _ => false,
        }),
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
        .skip_while(
            |s| matches!(s, Stmt::Expr(e) if matches!(e.value.as_ref(), Expr::StringLiteral(_))),
        )
        .collect();

    non_docstring
        .iter()
        .all(|s| matches!(s, Stmt::Expr(e) if matches!(e.value.as_ref(), Expr::EllipsisLiteral(_))))
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
    (expr_simple_name(&call.func).as_deref() == Some("TypeVar"))
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
    let is_covariant = call.arguments.keywords.iter().any(|kw| {
        kw.arg.as_ref().is_some_and(|a| a.as_str() == "covariant")
            && matches!(&kw.value, Expr::BooleanLiteral(b) if b.value)
    });
    let is_contravariant = call.arguments.keywords.iter().any(|kw| {
        kw.arg
            .as_ref()
            .is_some_and(|a| a.as_str() == "contravariant")
            && matches!(&kw.value, Expr::BooleanLiteral(b) if b.value)
    });
    let has_infer_variance = call.arguments.keywords.iter().any(|kw| {
        kw.arg
            .as_ref()
            .is_some_and(|a| a.as_str() == "infer_variance")
            && matches!(&kw.value, Expr::BooleanLiteral(b) if b.value)
    });
    TypeVarCallInfo {
        name,
        constraint_count,
        has_default,
        has_bound,
        has_parameterized_bound,
        has_parameterized_constraint,
        is_covariant,
        is_contravariant,
        has_infer_variance,
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
                let is_reveal_type =
                    expr_simple_name(&call.func).is_some_and(|n| n == "reveal_type");
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
        let is_generic_or_protocol = matches!(sub.value.as_ref(), Expr::Name(n) if n.id.as_str() == "Generic" || n.id.as_str() == "Protocol")
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
    let keywords: Vec<(String, RhsKind)> = call
        .arguments
        .keywords
        .iter()
        .filter_map(|kw| {
            kw.arg
                .as_ref()
                .map(|name| (name.to_string(), classify_rhs(&kw.value)))
        })
        .collect();
    Some(CallSite {
        callee,
        args,
        keywords,
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

/// Extract the `frozen=True/False` flag from `@dataclass(frozen=...)`.
/// Returns `false` if no explicit `frozen=` is present (default is `False`).
fn dataclass_flag(class: &StmtClassDef, key: &str) -> bool {
    for dec in &class.decorator_list {
        let Expr::Call(call) = &dec.expression else {
            continue;
        };
        let is_dc = match call.func.as_ref() {
            Expr::Name(n) => n.id.as_str() == "dataclass",
            Expr::Attribute(a) => a.attr.as_str() == "dataclass",
            _ => false,
        };
        if !is_dc {
            continue;
        }
        for kw in &call.arguments.keywords {
            if kw.arg.as_ref().map(ruff_python_ast::Identifier::as_str) == Some(key) {
                return matches!(&kw.value, Expr::BooleanLiteral(b) if b.value);
            }
        }
    }
    false
}

/// Returns `true` when the annotation expression is `KW_ONLY`
/// (the sentinel that makes all following fields keyword-only).
fn annotation_is_kw_only(ann: &Expr) -> bool {
    matches!(ann, Expr::Name(n) if n.id.as_str() == "KW_ONLY")
}

/// For a field value expression, returns `Some(true)` when it is `field(kw_only=True, ...)`,
/// `Some(false)` when it is `field(kw_only=False, ...)`, and `None` otherwise.
fn field_kw_only_override(value: &Expr) -> Option<bool> {
    let Expr::Call(call) = value else { return None };
    let is_field_call = match call.func.as_ref() {
        Expr::Name(n) => n.id.as_str() == "field",
        Expr::Attribute(a) => a.attr.as_str() == "field",
        _ => false,
    };
    if !is_field_call {
        return None;
    }
    for kw in &call.arguments.keywords {
        if kw.arg.as_ref().map(ruff_python_ast::Identifier::as_str) == Some("kw_only") {
            return Some(matches!(&kw.value, Expr::BooleanLiteral(b) if b.value));
        }
    }
    None
}

fn dataclass_bool_flag_is_false(class: &StmtClassDef, key: &str) -> bool {
    for dec in &class.decorator_list {
        let Expr::Call(call) = &dec.expression else {
            continue;
        };
        let is_dc = match call.func.as_ref() {
            Expr::Name(n) => n.id.as_str() == "dataclass",
            Expr::Attribute(a) => a.attr.as_str() == "dataclass",
            _ => false,
        };
        if !is_dc {
            continue;
        }
        for kw in &call.arguments.keywords {
            if kw.arg.as_ref().map(ruff_python_ast::Identifier::as_str) == Some(key) {
                return matches!(&kw.value, Expr::BooleanLiteral(b) if !b.value);
            }
        }
    }
    false
}

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
        let Expr::Call(call) = node.value.as_ref() else {
            continue;
        };
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
                        item.key
                            .as_ref()
                            .is_some_and(|k| !matches!(k, Expr::StringLiteral(_)))
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

/// Collect module-level `NewType(...)` call sites.
///
/// Matches assignments of the form `Name = NewType("Name", BaseType)`.
fn collect_newtype_calls(stmts: &[Stmt]) -> Vec<NewTypeCallInfo> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::Assign(node) = stmt else { continue };
        let Expr::Call(call) = node.value.as_ref() else {
            continue;
        };
        let is_newtype = expr_simple_name(&call.func).as_deref() == Some("NewType")
            || matches!(call.func.as_ref(), Expr::Attribute(a) if a.attr.as_str() == "NewType");
        if !is_newtype {
            continue;
        }
        let Some(lhs_name) = node.targets.first().and_then(expr_simple_name) else {
            continue;
        };
        let declared_name = call.arguments.args.first().and_then(|arg| {
            if let Expr::StringLiteral(s) = arg {
                Some(s.value.to_str().to_owned())
            } else {
                None
            }
        });
        let base_type_span = call
            .arguments
            .args
            .get(1)
            .map(|a| text_range_to_span(ruff_text_size::Ranged::range(a)));
        out.push(NewTypeCallInfo {
            lhs_name,
            declared_name,
            positional_arg_count: call.arguments.args.len(),
            base_type_span,
            span: text_range_to_span(ruff_text_size::Ranged::range(call)),
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

/// Names of well-known typing forms that are NOT parameterized by `TypeVar`s even
/// when subscripted.  `Literal["x"]`, `Optional[int]`, etc. are valid `TypeVar`
/// bounds and constraints, so we must not flag them as "parameterized by `TypeVar`".
const TYPING_FORMS: &[&str] = &[
    "Literal",
    "Optional",
    "Union",
    "Final",
    "ClassVar",
    "Annotated",
    "Required",
    "NotRequired",
    "ReadOnly",
    "TypeAlias",
];

/// Returns `true` when an expression is a subscript parameterized by a potential
/// `TypeVar` — i.e. it is `list[T]` or similar, NOT a typing form like `Literal[...]`.
///
/// Used to detect cases like `TypeVar("T", bound=list[T])` where the bound is
/// parameterized by a free `TypeVar` rather than being a valid concrete generic.
fn expr_is_parameterized(expr: &Expr) -> bool {
    match expr {
        Expr::Subscript(sub) => {
            // Skip well-known typing forms: Literal["x"], Optional[T], etc.
            let base_name = expr_simple_name(&sub.value);
            if base_name
                .as_deref()
                .is_some_and(|n| TYPING_FORMS.contains(&n))
            {
                return false;
            }
            true
        }
        Expr::BinOp(bin) => expr_is_parameterized(&bin.left) || expr_is_parameterized(&bin.right),
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

// ---------------------------------------------------------------------------
// Multiple unbounded tuple detection (for BSK-E0047)
// ---------------------------------------------------------------------------

/// Counts the number of "unbounded" components in a `tuple[...]` slice expression.
///
/// An unbounded component is one of:
/// - `*tuple[T, ...]` — a starred subscript where the inner tuple ends with `...`
/// - `*Ts` / `*<Name>` — a starred name (`TypeVarTuple` unpack)
/// - `Unpack[tuple[T, ...]]` — legacy Unpack form
///
/// Returns the count of unbounded components found.
fn count_unbounded_in_tuple_slice(slice: &Expr) -> usize {
    let elements: &[Expr] = match slice {
        Expr::Tuple(t) => &t.elts,
        // Single-element tuple slice — check just this element
        other => return usize::from(is_unbounded_component(other)),
    };
    elements
        .iter()
        .filter(|e| is_unbounded_component(e))
        .count()
}

/// Returns `true` if this expression is an unbounded tuple component:
/// - `*tuple[T, ...]` — starred subscript with an ellipsis last element
/// - `*Name` — starred name (`TypeVarTuple` unpack)
/// - `Unpack[tuple[T, ...]]` — legacy form
fn is_unbounded_component(expr: &Expr) -> bool {
    match expr {
        Expr::Starred(starred) => match starred.value.as_ref() {
            // `*tuple[T, ...]` or `*tuple[str, *tuple[str, ...]]`
            Expr::Subscript(sub) => {
                if expr_simple_name(&sub.value).as_deref() != Some("tuple") {
                    return false;
                }
                inner_tuple_is_unbounded(&sub.slice)
            }
            // `*Ts` — TypeVarTuple unpack
            Expr::Name(_) => true,
            _ => false,
        },
        // `Unpack[tuple[T, ...]]` — legacy unpack form
        Expr::Subscript(sub) if expr_simple_name(&sub.value).as_deref() == Some("Unpack") => {
            match sub.slice.as_ref() {
                Expr::Subscript(inner_sub)
                    if expr_simple_name(&inner_sub.value).as_deref() == Some("tuple") =>
                {
                    inner_tuple_is_unbounded(&inner_sub.slice)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// Returns `true` when the slice of a `tuple[...]` represents an unbounded tuple
/// (i.e. the tuple contains an ellipsis: `tuple[T, ...]`).
fn inner_tuple_is_unbounded(slice: &Expr) -> bool {
    match slice {
        Expr::Tuple(t) => t.elts.last().is_some_and(|e| {
            matches!(e, Expr::EllipsisLiteral(_)) || is_unbounded_component(e) // nested unbounded: `*tuple[str, ...]`
        }),
        Expr::EllipsisLiteral(_) => true,
        // Single element that is itself an unbounded starred expr
        other => is_unbounded_component(other),
    }
}

/// Returns `true` if the annotation expression is a `tuple[...]` with multiple
/// unbounded components (more than one `*tuple[T, ...]` or `*Ts`).
fn annotation_has_multiple_unbounded(expr: &Expr) -> bool {
    match expr {
        Expr::Subscript(sub) if expr_simple_name(&sub.value).as_deref() == Some("tuple") => {
            count_unbounded_in_tuple_slice(&sub.slice) >= 2
        }
        _ => false,
    }
}

/// Collect all annotation spans that contain invalid multiple-unbounded-tuple patterns.
fn collect_multiple_unbounded_tuple_spans(stmts: &[Stmt]) -> Vec<Span> {
    let mut out = Vec::new();
    collect_multi_unbounded_from_stmts(stmts, &mut out);
    out
}

fn collect_multi_unbounded_from_stmts(stmts: &[Stmt], out: &mut Vec<Span>) {
    for stmt in stmts {
        collect_multi_unbounded_from_stmt(stmt, out);
    }
}

fn collect_multi_unbounded_from_stmt(stmt: &Stmt, out: &mut Vec<Span>) {
    match stmt {
        Stmt::AnnAssign(ann) => {
            if annotation_has_multiple_unbounded(&ann.annotation) {
                out.push(text_range_to_span(ann.annotation.range()));
            }
        }
        Stmt::FunctionDef(func) => {
            // Check parameter annotations
            let all_params = func
                .parameters
                .posonlyargs
                .iter()
                .chain(func.parameters.args.iter())
                .chain(func.parameters.kwonlyargs.iter());
            for param in all_params {
                if let Some(ann) = param.parameter.annotation.as_ref() {
                    if annotation_has_multiple_unbounded(ann) {
                        out.push(text_range_to_span(ann.range()));
                    }
                }
            }
            if let Some(vararg) = &func.parameters.vararg {
                if let Some(ann) = vararg.annotation.as_ref() {
                    if annotation_has_multiple_unbounded(ann) {
                        out.push(text_range_to_span(ann.range()));
                    }
                }
            }
            if let Some(kwarg) = &func.parameters.kwarg {
                if let Some(ann) = kwarg.annotation.as_ref() {
                    if annotation_has_multiple_unbounded(ann) {
                        out.push(text_range_to_span(ann.range()));
                    }
                }
            }
            // Check return annotation
            if let Some(ret) = func.returns.as_ref() {
                if annotation_has_multiple_unbounded(ret) {
                    out.push(text_range_to_span(ret.range()));
                }
            }
            // Recurse into function body
            collect_multi_unbounded_from_stmts(&func.body, out);
        }
        Stmt::ClassDef(cls) => {
            collect_multi_unbounded_from_stmts(&cls.body, out);
        }
        Stmt::If(if_stmt) => {
            collect_multi_unbounded_from_stmts(&if_stmt.body, out);
            collect_multi_unbounded_from_stmts(
                &if_stmt
                    .elif_else_clauses
                    .iter()
                    .flat_map(|c| c.body.iter())
                    .cloned()
                    .collect::<Vec<_>>(),
                out,
            );
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// assert_type type-aware collection
// ---------------------------------------------------------------------------

/// Collect all `assert_type(value, ExpectedType)` calls from the given statements,
#[allow(dead_code)]
/// using `params` as the in-scope parameter map (`name → annotation text`).
///
/// Recursively descends into function bodies (updating the param scope) and all
/// other control-flow constructs (preserving the current scope).
pub(crate) fn collect_assert_type_calls_from_stmts(
    stmts: &[Stmt],
    params: &[(&str, &str)],
    source: &str,
) -> Vec<AssertTypeCallInfo> {
    let mut out = Vec::new();
    for stmt in stmts {
        collect_assert_type_calls_from_stmt(stmt, params, source, &mut out);
    }
    out
}

fn collect_assert_type_calls_from_stmt(
    stmt: &Stmt,
    params: &[(&str, &str)],
    source: &str,
    out: &mut Vec<AssertTypeCallInfo>,
) {
    match stmt {
        Stmt::Expr(node) => {
            if let Expr::Call(call) = node.value.as_ref() {
                let is_assert_type =
                    expr_simple_name(&call.func).is_some_and(|n| n == "assert_type");
                if is_assert_type {
                    out.push(build_assert_type_call_info(call, params, source));
                }
            }
        }
        Stmt::FunctionDef(func) => {
            // Build new param scope for the function body.
            let new_params: Vec<(String, String)> =
                build_param_scope_owned(&func.parameters, source);
            let borrowed: Vec<(&str, &str)> = new_params
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            out.extend(collect_assert_type_calls_from_stmts(
                &func.body, &borrowed, source,
            ));
        }
        Stmt::ClassDef(cls) => {
            // Class bodies may contain methods; pass empty params at class level.
            out.extend(collect_assert_type_calls_from_stmts(&cls.body, &[], source));
        }
        Stmt::If(node) => {
            out.extend(collect_assert_type_calls_from_stmts(
                &node.body, params, source,
            ));
            for elif_else in &node.elif_else_clauses {
                out.extend(collect_assert_type_calls_from_stmts(
                    &elif_else.body,
                    params,
                    source,
                ));
            }
        }
        Stmt::For(node) => {
            out.extend(collect_assert_type_calls_from_stmts(
                &node.body, params, source,
            ));
            out.extend(collect_assert_type_calls_from_stmts(
                &node.orelse,
                params,
                source,
            ));
        }
        Stmt::While(node) => {
            out.extend(collect_assert_type_calls_from_stmts(
                &node.body, params, source,
            ));
            out.extend(collect_assert_type_calls_from_stmts(
                &node.orelse,
                params,
                source,
            ));
        }
        Stmt::With(node) => {
            out.extend(collect_assert_type_calls_from_stmts(
                &node.body, params, source,
            ));
        }
        Stmt::Try(node) => {
            out.extend(collect_assert_type_calls_from_stmts(
                &node.body, params, source,
            ));
            for handler in &node.handlers {
                let ExceptHandler::ExceptHandler(h) = handler;
                out.extend(collect_assert_type_calls_from_stmts(
                    &h.body, params, source,
                ));
            }
            out.extend(collect_assert_type_calls_from_stmts(
                &node.orelse,
                params,
                source,
            ));
            out.extend(collect_assert_type_calls_from_stmts(
                &node.finalbody,
                params,
                source,
            ));
        }
        Stmt::Match(node) => {
            for case in &node.cases {
                out.extend(collect_assert_type_calls_from_stmts(
                    &case.body, params, source,
                ));
            }
        }
        _ => {}
    }
}

/// Build the parameter scope for a function: a list of `(param_name, annotation_text)` pairs.
///
/// Parameters without annotations are excluded (no annotation text to compare against).
fn build_param_scope_owned(
    parameters: &ruff_python_ast::Parameters,
    source: &str,
) -> Vec<(String, String)> {
    parameters
        .posonlyargs
        .iter()
        .chain(parameters.args.iter())
        .chain(parameters.kwonlyargs.iter())
        .filter_map(|p| {
            let name = p.parameter.name.to_string();
            let ann = p.parameter.annotation.as_deref()?;
            let range = ann.range();
            let ann_text =
                source.get(range.start().to_u32() as usize..range.end().to_u32() as usize)?;
            Some((name, ann_text.to_owned()))
        })
        .collect()
}

/// Build an `AssertTypeCallInfo` for a single `assert_type(...)` call expression.
fn build_assert_type_call_info(
    call: &ruff_python_ast::ExprCall,
    params: &[(&str, &str)],
    source: &str,
) -> AssertTypeCallInfo {
    let arg_count = call.arguments.args.len();
    let span = text_range_to_span(call.range());

    if arg_count != 2 {
        // Arity error — type mismatch checking is not applicable.
        return AssertTypeCallInfo {
            arg_count,
            span,
            actual_type: None,
            expected_type: None,
            type_mismatch: false,
        };
    }

    let first_arg = &call.arguments.args[0];
    let second_arg = &call.arguments.args[1];

    // Determine the actual type of the first argument.
    let actual_type = resolve_actual_type(first_arg, params, source);

    // Extract the expected type text from the second argument.
    let expected_type = extract_type_text(second_arg, source);

    // Compare normalized forms.
    let type_mismatch = match (&actual_type, &expected_type) {
        (Some(actual), Some(expected)) => !types_match(actual, expected),
        _ => false,
    };

    AssertTypeCallInfo {
        arg_count,
        span,
        actual_type,
        expected_type,
        type_mismatch,
    }
}

/// Resolve the static type of `assert_type`'s first argument.
///
/// - If it is a name reference to a known parameter, returns its annotation text (normalized).
/// - If it is a literal, returns the corresponding primitive type name.
/// - Otherwise returns `None`.
fn resolve_actual_type(expr: &Expr, params: &[(&str, &str)], source: &str) -> Option<String> {
    match expr {
        Expr::Name(name) => {
            let param_name = name.id.as_str();
            params
                .iter()
                .find(|(n, _)| *n == param_name)
                .map(|(_, ann)| normalize_type_str(ann))
        }
        Expr::StringLiteral(_) => Some("str".to_owned()),
        Expr::NumberLiteral(n) => {
            if matches!(n.value, ruff_python_ast::Number::Float(_)) {
                Some("float".to_owned())
            } else {
                Some("int".to_owned())
            }
        }
        Expr::BooleanLiteral(_) => Some("bool".to_owned()),
        Expr::BytesLiteral(_) => Some("bytes".to_owned()),
        Expr::NoneLiteral(_) => Some("None".to_owned()),
        // Attribute accesses (`X.y`, `obj.attr`) cannot be typed without inference.
        Expr::Attribute(_) | Expr::Subscript(_) | Expr::Call(_) => None,
        _ => {
            // For other expressions, try to get the source text.
            let range = expr.range();
            source
                .get(range.start().to_u32() as usize..range.end().to_u32() as usize)
                .map(normalize_type_str)
        }
    }
}

/// Extract the text of a type expression (the second argument to `assert_type`).
fn extract_type_text(expr: &Expr, source: &str) -> Option<String> {
    let range = expr.range();
    source
        .get(range.start().to_u32() as usize..range.end().to_u32() as usize)
        .map(normalize_type_str)
}

/// Normalize a type annotation string for comparison.
///
/// Strips outer `Annotated[T, ...]` wrappers, trims whitespace, and collapses
/// internal spacing around `|` union operators.
fn normalize_type_str(ann: &str) -> String {
    let trimmed = ann.trim();
    // Strip Annotated[T, ...] → take first argument only.
    if let Some(inner) = strip_annotated_wrapper(trimmed) {
        return normalize_type_str(inner);
    }
    trimmed.to_owned()
}

/// If `ann` starts with `Annotated[`, return the first type argument (the actual type).
fn strip_annotated_wrapper(ann: &str) -> Option<&str> {
    let ann = ann.trim();
    if !ann.starts_with("Annotated[") {
        return None;
    }
    let inner_start = "Annotated[".len();
    let inner = &ann[inner_start..];
    // Find the end of the first argument (handle nested brackets).
    let mut depth = 0i32;
    let mut end = inner.len();
    for (i, ch) in inner.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => {
                if depth == 0 {
                    // Hit closing ] of Annotated without a comma — whole inner is one arg.
                    end = i;
                    break;
                }
                depth -= 1;
            }
            ',' if depth == 0 => {
                end = i;
                break;
            }
            _ => {}
        }
    }
    Some(inner[..end].trim())
}

/// Returns `true` when `actual` and `expected` are equivalent types (textual comparison).
fn types_match(actual: &str, expected: &str) -> bool {
    actual == expected
}

/// Recursively check if an expression contains `ReadOnly`.
fn annotation_contains_readonly_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Name(name) => name.id.as_str() == "ReadOnly",
        Expr::Attribute(attr) => attr.attr.as_str() == "ReadOnly",
        Expr::Subscript(sub) => {
            annotation_contains_readonly_expr(&sub.value)
                || annotation_contains_readonly_expr(&sub.slice)
        }
        Expr::BinOp(bin) => {
            annotation_contains_readonly_expr(&bin.left)
                || annotation_contains_readonly_expr(&bin.right)
        }
        Expr::Tuple(tuple) => tuple.elts.iter().any(annotation_contains_readonly_expr),
        _ => false,
    }
}

/// Extract `ReadOnly` field names from a functional `TypedDict(...)` call's dict literal.
///
/// Returns a set of field names that have `ReadOnly[...]` annotations.
fn functional_typeddict_readonly_fields(dict_expr: &Expr) -> std::collections::HashSet<String> {
    let Expr::Dict(dict) = dict_expr else {
        return std::collections::HashSet::new();
    };
    dict.items
        .iter()
        .filter_map(|item| {
            let key_expr = item.key.as_ref()?;
            let Expr::StringLiteral(key) = key_expr else {
                return None;
            };
            if annotation_contains_readonly_expr(&item.value) {
                Some(key.value.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Scan function body for `kwargs["key"] = val` where key is a `ReadOnly` field.
fn check_kwargs_readonly_violations(
    func: &StmtFunctionDef,
    td_readonly_fields: &std::collections::HashMap<String, std::collections::HashSet<String>>,
    out: &mut Vec<ReadOnlyViolationInfo>,
) {
    let Some(kwarg) = &func.parameters.kwarg else {
        return;
    };
    let Some(ann_expr) = kwarg.annotation.as_deref() else {
        return;
    };
    // Match Unpack[TypedDictName]
    let Expr::Subscript(sub) = ann_expr else {
        return;
    };
    if !matches!(sub.value.as_ref(), Expr::Name(n) if n.id == "Unpack") {
        return;
    }
    let Some(td_name) = expr_simple_name(&sub.slice) else {
        return;
    };
    let Some(readonly_fields) = td_readonly_fields.get(&td_name) else {
        return;
    };
    let kwarg_name = kwarg.name.to_string();
    for stmt in &func.body {
        let Stmt::Assign(assign) = stmt else {
            continue;
        };
        for target in &assign.targets {
            let Expr::Subscript(tsub) = target else {
                continue;
            };
            let Some(var_name) = expr_simple_name(&tsub.value) else {
                continue;
            };
            if var_name != kwarg_name {
                continue;
            }
            let Expr::StringLiteral(key_str) = tsub.slice.as_ref() else {
                continue;
            };
            let key = key_str.value.to_string();
            if readonly_fields.contains(&key) {
                out.push(ReadOnlyViolationInfo {
                    var_name,
                    field_name: Some(key),
                    kind: ReadOnlyViolationKind::SubscriptAssign,
                    span: text_range_to_span(assign.range()),
                });
            }
        }
    }
}

/// Build a map from `TypedDict` class name to its `ReadOnly` field names.
fn build_typeddict_readonly_map(
    stmts: &[Stmt],
    classes: &[ClassInfo],
) -> std::collections::HashMap<String, std::collections::HashSet<String>> {
    use std::collections::{HashMap, HashSet};
    let mut map: HashMap<String, HashSet<String>> = classes
        .iter()
        .filter(|cls| cls.is_typed_dict)
        .filter_map(|cls| {
            let fields: HashSet<String> = cls
                .attributes
                .iter()
                .filter(|a| a.is_readonly)
                .map(|a| a.name.clone())
                .collect();
            if fields.is_empty() {
                None
            } else {
                Some((cls.name.clone(), fields))
            }
        })
        .collect();
    // Functional form: `Name = TypedDict("Name", {"field": ReadOnly[...]})`
    for stmt in stmts {
        let Stmt::Assign(assign) = stmt else { continue };
        let Some(lhs_name) = assign.targets.first().and_then(expr_simple_name) else {
            continue;
        };
        let Expr::Call(call) = assign.value.as_ref() else {
            continue;
        };
        if !matches!(call.func.as_ref(), Expr::Name(n) if n.id == "TypedDict") {
            continue;
        }
        if let Some(second_arg) = call.arguments.args.get(1) {
            let fields = functional_typeddict_readonly_fields(second_arg);
            if !fields.is_empty() {
                map.insert(lhs_name, fields);
            }
        }
    }
    map
}

/// Build a map from variable name to its declared `TypedDict` type name.
fn build_var_type_map<'a>(
    stmts: &[Stmt],
    td_readonly_fields: &'a std::collections::HashMap<String, std::collections::HashSet<String>>,
) -> std::collections::HashMap<String, &'a str> {
    let mut map = std::collections::HashMap::new();
    for stmt in stmts {
        let Stmt::AnnAssign(ann) = stmt else { continue };
        let Some(var_name) = expr_simple_name(&ann.target) else {
            continue;
        };
        let Expr::Name(type_name) = ann.annotation.as_ref() else {
            continue;
        };
        if let Some((key, _)) = td_readonly_fields.get_key_value(type_name.id.as_str()) {
            map.insert(var_name, key.as_str());
        }
    }
    map
}

/// Collect `ReadOnly` violations from module-level statements and function bodies.
fn collect_readonly_violations(
    stmts: &[Stmt],
    classes: &[ClassInfo],
) -> Vec<ReadOnlyViolationInfo> {
    let td_readonly_fields = build_typeddict_readonly_map(stmts, classes);
    if td_readonly_fields.is_empty() {
        return Vec::new();
    }
    let var_type = build_var_type_map(stmts, &td_readonly_fields);
    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    let Expr::Subscript(sub) = target else {
                        continue;
                    };
                    let Some(var_name) = expr_simple_name(&sub.value) else {
                        continue;
                    };
                    let Some(&class_name) = var_type.get(&var_name) else {
                        continue;
                    };
                    let Some(fields) = td_readonly_fields.get(class_name) else {
                        continue;
                    };
                    let Expr::StringLiteral(key_str) = sub.slice.as_ref() else {
                        continue;
                    };
                    let key = key_str.value.to_string();
                    if fields.contains(&key) {
                        out.push(ReadOnlyViolationInfo {
                            var_name,
                            field_name: Some(key),
                            kind: ReadOnlyViolationKind::SubscriptAssign,
                            span: text_range_to_span(assign.range()),
                        });
                    }
                }
            }
            Stmt::Expr(expr_stmt) => {
                let Expr::Call(call) = expr_stmt.value.as_ref() else {
                    continue;
                };
                let Expr::Attribute(attr) = call.func.as_ref() else {
                    continue;
                };
                if attr.attr.as_str() != "update" {
                    continue;
                }
                let Some(var_name) = expr_simple_name(&attr.value) else {
                    continue;
                };
                if var_type.contains_key(&var_name) {
                    out.push(ReadOnlyViolationInfo {
                        var_name,
                        field_name: None,
                        kind: ReadOnlyViolationKind::UpdateCall,
                        span: text_range_to_span(expr_stmt.value.range()),
                    });
                }
            }
            Stmt::FunctionDef(func) => {
                check_kwargs_readonly_violations(func, &td_readonly_fields, &mut out);
            }
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Module-level bare and attribute assignment collection
// ---------------------------------------------------------------------------

/// Collect module-level bare assignments (`name = expr`).
///
/// Used by the checker to detect re-assignments to `Final`-annotated variables.
fn collect_module_bare_assignments(stmts: &[Stmt]) -> Vec<crate::scope::ModuleBareAssignment> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::Assign(node) = stmt else { continue };
        for target in &node.targets {
            if let Some(name) = expr_simple_name(target) {
                out.push(crate::scope::ModuleBareAssignment {
                    name,
                    name_span: text_range_to_span(target.range()),
                });
            }
        }
    }
    out
}

/// Collect module-level attribute assignments (`Class.attr = expr`).
///
/// Used by the checker to detect re-assignments to `Final` class attributes.
fn collect_module_attr_assignments(stmts: &[Stmt]) -> Vec<crate::scope::ModuleAttrAssignment> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::Assign(node) = stmt else { continue };
        for target in &node.targets {
            if let Expr::Attribute(attr) = target {
                if let Some(object_name) = expr_simple_name(&attr.value) {
                    out.push(crate::scope::ModuleAttrAssignment {
                        object_name,
                        attr_name: attr.attr.to_string(),
                        target_span: text_range_to_span(target.range()),
                    });
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Final violation collection stub
// ---------------------------------------------------------------------------

/// Returns `true` when the annotation text refers to `Final`.
fn ann_text_is_final(text: &str) -> bool {
    let t = text.trim();
    t == "Final"
        || t.starts_with("Final[")
        || t == "typing.Final"
        || t.starts_with("typing.Final[")
}

/// Collect the names of module-level `Final`-annotated variables from a statement list.
fn collect_file_final_names(
    stmts: &[Stmt],
    source: &str,
) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    for stmt in stmts {
        let Stmt::AnnAssign(ann) = stmt else { continue };
        let Expr::Name(n) = ann.target.as_ref() else { continue };
        let range = ann.annotation.range();
        let Some(ann_text) = source.get(range.start().to_u32() as usize..range.end().to_u32() as usize) else {
            continue;
        };
        if ann_text_is_final(ann_text) {
            names.insert(n.id.to_string());
        }
    }
    names
}

/// Collect the set of imported names that are declared `Final` in a sibling module.
///
/// For `from X import Y`, checks if `Y` is `Final` in `X.py`.
/// For `from X import *`, adds all `Final` names from `X.py`.
/// Only resolves simple (non-dotted) module names that map to local `.py` files.
fn collect_imported_final_names(
    stmts: &[Stmt],
    module_path: &str,
) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    let Some(module_dir) = std::path::Path::new(module_path).parent() else { return out };
    for stmt in stmts {
        let Stmt::ImportFrom(import_from) = stmt else { continue };
        let Some(module_name) = import_from.module.as_ref() else { continue };
        let module_str = module_name.to_string();
        // Only handle simple (undotted) local module names.
        if module_str.contains('.') {
            continue;
        }
        let sibling_path = module_dir.join(format!("{module_str}.py"));
        let Some(sibling_path_str) = sibling_path.to_str() else { continue };
        let Ok(sibling) = basilisk_parser::parse_file(sibling_path_str) else { continue };
        let sibling_finals = collect_file_final_names(&sibling.ast.body, &sibling.source);
        let is_star = import_from.names.iter().any(|a| a.name.as_str() == "*");
        if is_star {
            out.extend(sibling_finals);
        } else {
            for alias in &import_from.names {
                let name = alias.name.as_str();
                if sibling_finals.contains(name) {
                    out.insert(name.to_owned());
                }
            }
        }
    }
    out
}

/// Collect `Final` annotation violations from class and function bodies.
///
/// Detects:
/// - `ClassFinalWithoutInit`: class attr annotated `Final` with no value and no __init__ assignment
/// - `InstanceFinalOutsideInit`: `self.x: Final` in a non-`__init__` method
/// - `InstanceReassignAlreadyInitialized`: assigning to `self.X` in __init__ when `X: Final = value`
/// - `InstanceModifyFinal`: assigning/augmenting `self.X` in any method when `X: Final`
/// - `SubclassOverrideFinal`: child class defines attr that parent declares `Final`
/// - `FunctionLocalFinalModification`: modifying a function-local `Final` variable
/// - `GlobalFinalModification`: assigning to a global that is module-level `Final`
fn collect_final_violations(
    stmts: &[Stmt],
    classes: &[ClassInfo],
    source: &str,
) -> Vec<crate::scope::FinalViolationInfo> {
    use crate::scope::{FinalViolationInfo, FinalViolationKind};
    let mut out = Vec::new();

    // Collect module-level Final names for GlobalFinalModification.
    let module_final_names: std::collections::HashSet<&str> = stmts
        .iter()
        .filter_map(|s| {
            let Stmt::AnnAssign(ann) = s else { return None };
            let Expr::Name(n) = ann.target.as_ref() else { return None };
            let range = ann.annotation.range();
            let ann_text = source.get(range.start().to_u32() as usize..range.end().to_u32() as usize)?;
            ann_text_is_final(ann_text).then(|| n.id.as_str())
        })
        .collect();

    // Build a class-name -> Final-attr-names map for SubclassOverrideFinal.
    let class_finals: std::collections::HashMap<&str, std::collections::HashSet<&str>> = classes
        .iter()
        .map(|cls| {
            let finals: std::collections::HashSet<&str> = cls
                .attributes
                .iter()
                .filter(|a| {
                    a.has_annotation
                        && a.annotation_span
                            .and_then(|sp| {
                                source.get(sp.start as usize..sp.end as usize)
                            })
                            .is_some_and(ann_text_is_final)
                })
                .map(|a| a.name.as_str())
                .collect();
            (cls.name.as_str(), finals)
        })
        .collect();

    // Walk class definitions for per-class violations.
    for stmt in stmts {
        let Stmt::ClassDef(cls_def) = stmt else {
            // Walk function bodies for Global/Local Final violations.
            if let Stmt::FunctionDef(func) = stmt {
                collect_func_final_violations(func, &module_final_names, source, &mut out);
            }
            continue;
        };
        collect_class_final_violations(cls_def, classes, &class_finals, source, &mut out);
    }
    out
}

/// Collect Final violations inside a class definition.
fn collect_class_final_violations(
    cls_def: &StmtClassDef,
    all_classes: &[ClassInfo],
    class_finals: &std::collections::HashMap<&str, std::collections::HashSet<&str>>,
    source: &str,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
    use crate::scope::{FinalViolationInfo, FinalViolationKind};

    let class_name = cls_def.name.as_str();

    // Find parent class Final attrs for SubclassOverrideFinal.
    let base_names: Vec<&str> = cls_def
        .arguments
        .as_deref()
        .map(|args| {
            args.args
                .iter()
                .filter_map(|base| {
                    if let Expr::Name(n) = base {
                        Some(n.id.as_str())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    // Collect Final attrs from all parent classes.
    let parent_finals: std::collections::HashSet<&str> = base_names
        .iter()
        .filter_map(|name| class_finals.get(name))
        .flat_map(|set| set.iter().copied())
        .collect();

    // Collect Final attrs in THIS class (annotation-only or with value).
    let mut this_final_attrs: std::collections::HashMap<&str, bool> = std::collections::HashMap::new();
    // key = attr name, value = has_initializer
    for body_stmt in &cls_def.body {
        let Stmt::AnnAssign(ann) = body_stmt else { continue };
        let Expr::Name(n) = ann.target.as_ref() else { continue };
        let attr_name = n.id.as_str();
        let range = ann.annotation.range();
        let Some(ann_text) = source.get(range.start().to_u32() as usize..range.end().to_u32() as usize) else { continue };
        if ann_text_is_final(ann_text) {
            let has_value = ann.value.is_some();
            this_final_attrs.insert(attr_name, has_value);
        }
    }

    // Find attrs unconditionally assigned in __init__.
    let init_assigns: std::collections::HashSet<String> = cls_def
        .body
        .iter()
        .find_map(|s| {
            if let Stmt::FunctionDef(f) = s {
                if f.name.as_str() == "__init__" {
                    return Some(collect_unconditional_self_assigns(&f.body));
                }
            }
            None
        })
        .unwrap_or_default();

    // ClassFinalWithoutInit: attr has no initializer AND not in __init__ assignments.
    for (attr_name, has_value) in &this_final_attrs {
        if !has_value && !init_assigns.contains(*attr_name) {
            // Find the span of this annotation.
            for body_stmt in &cls_def.body {
                let Stmt::AnnAssign(ann) = body_stmt else { continue };
                let Expr::Name(n) = ann.target.as_ref() else { continue };
                if n.id.as_str() != *attr_name { continue; }
                out.push(FinalViolationInfo {
                    kind: FinalViolationKind::ClassFinalWithoutInit,
                    span: text_range_to_span(ann.range()),
                    name: attr_name.to_string(),
                });
                break;
            }
        }
    }

    // Walk all method bodies for instance Final violations.
    for body_stmt in &cls_def.body {
        let Stmt::FunctionDef(method) = body_stmt else { continue };
        let is_init = method.name.as_str() == "__init__";
        for method_stmt in &method.body {
            collect_instance_final_violations(
                method_stmt,
                is_init,
                &this_final_attrs,
                source,
                out,
            );
        }
    }

    // SubclassOverrideFinal: child class declares an attr that is Final in a parent.
    for body_stmt in &cls_def.body {
        let attr_name = match body_stmt {
            Stmt::Assign(assign) if assign.targets.len() == 1 => {
                if let Expr::Name(n) = &assign.targets[0] {
                    n.id.as_str()
                } else {
                    continue;
                }
            }
            Stmt::AnnAssign(ann) => {
                if let Expr::Name(n) = ann.target.as_ref() {
                    n.id.as_str()
                } else {
                    continue;
                }
            }
            _ => continue,
        };

        // Skip private name-mangled attributes.
        if attr_name.starts_with("__") && !attr_name.ends_with("__") {
            continue;
        }

        if parent_finals.contains(attr_name) {
            let span = match body_stmt {
                Stmt::Assign(assign) => text_range_to_span(assign.range()),
                Stmt::AnnAssign(ann) => text_range_to_span(ann.range()),
                _ => continue,
            };
            out.push(crate::scope::FinalViolationInfo {
                kind: FinalViolationKind::SubclassOverrideFinal,
                span,
                name: attr_name.to_string(),
            });
        }
    }

    // Recurse into nested class definitions.
    for body_stmt in &cls_def.body {
        if let Stmt::ClassDef(nested) = body_stmt {
            collect_class_final_violations(nested, all_classes, class_finals, source, out);
        }
    }
}

/// Check a single statement inside a method body for instance Final violations.
fn collect_instance_final_violations(
    stmt: &Stmt,
    is_init: bool,
    class_final_attrs: &std::collections::HashMap<&str, bool>,
    source: &str,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
    use crate::scope::{FinalViolationInfo, FinalViolationKind};
    match stmt {
        Stmt::AnnAssign(ann) => {
            // self.x: Final = ... outside __init__
            if !is_init {
                if let Expr::Attribute(attr) = ann.target.as_ref() {
                    let Some(ann_span) = Some(ann.annotation.range()) else { return };
                    let Some(ann_text) = source.get(
                        ann_span.start().to_u32() as usize..ann_span.end().to_u32() as usize
                    ) else { return };
                    if ann_text_is_final(ann_text) {
                        if let Expr::Name(self_name) = attr.value.as_ref() {
                            if self_name.id == "self" {
                                out.push(FinalViolationInfo {
                                    kind: FinalViolationKind::InstanceFinalOutsideInit,
                                    span: text_range_to_span(ann.range()),
                                    name: attr.attr.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        Stmt::Assign(assign) | Stmt::AugAssign(assign @ _) => {
            let targets: &[Expr] = match stmt {
                Stmt::Assign(a) => &a.targets,
                _ => return,
            };
            for target in targets {
                let Expr::Attribute(attr) = target else { continue };
                let Expr::Name(self_name) = attr.value.as_ref() else { continue };
                if self_name.id != "self" { continue; }
                let field_name = attr.attr.as_str();
                if let Some(&has_value) = class_final_attrs.get(field_name) {
                    let kind = if is_init && has_value {
                        FinalViolationKind::InstanceReassignAlreadyInitialized
                    } else if !is_init {
                        FinalViolationKind::InstanceModifyFinal
                    } else {
                        continue;
                    };
                    out.push(FinalViolationInfo {
                        kind,
                        span: text_range_to_span(assign.range()),
                        name: field_name.to_string(),
                    });
                }
            }
        }
        Stmt::AugAssign(aug) => {
            // self.X += 1 — augmented assignment to Final class attr
            let Expr::Attribute(attr) = aug.target.as_ref() else { return };
            let Expr::Name(self_name) = attr.value.as_ref() else { return };
            if self_name.id != "self" { return; }
            let field_name = attr.attr.as_str();
            if class_final_attrs.contains_key(field_name) {
                out.push(FinalViolationInfo {
                    kind: FinalViolationKind::InstanceModifyFinal,
                    span: text_range_to_span(aug.range()),
                    name: field_name.to_string(),
                });
            }
        }
        _ => {}
    }
}

/// Collect the names of attributes unconditionally assigned via `self.X = ...` in
/// the top-level statements of a function body (i.e., not inside if/for/while/try).
fn collect_unconditional_self_assigns(stmts: &[Stmt]) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    for stmt in stmts {
        let Stmt::Assign(assign) = stmt else { continue };
        for target in &assign.targets {
            let Expr::Attribute(attr) = target else { continue };
            let Expr::Name(n) = attr.value.as_ref() else { continue };
            if n.id == "self" {
                names.insert(attr.attr.to_string());
            }
        }
    }
    names
}

/// Collect Final violations inside a function body (GlobalFinalModification and
/// FunctionLocalFinalModification).
fn collect_func_final_violations(
    func: &StmtFunctionDef,
    module_final_names: &std::collections::HashSet<&str>,
    source: &str,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
    use crate::scope::{FinalViolationInfo, FinalViolationKind};
    // Find `global X` declarations to know which names are global Final.
    let global_final_names: std::collections::HashSet<&str> = func
        .body
        .iter()
        .filter_map(|s| {
            if let Stmt::Global(g) = s {
                Some(g.names.iter().filter_map(|name| {
                    if module_final_names.contains(name.as_str()) {
                        Some(name.as_str())
                    } else {
                        None
                    }
                }))
            } else {
                None
            }
        })
        .flatten()
        .collect();

    // Collect function-local Final variables (x: Final = ...) as we scan.
    let mut local_finals: std::collections::HashSet<String> = std::collections::HashSet::new();

    for stmt in &func.body {
        collect_func_stmt_final_violations(
            stmt,
            &global_final_names,
            &mut local_finals,
            source,
            out,
        );
    }
}

/// Process a single statement inside a function for Final violations.
#[allow(clippy::too_many_arguments)]
fn collect_func_stmt_final_violations(
    stmt: &Stmt,
    global_finals: &std::collections::HashSet<&str>,
    local_finals: &mut std::collections::HashSet<String>,
    source: &str,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
    use crate::scope::{FinalViolationInfo, FinalViolationKind};
    match stmt {
        Stmt::AnnAssign(ann) => {
            // Register x: Final = ... as a local Final.
            if let Expr::Name(n) = ann.target.as_ref() {
                let range = ann.annotation.range();
                if let Some(ann_text) = source.get(range.start().to_u32() as usize..range.end().to_u32() as usize) {
                    if ann_text_is_final(ann_text) {
                        local_finals.insert(n.id.to_string());
                    }
                }
            }
        }
        Stmt::Assign(assign) => {
            for target in &assign.targets {
                check_final_assign_target(target, global_finals, local_finals, out);
            }
        }
        Stmt::AugAssign(aug) => {
            check_final_assign_target(aug.target.as_ref(), global_finals, local_finals, out);
        }
        Stmt::For(for_stmt) => {
            check_final_assign_target(for_stmt.target.as_ref(), global_finals, local_finals, out);
        }
        Stmt::With(with_stmt) => {
            for item in &with_stmt.items {
                if let Some(opt_var) = &item.optional_vars {
                    check_final_assign_target(opt_var.as_ref(), global_finals, local_finals, out);
                }
            }
        }
        Stmt::Expr(expr_stmt) => {
            check_walrus_final(expr_stmt.value.as_ref(), global_finals, local_finals, out);
        }
        _ => {}
    }
}

/// Check if an assign target is a Final name and emit violations if so.
fn check_final_assign_target(
    target: &Expr,
    global_finals: &std::collections::HashSet<&str>,
    local_finals: &std::collections::HashSet<String>,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
    use crate::scope::{FinalViolationInfo, FinalViolationKind};
    match target {
        Expr::Name(n) => {
            let name = n.id.as_str();
            if global_finals.contains(name) {
                out.push(FinalViolationInfo {
                    kind: FinalViolationKind::GlobalFinalModification,
                    span: text_range_to_span(n.range()),
                    name: name.to_string(),
                });
            } else if local_finals.contains(name) {
                out.push(FinalViolationInfo {
                    kind: FinalViolationKind::FunctionLocalFinalModification,
                    span: text_range_to_span(n.range()),
                    name: name.to_string(),
                });
            }
        }
        Expr::Tuple(tup) => {
            // Handle tuple unpacking: (a, x) = ...
            for elt in &tup.elts {
                check_final_assign_target(elt, global_finals, local_finals, out);
            }
        }
        _ => {}
    }
}

/// Check an expression for walrus operator assignments to Final variables.
fn check_walrus_final(
    expr: &Expr,
    global_finals: &std::collections::HashSet<&str>,
    local_finals: &std::collections::HashSet<String>,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
    use crate::scope::{FinalViolationInfo, FinalViolationKind};
    if let Expr::Named(named) = expr {
        let Expr::Name(n) = named.target.as_ref() else { return };
        let name = n.id.as_str();
        if global_finals.contains(name) {
            out.push(FinalViolationInfo {
                kind: FinalViolationKind::GlobalFinalModification,
                span: text_range_to_span(n.range()),
                name: name.to_string(),
            });
        } else if local_finals.contains(name) {
            out.push(FinalViolationInfo {
                kind: FinalViolationKind::FunctionLocalFinalModification,
                span: text_range_to_span(n.range()),
                name: name.to_string(),
            });
        }
    }
}


/// Collect enum `_value_` type annotation violations.
///
/// When `_value_: T` is declared in an enum class body (annotation-only, no value),
/// every member literal that is type-incompatible with `T` is flagged, and any
/// `self._value_ = param` assignment in `__init__` where `param` has a type
/// annotation incompatible with `T` is also flagged.
fn collect_enum_value_type_violations(
    stmts: &[Stmt],
    source: &str,
) -> Vec<crate::scope::EnumValueTypeViolationInfo> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else { continue };
        if !stmt_class_is_direct_enum(cls) {
            continue;
        }
        let Some(declared_type) = find_enum_value_annotation(cls, source) else {
            continue;
        };
        let class_name = cls.name.to_string();
        check_enum_member_values(cls, source, &declared_type, &class_name, &mut out);
        check_enum_init_value_param(cls, source, &declared_type, &class_name, &mut out);
    }
    out
}

/// Checks member value assignments in an enum class body against `_value_: T`.
fn check_enum_member_values(
    cls: &StmtClassDef,
    _source: &str,
    declared_type: &str,
    class_name: &str,
    out: &mut Vec<crate::scope::EnumValueTypeViolationInfo>,
) {
    use crate::scope::{EnumValueTypeViolationInfo, EnumValueTypeViolationKind};
    for body_stmt in &cls.body {
        let Stmt::Assign(assign) = body_stmt else {
            continue;
        };
        if assign.targets.len() != 1 {
            continue;
        }
        let Expr::Name(name_expr) = &assign.targets[0] else {
            continue;
        };
        let member_name = name_expr.id.as_str();
        // Skip dunder and special names.
        if member_name.starts_with("__") || member_name == "_value_" {
            continue;
        }
        let Some(actual_type) = infer_member_literal_type(assign.value.as_ref()) else {
            continue;
        };
        if !enum_types_compatible(declared_type, &actual_type) {
            out.push(EnumValueTypeViolationInfo {
                kind: EnumValueTypeViolationKind::MemberValueTypeMismatch,
                span: text_range_to_span(assign.value.range()),
                class_name: class_name.to_owned(),
                declared_type: declared_type.to_owned(),
                actual_type,
            });
        }
    }
}

/// Checks `self._value_ = param` assignments in `__init__` against `_value_: T`.
fn check_enum_init_value_param(
    cls: &StmtClassDef,
    source: &str,
    declared_type: &str,
    class_name: &str,
    out: &mut Vec<crate::scope::EnumValueTypeViolationInfo>,
) {
    for body_stmt in &cls.body {
        let Stmt::FunctionDef(func) = body_stmt else {
            continue;
        };
        if func.name.as_str() != "__init__" {
            continue;
        }
        // Build parameter name -> annotation text map.
        let params: Vec<(&str, &str)> = func
            .parameters
            .posonlyargs
            .iter()
            .chain(func.parameters.args.iter())
            .filter_map(|p| {
                if p.parameter.name.as_str() == "self" {
                    return None;
                }
                let ann_expr = p.parameter.annotation.as_deref()?;
                let range = ann_expr.range();
                let ann_text = source
                    .get(range.start().to_u32() as usize..range.end().to_u32() as usize)?
                    .trim();
                Some((p.parameter.name.as_str(), ann_text))
            })
            .collect();

        for init_stmt in &func.body {
            check_init_self_value_assign(init_stmt, &params, source, declared_type, class_name, out);
        }
    }
}

/// Checks a single `__init__` statement for `self._value_ = param` pattern.
fn check_init_self_value_assign<'a>(
    stmt: &Stmt,
    params: &[(&'a str, &'a str)],
    _source: &str,
    declared_type: &str,
    class_name: &str,
    out: &mut Vec<crate::scope::EnumValueTypeViolationInfo>,
) {
    use crate::scope::{EnumValueTypeViolationInfo, EnumValueTypeViolationKind};
    let Stmt::Assign(assign) = stmt else { return };
    if assign.targets.len() != 1 {
        return;
    }
    let Expr::Attribute(attr) = &assign.targets[0] else { return };
    if attr.attr.as_str() != "_value_" {
        return;
    }
    let Expr::Name(self_name) = attr.value.as_ref() else { return };
    if self_name.id.as_str() != "self" {
        return;
    }
    // Found self._value_ = <expr>; check if RHS is a parameter name.
    let Expr::Name(rhs_name) = assign.value.as_ref() else { return };
    let param_name = rhs_name.id.as_str();
    let Some((_, ann_text)) = params.iter().find(|(n, _)| *n == param_name) else {
        return;
    };
    let actual_type = ann_text.trim().to_owned();
    if !enum_types_compatible(declared_type, &actual_type) {
        out.push(EnumValueTypeViolationInfo {
            kind: EnumValueTypeViolationKind::InitValueParamTypeMismatch,
            span: text_range_to_span(assign.range()),
            class_name: class_name.to_owned(),
            declared_type: declared_type.to_owned(),
            actual_type,
        });
    }
}

/// Returns `true` when the class directly inherits from one of the standard enum bases.
fn stmt_class_is_direct_enum(cls: &StmtClassDef) -> bool {
    let Some(args) = cls.arguments.as_deref() else {
        return false;
    };
    args.args.iter().any(|base| {
        if let Expr::Name(n) = base {
            ENUM_BASES.contains(&n.id.as_str())
        } else {
            false
        }
    })
}

/// Finds an `_value_: T` annotation-only declaration in the enum class body.
///
/// Returns the annotation text (e.g. `"int"`) if found, `None` otherwise.
fn find_enum_value_annotation(cls: &StmtClassDef, source: &str) -> Option<String> {
    for stmt in &cls.body {
        let Stmt::AnnAssign(ann) = stmt else {
            continue;
        };
        if ann.value.is_some() {
            continue; // Must be annotation-only (no initializer).
        }
        let Expr::Name(n) = ann.target.as_ref() else {
            continue;
        };
        if n.id != "_value_" {
            continue;
        }
        let range = ann.annotation.range();
        let ann_text = source
            .get(range.start().to_u32() as usize..range.end().to_u32() as usize)?
            .trim()
            .to_owned();
        return Some(ann_text);
    }
    None
}

/// Infers the primitive type name of a literal expression.
///
/// Returns `None` for non-literal expressions (tuples, calls, names, etc.) so
/// that only clearly-typed literals are checked against `_value_: T`.
fn infer_member_literal_type(expr: &Expr) -> Option<String> {
    match expr {
        Expr::StringLiteral(_) => Some("str".to_owned()),
        Expr::NumberLiteral(n) => {
            if matches!(n.value, ruff_python_ast::Number::Float(_)) {
                Some("float".to_owned())
            } else if matches!(n.value, ruff_python_ast::Number::Complex { .. }) {
                Some("complex".to_owned())
            } else {
                Some("int".to_owned())
            }
        }
        Expr::BooleanLiteral(_) => Some("bool".to_owned()),
        Expr::NoneLiteral(_) => Some("None".to_owned()),
        Expr::BytesLiteral(_) => Some("bytes".to_owned()),
        _ => None,
    }
}

/// Returns `true` when `actual` is compatible with `declared` as an enum `_value_` type.
///
/// - Identical types are always compatible.
/// - `bool` is compatible with `int` (bool is a subclass of int).
/// - `bool` and `int` are compatible with `float` (numeric tower).
fn enum_types_compatible(declared: &str, actual: &str) -> bool {
    if declared == actual {
        return true;
    }
    // bool is a subtype of int.
    if declared == "int" && actual == "bool" {
        return true;
    }
    // int and bool are compatible with float (PEP 3141 numeric tower).
    if declared == "float" && (actual == "int" || actual == "bool") {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Float parameter int-attr access collection (stub)
// ---------------------------------------------------------------------------

// Collect attribute accesses of `int`-only attributes on `float`-typed parameters.
// Full implementation is pending — returns empty for now.

// ---------------------------------------------------------------------------
// Literal string / enum member mismatch collection (stub)
// ---------------------------------------------------------------------------

// Collect `Literal["X.Y"]` vs `Literal[X.Y]` mismatches.
// Full implementation is pending — returns empty for now.

// ---------------------------------------------------------------------------
// Float parameter int-only attribute access collection
// ---------------------------------------------------------------------------

/// `int`-only attributes that are invalid to access on a `float`-typed parameter.
///
/// `numerator` and `denominator` are defined on `int` but not on `float`.
const INT_ONLY_FLOAT_ATTRS: &[&str] = &["numerator", "denominator"];

/// Collect attribute accesses of `int`-only attributes on `float`-typed parameters.
///
/// Only top-level statements of each function body are examined — accesses inside
/// `if`/`for`/`while`/`match`/`with`/`try` blocks are excluded so that
/// `isinstance`-guarded paths (where the parameter has been narrowed to `int`) are
/// not flagged.
pub(crate) fn collect_float_param_int_attr_accesses(
    stmts: &[Stmt],
    source: &str,
) -> Vec<FloatParamIntAttrAccess> {
    let mut out = Vec::new();
    for stmt in stmts {
        if let Stmt::FunctionDef(func) = stmt {
            collect_float_accesses_in_function(func, source, &mut out);
            // Recurse into nested functions.
            out.extend(collect_float_param_int_attr_accesses(&func.body, source));
        }
    }
    out
}

fn collect_float_accesses_in_function(
    func: &StmtFunctionDef,
    source: &str,
    out: &mut Vec<FloatParamIntAttrAccess>,
) {
    // Collect names of parameters annotated exactly as `float`.
    let float_params: Vec<&str> = func
        .parameters
        .posonlyargs
        .iter()
        .chain(func.parameters.args.iter())
        .chain(func.parameters.kwonlyargs.iter())
        .filter(|p| {
            p.parameter.annotation.as_deref().is_some_and(|ann| {
                let range = ann.range();
                source
                    .get(range.start().to_u32() as usize..range.end().to_u32() as usize)
                    .map(str::trim)
                    == Some("float")
            })
        })
        .map(|p| p.parameter.name.as_str())
        .collect();

    if float_params.is_empty() {
        return;
    }

    // Only walk the top-level statements of the function body (no recursion into
    // if/for/while/match/with/try blocks).
    for stmt in &func.body {
        let Stmt::Expr(expr_stmt) = stmt else {
            continue;
        };
        let Expr::Attribute(attr) = expr_stmt.value.as_ref() else {
            continue;
        };
        let Some(obj_name) = expr_simple_name(&attr.value) else {
            continue;
        };
        if !float_params.contains(&obj_name.as_str()) {
            continue;
        }
        let attr_name = attr.attr.as_str();
        if !INT_ONLY_FLOAT_ATTRS.contains(&attr_name) {
            continue;
        }
        out.push(FloatParamIntAttrAccess {
            param_name: obj_name,
            attr_name: attr_name.to_owned(),
            span: text_range_to_span(expr_stmt.range()),
        });
    }
}

// ---------------------------------------------------------------------------
// Literal string vs enum member mismatch collection
// ---------------------------------------------------------------------------

/// Collect annotated assignments inside function bodies where the declared type is
/// `Literal["X.Y"]` (a quoted string that looks like an enum member) but the RHS is
/// a parameter typed as `Literal[X.Y]` (the actual unquoted enum member).
///
/// This detects the mismatch between `Literal["Color.RED"]` (string) and
/// `Literal[Color.RED]` (enum member) when the RHS is the enum-literal-typed parameter.
pub(crate) fn collect_literal_string_enum_mismatches(
    stmts: &[Stmt],
    source: &str,
) -> Vec<LiteralStringEnumMismatch> {
    let mut out = Vec::new();
    for stmt in stmts {
        if let Stmt::FunctionDef(func) = stmt {
            collect_literal_mismatches_in_function(func, source, &mut out);
            // Recurse into nested functions.
            out.extend(collect_literal_string_enum_mismatches(&func.body, source));
        }
    }
    out
}

/// Returns the inner content of `Literal[...]` if `ann` starts with `Literal[` and ends with `]`.
fn literal_inner_content(ann: &str) -> Option<&str> {
    let ann = ann.trim();
    let inner_start = ann.find("Literal[")? + "Literal[".len();
    let body = &ann[inner_start..];
    body.strip_suffix(']')
}

/// Returns `true` when `s` is an enum member form: `Identifier.Identifier` (no spaces,
/// no quotes, no brackets, exactly one dot, both parts are simple identifiers).
fn is_enum_member_form(s: &str) -> bool {
    let s = s.trim();
    if s.contains(' ') || s.contains('[') || s.contains('(') || s.contains('"') || s.contains('\'')
    {
        return false;
    }
    let mut parts = s.splitn(2, '.');
    let Some(obj) = parts.next() else {
        return false;
    };
    let Some(attr) = parts.next() else {
        return false;
    };
    !attr.contains('.') && is_simple_ident(obj) && is_simple_ident(attr)
}

fn is_simple_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Returns `true` when `s` is a quoted string whose body equals `enum_form`.
///
/// E.g. `is_quoted_string_of("\"Color.RED\"", "Color.RED")` → `true`.
fn is_quoted_string_of(s: &str, enum_form: &str) -> bool {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        return inner == enum_form;
    }
    if let Some(inner) = s.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        return inner == enum_form;
    }
    false
}

fn collect_literal_mismatches_in_function(
    func: &StmtFunctionDef,
    source: &str,
    out: &mut Vec<LiteralStringEnumMismatch>,
) {
    // Build a list of (param_name, enum_form) pairs for parameters annotated as
    // `Literal[X.Y]` where X.Y is an enum member (e.g. `Literal[Color.RED]`).
    let param_enum_literals: Vec<(&str, &str)> = func
        .parameters
        .posonlyargs
        .iter()
        .chain(func.parameters.args.iter())
        .chain(func.parameters.kwonlyargs.iter())
        .filter_map(|p| {
            let ann_expr = p.parameter.annotation.as_deref()?;
            let range = ann_expr.range();
            let ann_text = source
                .get(range.start().to_u32() as usize..range.end().to_u32() as usize)?
                .trim();
            let inner = literal_inner_content(ann_text)?;
            let inner = inner.trim();
            if is_enum_member_form(inner) {
                Some((p.parameter.name.as_str(), inner))
            } else {
                None
            }
        })
        .collect();

    if param_enum_literals.is_empty() {
        return;
    }

    // Walk only the top-level statements of the function body.
    for stmt in &func.body {
        let Stmt::AnnAssign(ann_assign) = stmt else {
            continue;
        };
        // The RHS must be a simple name referring to a parameter.
        let Some(value_expr) = ann_assign.value.as_deref() else {
            continue;
        };
        let Some(rhs_name) = expr_simple_name(value_expr) else {
            continue;
        };

        // Find the enum form for this parameter.
        let Some((_param, enum_form)) = param_enum_literals
            .iter()
            .find(|(param, _)| *param == rhs_name.as_str())
        else {
            continue;
        };

        // Extract the annotation text of the LHS variable.
        let ann_range = ann_assign.annotation.range();
        let Some(ann_text) =
            source.get(ann_range.start().to_u32() as usize..ann_range.end().to_u32() as usize)
        else {
            continue;
        };
        let ann_text = ann_text.trim();

        // The annotation must be `Literal["X.Y"]` where the inner string equals the enum form.
        let Some(inner) = literal_inner_content(ann_text) else {
            continue;
        };
        let inner = inner.trim();
        if !is_quoted_string_of(inner, enum_form) {
            continue;
        }

        // Extract the variable name span.
        let Expr::Name(lhs_name) = ann_assign.target.as_ref() else {
            continue;
        };
        out.push(LiteralStringEnumMismatch {
            var_name: lhs_name.id.as_str().to_owned(),
            annotation: ann_text.to_owned(),
            enum_form: (*enum_form).to_owned(),
            span: text_range_to_span(lhs_name.range()),
        });
    }
}

// ---------------------------------------------------------------------------
// Module-level attribute access collection
// ---------------------------------------------------------------------------

/// Collect module-level `Name.attr` attribute accesses.
///
/// Used by E0059 to detect access to `__match_args__` on a dataclass with
/// `match_args=False`.
fn collect_module_attr_accesses(stmts: &[Stmt]) -> Vec<crate::scope::ModuleAttrAccessInfo> {
    let mut out = Vec::new();
    for stmt in stmts {
        collect_attr_accesses_from_stmt(stmt, &mut out);
    }
    out
}

fn collect_attr_accesses_from_stmt(stmt: &Stmt, out: &mut Vec<crate::scope::ModuleAttrAccessInfo>) {
    match stmt {
        Stmt::Expr(node) => collect_attr_accesses_from_expr(&node.value, out),
        Stmt::If(node) => {
            collect_attr_accesses_from_expr(&node.test, out);
            for s in &node.body {
                collect_attr_accesses_from_stmt(s, out);
            }
            for clause in &node.elif_else_clauses {
                if let Some(test) = &clause.test {
                    collect_attr_accesses_from_expr(test, out);
                }
                for s in &clause.body {
                    collect_attr_accesses_from_stmt(s, out);
                }
            }
        }
        _ => {}
    }
}

fn collect_attr_accesses_from_expr(expr: &Expr, out: &mut Vec<crate::scope::ModuleAttrAccessInfo>) {
    if let Expr::Attribute(attr) = expr {
        if let Some(object_name) = expr_simple_name(&attr.value) {
            out.push(crate::scope::ModuleAttrAccessInfo {
                object_name,
                attr_name: attr.attr.to_string(),
                span: text_range_to_span(expr.range()),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Annotated() direct-call detection
// ---------------------------------------------------------------------------

/// Collect spans of module-level calls where `Annotated` itself is called as a
/// function — either bare `Annotated(...)` or parameterized `Annotated[T, ...]()`.
///
/// Used by `BSK-E0045` to detect invalid direct invocation of `Annotated`.
fn collect_annotated_direct_calls(stmts: &[Stmt]) -> Vec<Span> {
    let mut out = Vec::new();
    for stmt in stmts {
        if let Stmt::Expr(node) = stmt {
            collect_annotated_calls_from_expr(&node.value, &mut out);
        }
    }
    out
}

fn collect_annotated_calls_from_expr(expr: &Expr, out: &mut Vec<Span>) {
    let Expr::Call(call) = expr else { return };
    let is_annotated_callee = match call.func.as_ref() {
        // `Annotated(...)` — bare name call
        Expr::Name(n) => n.id.as_str() == "Annotated",
        // `Annotated[T, ...]()` — subscript then call
        Expr::Subscript(s) => {
            matches!(s.value.as_ref(), Expr::Name(n) if n.id.as_str() == "Annotated")
        }
        _ => false,
    };
    if is_annotated_callee {
        out.push(text_range_to_span(expr.range()));
    }
}

// ---------------------------------------------------------------------------
// Module-level ordering comparison collection
// ---------------------------------------------------------------------------

/// Collect module-level `a < b` / `a <= b` / `a > b` / `a >= b` comparisons
/// where both operands are simple names.
///
/// Used by E0060 to detect cross-type ordering comparisons of `order=True`
/// dataclass instances.
fn collect_module_order_comparisons(
    stmts: &[Stmt],
) -> Vec<crate::scope::ModuleOrderComparisonInfo> {
    let mut out = Vec::new();
    for stmt in stmts {
        collect_order_comparisons_from_stmt(stmt, &mut out);
    }
    out
}

fn collect_order_comparisons_from_stmt(
    stmt: &Stmt,
    out: &mut Vec<crate::scope::ModuleOrderComparisonInfo>,
) {
    match stmt {
        Stmt::Expr(node) => collect_order_comparisons_from_expr(&node.value, out),
        Stmt::If(node) => {
            collect_order_comparisons_from_expr(&node.test, out);
            for s in &node.body {
                collect_order_comparisons_from_stmt(s, out);
            }
            for clause in &node.elif_else_clauses {
                if let Some(test) = &clause.test {
                    collect_order_comparisons_from_expr(test, out);
                }
                for s in &clause.body {
                    collect_order_comparisons_from_stmt(s, out);
                }
            }
        }
        _ => {}
    }
}

fn collect_order_comparisons_from_expr(
    expr: &Expr,
    out: &mut Vec<crate::scope::ModuleOrderComparisonInfo>,
) {
    let Expr::Compare(cmp) = expr else { return };
    let Some(left_name) = expr_simple_name(&cmp.left) else {
        return;
    };
    for (op, comparator) in cmp.ops.iter().zip(cmp.comparators.iter()) {
        let scope_op = match op {
            ruff_python_ast::CmpOp::Lt => crate::scope::CompareOp::Lt,
            ruff_python_ast::CmpOp::LtE => crate::scope::CompareOp::LtE,
            ruff_python_ast::CmpOp::Gt => crate::scope::CompareOp::Gt,
            ruff_python_ast::CmpOp::GtE => crate::scope::CompareOp::GtE,
            _ => continue,
        };
        let Some(right_name) = expr_simple_name(comparator) else {
            continue;
        };
        out.push(crate::scope::ModuleOrderComparisonInfo {
            left_name: left_name.clone(),
            right_name,
            op: scope_op,
            span: text_range_to_span(expr.range()),
        });
    }
}
