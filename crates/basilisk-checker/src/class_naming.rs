//! Implements [TYPEINF-ANNOTATION-RESOLUTION]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION
//! Naming the class that a type — or the annotation denoting one — refers to.
//!
//! Display surfaces look members up by CLASS NAME. Two things must answer that
//! question: an annotation (`list[int]` names `list`) and an inferred type
//! (`InferredType::LiteralString` names `str`). Both answers live here so the
//! knowledge is stated once, and so [NARROWPLAN-CHECKLIST] Stage 0.5 has a
//! single place to absorb when the shared `resolve_annotation` entry point
//! lands (docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md).
//!
//! Neither answer is reached by slicing source text or by rendering a type to a
//! display string and matching on it — both were real defects
//! ([#388](https://github.com/Nimblesite/Basilisk/issues/388),
//! [#389](https://github.com/Nimblesite/Basilisk/issues/389)).

use crate::types::{InferredType, LiteralValue};

/// The name of the class an annotation NAMES, ignoring type arguments.
///
/// `list[int]` names `list`; `dict[str, int]` names `dict`; a bare `Model`
/// names `Model`. Returns `None` for annotations that name no single class —
/// unions, `Optional[...]`, callables — so a caller cannot mistake an ambiguous
/// receiver for a concrete one.
///
/// Decided on the **AST**, via `ruff`, never by slicing the source text: the
/// project rule is "avoid regex to parse anything, use ruff", and
/// [NARROWPLAN-ANNOTATION-RESOLUTION] is explicit that annotation text must
/// stop being parsed by hand. Case is preserved, which
/// [`InferredType::from_annotation`] cannot do — it lowercases — and which a
/// user-class receiver depends on.
///
/// This is a narrow stand-in for the shared
/// `resolve_annotation(module, expr) → InferredType` entry point that
/// [NARROWPLAN-CHECKLIST] Stage 0.5 will introduce. When that lands, this
/// function's callers move to it and this goes away; keeping the logic here
/// rather than in a consumer means there is exactly one call site to migrate.
#[must_use]
pub fn annotation_class_name(annotation: &str) -> Option<String> {
    let parsed = ruff_python_parser::parse_expression(annotation).ok()?;
    class_name_of(parsed.expr())
}

/// The class an annotation expression names, if it names exactly one.
fn class_name_of(expr: &ruff_python_ast::Expr) -> Option<String> {
    match expr {
        // `list` / `Model`.
        ruff_python_ast::Expr::Name(name) => Some(name.id.to_string()),
        // `list[int]` — the subscript carries type arguments, not identity.
        ruff_python_ast::Expr::Subscript(subscript) => class_name_of(&subscript.value),
        // `typing.List` / `t.List` — the attribute tail is the class.
        ruff_python_ast::Expr::Attribute(attribute) => Some(attribute.attr.to_string()),
        // `"Model"` in a quoted forward reference.
        ruff_python_ast::Expr::StringLiteral(literal) => {
            annotation_class_name(literal.value.to_str())
        }
        // Everything else names no single class: `X | None`, `Callable[...]`
        // written as a call, literals, and anything unparseable as a type.
        _ => None,
    }
}

/// The name of the class a TYPE's members belong to, and whether that type is
/// provably a `LiteralString`.
///
/// The companion to [`annotation_class_name`] for receivers with no annotation.
/// `InferredType::LiteralString` names `str` — it is a `str` refinement (PEP
/// 675), not a class of its own — and the flag reports the refinement so a
/// caller can select the `LiteralString` overloads of `str` methods.
///
/// Returns `None` for a type that names no single class: `Unknown`, `Any`,
/// unions and optionals (whose members differ per arm), callables, and
/// generators. A caller must offer nothing there rather than guess.
///
/// Matched EXHAUSTIVELY on purpose, like the other [`InferredType`] walkers: a
/// catch-all would silently answer `None` for a future type-carrying variant,
/// and the resulting "no members" is indistinguishable from a genuine unknown.
/// Adding a variant must break this build.
#[must_use]
pub fn class_name_of_type(ty: &InferredType) -> Option<(String, bool)> {
    let plain = |name: &str| Some((name.to_owned(), false));
    match ty {
        // PEP 675: a `LiteralString` IS a `str`, refined. Members come from
        // `str`; the flag carries the refinement (GitHub #389).
        InferredType::LiteralString => Some(("str".to_owned(), true)),
        InferredType::Str => plain("str"),
        InferredType::Int => plain("int"),
        InferredType::Float => plain("float"),
        // A narrowing function's VALUE is a `bool` (PEP 647/742: the return type
        // is consistent with `bool`), so `Guard` shares bool's members.
        InferredType::Bool | InferredType::Guard { .. } => plain("bool"),
        InferredType::Bytes => plain("bytes"),
        // Type arguments do not change which class holds the members.
        InferredType::List(_) => plain("list"),
        InferredType::Set(_) => plain("set"),
        InferredType::Dict(_, _) => plain("dict"),
        InferredType::Tuple(_) => plain("tuple"),
        // A literal's members are its base type's; a string literal is a
        // `LiteralString` for the same reason the sentinel above is.
        InferredType::Literal(value) => match value {
            LiteralValue::Str(_) => Some(("str".to_owned(), true)),
            LiteralValue::Int(_) => plain("int"),
            LiteralValue::Float(_) => plain("float"),
            LiteralValue::Bool(_) => plain("bool"),
            LiteralValue::Bytes(_) => plain("bytes"),
        },
        InferredType::Named(name) => Some((name.clone(), false)),
        // A union names a class exactly when EVERY arm names the same one:
        // `int | int` (a list of int literals, a conditional expression) is an
        // int, while `int | str` is not any single class. The `LiteralString`
        // refinement survives only if every arm carries it.
        InferredType::Union(members) => {
            let mut named = members.iter().map(class_name_of_type);
            let first = named.next().flatten()?;
            named.try_fold(first, |(name, literal), arm| {
                arm.filter(|(arm_name, _)| *arm_name == name)
                    .map(|(_, arm_literal)| (name, literal && arm_literal))
            })
        }
        // Names no single class — offering one arm's members would be a guess.
        InferredType::Unknown
        | InferredType::Any
        | InferredType::Never
        | InferredType::None_
        | InferredType::Optional(_)
        | InferredType::Callable(_)
        | InferredType::Generator(_, _, _)
        | InferredType::TypeForm(_) => None,
    }
}

/// The type produced by ITERATING a value of this type.
///
/// `list[int]` yields `int`; iterating a `dict` yields its KEYS; a `str` yields
/// `str`. Returns `None` when the element type is not decidable from the type
/// alone — notably [`InferredType::Named`], whose iteration behaviour lives in
/// its class declaration and must be resolved by a caller that can read one.
///
/// A loop variable's type is exactly this ([#390]) — `for x in xs` binds an
/// element, not the container.
#[must_use]
pub fn element_type_of(ty: &InferredType) -> Option<InferredType> {
    match ty {
        InferredType::List(element) | InferredType::Set(element) => Some((**element).clone()),
        // Iterating a mapping yields its keys, as `for k in d` does.
        InferredType::Dict(key, _) => Some((**key).clone()),
        // A heterogeneous tuple yields the union of what it holds.
        InferredType::Tuple(elements) => elements.split_first().map(|(first, rest)| {
            rest.iter().fold(first.clone(), |acc, next| {
                InferredType::union(acc, next.clone())
            })
        }),
        // Iterating a string yields one-character strings.
        InferredType::Str
        | InferredType::LiteralString
        | InferredType::Literal(LiteralValue::Str(_)) => Some(InferredType::Str),
        InferredType::Generator(yielded, _, _) => Some((**yielded).clone()),
        // Decidable only from the class declaration, or not iterable at all.
        InferredType::Named(_)
        | InferredType::Unknown
        | InferredType::Any
        | InferredType::Never
        | InferredType::None_
        | InferredType::Int
        | InferredType::Float
        | InferredType::Bool
        | InferredType::Bytes
        | InferredType::Literal(_)
        | InferredType::Union(_)
        | InferredType::Optional(_)
        | InferredType::Callable(_)
        | InferredType::TypeForm(_)
        | InferredType::Guard { .. } => None,
    }
}

/// The class named by an annotation's FIRST type argument.
///
/// `Iterator[int]` → `int`, `Sequence[str]` → `str`. Used to read an element
/// type out of a class's own `__iter__`/`__next__` declaration, so the answer
/// comes from the stub rather than from a hardcoded table of what builtin
/// containers yield.
///
/// Decided on the ruff AST, like [`annotation_class_name`].
#[must_use]
pub fn annotation_type_argument(annotation: &str) -> Option<String> {
    let parsed = ruff_python_parser::parse_expression(annotation).ok()?;
    let ruff_python_ast::Expr::Subscript(subscript) = parsed.expr() else {
        return None;
    };
    // `X[a, b]` parses its arguments as a tuple; `X[a]` as the bare element.
    let first = match subscript.slice.as_ref() {
        ruff_python_ast::Expr::Tuple(tuple) => tuple.elts.first()?,
        other => other,
    };
    class_name_of(first)
}
