//! Implements [`generics_syntax_scoping`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! AST-driven PEP 695 scoping checks (violations 1-6) for `generics_syntax_scoping`.
//!
//! Every check consumes [`basilisk_resolver::Pep695Scoping`] — facts derived
//! from `ruff_python_ast` nodes — so string/comment/docstring content can never
//! be mistaken for real `class` / `def` / `type` declarations.

use std::collections::HashSet;

use basilisk_resolver::{GenericDefKind, Pep695AliasDef, Pep695Param, Pep695Scoping, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};

use super::CODE;

// ---------------------------------------------------------------------------
// Violation 1: a type-param bound references another param in the same list
// ---------------------------------------------------------------------------

/// Per PEP 695, a type parameter's bound must not reference *any* other type
/// parameter declared in the same list (whether earlier or later).
pub(super) fn check_bound_cross_references(
    scoping: &Pep695Scoping,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for def in &scoping.defs {
        check_param_list_cross_ref(&def.params, def.def_span, path, diagnostics);
    }
    for alias in &scoping.aliases {
        check_param_list_cross_ref(&alias.params, alias.name_span, path, diagnostics);
    }
}

fn check_param_list_cross_ref(
    params: &[Pep695Param],
    span: Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if params.len() < 2 {
        return;
    }
    let names: Vec<&str> = params.iter().map(|p| p.name.as_str()).collect();
    for (idx, param) in params.iter().enumerate() {
        let Some(other) = param.bound_refs.iter().find(|reference| {
            names
                .iter()
                .enumerate()
                .any(|(other_idx, name)| other_idx != idx && *name == reference.as_str())
        }) else {
            continue;
        };
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "PEP 695 type parameter `{}` bound references `{other}` from the same \
                 type parameter list",
                param.name
            ),
            span,
            path,
            Some(
                "Type parameter bounds cannot reference other type parameters in the same list"
                    .to_owned(),
            ),
            Some(
                "PEP 695: a compiler error is generated if the definition of a type parameter \
                 references another type parameter in the same list"
                    .to_owned(),
            ),
        ));
    }
}

// ---------------------------------------------------------------------------
// Violation 2a: a PEP 695 type parameter used at module scope
// ---------------------------------------------------------------------------

/// Type parameters are only in scope inside the generic class/function where
/// they are declared. A module-scope reference (e.g. `print(T)`) is a runtime
/// `NameError` unless a module-level binding of the same name precedes it.
pub(super) fn check_module_level_type_param_use(
    scoping: &Pep695Scoping,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let param_names = all_param_names(scoping);
    for (name, span) in &scoping.module_name_refs {
        if !param_names.contains(name.as_str()) {
            continue;
        }
        if has_prior_binding(scoping, name, span.start) {
            continue;
        }
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "PEP 695 type parameter `{name}` is not defined at module scope; it is only \
                 accessible inside the generic class or function where it is declared"
            ),
            *span,
            path,
            Some(format!(
                "`{name}` is a PEP 695 type parameter and is not bound at module scope"
            )),
            Some(
                "PEP 695: type parameter names are only defined inside the body of the generic \
                 class or function"
                    .to_owned(),
            ),
        ));
    }
}

// ---------------------------------------------------------------------------
// Violation 2b: a decorator uses the decorated definition's own type param
// ---------------------------------------------------------------------------

/// Decorators are evaluated *before* the decorated class/function's type
/// parameter scope is entered, so the definition's own type parameters are not
/// available in the decorator expression.
pub(super) fn check_decorator_uses_class_type_param(
    scoping: &Pep695Scoping,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for def in &scoping.defs {
        for decorator in &def.decorators {
            for param in &def.params {
                if !decorator.refs.iter().any(|r| r == &param.name) {
                    continue;
                }
                // A module-level binding of the name before the decorator means
                // the decorator references that variable, not the type param.
                if has_prior_binding(scoping, &param.name, decorator.span.start) {
                    continue;
                }
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "PEP 695 type parameter `{}` is not defined at this point: it belongs \
                         to the decorated definition, not the decorator call",
                        param.name
                    ),
                    decorator.span,
                    path,
                    Some(format!(
                        "`{}` is a type parameter of the decorated definition; it is not in \
                         scope in the decorator arguments",
                        param.name
                    )),
                    Some(
                        "PEP 695: type parameter scopes are entered after the decorator \
                         expressions are evaluated"
                            .to_owned(),
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Violation 3: a method re-declares an enclosing class's type parameter
// ---------------------------------------------------------------------------

/// Inside `class Foo[T]`, a method that declares its own `[T]` shadows the
/// class type parameter — a PEP 695 scoping violation.
pub(super) fn check_method_redefines_class_type_param(
    scoping: &Pep695Scoping,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for def in &scoping.defs {
        if def.kind != GenericDefKind::Function || def.enclosing_class_params.is_empty() {
            continue;
        }
        for param in &def.params {
            if !def.enclosing_class_params.contains(&param.name) {
                continue;
            }
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Method type parameter `{}` shadows the enclosing class's type parameter \
                     of the same name",
                    param.name
                ),
                def.name_span,
                path,
                Some(format!(
                    "Rename the method's type parameter `{}` to avoid shadowing the class type \
                     parameter",
                    param.name
                )),
                Some(
                    "PEP 695: a method that defines its own type parameter with the same name as \
                     an enclosing class type parameter creates a scoping violation"
                        .to_owned(),
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Violation 4: a `type` statement references an old-style TypeVar
// ---------------------------------------------------------------------------

/// PEP 695 `type` aliases must not reference `TypeVar`/`ParamSpec`/
/// `TypeVarTuple` objects created outside the statement's own scope.
pub(super) fn check_type_alias_uses_old_typevar(
    scoping: &Pep695Scoping,
    old_typevar_names: &[&str],
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for alias in &scoping.aliases {
        let new_params: HashSet<&str> = alias.params.iter().map(|p| p.name.as_str()).collect();
        let Some(old_tv) = old_typevar_names
            .iter()
            .find(|name| !new_params.contains(**name) && alias.rhs_refs.iter().any(|r| r == *name))
        else {
            continue;
        };
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!("PEP 695 `type` statement uses old-style TypeVar `{old_tv}`"),
            alias.name_span,
            path,
            Some(format!(
                "Use PEP 695 type parameter syntax instead: `type Alias[{old_tv}] = ...`"
            )),
            Some(
                "PEP 695: type aliases defined with `type` must not reference TypeVars created \
                 outside the statement's scope"
                    .to_owned(),
            ),
        ));
    }
}

// ---------------------------------------------------------------------------
// Violation 5: a `type` statement inside a function body
// ---------------------------------------------------------------------------

/// PEP 695 type aliases are only valid at module or class scope.
pub(super) fn check_type_alias_in_function(
    scoping: &Pep695Scoping,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for alias in &scoping.aliases {
        if !alias.in_function {
            continue;
        }
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            "PEP 695 `type` statement is not allowed inside a function body".to_owned(),
            alias.name_span,
            path,
            Some("Move the type alias to module or class scope".to_owned()),
            Some(
                "PEP 695: type aliases defined with `type` are only valid at module or class scope"
                    .to_owned(),
            ),
        ));
    }
}

// ---------------------------------------------------------------------------
// Violation 6: a circular `type` alias definition
// ---------------------------------------------------------------------------

/// A `type` alias is circular when its recursion fails the Stage 3
/// acceptance conditions ([TYPEINF-TARGET-TYPELEVEL],
/// [`crate::tyeval::accept`]): **unguarded** self-reference (`type X = X`,
/// `type X = int | X` — union arms do not guard, so no weak head normal
/// form exists) or **non-regular** self-application (arguments grow per
/// unfold, e.g. `type R[T] = set[R[list[T]]]`). Ordinary guarded recursion
/// — `type J = list[J]`, the canonical `JsonValue` union, identity- or
/// ground-argument applications — is the PEP 695-mandated valid form and
/// produces NO diagnostic
/// ([#371](https://github.com/Nimblesite/Basilisk/issues/371)).
pub(super) fn check_type_alias_circular(
    module: &basilisk_resolver::ResolvedModule,
    scoping: &Pep695Scoping,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use crate::tyeval::{classify, lower_module_aliases, Acceptance};

    if let Some(parsed) = crate::rules::shared::parse_module(module) {
        for lowered in lower_module_aliases(&parsed.ast) {
            let detail = match classify(&lowered.name, &lowered.def) {
                Acceptance::Accepted => continue,
                Acceptance::Unguarded => "references itself",
                Acceptance::NonRegular => "references itself with different type arguments",
            };
            push_circular(
                &lowered.name,
                crate::span_util::text_range_to_span(lowered.name_range),
                detail,
                path,
                diagnostics,
            );
        }
    }

    check_mutual_alias_cycles(scoping, path, diagnostics);
}

/// Detect *mutual* / longer cycles between aliases connected by bare references
/// (`type A = B`, `type B = A`). Only top-level bare references count — recursion
/// through a container (`type A = list[B]`) terminates and is legitimate, so it
/// is excluded via `rhs_bare_refs`.
fn check_mutual_alias_cycles(
    scoping: &Pep695Scoping,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use std::collections::HashMap;

    let alias_by_name: HashMap<&str, &Pep695AliasDef> = scoping
        .aliases
        .iter()
        .map(|a| (a.name.as_str(), a))
        .collect();

    for alias in &scoping.aliases {
        if reaches_self(&alias.name, alias, &alias_by_name) {
            push_circular(
                &alias.name,
                alias.name_span,
                "is part of a circular alias chain",
                path,
                diagnostics,
            );
        }
    }
}

/// `true` when following bare references from `current` returns to `start`
/// through a chain of length ≥ 2 (a self-loop is handled separately).
fn reaches_self(
    start: &str,
    current: &Pep695AliasDef,
    alias_by_name: &std::collections::HashMap<&str, &Pep695AliasDef>,
) -> bool {
    let mut visited = std::collections::HashSet::new();
    let mut stack: Vec<&str> = current
        .rhs_bare_refs
        .iter()
        .filter(|r| r.as_str() != start) // exclude the trivial self-loop
        .map(String::as_str)
        .collect();
    while let Some(name) = stack.pop() {
        if name == start {
            return true;
        }
        if !visited.insert(name) {
            continue;
        }
        if let Some(next) = alias_by_name.get(name) {
            stack.extend(next.rhs_bare_refs.iter().map(String::as_str));
        }
    }
    false
}

fn push_circular(
    name: &str,
    name_span: Span,
    detail: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!("Circular type alias definition: `{name}` {detail}"),
        name_span,
        path,
        Some(
            "A recursive type alias must reference itself beneath a type constructor \
             (e.g. `type Json = int | list[Json]`) with non-growing type arguments"
                .to_owned(),
        ),
        None,
    ));
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// All PEP 695 type parameter names declared anywhere in the module.
fn all_param_names(scoping: &Pep695Scoping) -> HashSet<&str> {
    scoping
        .defs
        .iter()
        .flat_map(|d| d.params.iter())
        .chain(scoping.aliases.iter().flat_map(|a| a.params.iter()))
        .map(|p| p.name.as_str())
        .collect()
}

/// Is `name` bound at module scope strictly before byte offset `before`?
fn has_prior_binding(scoping: &Pep695Scoping, name: &str, before: u32) -> bool {
    scoping
        .module_bindings
        .iter()
        .any(|(bound, offset)| bound == name && *offset < before)
}
