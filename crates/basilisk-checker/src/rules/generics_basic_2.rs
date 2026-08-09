//! Implements [`generics_basic_2`] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
//! `generics_basic_2`: a non-type-variable argument in `Generic[...]` or
//! `Protocol[...]`.
//!
//! [PEP 484](https://peps.python.org/pep-0484/#generics) requires every
//! argument of `Generic[...]` to be a type variable, and
//! [PEP 544](https://peps.python.org/pep-0544/#generic-protocols) says the same
//! of `Protocol[...]`. A concrete type in that position is an error:
//!
//! ```python
//! class Bad1(Generic[int]): ...      # E — `int` is not a type variable
//! class Bad2(Protocol[int]): ...     # E — `int` is not a type variable
//! ```
//!
//! # How the verdict is reached
//!
//! REBUILT. The deleted body read `ClassInfo::base_expression_names`, a
//! `Vec<String>` of RENDERED simple names, and matched them against a set of
//! type-variable names harvested the same way. That made the diagnostic a
//! function of spelling in both directions — an aliased type variable was
//! invisible, and an unrelated class spelled like one was accepted.
//!
//! Almost nothing here reads a name, and the one place that does is called out
//! below rather than hidden. Each piece of the judgment is a resolution:
//!
//! * **Is this base `Generic`/`Protocol`?** — the subscript's head expression
//!   is resolved through the module's binding table to a [`TypingForm`], so
//!   `from typing import Generic as G`, `typing.Generic`, and
//!   `typing_extensions.Protocol` all count, and a local `class Generic` does
//!   not.
//! * **Is this argument a type variable?** — two ways, in the order Python
//!   binds them. A legacy `T = TypeVar("T")` is reached by following the
//!   name's binding to the EXPRESSION it was assigned, and asking whether that
//!   expression calls a type-parameter factory; that is definition-site
//!   identity and holds through any number of aliases. `*Ts` and `Unpack[Ts]`
//!   ([PEP 646](https://peps.python.org/pep-0646/)) unwrap to the same
//!   question.
//!
//!   A [PEP 695](https://peps.python.org/pep-0695/) type parameter is the
//!   exception: it is matched by comparing the argument's spelling against the
//!   names in the class's own `type_params` list. That IS a string comparison.
//!   It is sound only because of how narrow it is — the type-parameter scope
//!   is opened by the `class` statement itself and closed at the end of the
//!   base list, it binds exactly the names in that list, and nothing can
//!   shadow or rebind them in between — so name equality within it is the
//!   scope lookup rather than a guess about what a spelling might mean. It is
//!   still the weakest link here, and it would become wrong the moment this
//!   comparison were reused anywhere the scope is not that small.
//!
//!   The lawful replacement is a scoped binding environment that resolves a
//!   PEP 695 parameter use to its declaration NODE. That does not exist yet.
//! * **Is this argument provably NOT one?** — it resolves to a class this
//!   module defines, to a canonical form that is not a type-parameter factory,
//!   or it is a literal. Anything else is UNKNOWN and reports nothing: an
//!   argument imported from a module this checker cannot see may well be a
//!   type variable ([CHKARCH-CONFORMANCE-MODE]).

use std::collections::HashSet;

use basilisk_resolver::{BindingTable, ResolvedModule, TypingForm};
use ruff_python_ast::{Expr, Stmt, StmtClassDef, TypeParam};
use ruff_text_size::{Ranged, TextRange};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::text_range_to_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_basic_2",
    docs_url: "https://www.basilisk-python.dev/errors/generics_basic_2",
};

fn make_diagnostic(message: String, span: basilisk_resolver::Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        message,
        span,
        path,
        Some(
            "All arguments to `Generic[...]` must be TypeVar, TypeVarTuple, \
             or ParamSpec instances"
                .to_owned(),
        ),
        Some("PEP 484: `Generic[int]` is invalid; use a TypeVar instead".to_owned()),
    )
}

/// What an argument inside `Generic[...]` / `Protocol[...]` was resolved to be.
///
/// Three-valued on purpose. The rule reports on [`Self::NotATypeVariable`] and
/// on nothing else, so an argument this module cannot resolve produces silence
/// rather than a guess ([CHKARCH-CONFORMANCE-MODE]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Argument {
    /// Resolved to a type parameter: a PEP 695 type param, or a name bound to
    /// a `TypeVar`/`TypeVarTuple`/`ParamSpec` construction.
    TypeVariable,
    /// Resolved to something that provably is not one.
    NotATypeVariable,
    /// Resolved to nothing this module can see.
    Unknown,
}

/// Emits `generics_basic_2` when a non-type-variable appears in `Generic[...]`
/// or `Protocol[...]`.
pub(crate) struct NonTypeVarInGeneric;

impl Rule for NonTypeVarInGeneric {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };
        let bindings = &module.bindings;
        let factories = type_parameter_constructions(bindings, &parsed.ast.body);
        let mut classes = Vec::new();
        collect_class_defs(&parsed.ast.body, &mut classes);
        for class in classes {
            check_class(class, bindings, &factories, &module.path, diagnostics);
        }
    }
}

/// Every class statement in the module, including those nested in functions
/// and other classes.
fn collect_class_defs<'a>(body: &'a [Stmt], out: &mut Vec<&'a StmtClassDef>) {
    for stmt in body {
        match stmt {
            Stmt::ClassDef(class) => {
                out.push(class);
                collect_class_defs(&class.body, out);
            }
            Stmt::FunctionDef(func) => collect_class_defs(&func.body, out),
            Stmt::If(stmt) => {
                collect_class_defs(&stmt.body, out);
                for clause in &stmt.elif_else_clauses {
                    collect_class_defs(&clause.body, out);
                }
            }
            Stmt::While(stmt) => {
                collect_class_defs(&stmt.body, out);
                collect_class_defs(&stmt.orelse, out);
            }
            Stmt::For(stmt) => {
                collect_class_defs(&stmt.body, out);
                collect_class_defs(&stmt.orelse, out);
            }
            Stmt::With(stmt) => collect_class_defs(&stmt.body, out),
            Stmt::Try(stmt) => {
                collect_class_defs(&stmt.body, out);
                collect_class_defs(&stmt.orelse, out);
                collect_class_defs(&stmt.finalbody, out);
                for handler in &stmt.handlers {
                    let ruff_python_ast::ExceptHandler::ExceptHandler(handler) = handler;
                    collect_class_defs(&handler.body, out);
                }
            }
            Stmt::Match(stmt) => {
                for case in &stmt.cases {
                    collect_class_defs(&case.body, out);
                }
            }
            _ => {}
        }
    }
}

/// Check one class's `Generic[...]` / `Protocol[...]` bases.
fn check_class(
    class: &StmtClassDef,
    bindings: &BindingTable,
    factories: &HashSet<TextRange>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(arguments) = class.arguments.as_deref() else {
        return;
    };
    // The one string comparison in this rule; see the module banner. A PEP 695
    // type-parameter list opens a scope that immediately encloses the base
    // list and binds exactly the parameters it declares, so within that scope
    // name equality is the lookup. Collected as `&str` because there is no
    // scoped binding environment to resolve the use site against yet.
    let type_params: Vec<&str> = class
        .type_params
        .iter()
        .flat_map(|list| list.iter())
        .map(|param| match param {
            TypeParam::TypeVar(param) => param.name.as_str(),
            TypeParam::TypeVarTuple(param) => param.name.as_str(),
            TypeParam::ParamSpec(param) => param.name.as_str(),
        })
        .collect();

    for base in arguments.args.iter() {
        let Expr::Subscript(subscript) = base else {
            continue;
        };
        if !matches!(
            bindings.form_of(&subscript.value),
            Some(TypingForm::Generic | TypingForm::Protocol)
        ) {
            continue;
        }
        for argument in basilisk_parser::subscript_elements(subscript) {
            if classify(argument, bindings, factories, &type_params) != Argument::NotATypeVariable {
                continue;
            }
            diagnostics.push(make_diagnostic(
                "Argument to `Generic[...]` must be a type variable".to_owned(),
                text_range_to_span(argument.range()),
                path,
            ));
        }
    }
}

/// What one argument of `Generic[...]` / `Protocol[...]` denotes.
fn classify(
    expr: &Expr,
    bindings: &BindingTable,
    factories: &HashSet<TextRange>,
    type_params: &[&str],
) -> Argument {
    // PEP 646 spells an unpacked `TypeVarTuple` two ways, and both ask the
    // same question of the operand.
    if let Some(inner) = unpacked_operand(expr, bindings) {
        return classify(inner, bindings, factories, type_params);
    }
    if let Expr::Name(name) = expr {
        // The PEP 695 scope binds this name before any outer one can.
        if type_params.contains(&name.id.as_str()) {
            return Argument::TypeVariable;
        }
    }
    // A name bound by an assignment: follow it to the EXPRESSION it was bound
    // to — through any number of aliases — and ask whether that expression
    // constructed a type parameter. Two `TypeVar`s spelled alike are two
    // ranges here, and one `TypeVar` under three names is one range.
    if let Some(value) = bindings.local_value_binding(expr) {
        return if factories.contains(&value) {
            Argument::TypeVariable
        } else {
            Argument::NotATypeVariable
        };
    }
    // A class this module defines is never a type variable.
    if bindings.local_class_definition(expr).is_some() {
        return Argument::NotATypeVariable;
    }
    // A symbol the canonical registry describes: `int`, `str`, `typing.Any`.
    // The type-parameter factories themselves are excluded because naming the
    // factory rather than an instance of it is a different diagnostic.
    if let Some(form) = bindings.form_of_with_builtins(expr) {
        return if form.is_type_parameter_factory() {
            Argument::Unknown
        } else {
            Argument::NotATypeVariable
        };
    }
    match expr {
        // A literal in a type-parameter position cannot be a type variable
        // under any binding.
        Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::List(_)
        | Expr::Dict(_)
        | Expr::Set(_)
        | Expr::Tuple(_) => Argument::NotATypeVariable,
        // Everything else — an import, a dotted path outside the registry, a
        // string forward reference — is unresolved from here, and an
        // unresolved argument is not evidence of an error.
        _ => Argument::Unknown,
    }
}

/// The operand of a PEP 646 unpack, written either way.
///
/// `*Ts` is syntax; `Unpack[Ts]` is a subscript of a registry symbol, resolved
/// through the binding table so an aliased import unwraps identically.
fn unpacked_operand<'e>(expr: &'e Expr, bindings: &BindingTable) -> Option<&'e Expr> {
    match expr {
        Expr::Starred(starred) => Some(starred.value.as_ref()),
        Expr::Subscript(subscript)
            if bindings.form_of(&subscript.value) == Some(TypingForm::Unpack) =>
        {
            Some(subscript.slice.as_ref())
        }
        _ => None,
    }
}

/// The range of every expression in this module that CONSTRUCTS a type
/// parameter — `TypeVar(...)`, `TypeVarTuple(...)`, `ParamSpec(...)`.
///
/// A range, not a name: it is the identity `BindingTable::local_value_binding`
/// hands back for any name bound to that construction, however many aliases
/// away.
fn type_parameter_constructions(bindings: &BindingTable, body: &[Stmt]) -> HashSet<TextRange> {
    body.iter()
        .filter_map(|stmt| match stmt {
            Stmt::Assign(assign) => Some(assign.value.as_ref()),
            Stmt::AnnAssign(assign) => assign.value.as_deref(),
            _ => None,
        })
        .filter(|value| {
            matches!(value, Expr::Call(call)
                if bindings
                    .form_of(&call.func)
                    .is_some_and(TypingForm::is_type_parameter_factory))
        })
        .map(Ranged::range)
        .collect()
}
