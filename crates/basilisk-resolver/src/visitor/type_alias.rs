//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Type Alias visitor functions.

use ruff_python_ast::{Expr, Stmt, TypeParam};
use ruff_text_size::Ranged;

use crate::scope::{
    NamedTupleDefInfo, NewTypeCallInfo, Span, TypeAliasDefInfo, TypeAliasTypeCallInfo,
    TypeStatementInfo,
};

use super::class_info_ext::expr_simple_name;
use super::core::{source_slice_range, text_range_to_span};
use super::dataclass::extract_string_list;
use super::final_readonly::collect_final_string_constants;
use super::function_info::{
    collect_name_refs_from_expr, collect_string_refs_from_expr, parse_defaults_count,
};

pub(super) fn collect_newtype_calls(stmts: &[Stmt]) -> Vec<NewTypeCallInfo> {
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

/// Collect functional-form `NamedTuple` / `namedtuple` definitions from module-level code.
///
/// Handles all standard forms:
/// - `typing.NamedTuple("N", [("x", int), ...])` — list form (with types)
/// - `typing.NamedTuple("N", (("x", int), ...))` — tuple form (with types)
/// - `collections.namedtuple("N", ["x", "y"])` — list of string literals (no types)
/// - `collections.namedtuple("N", ("x", "y"))` — tuple of string literals (no types)
/// - `collections.namedtuple("N", "x y")` — space/comma-separated string (no types)
/// - Any of the above with `defaults=(v, ...)` keyword
///
/// Field names that reference `Final` string-literal constants are resolved to
/// the constant's value (e.g. `X: Final = "x"` makes `X` resolve to `"x"`).
pub(super) fn collect_namedtuple_defs(stmts: &[Stmt], source: &str) -> Vec<NamedTupleDefInfo> {
    let final_string_constants: std::collections::HashMap<&str, &str> =
        collect_final_string_constants(stmts, source);

    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::Assign(node) = stmt else { continue };
        let Expr::Call(call) = node.value.as_ref() else {
            continue;
        };

        // Check callee: `NamedTuple`, `typing.NamedTuple`, `namedtuple`, or
        // `collections.namedtuple`.
        let callee_name = match call.func.as_ref() {
            Expr::Name(n) => Some(n.id.as_str()),
            Expr::Attribute(a) => Some(a.attr.as_str()),
            _ => None,
        };
        let Some(callee) = callee_name else { continue };
        let is_typing_nt = callee == "NamedTuple";
        let is_collections_nt = callee == "namedtuple";
        if !is_typing_nt && !is_collections_nt {
            continue;
        }

        let Some(lhs_name) = node.targets.first().and_then(expr_simple_name) else {
            continue;
        };
        let Some(fields_arg) = call.arguments.args.get(1) else {
            continue;
        };

        // Skip `namedtuple(..., rename=True)` — the renamed field names cannot be
        // determined statically (e.g. `_0`, `_1`, ...) so we don't track these.
        let has_rename_true = is_collections_nt
            && call.arguments.keywords.iter().any(|kw| {
                kw.arg.as_ref().is_some_and(|arg| arg.as_str() == "rename")
                    && matches!(&kw.value, Expr::BooleanLiteral(b) if b.value)
            });
        if has_rename_true {
            continue;
        }

        // Parse `defaults` keyword argument to determine how many trailing fields
        // have default values.
        let defaults_count = parse_defaults_count(&call.arguments.keywords);

        if is_typing_nt {
            // typing.NamedTuple: second arg is a list or tuple of (name, type) pairs.
            let pairs: Option<&[Expr]> = match fields_arg {
                Expr::List(l) => Some(&l.elts),
                Expr::Tuple(t) => Some(&t.elts),
                _ => None,
            };
            let Some(elts) = pairs else { continue };

            let mut field_names = Vec::new();
            let mut field_types = Vec::new();
            for elt in elts {
                let Expr::Tuple(tuple_expr) = elt else {
                    continue;
                };
                if tuple_expr.elts.len() < 2 {
                    continue;
                }
                let Some(first_elt) = tuple_expr.elts.first() else {
                    continue;
                };
                let field_name = match first_elt {
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
                let Some(second_elt) = tuple_expr.elts.get(1) else {
                    continue;
                };
                let type_range = second_elt.range();
                let field_type = source_slice_range(source, type_range)
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
                    defaults_count,
                    has_types: true,
                    span: text_range_to_span(call.range()),
                });
            }
        } else {
            // collections.namedtuple: second arg gives field names only (no types).
            let field_names = parse_namedtuple_field_names(fields_arg, &final_string_constants);
            if !field_names.is_empty() {
                out.push(NamedTupleDefInfo {
                    lhs_name,
                    field_names,
                    field_types: Vec::new(),
                    defaults_count,
                    has_types: false,
                    span: text_range_to_span(call.range()),
                });
            }
        }
    }
    out
}

/// Parse field names from a `collections.namedtuple` second argument.
///
/// Supports:
/// - `["x", "y"]` — list of string literals
/// - `("x", "y")` — tuple of string literals
/// - `"x y"` or `"x, y"` — space/comma-separated string literal
pub(super) fn parse_namedtuple_field_names(
    fields_arg: &Expr,
    final_constants: &std::collections::HashMap<&str, &str>,
) -> Vec<String> {
    match fields_arg {
        Expr::List(l) => extract_string_list(&l.elts, final_constants),
        Expr::Tuple(t) => extract_string_list(&t.elts, final_constants),
        Expr::StringLiteral(s) => {
            // Split on commas or whitespace.
            let raw = s.value.to_str();
            raw.split(|c: char| c == ',' || c.is_whitespace())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Extract string literal field names from a list/tuple of string literals.
pub(super) fn type_param_name(tp: &TypeParam) -> String {
    match tp {
        TypeParam::TypeVar(tv) => tv.name.to_string(),
        TypeParam::TypeVarTuple(tvt) => tvt.name.to_string(),
        TypeParam::ParamSpec(ps) => ps.name.to_string(),
    }
}

pub(super) fn is_user_defined_type_alias(ty: &str) -> bool {
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
        "TypeForm",
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
pub(super) fn collect_type_alias_defs(stmts: &[Stmt]) -> Vec<TypeAliasDefInfo> {
    let mut out = Vec::new();
    for stmt in stmts {
        // Handle `Name: TypeAlias = expr` annotated assignments.
        if let Stmt::AnnAssign(ann) = stmt {
            let is_type_alias = matches!(
                ann.annotation.as_ref(),
                Expr::Name(n) if n.id.as_str() == "TypeAlias"
            );
            if !is_type_alias {
                continue;
            }
            let Some(name) = expr_simple_name(ann.target.as_ref()) else {
                continue;
            };
            let Some(rhs) = ann.value.as_ref() else {
                continue;
            };
            out.push(build_type_alias_info(name, rhs, stmt));
            continue;
        }

        // Handle bare `Name = Expr[...]` assignments (implicit type aliases).
        if let Stmt::Assign(assign) = stmt {
            if assign.targets.len() != 1 {
                continue;
            }
            let Some(first_target) = assign.targets.first() else {
                continue;
            };
            let Some(name) = expr_simple_name(first_target) else {
                continue;
            };
            // Only treat subscript RHS as implicit alias (Name = Something[...])
            if matches!(assign.value.as_ref(), Expr::Subscript(_)) {
                out.push(build_type_alias_info(name, &assign.value, stmt));
            }
        }
    }
    out
}

/// Helper to build a `TypeAliasDefInfo` from an alias name and RHS expression.
pub(super) fn build_type_alias_info(name: String, rhs: &Expr, stmt: &Stmt) -> TypeAliasDefInfo {
    let mut rhs_names = Vec::new();
    collect_name_refs_from_expr(rhs, &mut rhs_names);

    let (rhs_base_name, rhs_type_arg_names) = match rhs {
        Expr::Subscript(sub) => {
            let base = expr_simple_name(&sub.value);
            let arg_names = match sub.slice.as_ref() {
                Expr::Tuple(tup) => tup.elts.iter().filter_map(expr_simple_name).collect(),
                single => expr_simple_name(single).into_iter().collect(),
            };
            (base, arg_names)
        }
        _ => (None, Vec::new()),
    };

    let mut rhs_string_refs = Vec::new();
    collect_string_refs_from_expr(rhs, &mut rhs_string_refs);

    let span = Span::from(stmt.range());

    TypeAliasDefInfo {
        name,
        rhs_names,
        rhs_base_name,
        rhs_type_arg_names,
        rhs_string_refs,
        span,
    }
}

// ---------------------------------------------------------------------------
// protocols_explicit: Protocol instantiation violation detection
// ---------------------------------------------------------------------------

/// Check if a class is a Protocol (has `Protocol` in its bases).
pub(super) fn collect_type_statements(stmts: &[Stmt]) -> Vec<TypeStatementInfo> {
    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::TypeAlias(ta) => {
                if let Some(name_str) = expr_simple_name(&ta.name) {
                    out.push(TypeStatementInfo {
                        name: name_str,
                        rhs_span: text_range_to_span(ta.value.range()),
                        name_span: text_range_to_span(ta.name.range()),
                        param_names: ta
                            .type_params
                            .as_deref()
                            .map(|tps| tps.type_params.iter().map(type_param_name).collect())
                            .unwrap_or_default(),
                    });
                }
            }
            Stmt::ClassDef(cls) => out.extend(collect_type_statements(&cls.body)),
            Stmt::FunctionDef(func) => out.extend(collect_type_statements(&func.body)),
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------
// TypeAliasType(...) call collection
// ---------------------------------------------------------------------------

/// Collect `X = TypeAliasType('X', rhs)` and `X: ann = TypeAliasType('X', rhs)` calls.
pub(super) fn collect_type_alias_type_calls(stmts: &[Stmt]) -> Vec<TypeAliasTypeCallInfo> {
    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::Assign(node) => {
                if let Some(info) = type_alias_type_call_from_expr(&node.value, &node.targets) {
                    out.push(info);
                }
            }
            Stmt::AnnAssign(node) => {
                if let Some(val) = &node.value {
                    if let Some(lhs_name) = expr_simple_name(&node.target) {
                        if let Some(info) = type_alias_type_call_from_value(val, &lhs_name) {
                            out.push(info);
                        }
                    }
                }
            }
            Stmt::ClassDef(cls) => out.extend(collect_type_alias_type_calls(&cls.body)),
            Stmt::FunctionDef(func) => out.extend(collect_type_alias_type_calls(&func.body)),
            _ => {}
        }
    }
    out
}

/// Extract `TypeAliasType(...)` info from a regular assignment's value + targets.
pub(super) fn type_alias_type_call_from_expr(
    value: &Expr,
    targets: &[Expr],
) -> Option<TypeAliasTypeCallInfo> {
    let lhs_name = targets.first().and_then(expr_simple_name)?;
    type_alias_type_call_from_value(value, &lhs_name)
}

/// Extract `TypeAliasType(...)` info from an expression and known LHS name.
pub(super) fn type_alias_type_call_from_value(
    value: &Expr,
    lhs_name: &str,
) -> Option<TypeAliasTypeCallInfo> {
    let Expr::Call(call) = value else { return None };
    let callee = expr_simple_name(&call.func)?;
    if callee != "TypeAliasType" {
        return None;
    }
    let rhs_span = call
        .arguments
        .args
        .get(1)
        .map(|e| text_range_to_span(e.range()));
    Some(TypeAliasTypeCallInfo {
        lhs_name: lhs_name.to_owned(),
        rhs_span,
        span: text_range_to_span(call.range),
    })
}

// ---------------------------------------------------------------------------
// TypeAliasType violation detection
// ---------------------------------------------------------------------------

use crate::scope::{TypeAliasTypeViolation, TypeAliasTypeViolationKind};

/// Check whether an expression is a valid type expression for `TypeAliasType`.
///
/// Valid forms: names, subscripts on type names (`list[int]`), union (`X | Y`),
/// string literals (forward refs), `None`, `...` (ellipsis), starred (`*Ts`).
///
/// When `top_level` is true, tuples are NOT valid (the value argument to
/// `TypeAliasType` must be a single type expression, not a tuple of types).
fn is_valid_type_expression(expr: &Expr, top_level: bool) -> bool {
    match expr {
        // Simple names, string forward refs, None, Ellipsis, starred (*Ts),
        // and attribute access (typing.List) are always valid type forms.
        Expr::Name(_)
        | Expr::StringLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::EllipsisLiteral(_)
        | Expr::Starred(_)
        | Expr::Attribute(_) => true,
        // Subscript: `list[int]`, `dict[str, int]`, etc.
        Expr::Subscript(sub) => is_valid_type_expression(&sub.value, false),
        // Union: `int | str`
        Expr::BinOp(bin) => {
            matches!(bin.op, ruff_python_ast::Operator::BitOr)
                && is_valid_type_expression(&bin.left, false)
                && is_valid_type_expression(&bin.right, false)
        }
        // Tuple: only valid inside subscripts (e.g. `dict[str, int]`), never
        // at the top level of a TypeAliasType value.
        Expr::Tuple(t) => !top_level && t.elts.iter().all(|e| is_valid_type_expression(e, false)),
        // Everything else is invalid: list, dict, set, call, lambda,
        // comprehension, conditional, boolean op, f-string, int/bool literals.
        _ => false,
    }
}

/// Names that are valid type references in a `TypeAliasType` value.
fn collect_type_names_in_scope(
    stmts: &[Stmt],
    typevar_names: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    let mut names: std::collections::HashSet<String> = typevar_names.clone();
    for stmt in stmts {
        match stmt {
            Stmt::ClassDef(cls) => {
                let _ = names.insert(cls.name.to_string());
            }
            Stmt::Assign(node) => {
                if let Some(lhs) = node.targets.first().and_then(expr_simple_name) {
                    // TypeAliasType calls
                    if let Expr::Call(call) = node.value.as_ref() {
                        if expr_simple_name(&call.func).as_deref() == Some("TypeAliasType") {
                            let _ = names.insert(lhs.clone());
                        }
                    }
                    // Subscript RHS (implicit type alias): `X = list[int]`
                    if matches!(node.value.as_ref(), Expr::Subscript(_)) {
                        let _ = names.insert(lhs);
                    }
                }
            }
            Stmt::AnnAssign(ann) => {
                // `X: TypeAlias = ...`
                if matches!(ann.annotation.as_ref(), Expr::Name(n) if n.id.as_str() == "TypeAlias")
                {
                    if let Some(name) = expr_simple_name(&ann.target) {
                        let _ = names.insert(name);
                    }
                }
            }
            Stmt::TypeAlias(ta) => {
                if let Some(name) = expr_simple_name(&ta.name) {
                    let _ = names.insert(name);
                }
            }
            Stmt::Import(imp) => {
                for alias in &imp.names {
                    let name = alias
                        .asname
                        .as_ref()
                        .map_or_else(|| alias.name.to_string(), std::string::ToString::to_string);
                    let _ = names.insert(name);
                }
            }
            Stmt::ImportFrom(imp) => {
                for alias in &imp.names {
                    let name = alias
                        .asname
                        .as_ref()
                        .map_or_else(|| alias.name.to_string(), std::string::ToString::to_string);
                    let _ = names.insert(name);
                }
            }
            _ => {}
        }
    }
    // Add builtins.
    for builtin in &[
        "int",
        "str",
        "float",
        "bool",
        "bytes",
        "complex",
        "list",
        "dict",
        "set",
        "frozenset",
        "tuple",
        "type",
        "object",
        "None",
        "True",
        "False",
        "Ellipsis",
        "memoryview",
        "bytearray",
        "range",
        "slice",
        "property",
        "classmethod",
        "staticmethod",
        "super",
    ] {
        let _ = names.insert((*builtin).to_owned());
    }
    names
}

/// Detect violations in `TypeAliasType(...)` calls.
pub(super) fn collect_type_alias_type_violations(
    stmts: &[Stmt],
    typevar_names: &std::collections::HashSet<String>,
) -> Vec<TypeAliasTypeViolation> {
    let mut out = Vec::new();
    let class_scope_tvs = std::collections::HashSet::new();
    let type_names = collect_type_names_in_scope(stmts, typevar_names);
    collect_tat_violations_inner(
        stmts,
        typevar_names,
        &class_scope_tvs,
        &type_names,
        &mut out,
    );

    // Post-pass: detect cross-reference circular dependencies between aliases.
    // Build a map from alias name to string refs in value, then find cycles.
    let alias_refs = collect_tat_string_refs(stmts);
    let alias_names: std::collections::HashSet<&str> =
        alias_refs.keys().map(String::as_str).collect();
    for (alias_name, (refs, span)) in &alias_refs {
        for referenced in refs {
            if alias_names.contains(referenced.as_str()) && referenced != alias_name {
                // Check if the referenced alias references back to this one.
                if let Some((back_refs, _)) = alias_refs.get(referenced) {
                    if back_refs.iter().any(|r| r == alias_name) {
                        out.push(TypeAliasTypeViolation {
                            span: *span,
                            kind: TypeAliasTypeViolationKind::CircularReference,
                            alias_name: alias_name.clone(),
                        });
                        break;
                    }
                }
            }
        }
    }

    // Post-pass: detect invalid attribute accesses on TypeAliasType instances.
    let tat_names = collect_tat_names(stmts);
    collect_tat_attr_violations(stmts, &tat_names, &mut out);

    // Post-pass: detect incorrect type arg counts on TypeAliasType subscripts.
    collect_tat_subscript_violations(stmts, &tat_names, &mut out);

    out
}

/// Info about a `TypeAliasType` call: how many non-variadic type params it has,
/// and whether it has a variadic one (`TypeVarTuple`).
struct TatInfo {
    /// Total number of type params (used for exact count checking).
    param_count: usize,
}

/// Build a map from `TypeAliasType` alias names to their type param info.
fn collect_tat_names(stmts: &[Stmt]) -> std::collections::HashMap<String, TatInfo> {
    let mut map = std::collections::HashMap::new();
    for stmt in stmts {
        let Stmt::Assign(node) = stmt else { continue };
        let Some(lhs) = node.targets.first().and_then(expr_simple_name) else {
            continue;
        };
        let Expr::Call(call) = node.value.as_ref() else {
            continue;
        };
        if expr_simple_name(&call.func).as_deref() != Some("TypeAliasType") {
            continue;
        }
        let mut param_count = 0usize;
        for kw in &call.arguments.keywords {
            if kw.arg.as_ref().is_some_and(|a| a.as_str() == "type_params") {
                if let Expr::Tuple(tup) = &kw.value {
                    param_count = tup.elts.len();
                }
            }
        }
        let _ = map.insert(lhs, TatInfo { param_count });
    }
    map
}

/// Valid attributes on `TypeAliasType` instances.
const TAT_VALID_ATTRS: &[&str] = &[
    "__value__",
    "__type_params__",
    "__name__",
    "__module__",
    "__qualname__",
];

/// Detect invalid attribute accesses on `TypeAliasType` names.
fn collect_tat_attr_violations(
    stmts: &[Stmt],
    tat_names: &std::collections::HashMap<String, TatInfo>,
    out: &mut Vec<TypeAliasTypeViolation>,
) {
    for stmt in stmts {
        collect_tat_attr_violations_in_expr_stmt(stmt, tat_names, out);
    }
}

fn collect_tat_attr_violations_in_expr_stmt(
    stmt: &Stmt,
    tat_names: &std::collections::HashMap<String, TatInfo>,
    out: &mut Vec<TypeAliasTypeViolation>,
) {
    // Walk all expressions in the statement looking for Attribute access.
    match stmt {
        Stmt::Expr(node) => {
            collect_tat_attr_violations_in_expr(&node.value, tat_names, out);
        }
        Stmt::Assign(node) => {
            collect_tat_attr_violations_in_expr(&node.value, tat_names, out);
        }
        Stmt::AnnAssign(node) => {
            if let Some(ref val) = node.value {
                collect_tat_attr_violations_in_expr(val, tat_names, out);
            }
        }
        Stmt::Return(node) => {
            if let Some(ref val) = node.value {
                collect_tat_attr_violations_in_expr(val, tat_names, out);
            }
        }
        Stmt::If(node) => {
            collect_tat_attr_violations_in_expr(&node.test, tat_names, out);
            for s in &node.body {
                collect_tat_attr_violations_in_expr_stmt(s, tat_names, out);
            }
            for s in &node.elif_else_clauses {
                for s2 in &s.body {
                    collect_tat_attr_violations_in_expr_stmt(s2, tat_names, out);
                }
            }
        }
        Stmt::FunctionDef(node) => {
            for s in &node.body {
                collect_tat_attr_violations_in_expr_stmt(s, tat_names, out);
            }
        }
        _ => {}
    }
}

fn collect_tat_attr_violations_in_expr(
    expr: &Expr,
    tat_names: &std::collections::HashMap<String, TatInfo>,
    out: &mut Vec<TypeAliasTypeViolation>,
) {
    match expr {
        Expr::Attribute(attr) => {
            if let Expr::Name(name) = attr.value.as_ref() {
                if tat_names.contains_key(name.id.as_str())
                    && !TAT_VALID_ATTRS.contains(&attr.attr.as_str())
                {
                    out.push(TypeAliasTypeViolation {
                        span: text_range_to_span(attr.range()),
                        kind: TypeAliasTypeViolationKind::InvalidAttributeAccess {
                            attr_name: attr.attr.to_string(),
                        },
                        alias_name: name.id.to_string(),
                    });
                }
            }
            // Also recurse into the value.
            collect_tat_attr_violations_in_expr(&attr.value, tat_names, out);
        }
        Expr::Call(call) => {
            collect_tat_attr_violations_in_expr(&call.func, tat_names, out);
            for arg in &call.arguments.args {
                collect_tat_attr_violations_in_expr(arg, tat_names, out);
            }
        }
        Expr::BinOp(bin) => {
            collect_tat_attr_violations_in_expr(&bin.left, tat_names, out);
            collect_tat_attr_violations_in_expr(&bin.right, tat_names, out);
        }
        Expr::Subscript(sub) => {
            collect_tat_attr_violations_in_expr(&sub.value, tat_names, out);
            collect_tat_attr_violations_in_expr(&sub.slice, tat_names, out);
        }
        Expr::Tuple(tup) => {
            for elt in &tup.elts {
                collect_tat_attr_violations_in_expr(elt, tat_names, out);
            }
        }
        _ => {}
    }
}

/// Detect incorrect type arg counts when subscripting a `TypeAliasType` alias.
fn collect_tat_subscript_violations(
    stmts: &[Stmt],
    tat_names: &std::collections::HashMap<String, TatInfo>,
    out: &mut Vec<TypeAliasTypeViolation>,
) {
    for stmt in stmts {
        // Look for annotation subscripts like `x: GoodAlias5[int, int, ...]`
        if let Stmt::AnnAssign(node) = stmt {
            check_tat_subscript_in_expr(&node.annotation, tat_names, out);
        }
    }
}

fn check_tat_subscript_in_expr(
    expr: &Expr,
    tat_names: &std::collections::HashMap<String, TatInfo>,
    out: &mut Vec<TypeAliasTypeViolation>,
) {
    if let Expr::Subscript(sub) = expr {
        if let Expr::Name(name) = sub.value.as_ref() {
            if let Some(info) = tat_names.get(name.id.as_str()) {
                // Count the provided args.
                let arg_count = match sub.slice.as_ref() {
                    Expr::Tuple(tup) => tup.elts.len(),
                    _ => 1,
                };
                // Flag when arg count doesn't match the declared param count.
                // A too-few check catches both exact mismatches and under-supply.
                if info.param_count > 0 && arg_count < info.param_count {
                    out.push(TypeAliasTypeViolation {
                        span: text_range_to_span(sub.range()),
                        kind: TypeAliasTypeViolationKind::IncorrectTypeArgCount {
                            expected: info.param_count,
                            actual: arg_count,
                        },
                        alias_name: name.id.to_string(),
                    });
                }
            }
        }
        // Also recurse into the slice for nested subscripts.
        check_tat_subscript_in_expr(&sub.slice, tat_names, out);
    } else if let Expr::BinOp(bin) = expr {
        check_tat_subscript_in_expr(&bin.left, tat_names, out);
        check_tat_subscript_in_expr(&bin.right, tat_names, out);
    }
}

/// Collect string references from `TypeAliasType` values for cross-reference detection.
fn collect_tat_string_refs(
    stmts: &[Stmt],
) -> std::collections::HashMap<String, (Vec<String>, Span)> {
    let mut map = std::collections::HashMap::new();
    for stmt in stmts {
        let (lhs_name, value) = match stmt {
            Stmt::Assign(node) => {
                let Some(name) = node.targets.first().and_then(expr_simple_name) else {
                    continue;
                };
                (name, &*node.value)
            }
            _ => continue,
        };
        let Expr::Call(call) = value else { continue };
        let Some(callee) = expr_simple_name(&call.func) else {
            continue;
        };
        if callee != "TypeAliasType" {
            continue;
        }
        if let Some(rhs_expr) = call.arguments.args.get(1) {
            let mut string_refs = Vec::new();
            collect_string_refs_from_expr(rhs_expr, &mut string_refs);
            // Also collect direct name refs.
            let mut name_refs = Vec::new();
            collect_name_refs_from_expr(rhs_expr, &mut name_refs);
            string_refs.extend(name_refs);
            let _ = map.insert(lhs_name, (string_refs, text_range_to_span(call.range)));
        }
    }
    map
}

fn collect_tat_violations_inner(
    stmts: &[Stmt],
    typevar_names: &std::collections::HashSet<String>,
    class_scope_tvs: &std::collections::HashSet<String>,
    type_names: &std::collections::HashSet<String>,
    out: &mut Vec<TypeAliasTypeViolation>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign(node) => {
                check_tat_call(
                    &node.value,
                    &node.targets,
                    typevar_names,
                    class_scope_tvs,
                    type_names,
                    out,
                );
            }
            Stmt::AnnAssign(node) => {
                if let Some(val) = &node.value {
                    if let Some(lhs_name) = expr_simple_name(&node.target) {
                        check_tat_call_with_name(
                            val,
                            &lhs_name,
                            typevar_names,
                            class_scope_tvs,
                            type_names,
                            out,
                        );
                    }
                }
            }
            Stmt::ClassDef(cls) => {
                // Collect type params from Generic[...] / Protocol[...] bases.
                let mut inner_scope = class_scope_tvs.clone();
                if let Some(args) = cls.arguments.as_ref() {
                    for base in &args.args {
                        if let Expr::Subscript(sub) = base {
                            let is_generic = matches!(sub.value.as_ref(),
                                Expr::Name(n) if n.id.as_str() == "Generic" || n.id.as_str() == "Protocol"
                            );
                            if is_generic {
                                let elts: &[Expr] = match sub.slice.as_ref() {
                                    Expr::Tuple(t) => &t.elts,
                                    other => std::slice::from_ref(other),
                                };
                                for elt in elts {
                                    if let Some(name) = expr_simple_name(elt) {
                                        let _ = inner_scope.insert(name);
                                    }
                                    // Starred `*Ts`
                                    if let Expr::Starred(s) = elt {
                                        if let Some(name) = expr_simple_name(&s.value) {
                                            let _ = inner_scope.insert(name);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // PEP 695 type params
                if let Some(tp) = &cls.type_params {
                    for param in &tp.type_params {
                        let _ = inner_scope.insert(type_param_name(param));
                    }
                }
                collect_tat_violations_inner(
                    &cls.body,
                    typevar_names,
                    &inner_scope,
                    type_names,
                    out,
                );
            }
            Stmt::FunctionDef(func) => {
                collect_tat_violations_inner(
                    &func.body,
                    typevar_names,
                    class_scope_tvs,
                    type_names,
                    out,
                );
            }
            _ => {}
        }
    }
}

fn check_tat_call(
    value: &Expr,
    targets: &[Expr],
    typevar_names: &std::collections::HashSet<String>,
    class_scope_tvs: &std::collections::HashSet<String>,
    type_names: &std::collections::HashSet<String>,
    out: &mut Vec<TypeAliasTypeViolation>,
) {
    let Some(lhs_name) = targets.first().and_then(expr_simple_name) else {
        return;
    };
    check_tat_call_with_name(
        value,
        &lhs_name,
        typevar_names,
        class_scope_tvs,
        type_names,
        out,
    );
}

fn check_tat_call_with_name(
    value: &Expr,
    lhs_name: &str,
    typevar_names: &std::collections::HashSet<String>,
    class_scope_tvs: &std::collections::HashSet<String>,
    type_names: &std::collections::HashSet<String>,
    out: &mut Vec<TypeAliasTypeViolation>,
) {
    let Expr::Call(call) = value else { return };
    let Some(callee) = expr_simple_name(&call.func) else {
        return;
    };
    if callee != "TypeAliasType" {
        return;
    }

    // Check the second argument (the type expression).
    if let Some(rhs_expr) = call.arguments.args.get(1) {
        // Check 1: Invalid type expression.
        let syntactically_invalid = !is_valid_type_expression(rhs_expr, true);
        // Also flag bare Name references to non-type variables (e.g. `var1 = 3`).
        let semantic_invalid = if let Expr::Name(n) = rhs_expr {
            let ref_name = n.id.as_str();
            !type_names.contains(ref_name)
                && !typevar_names.contains(ref_name)
                && !class_scope_tvs.contains(ref_name)
        } else {
            false
        };
        if syntactically_invalid || semantic_invalid {
            out.push(TypeAliasTypeViolation {
                span: text_range_to_span(rhs_expr.range()),
                kind: TypeAliasTypeViolationKind::InvalidTypeExpression,
                alias_name: lhs_name.to_owned(),
            });
        }

        // Check 2: Circular self-reference (direct name ref or string ref).
        // When type_params is present, string forward references to the alias
        // itself are valid (recursive type alias) ONLY when using the same
        // type params, not concrete types.
        let tp_names_for_circular = extract_type_params_names(&call.arguments);
        if expr_has_circular_self_ref(rhs_expr, lhs_name, tp_names_for_circular.as_ref()) {
            out.push(TypeAliasTypeViolation {
                span: text_range_to_span(call.range),
                kind: TypeAliasTypeViolationKind::CircularReference,
                alias_name: lhs_name.to_owned(),
            });
        }

        // Check 3: TypeVars used in value but not declared in type_params.
        // TypeVars from an enclosing class scope (Generic[T]) are always in scope.
        let type_params_names = extract_type_params_names(&call.arguments);
        if let Some(ref tp_names) = type_params_names {
            let mut used_names = Vec::new();
            collect_name_refs_from_expr(rhs_expr, &mut used_names);
            for name in &used_names {
                if typevar_names.contains(name.as_str())
                    && !tp_names.contains(name.as_str())
                    && !class_scope_tvs.contains(name.as_str())
                {
                    out.push(TypeAliasTypeViolation {
                        span: text_range_to_span(call.range),
                        kind: TypeAliasTypeViolationKind::UndeclaredTypeVar {
                            typevar_name: name.clone(),
                        },
                        alias_name: lhs_name.to_owned(),
                    });
                }
            }
        } else {
            // No type_params kwarg: any TypeVar usage is an error unless
            // the TypeVar is from an enclosing class scope.
            let mut used_names = Vec::new();
            collect_name_refs_from_expr(rhs_expr, &mut used_names);
            let has_type_params_kwarg = call
                .arguments
                .keywords
                .iter()
                .any(|kw| kw.arg.as_ref().is_some_and(|a| a.as_str() == "type_params"));
            if !has_type_params_kwarg {
                for name in &used_names {
                    if typevar_names.contains(name.as_str())
                        && !class_scope_tvs.contains(name.as_str())
                    {
                        out.push(TypeAliasTypeViolation {
                            span: text_range_to_span(call.range),
                            kind: TypeAliasTypeViolationKind::UndeclaredTypeVar {
                                typevar_name: name.clone(),
                            },
                            alias_name: lhs_name.to_owned(),
                        });
                    }
                }
            }
        }
    }

    // Check 4: Non-literal type_params keyword.
    for kw in &call.arguments.keywords {
        if kw.arg.as_ref().is_some_and(|a| a.as_str() == "type_params")
            && !matches!(&kw.value, Expr::Tuple(_))
        {
            out.push(TypeAliasTypeViolation {
                span: text_range_to_span(kw.value.range()),
                kind: TypeAliasTypeViolationKind::NonLiteralTypeParams,
                alias_name: lhs_name.to_owned(),
            });
        }
    }
}

/// Check if an expression contains a DIRECT circular self-reference.
///
/// A direct self-reference is:
/// - A `Name` node matching the alias name (e.g. `list[BadAlias21]`)
/// - A string literal that IS exactly the alias name (e.g. `"BadAlias4"`)
/// - A string like `"Name[concrete_type]"` where the args are NOT all type params
///
/// When `type_params` contains all the type parameter names, string forward
/// references like `"GoodAlias[T, S]"` that use ONLY those params are valid
/// recursive aliases and NOT flagged.
fn expr_has_circular_self_ref(
    expr: &Expr,
    name: &str,
    type_params: Option<&std::collections::HashSet<String>>,
) -> bool {
    match expr {
        // Direct name reference: `list[BadAlias21]`
        Expr::Name(n) => n.id.as_str() == name,
        // String literal checks.
        Expr::StringLiteral(s) => {
            let text = s.value.to_str().trim();
            if text == name {
                // Bare self-reference: always circular.
                return true;
            }
            // Check for `Name[...]` pattern.
            if let Some(rest) = text.strip_prefix(name) {
                if rest.starts_with('[') {
                    // Has type_params? Check if the args are all type params.
                    if let Some(tp_names) = type_params {
                        // Extract the args between [ and ].
                        let inner = &rest[1..rest.len().saturating_sub(1)];
                        let args: Vec<&str> = inner.split(',').map(str::trim).collect();
                        // If ALL args are type param names, it's a valid recursive ref.
                        let all_type_params = args.iter().all(|a| tp_names.contains(*a));
                        // Only flag if NOT all type params (i.e. uses concrete types).
                        return !all_type_params;
                    }
                    // No type_params: always circular.
                    return true;
                }
            }
            false
        }
        Expr::Subscript(sub) => {
            expr_has_circular_self_ref(&sub.value, name, type_params)
                || expr_has_circular_self_ref(&sub.slice, name, type_params)
        }
        Expr::BinOp(bin) => {
            expr_has_circular_self_ref(&bin.left, name, type_params)
                || expr_has_circular_self_ref(&bin.right, name, type_params)
        }
        Expr::Tuple(t) => t
            .elts
            .iter()
            .any(|e| expr_has_circular_self_ref(e, name, type_params)),
        Expr::Starred(s) => expr_has_circular_self_ref(&s.value, name, type_params),
        _ => false,
    }
}

/// Extract the names from a `type_params=(T, S, ...)` keyword argument.
fn extract_type_params_names(
    arguments: &ruff_python_ast::Arguments,
) -> Option<std::collections::HashSet<String>> {
    for kw in &arguments.keywords {
        if kw.arg.as_ref().is_some_and(|a| a.as_str() == "type_params") {
            let Expr::Tuple(tuple) = &kw.value else {
                return None;
            };
            let names: std::collections::HashSet<String> =
                tuple.elts.iter().filter_map(expr_simple_name).collect();
            return Some(names);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Invalid annotation collection (e.g. bare ellipsis in tuple subscript)
// ---------------------------------------------------------------------------
