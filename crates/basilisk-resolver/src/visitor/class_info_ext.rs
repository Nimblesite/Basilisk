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
use super::function_info::collect_name_refs_from_expr;
use super::generics::extract_generic_params;
use super::type_alias::type_param_name;

pub(super) fn class_info_from(
    class: &StmtClassDef,
    functions: &mut Vec<FunctionInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
) -> ClassInfo {
    let bases = extract_class_bases(class);

    let (attributes, method_names, method_decorators) =
        collect_class_body(class, functions, match_stmts);

    let (generic_params, generic_non_typevar_args) = extract_generic_params(class);

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
        class_keywords: extract_class_keywords(class),
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
            // scope-resolution rules (e.g. names_undefined) see the real binding. Plain
            // `import X` / `import X.Y` keeps `names` empty; its bound name is the
            // top-level module, derived from `module`.
            names: alias
                .asname
                .as_ref()
                .map(|asname| vec![asname.to_string()])
                .unwrap_or_default(),
            span: text_range_to_span(node.range),
            name_spans: alias_name_spans(alias),
            kind: ImportKind::Plain,
            resolution: ImportResolution::Unresolved,
            resolved_path: None,
            package_dep_kind: None,
            package_version: None,
            package_name: None,
            stub_distribution: None,
            unresolved_reason: None,
        })
        .collect()
}

/// Identifier spans of an import alias: the imported name and, when present,
/// its `as` alias — never the `as` keyword itself (GitHub #286).
fn alias_name_spans(alias: &Alias) -> Vec<Span> {
    let mut spans = vec![text_range_to_span(alias.name.range)];
    if let Some(asname) = &alias.asname {
        spans.push(text_range_to_span(asname.range));
    }
    spans
}

pub(super) fn import_from_infos_from(node: &StmtImportFrom) -> Vec<ImportInfo> {
    let module = node
        .module
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();

    let is_star = node.names.iter().any(|a| a.name.as_str() == "*");

    // The module path identifier, when written out (`from . import x` has none).
    let module_span = node
        .module
        .as_ref()
        .map(|module| text_range_to_span(module.range));

    if is_star {
        return vec![ImportInfo {
            module,
            names: Vec::new(),
            span: text_range_to_span(node.range),
            name_spans: module_span.into_iter().collect(),
            kind: ImportKind::Star,
            resolution: ImportResolution::Unresolved,
            resolved_path: None,
            package_dep_kind: None,
            package_version: None,
            package_name: None,
            stub_distribution: None,
            unresolved_reason: None,
        }];
    }

    let names: Vec<String> = node.names.iter().map(alias_name).collect();
    let name_spans = module_span
        .into_iter()
        .chain(node.names.iter().flat_map(alias_name_spans))
        .collect();
    vec![ImportInfo {
        module,
        names,
        span: text_range_to_span(node.range),
        name_spans,
        kind: ImportKind::From,
        resolution: ImportResolution::Unresolved,
        resolved_path: None,
        package_dep_kind: None,
        package_version: None,
        package_name: None,
        stub_distribution: None,
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
    let has_structural_pattern = node.cases.iter().any(case_has_structural_pattern);
    MatchStmtInfo {
        span: text_range_to_span(node.range),
        has_wildcard,
        has_structural_pattern,
    }
}

pub(super) fn is_wildcard_case(case: &MatchCase) -> bool {
    // A case with a guard (`case x if cond:`) is never irrefutable.
    case.guard.is_none() && is_wildcard_pattern(&case.pattern)
}

/// A pattern that matches *every* value. The bare `case _:` (`MatchAs` with no
/// name and no sub-pattern) and a bare capture `case name:` (`MatchAs` with a
/// name but no sub-pattern) are both irrefutable — Python binds the subject and
/// always succeeds — so each makes a `match` exhaustive.
pub(super) fn is_wildcard_pattern(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::MatchAs(ma) => ma.pattern.is_none(),
        Pattern::MatchOr(mo) => mo.patterns.iter().any(is_wildcard_pattern),
        _ => false,
    }
}

/// `true` if the match performs structural decomposition (sequence/mapping
/// patterns). Such matches narrow open-ended shapes (e.g. tuple unions of mixed
/// arity) where a catch-all is not required for correctness, so exhaustiveness
/// (`match_exhaustiveness`) does not apply — matching the reference checkers, which do not
/// flag these.
fn case_has_structural_pattern(case: &MatchCase) -> bool {
    fn is_structural(pattern: &Pattern) -> bool {
        match pattern {
            Pattern::MatchSequence(_) | Pattern::MatchMapping(_) => true,
            Pattern::MatchOr(mo) => mo.patterns.iter().any(is_structural),
            _ => false,
        }
    }
    is_structural(&case.pattern)
}

// ---------------------------------------------------------------------------
// Decorator helpers
// ---------------------------------------------------------------------------

/// A decorator's name as spelled — the FULL dotted path for attribute
/// spellings (`typing.overload` → `"typing.overload"`, never just
/// `"overload"`), because whether `t.overload` IS `typing.overload` is a
/// binding question the consumer answers by resolving `t`
/// ([#380](https://github.com/Nimblesite/Basilisk/issues/380)). Dropping the
/// qualifier here would make that question unanswerable everywhere
/// downstream. A call decorator reports its callee (`@cache(size=1)` →
/// `"cache"`).
pub(super) fn decorator_name(dec: &Decorator) -> Option<String> {
    match &dec.expression {
        Expr::Call(call) => dotted_expr_name(&call.func),
        expr => dotted_expr_name(expr),
    }
}

/// Extract the decorator name together with the span of the name identifier.
pub(super) fn decorator_name_and_span(dec: &Decorator) -> Option<(String, Span)> {
    match &dec.expression {
        Expr::Call(call) => {
            dotted_expr_name(&call.func).map(|name| (name, text_range_to_span(call.func.range())))
        }
        expr => dotted_expr_name(expr).map(|name| (name, text_range_to_span(expr.range()))),
    }
}

/// Render `a.b.c` from a name or attribute chain; `None` for anything else.
fn dotted_expr_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attr) => {
            dotted_expr_name(&attr.value).map(|value| format!("{value}.{}", attr.attr))
        }
        _ => None,
    }
}

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
