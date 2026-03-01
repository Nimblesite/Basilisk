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
    GenericParamInfo, HistoricalPositionalViolation, HistoricalPositionalViolationKind, ImportInfo,
    ImportKind, LiteralStringEnumMismatch, MatchStmtInfo, NamedTupleDefInfo, NewTypeCallInfo,
    ParameterInfo, Pep695BoundViolation, Pep695BoundViolationKind,
    ProtocolSelfViolation, ReadOnlyViolationInfo, ReadOnlyViolationKind, ResolvedModule,
    ReturnAnnotationKind, ReturnStmtInfo, RevealTypeCallInfo, RhsKind, Span, TypeVarCallInfo,
    TypedDictCallInfo, TypedDictKeyViolation, TypedDictKeyViolationKind, TypedDictSecondArgKind,
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

    // Post-process: apply @dataclass_transform factory semantics.
    apply_dataclass_transform(&module.ast.body, &mut classes, &functions);

    let calls = collect_module_level_calls(&module.ast.body);
    let typevar_calls = collect_typevar_calls(&module.ast.body);
    let reveal_type_calls = collect_reveal_type_calls(&module.ast.body);
    let assert_type_calls =
        collect_assert_type_calls_from_stmts(&module.ast.body, &[], &module.source);
    let typeddict_calls = collect_typeddict_calls(&module.ast.body);
    let newtype_calls = collect_newtype_calls(&module.ast.body);
    let namedtuple_defs = collect_namedtuple_defs(&module.ast.body, &module.source);
    let multiple_unbounded_tuple_spans = collect_multiple_unbounded_tuple_spans(&module.ast.body);

    let module_bare_assignments = collect_module_bare_assignments(&module.ast.body);
    let module_attr_assignments = collect_module_attr_assignments(&module.ast.body);
    let final_violations = collect_final_violations(&module.ast.body, &classes, &module.source);
    let float_param_int_attr_accesses =
        collect_float_param_int_attr_accesses(&module.ast.body, &module.source);
    let literal_string_enum_mismatches =
        collect_literal_string_enum_mismatches(&module.ast.body, &module.source);
    let readonly_violations = collect_readonly_violations(&module.ast.body, &classes);
    let protocol_self_violations =
        collect_protocol_self_violations(&module.ast.body, &classes, &functions, &module.source);
    let typeddict_class_names: std::collections::HashSet<&str> = classes
        .iter()
        .filter(|c| c.is_typed_dict)
        .map(|c| c.name.as_str())
        .collect();
    let isinstance_typeddict_violations =
        collect_isinstance_typeddict_violations(&module.ast.body, &typeddict_class_names);
    let typeddict_key_violations =
        collect_typeddict_key_violations(&module.ast.body, &classes, &module.source);
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
        namedtuple_defs,
        float_param_int_attr_accesses,
        literal_string_enum_mismatches,
        enum_value_type_violations: collect_enum_value_type_violations(&module.ast.body, &module.source),
        local_classvar_violations: Vec::new(),
        pep695_bound_violations: collect_pep695_bound_violations(&module.ast.body),
        historical_positional_violations: collect_historical_positional_violations(&module.ast.body),
        invalid_string_annotations: Vec::new(),
        protocol_self_violations,
        isinstance_typeddict_violations,
        typeddict_key_violations,
        path: module.path.clone(),
        source: module.source.clone(),
    }
}

// ---------------------------------------------------------------------------
// Historical positional-only parameter violation detection
// ---------------------------------------------------------------------------

fn is_historical_posonly_name(name: &str) -> bool {
    name.starts_with("__") && !name.ends_with("__")
}

/// Build a map from function/method name to the set of parameter names that
/// are positional-only by the historical `__name` convention.
///
/// Only parameters in `args` (not `kwonlyargs`) count; functions that use
/// PEP 570 `/` syntax (`posonlyargs` is non-empty) are excluded.
fn collect_historical_posonly_func_params(
    stmts: &[Stmt],
) -> std::collections::HashMap<String, std::collections::HashSet<String>> {
    let mut map = std::collections::HashMap::new();
    collect_posonly_params_from_stmts(stmts, &mut map);
    map
}

fn collect_posonly_params_from_stmts(
    stmts: &[Stmt],
    map: &mut std::collections::HashMap<String, std::collections::HashSet<String>>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                let params = &func.parameters;
                // Historical convention does not apply when PEP 570 `/` is used.
                if params.posonlyargs.is_empty() {
                    let posonly: std::collections::HashSet<String> = params
                        .args
                        .iter()
                        .map(|p| p.parameter.name.as_str())
                        .filter(|name| is_historical_posonly_name(name))
                        .map(str::to_owned)
                        .collect();
                    if !posonly.is_empty() {
                        map.insert(func.name.to_string(), posonly);
                    }
                }
                collect_posonly_params_from_stmts(&func.body, map);
            }
            Stmt::ClassDef(cls) => {
                collect_posonly_params_from_stmts(&cls.body, map);
            }
            _ => {}
        }
    }
}

fn collect_historical_positional_violations(stmts: &[Stmt]) -> Vec<HistoricalPositionalViolation> {
    let posonly_map = collect_historical_posonly_func_params(stmts);
    let mut out = Vec::new();
    collect_hist_violations_from_stmts(stmts, &posonly_map, &mut out);
    out
}

fn collect_hist_violations_from_stmts(
    stmts: &[Stmt],
    posonly_map: &std::collections::HashMap<String, std::collections::HashSet<String>>,
    out: &mut Vec<HistoricalPositionalViolation>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                check_func_for_hist_posonly_violation(func, out);
                collect_hist_violations_from_stmts(&func.body, posonly_map, out);
            }
            Stmt::ClassDef(cls) => {
                collect_hist_violations_from_stmts(&cls.body, posonly_map, out);
            }
            Stmt::Expr(e) => {
                collect_hist_violations_from_expr(&e.value, posonly_map, out);
            }
            Stmt::Assign(a) => {
                collect_hist_violations_from_expr(&a.value, posonly_map, out);
            }
            Stmt::AnnAssign(a) => {
                if let Some(val) = &a.value {
                    collect_hist_violations_from_expr(val, posonly_map, out);
                }
            }
            Stmt::Return(r) => {
                if let Some(val) = &r.value {
                    collect_hist_violations_from_expr(val, posonly_map, out);
                }
            }
            Stmt::If(node) => {
                collect_hist_violations_from_stmts(&node.body, posonly_map, out);
                for clause in &node.elif_else_clauses {
                    collect_hist_violations_from_stmts(&clause.body, posonly_map, out);
                }
            }
            Stmt::For(node) => {
                collect_hist_violations_from_stmts(&node.body, posonly_map, out);
                collect_hist_violations_from_stmts(&node.orelse, posonly_map, out);
            }
            Stmt::While(node) => {
                collect_hist_violations_from_stmts(&node.body, posonly_map, out);
                collect_hist_violations_from_stmts(&node.orelse, posonly_map, out);
            }
            Stmt::With(node) => {
                collect_hist_violations_from_stmts(&node.body, posonly_map, out);
            }
            Stmt::Try(node) => {
                collect_hist_violations_from_stmts(&node.body, posonly_map, out);
                for handler in &node.handlers {
                    let ExceptHandler::ExceptHandler(eh) = handler;
                    collect_hist_violations_from_stmts(&eh.body, posonly_map, out);
                }
                collect_hist_violations_from_stmts(&node.orelse, posonly_map, out);
                collect_hist_violations_from_stmts(&node.finalbody, posonly_map, out);
            }
            _ => {}
        }
    }
}

fn check_func_for_hist_posonly_violation(
    func: &StmtFunctionDef,
    out: &mut Vec<HistoricalPositionalViolation>,
) {
    let params = &func.parameters;
    if !params.posonlyargs.is_empty() {
        return;
    }
    let mut seen_keyword_param = false;
    for (i, param) in params.args.iter().enumerate() {
        let name = param.parameter.name.as_str();
        if i == 0 && (name == "self" || name == "cls") {
            continue;
        }
        if is_historical_posonly_name(name) {
            if seen_keyword_param {
                out.push(HistoricalPositionalViolation {
                    kind: HistoricalPositionalViolationKind::PositionalOnlyAfterKeyword,
                    span: text_range_to_span(param.parameter.name.range()),
                    name: name.to_owned(),
                });
            }
        } else {
            seen_keyword_param = true;
        }
    }
}

/// Extract the simple function/method name from a call expression's function part.
fn call_func_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(name) => Some(name.id.as_str()),
        Expr::Attribute(attr) => Some(attr.attr.as_str()),
        _ => None,
    }
}

fn collect_hist_violations_from_expr(
    expr: &Expr,
    posonly_map: &std::collections::HashMap<String, std::collections::HashSet<String>>,
    out: &mut Vec<HistoricalPositionalViolation>,
) {
    let Expr::Call(call) = expr else { return };

    let func_name = call_func_name(&call.func);

    for kw in &call.arguments.keywords {
        if let Some(arg_name) = &kw.arg {
            let name_str = arg_name.as_str();
            if is_historical_posonly_name(name_str) {
                // Only flag if we can confirm this param is positional-only in the callee.
                let is_violation = func_name.is_some_and(|fname| {
                    posonly_map
                        .get(fname)
                        .is_some_and(|params| params.contains(name_str))
                });
                if is_violation {
                    out.push(HistoricalPositionalViolation {
                        kind: HistoricalPositionalViolationKind::KeywordPassedToPositionalOnly,
                        span: text_range_to_span(kw.range()),
                        name: name_str.to_owned(),
                    });
                }
            }
        }
    }
    for arg in &call.arguments.args {
        collect_hist_violations_from_expr(arg, posonly_map, out);
    }
    collect_hist_violations_from_expr(&call.func, posonly_map, out);
}

// ---------------------------------------------------------------------------

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
                let is_init_var = annotation_is_init_var(&ann.annotation);
                let field_kw_only = ann.value.as_deref().and_then(field_kw_only_override);
                // Determine kw_only: explicit field() override wins; then sentinel; then class default.
                let is_kw_only =
                    field_kw_only.unwrap_or(after_kw_only_sentinel || class_kw_only);
                let is_init_false = ann.value.as_deref().is_some_and(field_init_is_false);
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
                    is_init_false,
                    is_init_var,
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
                            is_init_false: false,
                            is_init_var: false,
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

    let metaclass_name: Option<String> = class
        .arguments
        .as_ref()
        .and_then(|args| {
            args.keywords
                .iter()
                .find(|kw| kw.arg.as_ref().is_some_and(|a| a.as_str() == "metaclass"))
                .and_then(|kw| expr_simple_name(&kw.value))
        });

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
        metaclass_name,
    }
}

// ---------------------------------------------------------------------------
// dataclass_transform post-processing
// ---------------------------------------------------------------------------

/// A `@dataclass_transform` factory detected at module level.
#[allow(dead_code)]
struct DcTransformFactory {
    /// The function name decorated with `@dataclass_transform(...)`.
    name: String,
    /// `kw_only_default` from the decorator (default `false`).
    kw_only_default: bool,
    /// Field specifier function names extracted from `field_specifiers=(...)`.
    field_specifier_names: Vec<String>,
}

/// Overload parameter info for one definition of a field specifier function.
#[allow(dead_code)]
struct FieldSpecOverload {
    /// Names of keyword-only parameters that do NOT have defaults (required).
    required_kwargs: Vec<String>,
    /// Default value of `init` in this overload, if present.
    init_default: Option<bool>,
    /// Default value of `kw_only` in this overload, if present.
    kw_only_default: Option<bool>,
}

/// Scan module-level statements for `@dataclass_transform(...)` decorated functions
/// and apply their semantics to classes decorated by those factories.
#[allow(dead_code)]
fn apply_dataclass_transform(
    stmts: &[Stmt],
    classes: &mut [ClassInfo],
    functions: &[FunctionInfo],
) {
    let factories = collect_dc_transform_factories(stmts);

    if factories.is_empty() {
        return;
    }
    let mut specifier_overloads: std::collections::HashMap<&str, Vec<FieldSpecOverload>> =
        std::collections::HashMap::new();
    for factory in &factories {
        for spec_name in &factory.field_specifier_names {
            if specifier_overloads.contains_key(spec_name.as_str()) {
                continue;
            }
            let overloads = build_field_specifier_overloads(stmts, spec_name, functions);
            specifier_overloads.insert(spec_name.as_str(), overloads);
        }
    }

    for cls in classes.iter_mut() {
        let Some(factory) = find_matching_factory(stmts, &cls.name, &factories) else {
            continue;
        };
        cls.is_dataclass = true;
        cls.is_dataclass_kw_only = factory.kw_only_default;

        if let Some(class_def) = find_class_def(stmts, &cls.name) {
            resolve_transform_field_attrs(
                class_def,
                &mut cls.attributes,
                &factory.field_specifier_names,
                &specifier_overloads,
                factory.kw_only_default,
            );
        }
    }
}

/// Collect `@dataclass_transform(...)` decorated functions at module level.
#[allow(dead_code)]
fn collect_dc_transform_factories(stmts: &[Stmt]) -> Vec<DcTransformFactory> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::FunctionDef(func) = stmt else {
            continue;
        };
        for dec in &func.decorator_list {
            let (is_dc_transform, kw_only_default, field_specifier_names) =
                parse_dataclass_transform_decorator(&dec.expression);
            if is_dc_transform {
                out.push(DcTransformFactory {
                    name: func.name.to_string(),
                    kw_only_default,
                    field_specifier_names,
                });
            }
        }
    }
    out
}

/// Parse a `@dataclass_transform(...)` expression.
///
/// Returns `(is_dc_transform, kw_only_default, field_specifier_names)`.
fn parse_dataclass_transform_decorator(expr: &Expr) -> (bool, bool, Vec<String>) {
    let Expr::Call(call) = expr else {
        if let Expr::Name(n) = expr {
            if n.id.as_str() == "dataclass_transform" {
                return (true, false, Vec::new());
            }
        }
        return (false, false, Vec::new());
    };
    let is_dc = match call.func.as_ref() {
        Expr::Name(n) => n.id.as_str() == "dataclass_transform",
        Expr::Attribute(a) => a.attr.as_str() == "dataclass_transform",
        _ => false,
    };
    if !is_dc {
        return (false, false, Vec::new());
    }

    let mut kw_only_default = false;
    let mut field_specifier_names = Vec::new();

    for kw in &call.arguments.keywords {
        let Some(arg_name) = kw.arg.as_ref() else {
            continue;
        };
        match arg_name.as_str() {
            "kw_only_default" => {
                kw_only_default = matches!(&kw.value, Expr::BooleanLiteral(b) if b.value);
            }
            "field_specifiers" => {
                if let Expr::Tuple(tup) = &kw.value {
                    for elt in &tup.elts {
                        if let Some(name) = expr_simple_name(elt) {
                            field_specifier_names.push(name);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    (true, kw_only_default, field_specifier_names)
}

/// Build overload info for a field specifier function.
fn build_field_specifier_overloads(
    stmts: &[Stmt],
    spec_name: &str,
    functions: &[FunctionInfo],
) -> Vec<FieldSpecOverload> {
    let mut overloads = Vec::new();

    let has_overloads = functions.iter().any(|f| {
        f.name == spec_name
            && f.class_name.is_none()
            && f.decorators.iter().any(|d| d == "overload")
    });

    for stmt in stmts {
        let Stmt::FunctionDef(func) = stmt else {
            continue;
        };
        if func.name.as_str() != spec_name {
            continue;
        }

        let is_overload = func.decorator_list.iter().any(|d| {
            matches!(decorator_name(d), Some(n) if n == "overload")
        });

        if has_overloads && !is_overload {
            continue;
        }

        let params = &func.parameters;
        let mut required_kwargs = Vec::new();
        let mut init_default = None;
        let mut kw_only_default = None;

        for pwd in &params.kwonlyargs {
            let param_name = pwd.parameter.name.as_str();
            let has_default = pwd.default.is_some();

            if param_name == "init" {
                if let Some(default_expr) = &pwd.default {
                    init_default = Some(
                        matches!(default_expr.as_ref(), Expr::BooleanLiteral(b) if b.value),
                    );
                }
            } else if param_name == "kw_only" {
                if let Some(default_expr) = &pwd.default {
                    kw_only_default = Some(
                        matches!(default_expr.as_ref(), Expr::BooleanLiteral(b) if b.value),
                    );
                }
            }

            if !has_default && param_name != "init" && param_name != "kw_only" {
                required_kwargs.push(param_name.to_string());
            }
        }

        for pwd in params.posonlyargs.iter().chain(params.args.iter()) {
            let param_name = pwd.parameter.name.as_str();
            let has_default = pwd.default.is_some();

            if param_name == "init" {
                if let Some(default_expr) = &pwd.default {
                    init_default = Some(
                        matches!(default_expr.as_ref(), Expr::BooleanLiteral(b) if b.value),
                    );
                }
            } else if param_name == "kw_only" {
                if let Some(default_expr) = &pwd.default {
                    kw_only_default = Some(
                        matches!(default_expr.as_ref(), Expr::BooleanLiteral(b) if b.value),
                    );
                }
            }

            if !has_default && param_name != "init" && param_name != "kw_only" {
                required_kwargs.push(param_name.to_string());
            }
        }

        overloads.push(FieldSpecOverload {
            required_kwargs,
            init_default,
            kw_only_default,
        });
    }

    overloads
}

/// Find which factory decorates a class, if any.
fn find_matching_factory<'a>(
    stmts: &[Stmt],
    class_name: &str,
    factories: &'a [DcTransformFactory],
) -> Option<&'a DcTransformFactory> {
    let class_def = find_class_def(stmts, class_name)?;
    for dec in &class_def.decorator_list {
        let callee = match &dec.expression {
            Expr::Call(call) => match call.func.as_ref() {
                Expr::Name(n) => n.id.as_str(),
                Expr::Attribute(a) => a.attr.as_str(),
                _ => continue,
            },
            Expr::Name(n) => n.id.as_str(),
            _ => continue,
        };
        for factory in factories {
            if factory.name == callee {
                return Some(factory);
            }
        }
    }
    None
}

/// Find a class definition by name in the top-level statements.
fn find_class_def<'a>(stmts: &'a [Stmt], name: &str) -> Option<&'a StmtClassDef> {
    for stmt in stmts {
        if let Stmt::ClassDef(cls) = stmt {
            if cls.name.as_str() == name {
                return Some(cls);
            }
        }
    }
    None
}

/// Resolve `is_init_false` and `is_kw_only` for attributes of a `dataclass_transform` class.
fn resolve_transform_field_attrs(
    class_def: &StmtClassDef,
    attributes: &mut [AttributeInfo],
    field_specifier_names: &[String],
    specifier_overloads: &std::collections::HashMap<&str, Vec<FieldSpecOverload>>,
    kw_only_default: bool,
) {
    let mut attr_idx = 0;
    for stmt in &class_def.body {
        let Stmt::AnnAssign(ann) = stmt else {
            continue;
        };
        let Some(attr_name) = expr_simple_name(&ann.target) else {
            continue;
        };
        if attr_name == "_" && annotation_is_kw_only(&ann.annotation) {
            continue;
        }

        let Some(attr) = attributes.get_mut(attr_idx) else {
            break;
        };
        if attr.name != attr_name {
            attr_idx += 1;
            continue;
        }
        attr_idx += 1;

        let Some(value_expr) = ann.value.as_deref() else {
            if kw_only_default {
                attr.is_kw_only = true;
            }
            continue;
        };

        let Expr::Call(call) = value_expr else {
            continue;
        };

        let callee_name = match call.func.as_ref() {
            Expr::Name(n) => n.id.as_str(),
            Expr::Attribute(a) => a.attr.as_str(),
            _ => continue,
        };

        if !field_specifier_names.iter().any(|n| n == callee_name) {
            continue;
        }

        let Some(overloads) = specifier_overloads.get(callee_name) else {
            continue;
        };

        let call_kwargs: Vec<&str> = call
            .arguments
            .keywords
            .iter()
            .filter_map(|kw| kw.arg.as_ref().map(ruff_python_ast::Identifier::as_str))
            .collect();

        let explicit_init: Option<bool> = call.arguments.keywords.iter().find_map(|kw| {
            if kw.arg.as_ref().is_some_and(|a| a.as_str() == "init") {
                Some(matches!(&kw.value, Expr::BooleanLiteral(b) if b.value))
            } else {
                None
            }
        });

        let explicit_kw_only: Option<bool> = call.arguments.keywords.iter().find_map(|kw| {
            if kw.arg.as_ref().is_some_and(|a| a.as_str() == "kw_only") {
                Some(matches!(&kw.value, Expr::BooleanLiteral(b) if b.value))
            } else {
                None
            }
        });

        let mut matched_init: Option<bool> = None;
        let mut matched_kw_only: Option<bool> = None;

        for overload in overloads {
            let all_required_present = overload
                .required_kwargs
                .iter()
                .all(|req| call_kwargs.contains(&req.as_str()));
            if all_required_present {
                matched_init = overload.init_default;
                matched_kw_only = overload.kw_only_default;
                break;
            }
        }

        let effective_init = explicit_init.or(matched_init);
        let effective_kw_only = explicit_kw_only.or(matched_kw_only);

        if effective_init == Some(false) {
            attr.is_init_false = true;
        }
        attr.is_kw_only = effective_kw_only.unwrap_or(kw_only_default);
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
        Expr::NumberLiteral(n) => match n.value {
            ruff_python_ast::Number::Float(_) => RhsKind::FloatLiteral,
            ruff_python_ast::Number::Complex { .. } => RhsKind::Other,
            ruff_python_ast::Number::Int(_) => RhsKind::IntLiteral,
        },
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

/// Returns `true` when the annotation expression is `InitVar[T]`.
///
/// Matches both `InitVar[T]` and `dataclasses.InitVar[T]`.
fn annotation_is_init_var(ann: &Expr) -> bool {
    match ann {
        Expr::Subscript(sub) => {
            // Check if the base is "InitVar" or "dataclasses.InitVar"
            match sub.value.as_ref() {
                Expr::Name(n) => n.id.as_str() == "InitVar",
                Expr::Attribute(attr) => attr.attr.as_str() == "InitVar",
                _ => false,
            }
        }
        _ => false,
    }
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

/// For a field value expression, returns `Some(true)` when it is `field(init=True, ...)`
/// or a field specifier with `init=True`, `Some(false)` when `init=False`, and `None` otherwise.
#[allow(dead_code)]
fn field_init_override(value: &Expr, field_specifier_names: &[&str]) -> Option<bool> {
    let Expr::Call(call) = value else { return None };
    let is_field_call = match call.func.as_ref() {
        Expr::Name(n) => {
            n.id.as_str() == "field"
                || field_specifier_names.contains(&n.id.as_str())
        }
        Expr::Attribute(a) => a.attr.as_str() == "field",
        _ => false,
    };
    if !is_field_call {
        return None;
    }
    for kw in &call.arguments.keywords {
        if kw
            .arg
            .as_ref()
            .map(ruff_python_ast::Identifier::as_str)
            == Some("init")
        {
            return Some(matches!(&kw.value, Expr::BooleanLiteral(b) if b.value));
        }
    }
    None
}

/// Returns `true` when the value expression is a `field(init=False, ...)` call.
///
/// Only checks calls to the standard `dataclasses.field` function.  Field specifier
/// calls from `@dataclass_transform` are resolved in `apply_dataclass_transform`.
fn field_init_is_false(value: &Expr) -> bool {
    let Expr::Call(call) = value else { return false };
    let is_field_call = match call.func.as_ref() {
        Expr::Name(n) => n.id.as_str() == "field",
        Expr::Attribute(a) => a.attr.as_str() == "field",
        _ => false,
    };
    if !is_field_call {
        return false;
    }
    call.arguments.keywords.iter().any(|kw| {
        kw.arg
            .as_ref()
            .is_some_and(|a| a.as_str() == "init")
            && matches!(&kw.value, Expr::BooleanLiteral(b) if !b.value)
    })
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

/// Collect functional-form `NamedTuple` definitions from module-level code.
///
/// Matches assignments of the form:
/// ```python
/// N = NamedTuple("N", [(field_name, field_type), ...])
/// ```
///
/// Field names that reference `Final` string-literal constants are resolved to
/// the constant's value (e.g. `X: Final = "x"` makes `X` resolve to `"x"`).
fn collect_namedtuple_defs(stmts: &[Stmt], source: &str) -> Vec<NamedTupleDefInfo> {
    // First, build a map of Final string-literal constants for field-name resolution.
    let final_string_constants: std::collections::HashMap<&str, &str> =
        collect_final_string_constants(stmts, source);

    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::Assign(node) = stmt else { continue };
        let Expr::Call(call) = node.value.as_ref() else {
            continue;
        };
        // Callee must be `NamedTuple` or `typing.NamedTuple`.
        let is_namedtuple = expr_simple_name(&call.func).as_deref() == Some("NamedTuple")
            || matches!(call.func.as_ref(), Expr::Attribute(a) if a.attr.as_str() == "NamedTuple");
        if !is_namedtuple {
            continue;
        }
        let Some(lhs_name) = node.targets.first().and_then(expr_simple_name) else {
            continue;
        };
        // Second positional argument should be a list of (name, type) tuples.
        let Some(fields_arg) = call.arguments.args.get(1) else {
            continue;
        };
        let Expr::List(list_expr) = fields_arg else {
            continue;
        };
        let mut field_names = Vec::new();
        let mut field_types = Vec::new();
        for elt in &list_expr.elts {
            let Expr::Tuple(tuple_expr) = elt else {
                continue;
            };
            if tuple_expr.elts.len() < 2 {
                continue;
            }
            // Field name: string literal or Final constant reference.
            let field_name = match &tuple_expr.elts[0] {
                Expr::StringLiteral(s) => s.value.to_str().to_owned(),
                Expr::Name(n) => {
                    if let Some(resolved) = final_string_constants.get(n.id.as_str()) {
                        (*resolved).to_owned()
                    } else {
                        n.id.to_string()
                    }
                }
                _ => continue,
            };
            // Field type: extract source text.
            let type_range = tuple_expr.elts[1].range();
            let field_type = source
                .get(type_range.start().to_u32() as usize..type_range.end().to_u32() as usize)
                .unwrap_or("")
                .to_owned();
            field_names.push(field_name);
            field_types.push(field_type);
        }
        if !field_names.is_empty() {
            out.push(NamedTupleDefInfo {
                lhs_name,
                field_names,
                field_types,
                span: text_range_to_span(call.range()),
            });
        }
    }
    out
}

/// Collect `Final` string-literal constant bindings from module-level statements.
///
/// Returns a map from variable name to the string value (e.g., `X: Final = "x"` yields
/// `"X" -> "x"`).  Only `Final` / `Final[str]` annotations with string-literal RHS
/// are included.
#[allow(dead_code)]
fn collect_final_string_constants<'a>(
    stmts: &'a [Stmt],
    source: &'a str,
) -> std::collections::HashMap<&'a str, &'a str> {
    let mut map = std::collections::HashMap::new();
    for stmt in stmts {
        let Stmt::AnnAssign(ann) = stmt else { continue };
        let Expr::Name(n) = ann.target.as_ref() else {
            continue;
        };
        let range = ann.annotation.range();
        let Some(ann_text) = source.get(range.start().to_u32() as usize..range.end().to_u32() as usize) else {
            continue;
        };
        if !ann_text_is_final(ann_text) {
            continue;
        }
        // RHS must be a string literal.
        let Some(val) = ann.value.as_deref() else {
            continue;
        };
        let Expr::StringLiteral(s) = val else { continue };
        map.insert(n.id.as_str(), s.value.to_str());
    }
    map
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
// PEP 695 type parameter bound violation detection (for BSK-E0087)
// ---------------------------------------------------------------------------

/// Walk all statements and collect PEP 695 type parameter bound violations.
///
/// Detects invalid `TypeVar` bound/constraint forms in PEP 695 `class Foo[T: ...]` syntax:
/// - List literal as bound: `class Foo[T: [str, int]]`
/// - Empty constraint tuple: `class Foo[T: ()]`
/// - Single-element constraint tuple: `class Foo[T: (str,)]`
/// - Non-literal (variable) constraint: `class Foo[T: t1]` where `t1 = (str, bytes)`
/// - Invalid constraint element: `class Foo[T: (3, bytes)]` (integer literal in tuple)
fn collect_pep695_bound_violations(stmts: &[Stmt]) -> Vec<Pep695BoundViolation> {
    let bare_names: std::collections::HashSet<String> = stmts
        .iter()
        .filter_map(|stmt| {
            let Stmt::Assign(node) = stmt else {
                return None;
            };
            node.targets.first().and_then(expr_simple_name)
        })
        .collect();

    let mut out = Vec::new();
    collect_pep695_violations_from_stmts(
        stmts,
        &bare_names,
        &std::collections::HashSet::new(),
        &mut out,
    );
    out
}

fn collect_pep695_violations_from_stmts(
    stmts: &[Stmt],
    bare_names: &std::collections::HashSet<String>,
    outer_typeparams: &std::collections::HashSet<String>,
    out: &mut Vec<Pep695BoundViolation>,
) {
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else {
            continue;
        };
        let class_name = cls.name.to_string();

        // Collect the current class's TypeParam names.
        let current_typeparams: std::collections::HashSet<String> = cls
            .type_params
            .as_ref()
            .map(|tp| {
                tp.type_params
                    .iter()
                    .map(type_param_name)
                    .collect()
            })
            .unwrap_or_default();

        if let Some(type_params) = &cls.type_params {
            for tp in &type_params.type_params {
                if let TypeParam::TypeVar(tv) = tp {
                    if let Some(bound) = &tv.bound {
                        check_typevar_bound_expr(
                            bound,
                            &class_name,
                            tv.name.as_str(),
                            bare_names,
                            &current_typeparams,
                            outer_typeparams,
                            out,
                        );
                    }
                }
            }
        }

        // When recursing into nested classes, the current TypeParams become outer TypeParams.
        let mut new_outer = outer_typeparams.clone();
        new_outer.extend(current_typeparams);
        collect_pep695_violations_from_stmts(&cls.body, bare_names, &new_outer, out);
    }
}

fn check_typevar_bound_expr(
    bound: &Expr,
    class_name: &str,
    type_param: &str,
    bare_names: &std::collections::HashSet<String>,
    current_typeparams: &std::collections::HashSet<String>,
    outer_typeparams: &std::collections::HashSet<String>,
    out: &mut Vec<Pep695BoundViolation>,
) {
    match bound {
        Expr::List(list) => {
            out.push(Pep695BoundViolation {
                kind: Pep695BoundViolationKind::ListLiteralBound,
                class_name: class_name.to_owned(),
                type_param_name: type_param.to_owned(),
                span: text_range_to_span(list.range()),
            });
        }
        Expr::Tuple(tup) => {
            if tup.elts.is_empty() {
                out.push(Pep695BoundViolation {
                    kind: Pep695BoundViolationKind::EmptyTuple,
                    class_name: class_name.to_owned(),
                    type_param_name: type_param.to_owned(),
                    span: text_range_to_span(tup.range()),
                });
            } else if tup.elts.len() == 1 {
                out.push(Pep695BoundViolation {
                    kind: Pep695BoundViolationKind::SingleElementTuple,
                    class_name: class_name.to_owned(),
                    type_param_name: type_param.to_owned(),
                    span: text_range_to_span(tup.range()),
                });
            } else {
                // Check for invalid elements and outer-scope TypeVar references.
                let mut emitted = false;
                for elt in &tup.elts {
                    if !is_valid_constraint_element(elt) {
                        out.push(Pep695BoundViolation {
                            kind: Pep695BoundViolationKind::InvalidConstraintElement,
                            class_name: class_name.to_owned(),
                            type_param_name: type_param.to_owned(),
                            span: text_range_to_span(elt.range()),
                        });
                        emitted = true;
                        break;
                    }
                }
                if !emitted {
                    for elt in &tup.elts {
                        if bound_refs_outer_typeparam(elt, current_typeparams, outer_typeparams) {
                            out.push(Pep695BoundViolation {
                                kind: Pep695BoundViolationKind::OuterScopeTypeVarInBound,
                                class_name: class_name.to_owned(),
                                type_param_name: type_param.to_owned(),
                                span: text_range_to_span(elt.range()),
                            });
                            break;
                        }
                    }
                }
            }
        }
        Expr::Name(name) if bare_names.contains(name.id.as_str()) => {
            out.push(Pep695BoundViolation {
                kind: Pep695BoundViolationKind::NonLiteralConstraint,
                class_name: class_name.to_owned(),
                type_param_name: type_param.to_owned(),
                span: text_range_to_span(name.range()),
            });
        }
        // Check if the bound itself references an outer-scope TypeVar (e.g. `T: dict[str, V]`).
        bound_expr if bound_refs_outer_typeparam(bound_expr, current_typeparams, outer_typeparams) => {
            out.push(Pep695BoundViolation {
                kind: Pep695BoundViolationKind::OuterScopeTypeVarInBound,
                class_name: class_name.to_owned(),
                type_param_name: type_param.to_owned(),
                span: text_range_to_span(bound_expr.range()),
            });
        }
        _ => {}
    }
}

/// Returns `true` if the expression references an outer-scope `TypeParam` or a
/// TypeVar-like name that is not in the current class's `TypeParam` set.
///
/// Used to detect cases like `class Nested[T: dict[str, V]]` where `V` is from
/// an outer class, or `class Foo[T: (list[S], str)]` where `S` is unresolved.
fn bound_refs_outer_typeparam(
    expr: &Expr,
    current_typeparams: &std::collections::HashSet<String>,
    outer_typeparams: &std::collections::HashSet<String>,
) -> bool {
    match expr {
        Expr::Name(name) => {
            let n = name.id.as_str();
            // Explicitly an outer TypeVar, or a TypeVar-like single-letter uppercase name
            // not in the current class's TypeParam set.
            outer_typeparams.contains(n)
                || (is_typevar_like_name(n) && !current_typeparams.contains(n))
        }
        Expr::Subscript(sub) => {
            // Check the type arguments of a generic type expression, not the base type.
            // e.g. for `list[S]`, we check `S` not `list`.
            bound_refs_outer_typeparam(&sub.slice, current_typeparams, outer_typeparams)
        }
        Expr::Tuple(t) => t
            .elts
            .iter()
            .any(|e| bound_refs_outer_typeparam(e, current_typeparams, outer_typeparams)),
        Expr::BinOp(bin) => {
            bound_refs_outer_typeparam(&bin.left, current_typeparams, outer_typeparams)
                || bound_refs_outer_typeparam(&bin.right, current_typeparams, outer_typeparams)
        }
        _ => false,
    }
}

/// Returns `true` if the name looks like a `TypeVar` by the single-letter uppercase convention.
///
/// Single-letter uppercase names (e.g. `T`, `S`, `V`) are almost universally `TypeVars`.
/// Multi-letter names could be concrete types (e.g. `str`, `int`, `ForwardReference`).
fn is_typevar_like_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() == 1 && bytes[0].is_ascii_uppercase()
}

/// Returns `false` if this expression is not a valid constraint tuple element.
///
/// Valid elements are type expressions: names, subscripts, binary ops, string
/// literals (forward references), etc.
/// Invalid elements include numeric and bytes literals (not types).
fn is_valid_constraint_element(expr: &Expr) -> bool {
    !matches!(expr, Expr::NumberLiteral(_) | Expr::BytesLiteral(_))
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

/// Returns `true` if the annotation expression is a `tuple[...]` with an invalid form.
///
/// Invalid forms include:
/// - Multiple unbounded components: `tuple[*tuple[T, ...], *Ts]`
/// - Bare ellipsis as the only element: `tuple[...]`
/// - Ellipsis not at the second position (with exactly one preceding type): `tuple[..., int]`,
///   `tuple[int, ..., int]`
/// - More than one non-ellipsis type before the ellipsis: `tuple[int, int, ...]`
/// - Non-variadic starred expression paired with ellipsis: `tuple[*tuple[str], ...]`
fn annotation_has_multiple_unbounded(expr: &Expr) -> bool {
    let Expr::Subscript(sub) = expr else {
        return false;
    };
    if expr_simple_name(&sub.value).as_deref() != Some("tuple") {
        return false;
    }
    // Check for multiple unbounded components (original rule)
    if count_unbounded_in_tuple_slice(&sub.slice) >= 2 {
        return true;
    }
    // Check for invalid ellipsis forms
    tuple_slice_has_invalid_ellipsis(&sub.slice)
}

/// Returns `true` when a `tuple[...]` slice has an invalid ellipsis placement.
///
/// Valid: `tuple[T, ...]` — exactly two elements, first is a type, second is `...`
///   (and the first must not be a non-variadic starred expression)
/// Everything else with a bare `...` is invalid.
fn tuple_slice_has_invalid_ellipsis(slice: &Expr) -> bool {
    match slice {
        // Single `...` element: `tuple[...]` — invalid
        Expr::EllipsisLiteral(_) => true,
        Expr::Tuple(t) => {
            let elts = &t.elts;
            // Find all bare EllipsisLiteral positions
            let ellipsis_count = elts
                .iter()
                .filter(|e| matches!(e, Expr::EllipsisLiteral(_)))
                .count();
            if ellipsis_count == 0 {
                return false; // No bare ellipsis — nothing to validate here
            }
            // Valid form: exactly 2 elements, first is NOT ellipsis, second IS ellipsis
            if elts.len() == 2 && matches!(elts[1], Expr::EllipsisLiteral(_)) {
                // `tuple[T, ...]` is valid only if T is not itself a starred expression.
                // Both `tuple[*tuple[str], ...]` (non-variadic) and
                // `tuple[*tuple[str, ...], ...]` (variadic) are invalid.
                return matches!(elts[0], Expr::Starred(_));
            }
            // Any other placement of bare `...` is invalid:
            // - More than one ellipsis
            // - Ellipsis not at position 1 (e.g. `tuple[..., int]`)
            // - More than 2 elements with ellipsis at end (e.g. `tuple[int, int, ...]`)
            true
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
    // Skip when the actual type is a user-defined type alias that we cannot expand
    // without a full type engine — comparing alias names to their expansions produces
    // false positives (e.g. `GoodTypeAlias1` != `int | str` even though they're equal).
    let type_mismatch = match (&actual_type, &expected_type) {
        (Some(actual), Some(expected)) => {
            !types_match(actual, expected) && !is_user_defined_type_alias(actual)
        }
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
fn resolve_actual_type(expr: &Expr, params: &[(&str, &str)], _source: &str) -> Option<String> {
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
        // Complex expressions (attribute access, subscripts, calls, binary ops, etc.)
        // cannot be typed without full type inference — returning source text produces
        // false positives when compared against expected types textually.
        _ => None,
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

/// Returns `true` when `actual` type is a user-defined type alias that cannot be
/// resolved without a full type engine. These produce false positives because we
/// compare annotation text without expanding aliases.
///
/// A type is a user-defined alias when its base identifier (the part before `[`)
/// starts with an uppercase letter and is not a known typing special form.
fn is_user_defined_type_alias(ty: &str) -> bool {
    const KNOWN_FORMS: &[&str] = &[
        "Any",
        "Never",
        "NoReturn",
        "Self",
        "LiteralString",
        "TypeGuard",
        "TypeIs",
        "Literal",
        "Optional",
        "Union",
        "Annotated",
        "Callable",
        "ClassVar",
        "Final",
        "Protocol",
        "TypedDict",
        "NamedTuple",
        "Generic",
        "Tuple",
        "List",
        "Dict",
        "Set",
        "FrozenSet",
        "Type",
        "Deque",
        "None",
        "Awaitable",
        "Coroutine",
        "AsyncGenerator",
        "Generator",
        "Iterator",
        "Iterable",
        "Sequence",
        "Mapping",
        "MutableMapping",
        "MutableSequence",
        "MutableSet",
        "ChainMap",
        "Counter",
        "DefaultDict",
        "OrderedDict",
        "Concatenate",
        "ParamSpec",
        "ParamSpecArgs",
        "ParamSpecKwargs",
        "TypeVar",
        "TypeVarTuple",
        "Unpack",
        "Required",
        "NotRequired",
        "ReadOnly",
        "TypeAlias",
        "SupportsInt",
        "SupportsFloat",
        "SupportsComplex",
        "SupportsBytes",
        "SupportsAbs",
        "SupportsRound",
        "AbstractSet",
        "ByteString",
        "IO",
        "TextIO",
        "BinaryIO",
        "Pattern",
        "Match",
        "AnyStr",
        "Text",
        "ContextManager",
        "AsyncContextManager",
        "Hashable",
        "Sized",
        "Reversible",
        "Collection",
        "Container",
        "ItemsView",
        "KeysView",
        "ValuesView",
        "AbstractContextManager",
    ];

    let base = ty.trim().split('[').next().unwrap_or(ty.trim()).trim();
    if base.is_empty()
        || base.contains('|')
        || base.contains(' ')
        || base.contains(',')
        || base.contains('(')
        || base.contains(')')
    {
        return false;
    }
    let Some(first) = base.chars().next() else {
        return false;
    };
    if !first.is_ascii_uppercase() {
        return false;
    }
    !KNOWN_FORMS.contains(&base)
}

/// Returns `true` when `actual` and `expected` are equivalent types (textual comparison).
///
/// Handles common equivalences:
/// - Direct string equality
/// - Bare generic vs `Generic[Any]` (e.g. `list` == `list[Any]`)
/// - `type` == `type[Any]`
/// - Quoted forward references: `"ClassA"` == `ClassA`
fn types_match(actual: &str, expected: &str) -> bool {
    let actual = actual.trim();
    let expected = expected.trim();

    if actual == expected {
        return true;
    }

    // Handle quoted forward references: `"ClassA"` == `ClassA`
    let actual_unquoted = if (actual.starts_with('"') && actual.ends_with('"'))
        || (actual.starts_with('\'') && actual.ends_with('\''))
    {
        &actual[1..actual.len() - 1]
    } else {
        actual
    };
    if actual_unquoted != actual && actual_unquoted == expected {
        return true;
    }

    // Handle bare generic == specialized with All-Any params.
    // e.g. `list` == `list[Any]`, `type` == `type[Any]`, `dict` == `dict[Any, Any]`
    if !actual.contains('[') && !actual.contains('|') {
        if let Some(rest) = expected.strip_prefix(actual) {
            if let Some(inner) = rest.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                let all_any = inner
                    .split(',')
                    .all(|p| matches!(p.trim(), "Any" | "..." | "*tuple[Any, ...]"));
                if all_any {
                    return true;
                }
            }
        }
    }

    false
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

/// Collect `TypedDict` key/value violations from module-level statements.
///
/// Detects:
/// - Subscript assignments with invalid keys: `movie["director"] = "Ridley Scott"`
/// - Subscript assignments with wrong value type: `movie["year"] = "1982"`
/// - Annotated dict literal assignments with invalid or missing keys: `movie2: Movie = {"title": ...}`
fn collect_typeddict_key_violations<'a>(
    stmts: &[Stmt],
    classes: &'a [ClassInfo],
    source: &'a str,
) -> Vec<TypedDictKeyViolation> {
    use std::collections::HashMap;
    type FieldMap<'x> = HashMap<&'x str, (Vec<&'x str>, HashMap<&'x str, String>)>;

    let typeddict_fields: FieldMap<'a> = classes
        .iter()
        .filter(|c| c.is_typed_dict)
        .map(|c| {
            let all_fields: Vec<&str> = c.attributes.iter().map(|a| a.name.as_str()).collect();
            let field_types: HashMap<&str, String> = c
                .attributes
                .iter()
                .filter_map(|a| {
                    let span = a.annotation_span?;
                    let type_text = source.get(span.start as usize..span.end as usize)?.trim().to_owned();
                    Some((a.name.as_str(), type_text))
                })
                .collect();
            (c.name.as_str(), (all_fields, field_types))
        })
        .collect();

    if typeddict_fields.is_empty() {
        return Vec::new();
    }

    let var_type: HashMap<String, &str> = stmts
        .iter()
        .filter_map(|s| {
            let Stmt::AnnAssign(ann) = s else { return None };
            let var_name = expr_simple_name(&ann.target)?;
            let Expr::Name(type_name) = ann.annotation.as_ref() else { return None };
            let class_name = type_name.id.as_str();
            typeddict_fields.contains_key(class_name).then_some((var_name, class_name))
        })
        .collect();

    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::Assign(node) => check_subscript_violations(node, &var_type, &typeddict_fields, &mut out),
            Stmt::AnnAssign(node) => check_ann_assign_violations(node, &typeddict_fields, &mut out),
            _ => {}
        }
    }
    out
}

fn check_subscript_violations<'a>(
    node: &StmtAssign,
    var_type: &std::collections::HashMap<String, &'a str>,
    typeddict_fields: &std::collections::HashMap<&'a str, (Vec<&'a str>, std::collections::HashMap<&'a str, String>)>,
    out: &mut Vec<TypedDictKeyViolation>,
) {
    use ruff_text_size::Ranged as _;
    for target in &node.targets {
        let Expr::Subscript(sub) = target else { continue };
        let Some(var_name) = expr_simple_name(&sub.value) else { continue };
        let Some(&class_name) = var_type.get(&var_name) else { continue };
        let Some((all_fields, field_types)) = typeddict_fields.get(class_name) else { continue };
        let Expr::StringLiteral(key_str) = sub.slice.as_ref() else { continue };
        let key = key_str.value.to_string();
        if !all_fields.contains(&key.as_str()) {
            out.push(TypedDictKeyViolation {
                span: text_range_to_span(node.range()),
                class_name: class_name.to_owned(),
                kind: TypedDictKeyViolationKind::InvalidSubscriptKey { key },
            });
        } else if let Some(expected) = field_types.get(key.as_str()) {
            if let Some(actual) = expr_literal_type_name(&node.value) {
                if !typeddict_field_type_compatible(actual, expected) {
                    out.push(TypedDictKeyViolation {
                        span: text_range_to_span(node.range()),
                        class_name: class_name.to_owned(),
                        kind: TypedDictKeyViolationKind::WrongSubscriptValueType {
                            key,
                            expected: expected.clone(),
                        },
                    });
                }
            }
        }
    }
}

fn check_ann_assign_violations<'a>(
    node: &StmtAnnAssign,
    typeddict_fields: &std::collections::HashMap<&'a str, (Vec<&'a str>, std::collections::HashMap<&'a str, String>)>,
    out: &mut Vec<TypedDictKeyViolation>,
) {
    use ruff_text_size::Ranged as _;
    let Some(value) = &node.value else { return };
    let Expr::Name(ann_name) = node.annotation.as_ref() else { return };
    let class_name = ann_name.id.as_str();
    let Some((all_fields, _)) = typeddict_fields.get(class_name) else { return };
    let Expr::Dict(dict) = value.as_ref() else { return };

    let literal_keys: Vec<String> = dict.items.iter().filter_map(|item| {
        let key = item.key.as_ref()?;
        let Expr::StringLiteral(s) = key else { return None };
        Some(s.value.to_string())
    }).collect();

    let invalid_keys: Vec<String> = literal_keys.iter()
        .filter(|k| !all_fields.contains(&k.as_str())).cloned().collect();
    let missing_keys: Vec<String> = all_fields.iter()
        .filter(|&&f| !literal_keys.iter().any(|k| k == f))
        .map(|s| (*s).to_owned()).collect();

    if !invalid_keys.is_empty() || !missing_keys.is_empty() {
        out.push(TypedDictKeyViolation {
            span: text_range_to_span(node.range()),
            class_name: class_name.to_owned(),
            kind: TypedDictKeyViolationKind::InvalidDictLiteral { invalid_keys, missing_keys },
        });
    }
}

/// Return the inferred type name for a literal expression, or `None` if not a literal.
fn expr_literal_type_name(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::StringLiteral(_) | Expr::FString(_) => Some("str"),
        Expr::NumberLiteral(n) => Some(match n.value {
            ruff_python_ast::Number::Float(_) => "float",
            ruff_python_ast::Number::Complex { .. } => "complex",
            ruff_python_ast::Number::Int(_) => "int",
        }),
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::NoneLiteral(_) => Some("None"),
        _ => None,
    }
}

/// Return `true` if an actual literal type is compatible with an expected `TypedDict` field type.
fn typeddict_field_type_compatible(actual: &str, expected: &str) -> bool {
    actual == expected
        || (actual == "bool" && expected == "int")
        || (actual == "int" && expected == "float")
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
        collect_class_final_violations(cls_def, &class_finals, source, &mut out);
    }
    out
}

/// Collect Final violations inside a class definition.
fn collect_class_final_violations(
    cls_def: &StmtClassDef,
    class_finals: &std::collections::HashMap<&str, std::collections::HashSet<&str>>,
    source: &str,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
    use crate::scope::{FinalViolationInfo, FinalViolationKind};

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
                    name: (*attr_name).to_string(),
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

    collect_subclass_override_final(cls_def, &parent_finals, out);

    // Recurse into nested class definitions.
    for body_stmt in &cls_def.body {
        if let Stmt::ClassDef(nested) = body_stmt {
            collect_class_final_violations(nested, class_finals, source, out);
        }
    }
}

/// Detect a child class declaring an attr that is `Final` in a parent.
fn collect_subclass_override_final(
    cls_def: &StmtClassDef,
    parent_finals: &std::collections::HashSet<&str>,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
    use crate::scope::FinalViolationKind;
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
        Stmt::Assign(assign) => {
            for target in &assign.targets {
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
        match stmt {
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    let Expr::Attribute(attr) = target else { continue };
                    let Expr::Name(n) = attr.value.as_ref() else { continue };
                    if n.id == "self" {
                        names.insert(attr.attr.to_string());
                    }
                }
            }
            // An if/else where both branches assign self.X counts as unconditional.
            Stmt::If(if_stmt) if !if_stmt.elif_else_clauses.is_empty() => {
                let has_else = if_stmt
                    .elif_else_clauses
                    .last()
                    .is_some_and(|clause| clause.test.is_none());
                if has_else {
                    let if_assigns = collect_self_assigns_from_stmts(&if_stmt.body);
                    // Intersect with all elif/else branch assigns.
                    let mut common = if_assigns;
                    for clause in &if_stmt.elif_else_clauses {
                        let branch_assigns = collect_self_assigns_from_stmts(&clause.body);
                        common.retain(|name| branch_assigns.contains(name));
                    }
                    names.extend(common);
                }
            }
            _ => {}
        }
    }
    names
}

/// Collect `self.X` assignment targets from a list of statements (non-recursive,
/// only top-level assigns).
fn collect_self_assigns_from_stmts(stmts: &[Stmt]) -> std::collections::HashSet<String> {
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

/// Collect Final violations inside a function body (`GlobalFinalModification` and
/// `FunctionLocalFinalModification`).
fn collect_func_final_violations(
    func: &StmtFunctionDef,
    module_final_names: &std::collections::HashSet<&str>,
    source: &str,
    out: &mut Vec<crate::scope::FinalViolationInfo>,
) {
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
            // Also check for walrus operators in the RHS: `a = (x := 4)`.
            check_walrus_final(&assign.value, global_finals, local_finals, out);
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

// ---------------------------------------------------------------------------
// Protocol Self-return conformance violation detection
// ---------------------------------------------------------------------------

/// Collect protocol `Self`-return conformance violations from function bodies.
///
/// When a function has a parameter typed as a `Protocol` with a `Self`-returning
/// method, and that function is called with an argument whose class's corresponding
/// method does not return `Self` or the class itself, this is a protocol violation.
#[allow(dead_code)]
fn collect_protocol_self_violations(
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
                        source
                            .get(span.start as usize..span.end as usize)
                            .map(str::trim)
                            == Some("Self")
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
                        source
                            .get(span.start as usize..span.end as usize)
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
                        source
                            .get(span.start as usize..span.end as usize)
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
#[allow(dead_code)]
fn collect_protocol_violations_from_stmts(
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
#[allow(dead_code)]
fn check_protocol_violations_in_function(
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
                source
                    .get(range.start().to_u32() as usize..range.end().to_u32() as usize)
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
fn collect_isinstance_typeddict_violations(
    stmts: &[Stmt],
    typeddict_names: &std::collections::HashSet<&str>,
) -> Vec<Span> {
    let mut out = Vec::new();
    collect_isinstance_typeddict_in_stmts(stmts, typeddict_names, &mut out);
    out
}

fn collect_isinstance_typeddict_in_stmts(
    stmts: &[Stmt],
    typeddict_names: &std::collections::HashSet<&str>,
    out: &mut Vec<Span>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::If(node) => {
                collect_isinstance_typeddict_in_expr(&node.test, typeddict_names, out);
                collect_isinstance_typeddict_in_stmts(&node.body, typeddict_names, out);
                for clause in &node.elif_else_clauses {
                    if let Some(test) = &clause.test {
                        collect_isinstance_typeddict_in_expr(test, typeddict_names, out);
                    }
                    collect_isinstance_typeddict_in_stmts(&clause.body, typeddict_names, out);
                }
            }
            Stmt::Expr(node) => {
                collect_isinstance_typeddict_in_expr(&node.value, typeddict_names, out);
            }
            Stmt::Assign(node) => {
                collect_isinstance_typeddict_in_expr(&node.value, typeddict_names, out);
            }
            Stmt::AnnAssign(node) => {
                if let Some(val) = &node.value {
                    collect_isinstance_typeddict_in_expr(val, typeddict_names, out);
                }
            }
            Stmt::While(node) => {
                collect_isinstance_typeddict_in_expr(&node.test, typeddict_names, out);
                collect_isinstance_typeddict_in_stmts(&node.body, typeddict_names, out);
            }
            Stmt::For(node) => {
                collect_isinstance_typeddict_in_stmts(&node.body, typeddict_names, out);
            }
            Stmt::FunctionDef(node) => {
                collect_isinstance_typeddict_in_stmts(&node.body, typeddict_names, out);
            }
            Stmt::ClassDef(node) => {
                collect_isinstance_typeddict_in_stmts(&node.body, typeddict_names, out);
            }
            _ => {}
        }
    }
}

fn collect_isinstance_typeddict_in_expr(
    expr: &Expr,
    typeddict_names: &std::collections::HashSet<&str>,
    out: &mut Vec<Span>,
) {
    use ruff_text_size::Ranged as _;
    let Expr::Call(call) = expr else { return };
    let callee_is_isinstance = matches!(
        call.func.as_ref(),
        Expr::Name(n) if n.id == "isinstance" || n.id == "issubclass"
    );
    if !callee_is_isinstance {
        return;
    }
    let Some(second_arg) = call.arguments.args.get(1) else {
        return;
    };
    if let Expr::Name(name) = second_arg {
        if typeddict_names.contains(name.id.as_str()) {
            let range = call.range();
            out.push(Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            });
        }
    }
}

