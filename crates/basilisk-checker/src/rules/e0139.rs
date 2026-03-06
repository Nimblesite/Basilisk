//! BSK-E0139: Invalid `TypeVarTuple` specialization of generic alias.
//!
//! Two related violations are detected:
//!
//! 1. **Unpack in non-TypeVarTuple generic**: When a generic alias is defined
//!    using only regular `TypeVar`s (no `TypeVarTuple`), you cannot specialise
//!    it with an unpacked `TypeVarTuple` (`*Ts`) or an unpacked homogeneous
//!    tuple (`*tuple[T, ...]`).
//!
//! ```python
//! T = TypeVar("T")
//! IntTupleGeneric = tuple[int, T]
//!
//! IntTupleGeneric[str]              # OK
//! IntTupleGeneric[*Ts]              # E — Ts is a TypeVarTuple, not a TypeVar
//! IntTupleGeneric[*tuple[float, ...]]  # E — unpacked tuple not allowed here
//! ```
//!
//! 2. **Too few type arguments for TypeVarTuple+TypeVar alias**: When a
//!    generic alias contains both a `TypeVarTuple` and one or more regular
//!    `TypeVar`s, every specialisation must supply at least as many arguments
//!    as there are regular `TypeVar`s (the `TypeVarTuple` absorbs the rest).
//!
//! ```python
//! T1, T2 = TypeVar("T1"), TypeVar("T2")
//! Ts = TypeVarTuple("Ts")
//! TA7 = tuple[*Ts, T1, T2]
//!
//! v1: TA7[int]         # E — requires at least two type arguments (T1, T2)
//! v2: TA7[int, str]    # OK — T1=int, T2=str, Ts=()
//! ```

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0139",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0139",
};

/// Emits BSK-E0139 for invalid `TypeVarTuple` specializations of generic aliases.
pub(crate) struct TypeVarTupleSpecializationViolation;

impl Rule for TypeVarTupleSpecializationViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Collect the names of all TypeVarTuple definitions in this module.
        let typevartuple_names: Vec<&str> = module
            .typevar_calls
            .iter()
            .filter(|tv| tv.is_typevartuple)
            .map(|tv| tv.name.as_str())
            .collect();

        // Collect the names of all regular TypeVar definitions.
        let typevar_names: Vec<&str> = module
            .typevar_calls
            .iter()
            .filter(|tv| !tv.is_typevartuple && !tv.is_paramspec)
            .map(|tv| tv.name.as_str())
            .collect();

        // Re-parse the AST so we can walk statements.
        let Ok(parsed) = basilisk_parser::parse_source(source.clone(), path.clone()) else {
            return;
        };

        // Build a map of module-level alias name -> AliasInfo by scanning
        // top-level assignments of the form `Name = <subscript-or-name>`.
        let alias_map = build_alias_map(
            &parsed.ast.body,
            source,
            &typevartuple_names,
            &typevar_names,
        );

        if alias_map.is_empty() {
            return;
        }

        // Walk all statements and check subscript expressions.
        check_stmts(
            &parsed.ast.body,
            &alias_map,
            &typevartuple_names,
            path,
            diagnostics,
        );
    }
}

/// Information about a module-level type alias.
#[derive(Debug)]
struct AliasInfo {
    /// Number of regular `TypeVars` in the alias definition.
    regular_typevar_count: usize,
    /// Whether the alias contains a `TypeVarTuple`.
    has_typevartuple: bool,
}

/// Build a map from alias name to `AliasInfo` by scanning module-level
/// assignments like `Name = tuple[int, T]` or `Name = tuple[*Ts, T1, T2]`.
fn build_alias_map<'a>(
    stmts: &[Stmt],
    source: &str,
    typevartuple_names: &[&'a str],
    typevar_names: &[&'a str],
) -> std::collections::HashMap<String, AliasInfo> {
    let mut map = std::collections::HashMap::new();

    for stmt in stmts {
        // Plain assignment: `Name = tuple[...]` or `Name = SomeAlias[...]`
        let Stmt::Assign(assign) = stmt else {
            continue;
        };
        if assign.targets.len() != 1 {
            continue;
        }
        let Expr::Name(lhs_name) = &assign.targets[0] else {
            continue;
        };

        // Get the RHS text from source.
        let rhs_range = assign.value.range();
        let Some(rhs_text) = source.get(rhs_range.start().to_usize()..rhs_range.end().to_usize())
        else {
            continue;
        };

        let info = analyse_alias_rhs(rhs_text.trim(), typevartuple_names, typevar_names);
        if info.regular_typevar_count > 0 || info.has_typevartuple {
            map.insert(lhs_name.id.to_string(), info);
        }
    }

    map
}

/// Analyse the RHS of a type alias to count its TypeVar/TypeVarTuple parameters.
fn analyse_alias_rhs(rhs: &str, typevartuple_names: &[&str], typevar_names: &[&str]) -> AliasInfo {
    // If no brackets, this might be a bare name — treat as zero params.
    let Some(bracket_pos) = rhs.find('[') else {
        return AliasInfo {
            regular_typevar_count: 0,
            has_typevartuple: false,
        };
    };

    let Some(inner) = rhs.get(bracket_pos + 1..rhs.len().saturating_sub(1)) else {
        return AliasInfo {
            regular_typevar_count: 0,
            has_typevartuple: false,
        };
    };

    let args = split_top_level_commas(inner);
    let mut regular_typevar_count = 0usize;
    let mut has_typevartuple = false;

    for arg in &args {
        let arg = arg.trim();
        // Starred argument: `*Ts` — a TypeVarTuple unpack.
        if let Some(name) = arg.strip_prefix('*') {
            let name = name.trim();
            // `*tuple[...]` is an unpacked homogeneous tuple, not a TVT name.
            if !name.starts_with("tuple[") && typevartuple_names.contains(&name) {
                has_typevartuple = true;
            }
        } else if typevar_names.contains(&arg) {
            regular_typevar_count = regular_typevar_count.saturating_add(1);
        }
    }

    AliasInfo {
        regular_typevar_count,
        has_typevartuple,
    }
}

/// Walk statements to find subscript expressions that specialise known aliases.
fn check_stmts(
    stmts: &[Stmt],
    alias_map: &std::collections::HashMap<String, AliasInfo>,
    typevartuple_names: &[&str],
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Expr(expr_stmt) => {
                check_expr(
                    &expr_stmt.value,
                    alias_map,
                    typevartuple_names,
                    path,
                    diagnostics,
                );
            }
            Stmt::Assign(assign) => {
                check_expr(
                    &assign.value,
                    alias_map,
                    typevartuple_names,
                    path,
                    diagnostics,
                );
                for target in &assign.targets {
                    check_expr(target, alias_map, typevartuple_names, path, diagnostics);
                }
            }
            Stmt::AnnAssign(ann_assign) => {
                // Check the annotation expression (e.g. `v1: TA7[int]`).
                check_expr(
                    &ann_assign.annotation,
                    alias_map,
                    typevartuple_names,
                    path,
                    diagnostics,
                );
                if let Some(value) = &ann_assign.value {
                    check_expr(value, alias_map, typevartuple_names, path, diagnostics);
                }
            }
            Stmt::FunctionDef(func) => {
                check_stmts(&func.body, alias_map, typevartuple_names, path, diagnostics);
            }
            Stmt::ClassDef(cls) => {
                check_stmts(&cls.body, alias_map, typevartuple_names, path, diagnostics);
            }
            _ => {}
        }
    }
}

/// Check a single expression for invalid alias specialisations.
fn check_expr(
    expr: &Expr,
    alias_map: &std::collections::HashMap<String, AliasInfo>,
    typevartuple_names: &[&str],
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Subscript(sub) => {
            // Check the inner slice first (recursively).
            check_expr(&sub.slice, alias_map, typevartuple_names, path, diagnostics);

            // Get the alias name being specialised.
            let Expr::Name(alias_name) = sub.value.as_ref() else {
                // Recurse into the value expression too.
                check_expr(&sub.value, alias_map, typevartuple_names, path, diagnostics);
                return;
            };

            let Some(info) = alias_map.get(alias_name.id.as_str()) else {
                return;
            };

            let span = Span {
                start: sub.range().start().to_u32(),
                end: sub.range().end().to_u32(),
            };

            // Determine the provided type arguments from the slice.
            let provided_args = collect_type_args(&sub.slice);

            if info.has_typevartuple {
                // Violation 2: alias has a TypeVarTuple — must supply at least
                // `regular_typevar_count` arguments.
                check_too_few_args_for_tvt_alias(
                    &provided_args,
                    alias_name.id.as_str(),
                    info.regular_typevar_count,
                    span,
                    path,
                    diagnostics,
                );
            } else {
                // Violation 1: alias has only regular TypeVars — no unpacked
                // TypeVarTuple or *tuple[...] arguments are allowed.
                check_unpack_in_plain_generic(
                    &provided_args,
                    alias_name.id.as_str(),
                    typevartuple_names,
                    span,
                    path,
                    diagnostics,
                );
            }
        }
        Expr::Tuple(tup) => {
            for elt in &tup.elts {
                check_expr(elt, alias_map, typevartuple_names, path, diagnostics);
            }
        }
        _ => {}
    }
}

/// Collect the type arguments of a subscript slice expression as text tokens.
///
/// For `Alias[A, B, C]` the slice will be a `Tuple(A, B, C)` or just `A` when
/// there is a single argument.
fn collect_type_args(slice: &Expr) -> Vec<SliceArg> {
    match slice {
        Expr::Tuple(tup) => tup.elts.iter().map(classify_slice_elt).collect(),
        other => vec![classify_slice_elt(other)],
    }
}

/// Classification of a single type argument in a subscript.
#[derive(Debug)]
enum SliceArg {
    /// `*Ts` — a starred `TypeVarTuple` name.
    StarredName(String),
    /// `*tuple[...]` — a starred unpacked homogeneous tuple.
    StarredTuple,
    /// Any other (plain) type argument.
    Plain,
}

fn classify_slice_elt(elt: &Expr) -> SliceArg {
    match elt {
        Expr::Starred(starred) => match starred.value.as_ref() {
            Expr::Name(n) => SliceArg::StarredName(n.id.to_string()),
            Expr::Subscript(sub) => {
                if matches!(sub.value.as_ref(), Expr::Name(n) if n.id.as_str() == "tuple") {
                    SliceArg::StarredTuple
                } else {
                    SliceArg::StarredName(String::new())
                }
            }
            _ => SliceArg::StarredName(String::new()),
        },
        _ => SliceArg::Plain,
    }
}

/// Check that no unpacked `TypeVarTuple` or `*tuple[...]` arguments are used
/// when specialising a generic alias that contains only regular `TypeVars`.
fn check_unpack_in_plain_generic(
    args: &[SliceArg],
    alias_name: &str,
    typevartuple_names: &[&str],
    span: Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for arg in args {
        match arg {
            SliceArg::StarredName(name) if typevartuple_names.contains(&name.as_str()) => {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Cannot use unpacked `TypeVarTuple` `*{name}` as a type argument \
                         to `{alias_name}`, which has no `TypeVarTuple` parameter"
                    ),
                    span,
                    path: path.to_owned(),
                    help: Some(format!(
                        "`{alias_name}` is parameterised with regular `TypeVar`s only; \
                         use a plain type instead of `*{name}`"
                    )),
                    note: Some(
                        "PEP 646: a `TypeVarTuple` unpack `*Ts` may only be used to \
                         specialise a generic that contains a `TypeVarTuple` parameter"
                            .to_owned(),
                    ),
                });
                return;
            }
            SliceArg::StarredTuple => {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Cannot use unpacked tuple `*tuple[...]` as a type argument \
                         to `{alias_name}`, which has no `TypeVarTuple` parameter"
                    ),
                    span,
                    path: path.to_owned(),
                    help: Some(format!(
                        "`{alias_name}` is parameterised with regular `TypeVar`s only; \
                         provide plain type arguments instead of `*tuple[...]`"
                    )),
                    note: Some(
                        "PEP 646: `*tuple[T, ...]` may only appear in the argument list \
                         of a generic that contains a `TypeVarTuple` parameter"
                            .to_owned(),
                    ),
                });
                return;
            }
            _ => {}
        }
    }
}

/// Check that a TypeVarTuple+TypeVar alias receives at least
/// `min_required` type arguments when specialised.
fn check_too_few_args_for_tvt_alias(
    args: &[SliceArg],
    alias_name: &str,
    min_required: usize,
    span: Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Count only the plain (non-starred) arguments, because a starred
    // TypeVarTuple argument can satisfy multiple TypeVar slots.
    // If the caller uses `*Ts` we cannot statically determine the count, so
    // skip the check.
    let has_starred = args.iter().any(|a| !matches!(a, SliceArg::Plain));
    if has_starred {
        return;
    }

    let provided = args.len();
    if provided < min_required {
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "`{alias_name}` requires at least {min_required} type argument{} \
                 (one per regular `TypeVar`), but {provided} {} provided",
                if min_required == 1 { "" } else { "s" },
                if provided == 1 { "was" } else { "were" },
            ),
            span,
            path: path.to_owned(),
            help: Some(format!(
                "Supply at least {min_required} type argument{} to satisfy the \
                 regular `TypeVar` parameter{} of `{alias_name}`",
                if min_required == 1 { "" } else { "s" },
                if min_required == 1 { "" } else { "s" },
            )),
            note: Some(
                "PEP 646: when a generic alias contains both a `TypeVarTuple` and \
                 regular `TypeVar`s, every specialisation must provide at least as \
                 many arguments as there are regular `TypeVar`s"
                    .to_owned(),
            ),
        });
    }
}

/// Split a comma-separated type argument list at top-level commas,
/// respecting bracket nesting.
fn split_top_level_commas(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' | '(' => depth = depth.saturating_add(1),
            ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&inner[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    let last = &inner[start..];
    if !last.trim().is_empty() {
        parts.push(last);
    }
    parts
}
