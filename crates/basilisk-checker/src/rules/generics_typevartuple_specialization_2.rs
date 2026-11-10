//! Implements [`generics_typevartuple_specialization_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_typevartuple_specialization_2`: Invalid `TypeVarTuple` specialization of generic alias.
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

use basilisk_resolver::{BindingTable, ResolvedModule, Span, TypingForm};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use crate::diagnostic::{error_diag_help_note, error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_typevartuple_specialization_2",
    docs_url: "https://www.basilisk-python.dev/errors/generics_typevartuple_specialization_2",
};

/// Emits `generics_typevartuple_specialization_2` for invalid `TypeVarTuple` specializations of generic aliases.
pub(crate) struct TypeVarTupleSpecializationViolation;

impl Rule for TypeVarTupleSpecializationViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let path = &module.path;

        // Collect the names of all TypeVarTuple definitions in this module.
        let typevartuple_names: Vec<&str> =
            basilisk_resolver::collect_names_where(&module.typevar_calls, |tv| tv.is_typevartuple);

        // Collect the names of all regular TypeVar definitions.
        let typevar_names: Vec<&str> = module
            .typevar_calls
            .iter()
            .filter(|tv| !tv.is_typevartuple && !tv.is_paramspec)
            .map(|tv| tv.name.as_str())
            .collect();

        // Re-parse the AST so we can walk statements.
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };

        // Build a map of module-level alias name -> AliasInfo by scanning
        // top-level assignments of the form `Name = <subscript-or-name>`.
        let alias_map = build_alias_map(&parsed.ast.body, &typevartuple_names, &typevar_names);

        if alias_map.is_empty() {
            return;
        }

        // Walk all statements and check subscript expressions.
        check_stmts(
            &parsed.ast.body,
            &alias_map,
            &typevartuple_names,
            &module.bindings,
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
fn build_alias_map(
    stmts: &[Stmt],
    typevartuple_names: &[&str],
    typevar_names: &[&str],
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
        let Some(first_target) = assign.targets.first() else {
            continue;
        };
        let Expr::Name(lhs_name) = first_target else {
            continue;
        };

        let info = analyse_alias_value(&assign.value, typevartuple_names, typevar_names);
        if info.regular_typevar_count > 0 || info.has_typevartuple {
            let _ = map.insert(lhs_name.id.to_string(), info);
        }
    }

    map
}

/// Analyse the RHS of a type alias to count its TypeVar/TypeVarTuple
/// parameters, from the AST: the slice elements of the subscripted RHS.
/// `TypeVar`/`TypeVarTuple` references are matched by declared name identity,
/// never source text ([ASTREBUILD-LAW]).
fn analyse_alias_value(
    value: &Expr,
    typevartuple_names: &[&str],
    typevar_names: &[&str],
) -> AliasInfo {
    let mut regular_typevar_count = 0usize;
    let mut has_typevartuple = false;

    if let Expr::Subscript(sub) = value {
        let elements: Vec<&Expr> = match sub.slice.as_ref() {
            Expr::Tuple(tuple) => tuple.elts.iter().collect(),
            single => vec![single],
        };
        for element in elements {
            match element {
                // Starred argument: `*Ts` — a TypeVarTuple unpack. A starred
                // non-name (`*tuple[...]`) is an unpacked tuple, not a TVT.
                Expr::Starred(starred) => {
                    if let Expr::Name(name) = starred.value.as_ref() {
                        if typevartuple_names.contains(&name.id.as_str()) {
                            has_typevartuple = true;
                        }
                    }
                }
                Expr::Name(name) if typevar_names.contains(&name.id.as_str()) => {
                    regular_typevar_count = regular_typevar_count.saturating_add(1);
                }
                _ => {}
            }
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
    bindings: &BindingTable,
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
                    bindings,
                    path,
                    diagnostics,
                );
            }
            Stmt::Assign(assign) => {
                check_expr(
                    &assign.value,
                    alias_map,
                    typevartuple_names,
                    bindings,
                    path,
                    diagnostics,
                );
                for target in &assign.targets {
                    check_expr(
                        target,
                        alias_map,
                        typevartuple_names,
                        bindings,
                        path,
                        diagnostics,
                    );
                }
            }
            Stmt::AnnAssign(ann_assign) => {
                // Check the annotation expression (e.g. `v1: TA7[int]`).
                check_expr(
                    &ann_assign.annotation,
                    alias_map,
                    typevartuple_names,
                    bindings,
                    path,
                    diagnostics,
                );
                if let Some(value) = &ann_assign.value {
                    check_expr(
                        value,
                        alias_map,
                        typevartuple_names,
                        bindings,
                        path,
                        diagnostics,
                    );
                }
            }
            Stmt::FunctionDef(func) => {
                check_stmts(
                    &func.body,
                    alias_map,
                    typevartuple_names,
                    bindings,
                    path,
                    diagnostics,
                );
            }
            Stmt::ClassDef(cls) => {
                check_stmts(
                    &cls.body,
                    alias_map,
                    typevartuple_names,
                    bindings,
                    path,
                    diagnostics,
                );
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
    bindings: &BindingTable,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Subscript(sub) => {
            // Check the inner slice first (recursively).
            check_expr(
                &sub.slice,
                alias_map,
                typevartuple_names,
                bindings,
                path,
                diagnostics,
            );

            // Get the alias name being specialised.
            let Expr::Name(alias_name) = sub.value.as_ref() else {
                // Recurse into the value expression too.
                check_expr(
                    &sub.value,
                    alias_map,
                    typevartuple_names,
                    bindings,
                    path,
                    diagnostics,
                );
                return;
            };

            let Some(info) = alias_map.get(alias_name.id.as_str()) else {
                return;
            };

            let span = Span::from(sub.range());

            // Determine the provided type arguments from the slice.
            let provided_args = collect_type_args(&sub.slice, bindings);

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
                check_expr(
                    elt,
                    alias_map,
                    typevartuple_names,
                    bindings,
                    path,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

/// Collect the type arguments of a subscript slice expression, classified.
///
/// For `Alias[A, B, C]` the slice will be a `Tuple(A, B, C)` or just `A` when
/// there is a single argument.
fn collect_type_args(slice: &Expr, bindings: &BindingTable) -> Vec<SliceArg> {
    match slice {
        Expr::Tuple(tup) => tup
            .elts
            .iter()
            .map(|elt| classify_slice_elt(elt, bindings))
            .collect(),
        other => vec![classify_slice_elt(other, bindings)],
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

/// Classify one type argument. The unpacked-tuple form is recognised by
/// resolving the starred subscript's base through the binding table to the
/// builtin `tuple` constructor (or its `typing.Tuple` alias) — never by the
/// spelling `tuple` ([ASTREBUILD-LAW]).
fn classify_slice_elt(elt: &Expr, bindings: &BindingTable) -> SliceArg {
    match elt {
        Expr::Starred(starred) => match starred.value.as_ref() {
            Expr::Name(n) => SliceArg::StarredName(n.id.to_string()),
            Expr::Subscript(sub) => {
                if matches!(
                    bindings.form_of_with_builtins(&sub.value),
                    Some(TypingForm::TupleClass | TypingForm::TupleAlias)
                ) {
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
                diagnostics.push(error_diag_help_note(
                    CODE.clone(),
                    format!(
                        "Cannot use unpacked `TypeVarTuple` `*{name}` as a type argument \
                         to `{alias_name}`, which has no `TypeVarTuple` parameter"
                    ),
                    span,
                    path,
                    format!(
                        "`{alias_name}` is parameterised with regular `TypeVar`s only; \
                         use a plain type instead of `*{name}`"
                    ),
                    "PEP 646: a `TypeVarTuple` unpack `*Ts` may only be used to \
                     specialise a generic that contains a `TypeVarTuple` parameter",
                ));
                return;
            }
            SliceArg::StarredTuple => {
                diagnostics.push(error_diag_help_note(
                    CODE.clone(),
                    format!(
                        "Cannot use unpacked tuple `*tuple[...]` as a type argument \
                         to `{alias_name}`, which has no `TypeVarTuple` parameter"
                    ),
                    span,
                    path,
                    format!(
                        "`{alias_name}` is parameterised with regular `TypeVar`s only; \
                         provide plain type arguments instead of `*tuple[...]`"
                    ),
                    "PEP 646: `*tuple[T, ...]` may only appear in the argument list \
                     of a generic that contains a `TypeVarTuple` parameter",
                ));
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
    // Count plain arguments plus starred *tuple* unpacks. A bare TypeVarTuple
    // unpack (`*Ts`) absorbs variadic positions but cannot fill regular TypeVar
    // slots. An unbounded `*tuple[X, ...]` unpack, however, can fill both the
    // TypeVarTuple and the remaining TypeVar slots, so it counts toward the
    // minimum required arguments.
    let plain_count = args
        .iter()
        .filter(|a| matches!(a, SliceArg::Plain | SliceArg::StarredTuple))
        .count();
    let provided = args.len();

    if plain_count < min_required {
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "`{alias_name}` requires at least {min_required} plain type argument{} \
                 (one per regular `TypeVar`), but {plain_count} {} provided \
                 (out of {provided} total)",
                if min_required == 1 { "" } else { "s" },
                if plain_count == 1 { "was" } else { "were" },
            ),
            span,
            path,
            Some(format!(
                "Supply at least {min_required} type argument{} to satisfy the \
                 regular `TypeVar` parameter{} of `{alias_name}`",
                if min_required == 1 { "" } else { "s" },
                if min_required == 1 { "" } else { "s" },
            )),
            Some(
                "PEP 646: when a generic alias contains both a `TypeVarTuple` and \
                 regular `TypeVar`s, every specialisation must provide at least as \
                 many arguments as there are regular `TypeVar`s"
                    .to_owned(),
            ),
        ));
    }
}
