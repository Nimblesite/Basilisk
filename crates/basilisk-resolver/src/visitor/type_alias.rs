//! Type Alias visitor functions.

use ruff_python_ast::{Expr, Stmt, TypeParam};
use ruff_text_size::Ranged;

use crate::scope::{
    NamedTupleDefInfo, NewTypeCallInfo, Span, TypeAliasDefInfo, TypeAliasTypeCallInfo,
    TypeStatementInfo,
};

use super::class_info_ext::expr_simple_name;
use super::core::text_range_to_span;
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
            let Some(name) = expr_simple_name(&assign.targets[0]) else {
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

    let span = Span {
        start: stmt.range().start().to_u32(),
        end: stmt.range().end().to_u32(),
    };

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
// BSK-E0099: Protocol instantiation violation detection
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
// Invalid annotation collection (e.g. bare ellipsis in tuple subscript)
// ---------------------------------------------------------------------------
