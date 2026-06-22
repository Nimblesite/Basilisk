//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Class Info Ext visitor functions.

use ruff_python_ast::{
    Alias, Decorator, Expr, MatchCase, Pattern, Stmt, StmtClassDef, StmtImport, StmtImportFrom,
    StmtMatch,
};
use ruff_text_size::Ranged;

use crate::scope::{
    BaseSubscriptEntry, ClassInfo, FunctionInfo, ImportInfo, ImportKind, ImportResolution,
    MatchStmtInfo, Span, TypeArg,
};

use super::calls_and_reveal::expr_to_type_arg;
use super::class_info::collect_class_body;
use super::core::text_range_to_span;
use super::dataclass::{dataclass_bool_flag_is_false, dataclass_flag};
use super::function_info::collect_name_refs_from_expr;
use super::generics::extract_generic_params;
use super::type_alias::type_param_name;
use super::ENUM_BASES;

pub(super) fn class_info_from(
    class: &StmtClassDef,
    functions: &mut Vec<FunctionInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
) -> ClassInfo {
    let bases = extract_class_bases(class);

    let pre_is_dataclass = class.decorator_list.iter().any(
        |d| matches!(decorator_name(d), Some(n) if n == "dataclass" || n.ends_with(".dataclass")),
    );
    let pre_is_dataclass_kw_only = pre_is_dataclass && dataclass_flag(class, "kw_only");

    let (attributes, method_names, method_decorators) =
        collect_class_body(class, functions, match_stmts, pre_is_dataclass_kw_only);

    let (generic_params, generic_non_typevar_args) = extract_generic_params(class);
    mark_protocol_stubs(class, &bases, functions);

    let class_decorators: Vec<String> = class
        .decorator_list
        .iter()
        .filter_map(decorator_name)
        .collect();

    let is_dataclass = class_decorators
        .iter()
        .any(|d| d == "dataclass" || d.ends_with(".dataclass"));

    let (base_expression_names, has_subscript_base) = extract_base_refs(class);

    ClassInfo {
        name: class.name.to_string(),
        name_span: text_range_to_span(class.name.range),
        def_span: text_range_to_span(class.range),
        bases,
        attributes,
        method_names,
        method_decorators,
        decorator_spans: class
            .decorator_list
            .iter()
            .filter_map(decorator_name_and_span)
            .collect(),
        generic_params,
        is_typed_dict: extract_is_typed_dict(class),
        is_typeddict_total: extract_is_typeddict_total(class),
        class_keywords: extract_class_keywords(class),
        is_dataclass,
        is_dataclass_frozen: is_dataclass && dataclass_flag(class, "frozen"),
        is_dataclass_kw_only: pre_is_dataclass_kw_only,
        is_dataclass_match_args_false: is_dataclass
            && dataclass_bool_flag_is_false(class, "match_args"),
        is_dataclass_order: is_dataclass && dataclass_flag(class, "order"),
        is_dataclass_unsafe_hash: is_dataclass && dataclass_flag(class, "unsafe_hash"),
        is_dataclass_eq_false: is_dataclass && dataclass_bool_flag_is_false(class, "eq"),
        is_dataclass_init_false: is_dataclass && dataclass_bool_flag_is_false(class, "init"),
        is_final: class_decorators
            .iter()
            .any(|d| d == "final" || d.rsplit('.').next() == Some("final")),
        is_enum: extract_is_enum(class),
        has_pep695_type_params: class.type_params.is_some(),
        pep695_type_param_names: class
            .type_params
            .as_deref()
            .map(|tp| tp.type_params.iter().map(type_param_name).collect())
            .unwrap_or_default(),
        base_expression_names,
        generic_non_typevar_args,
        metaclass_name: extract_metaclass_name(class),
        has_subscript_base,
        base_subscripts: extract_base_subscripts(class),
        is_dataclass_slots: is_dataclass && dataclass_flag(class, "slots"),
        has_manual_slots: class_has_manual_slots(class),
        docstring: extract_docstring(&class.body),
    }
}

/// Extract base class names from a class definition.
fn extract_class_bases(class: &StmtClassDef) -> Vec<String> {
    class
        .arguments
        .as_ref()
        .map(|args| {
            args.args
                .iter()
                .filter_map(|expr| {
                    if let Some(name) = expr_simple_name(expr) {
                        return Some(name);
                    }
                    if let ruff_python_ast::Expr::Attribute(attr) = expr {
                        return Some(attr.attr.to_string());
                    }
                    if let ruff_python_ast::Expr::Subscript(sub) = expr {
                        if let Some(name) = expr_simple_name(&sub.value) {
                            return Some(name);
                        }
                        if let ruff_python_ast::Expr::Attribute(attr) = sub.value.as_ref() {
                            return Some(attr.attr.to_string());
                        }
                    }
                    None
                })
                .collect()
        })
        .unwrap_or_default()
}

/// If this class is a Protocol or ABC, mark methods with `pass`-only bodies as stubs.
fn mark_protocol_stubs(class: &StmtClassDef, bases: &[String], functions: &mut [FunctionInfo]) {
    let is_protocol_or_abc = bases.iter().any(|b| b == "Protocol" || b == "ABC");
    if is_protocol_or_abc {
        let class_name_str = class.name.to_string();
        for func in functions.iter_mut() {
            if func.class_name.as_deref() == Some(&class_name_str)
                && !func.is_stub_body
                && body_is_pass_only(class, &func.name)
            {
                func.is_stub_body = true;
            }
        }
    }
}

fn extract_is_typed_dict(class: &StmtClassDef) -> bool {
    class.arguments.as_ref().is_some_and(|args| {
        args.args
            .iter()
            .any(|expr| expr_simple_name(expr).is_some_and(|n| n == "TypedDict"))
    })
}

fn extract_is_typeddict_total(class: &StmtClassDef) -> bool {
    let is_typed_dict = extract_is_typed_dict(class);
    !is_typed_dict
        || !class.arguments.as_ref().is_some_and(|args| {
            args.keywords.iter().any(|kw| {
                kw.arg.as_ref().is_some_and(|a| a.as_str() == "total")
                    && matches!(&kw.value, ruff_python_ast::Expr::BooleanLiteral(b) if !b.value)
            })
        })
}

fn extract_class_keywords(class: &StmtClassDef) -> Vec<String> {
    class
        .arguments
        .as_ref()
        .map(|args| {
            args.keywords
                .iter()
                .filter_map(|kw| kw.arg.as_ref().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_metaclass_name(class: &StmtClassDef) -> Option<String> {
    class.arguments.as_ref().and_then(|args| {
        args.keywords
            .iter()
            .find(|kw| kw.arg.as_ref().is_some_and(|a| a.as_str() == "metaclass"))
            .and_then(|kw| expr_simple_name(&kw.value))
    })
}

fn extract_is_enum(class: &StmtClassDef) -> bool {
    class.arguments.as_ref().is_some_and(|args| {
        args.args
            .iter()
            .any(|expr| expr_simple_name(expr).is_some_and(|n| ENUM_BASES.contains(&n.as_str())))
    })
}

/// Extract name references and subscript presence from base class expressions.
fn extract_base_refs(class: &StmtClassDef) -> (Vec<String>, bool) {
    class
        .arguments
        .as_ref()
        .map(|args| {
            let mut names = Vec::new();
            let mut has_sub = false;
            for expr in &args.args {
                collect_name_refs_from_expr(expr, &mut names);
                if matches!(expr, Expr::Subscript(_)) {
                    has_sub = true;
                }
            }
            (names, has_sub)
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// dataclass_transform post-processing
// ---------------------------------------------------------------------------

/// A `@dataclass_transform` factory detected at module level.
pub(super) struct DcTransformFactory {
    /// The function name decorated with `@dataclass_transform(...)`.
    pub(super) name: String,
    /// `kw_only_default` from the decorator (default `false`).
    pub(super) kw_only_default: bool,
    /// `frozen_default` from the decorator (default `false`).
    pub(super) frozen_default: bool,
    /// `order_default` from the decorator (default `false`).
    pub(super) order_default: bool,
    /// Field specifier function names extracted from `field_specifiers=(...)`.
    pub(super) field_specifier_names: Vec<String>,
}

/// Overload parameter info for one definition of a field specifier function.
pub(super) struct FieldSpecOverload {
    /// Names of keyword-only parameters that do NOT have defaults (required).
    pub(super) required_kwargs: Vec<String>,
    /// Default value of `init` in this overload, if present.
    pub(super) init_default: Option<bool>,
    /// Default value of `kw_only` in this overload, if present.
    pub(super) kw_only_default: Option<bool>,
}

/// Scan module-level statements for `@dataclass_transform(...)` decorated functions
/// and apply their semantics to classes decorated by those factories.
pub(super) fn parse_dataclass_transform_decorator(
    expr: &Expr,
) -> (bool, bool, bool, bool, Vec<String>) {
    let Expr::Call(call) = expr else {
        if let Expr::Name(n) = expr {
            if n.id.as_str() == "dataclass_transform" {
                return (true, false, false, false, Vec::new());
            }
        }
        return (false, false, false, false, Vec::new());
    };
    let is_dc = super::walks::is_name_or_attr_named(call.func.as_ref(), "dataclass_transform");
    if !is_dc {
        return (false, false, false, false, Vec::new());
    }

    let mut kw_only_default = false;
    let mut frozen_default = false;
    let mut order_default = false;
    let mut field_specifier_names = Vec::new();

    for kw in &call.arguments.keywords {
        let Some(arg_name) = kw.arg.as_ref() else {
            continue;
        };
        match arg_name.as_str() {
            "kw_only_default" => {
                kw_only_default = matches!(&kw.value, Expr::BooleanLiteral(b) if b.value);
            }
            "frozen_default" => {
                frozen_default = matches!(&kw.value, Expr::BooleanLiteral(b) if b.value);
            }
            "order_default" => {
                order_default = matches!(&kw.value, Expr::BooleanLiteral(b) if b.value);
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

    (
        true,
        kw_only_default,
        frozen_default,
        order_default,
        field_specifier_names,
    )
}

/// Build overload info for a field specifier function.
pub(super) fn extract_docstring(body: &[Stmt]) -> Option<String> {
    let first = body.first()?;
    let Stmt::Expr(expr_stmt) = first else {
        return None;
    };
    match expr_stmt.value.as_ref() {
        Expr::StringLiteral(s) => {
            let text = s.value.to_str().to_owned();
            // Trim leading/trailing whitespace from docstrings.
            Some(text.trim().to_owned())
        }
        _ => None,
    }
}

/// Returns `true` when a statement (recursively) contains a `yield` or `yield from`.
pub(super) fn body_is_stub(stmts: &[Stmt]) -> bool {
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

/// Check if a method in a class has a `pass`-only body (optionally preceded by a docstring).
///
/// This is used to mark Protocol/ABC methods with `pass` as stubs.
pub(super) fn body_is_pass_only(class: &StmtClassDef, method_name: &str) -> bool {
    for stmt in &class.body {
        if let Stmt::FunctionDef(func) = stmt {
            if func.name.as_str() == method_name {
                let non_docstring: Vec<&Stmt> = func
                    .body
                    .iter()
                    .skip_while(|s| {
                        matches!(s, Stmt::Expr(e) if matches!(e.value.as_ref(), Expr::StringLiteral(_)))
                    })
                    .collect();
                return non_docstring.iter().all(|s| matches!(s, Stmt::Pass(_)));
            }
        }
    }
    false
}

/// Collect annotated local variable declarations (`x: T` or `x: T = v`) from a
/// function body, recursing into nested blocks but not into nested function bodies.
///
/// Used to populate `FunctionInfo::local_vars` for downstream rules (e.g. E0047).
pub(super) fn import_infos_from(node: &StmtImport) -> Vec<ImportInfo> {
    node.names
        .iter()
        .map(|alias| ImportInfo {
            module: alias.name.to_string(),
            // `import X as Y` binds `Y`, not the module name — capture the alias so
            // scope-resolution rules (e.g. BSK-E0018) see the real binding. Plain
            // `import X` / `import X.Y` keeps `names` empty; its bound name is the
            // top-level module, derived from `module`.
            names: alias
                .asname
                .as_ref()
                .map(|asname| vec![asname.to_string()])
                .unwrap_or_default(),
            span: text_range_to_span(node.range),
            kind: ImportKind::Plain,
            resolution: ImportResolution::Unresolved,
            resolved_path: None,
            package_dep_kind: None,
            package_version: None,
            package_name: None,
            unresolved_reason: None,
        })
        .collect()
}

pub(super) fn import_from_infos_from(node: &StmtImportFrom) -> Vec<ImportInfo> {
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
            resolution: ImportResolution::Unresolved,
            resolved_path: None,
            package_dep_kind: None,
            package_version: None,
            package_name: None,
            unresolved_reason: None,
        }];
    }

    let names: Vec<String> = node.names.iter().map(alias_name).collect();
    vec![ImportInfo {
        module,
        names,
        span: text_range_to_span(node.range),
        kind: ImportKind::From,
        resolution: ImportResolution::Unresolved,
        resolved_path: None,
        package_dep_kind: None,
        package_version: None,
        package_name: None,
        unresolved_reason: None,
    }]
}

/// The locally bound name of an import alias: `asname` when present
/// (`from X import Y as Z` binds `Z`), otherwise the imported name.
pub(super) fn alias_name(alias: &Alias) -> String {
    alias
        .asname
        .as_ref()
        .map_or_else(|| alias.name.to_string(), ToString::to_string)
}

// ---------------------------------------------------------------------------
// Variable assignment info
// ---------------------------------------------------------------------------

pub(super) fn match_stmt_info_from(node: &StmtMatch) -> MatchStmtInfo {
    let has_wildcard = node.cases.iter().any(is_wildcard_case);
    MatchStmtInfo {
        span: text_range_to_span(node.range),
        has_wildcard,
    }
}

pub(super) fn is_wildcard_case(case: &MatchCase) -> bool {
    is_wildcard_pattern(&case.pattern)
}

pub(super) fn is_wildcard_pattern(pattern: &Pattern) -> bool {
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
pub(super) fn decorator_name(dec: &Decorator) -> Option<String> {
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

/// Extract the decorator name together with the span of the name identifier.
pub(super) fn decorator_name_and_span(dec: &Decorator) -> Option<(String, Span)> {
    match &dec.expression {
        Expr::Name(name) => Some((name.id.to_string(), text_range_to_span(name.range()))),
        Expr::Attribute(attr) => Some((attr.attr.to_string(), text_range_to_span(attr.range()))),
        Expr::Call(call) => match call.func.as_ref() {
            Expr::Name(name) => Some((name.id.to_string(), text_range_to_span(name.range()))),
            Expr::Attribute(attr) => {
                Some((attr.attr.to_string(), text_range_to_span(attr.range())))
            }
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
pub(super) fn expr_simple_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        _ => None,
    }
}

/// Recursively collect all `Name` references from an expression tree.
///
/// Used to find all identifier names referenced within base class expressions,
/// including those nested inside subscripts, tuples, and other compound forms.
pub(super) fn extract_base_subscripts(class: &StmtClassDef) -> Vec<BaseSubscriptEntry> {
    let Some(arguments) = class.arguments.as_ref() else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for expr in &arguments.args {
        let Expr::Subscript(sub) = expr else {
            continue;
        };
        let Some(base_name) = expr_simple_name(&sub.value) else {
            continue;
        };
        let (type_arg_names, type_args) = match sub.slice.as_ref() {
            Expr::Tuple(tup) => {
                let names: Vec<String> = tup.elts.iter().filter_map(expr_simple_name).collect();
                let args: Vec<TypeArg> = tup.elts.iter().map(expr_to_type_arg).collect();
                (names, args)
            }
            single => {
                let names: Vec<String> = expr_simple_name(single).into_iter().collect();
                let args: Vec<TypeArg> = vec![expr_to_type_arg(single)];
                (names, args)
            }
        };
        entries.push(BaseSubscriptEntry {
            base_name,
            type_arg_names,
            type_args,
            span: Span::from(sub.range()),
        });
    }
    entries
}

/// Check whether a class body contains a manual `__slots__` assignment.
///
/// Returns `true` if any statement in the class body is an assignment (plain
/// or annotated) whose target is `__slots__`.
pub(super) fn class_has_manual_slots(class: &StmtClassDef) -> bool {
    for stmt in &class.body {
        match stmt {
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    if expr_simple_name(target).as_deref() == Some("__slots__") {
                        return true;
                    }
                }
            }
            Stmt::AnnAssign(ann)
                if expr_simple_name(&ann.target).as_deref() == Some("__slots__") =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}
