//! Implements helpers for [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Shared helper functions used across multiple type checking rules.
//!
//! Consolidated from duplicated implementations in individual rule modules
//! to eliminate code duplication and improve maintainability.

mod class_walks;
pub(crate) mod judge;
pub(crate) mod module_types;
pub(crate) mod oracle;
pub(crate) mod returns_judge;
mod text_scan;
mod type_expr;

pub(crate) use class_walks::{
    any_base_name_matches, class_name_map, class_or_base_matches, method_name_map,
};
pub(crate) use type_expr::{
    annotation_is_type_alias, is_type_expression, ExprIndex, StringPolicy, TypeExprJudge,
};
pub(crate) use text_scan::{
    identifiers_followed_by, leading_indent, span_for_line, split_top_level_commas,
};

use std::collections::HashSet;

use crate::annotation::AnnotationResolver;
use crate::span_util::slice_span;
use crate::types::InferredType;
use basilisk_parser::ParsedModule;
use basilisk_resolver::{ResolvedModule, Span, TypeVarCallInfo};
use ruff_python_ast::{self as ast, Expr};

/// Is one of `decorators` the `typing.overload` decorator?
///
/// Resolved through the module's binding tables
/// ([TYPEINF-ANNOTATION-RESOLUTION], [#380](https://github.com/Nimblesite/Basilisk/issues/380)):
/// `@overload`, `@ov` after `from typing import overload as ov`,
/// `@typing.overload` / `@t.overload`, and `@o` after `o = overload` all
/// answer yes; a decorator merely *named* `overload` but bound from another
/// module answers no. Every rule that reasons about overload groups shares
/// this one predicate so the groups they form agree.
pub(crate) fn overload_decorated(resolver: &AnnotationResolver<'_>, decorators: &[String]) -> bool {
    decorators
        .iter()
        .any(|decorator| resolver.decorator_denotes(decorator, "overload"))
}

/// Spelling-level decorator match: `name` bare or as the final segment of a
/// dotted path (`@typing.final`, `@abc.abstractmethod`).
///
/// For guards where a qualified false match merely *skips* a check — never
/// invents a diagnostic. Rules whose diagnostics depend on what a decorator
/// IS resolve it through the binding tables instead ([`overload_decorated`]).
pub(crate) fn decorator_spelled(decorators: &[String], name: &str) -> bool {
    decorators
        .iter()
        .any(|d| d == name || d.rsplit('.').next() == Some(name))
}

/// Returns `true` when the annotation text denotes a `ClassVar[...]` type.
///
/// `ClassVar` fields are excluded from the dataclass `__init__` parameter list,
/// so dataclass rules (field ordering, constructor arity) skip them.
pub(crate) fn annotation_is_classvar(source: &str, span: Option<Span>) -> bool {
    let Some(text) = span.and_then(|span| slice_span(source, span)) else {
        return false;
    };
    let t = text.trim();
    t.starts_with("ClassVar[")
        || t.starts_with("ClassVar ")
        || t == "ClassVar"
        || t.contains(".ClassVar[")
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Return the resolved module's AST, parsing it once and sharing the result.
///
/// Every `Rule::check` implementation needs the AST and silently bails on parse
/// errors (those are reported separately as `BSK-0000`). Backed by the module's
/// [`LazyAst`](basilisk_resolver::LazyAst) cache, so the first rule to ask parses
/// the source and every later rule reuses it — a file is parsed once, not once
/// per parsing rule.
pub(crate) fn parse_module(module: &ResolvedModule) -> Option<&ParsedModule> {
    module.lazy_ast.get_or_parse(&module.source, &module.path)
}

// ---------------------------------------------------------------------------
// TypeVar helpers
// ---------------------------------------------------------------------------

/// Collect the names of every `TypeVarTuple` declared in the module.
///
/// The returned set borrows from the slice; both must outlive the set.
pub(crate) fn typevar_tuple_names(typevar_calls: &[TypeVarCallInfo]) -> HashSet<&str> {
    typevar_calls
        .iter()
        .filter(|tv| tv.is_typevartuple)
        .map(|tv| tv.name.as_str())
        .collect()
}

// ---------------------------------------------------------------------------
// Annotation parsing
// ---------------------------------------------------------------------------

/// Parse a subscript annotation like `Name[A, B]` into `(name, [A, B])`.
///
/// Returns a borrowed name and owned, trimmed type argument strings.
/// Returns `None` when the annotation is not a valid subscript form.
pub(crate) fn parse_subscript_annotation(text: &str) -> Option<(&str, Vec<String>)> {
    let bracket_pos = text.find('[')?;
    let name = text[..bracket_pos].trim();
    if name.is_empty() {
        return None;
    }
    let inner = text.get(bracket_pos + 1..text.rfind(']')?)?;
    let args: Vec<String> = split_top_level_commas(inner)
        .iter()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    if args.is_empty() {
        return None;
    }
    Some((name, args))
}

// ---------------------------------------------------------------------------
// Expression helpers
// ---------------------------------------------------------------------------

/// Extract the simple name from a `Name` expression.
pub(crate) fn expr_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

/// Convert an annotation expression to a readable string.
pub(crate) fn ann_str(expr: &Expr) -> String {
    match expr {
        Expr::Name(n) => n.id.to_string(),
        Expr::Subscript(s) => format!("{}[{}]", ann_str(&s.value), ann_str(&s.slice)),
        Expr::Attribute(a) => format!("{}.{}", ann_str(&a.value), a.attr),
        Expr::Tuple(t) => t.elts.iter().map(ann_str).collect::<Vec<_>>().join(", "),
        Expr::BinOp(b) => format!("{} | {}", ann_str(&b.left), ann_str(&b.right)),
        Expr::NoneLiteral(_) => "None".to_owned(),
        Expr::StringLiteral(s) => s.value.to_str().to_owned(),
        Expr::List(l) => format!(
            "[{}]",
            l.elts.iter().map(ann_str).collect::<Vec<_>>().join(", ")
        ),
        Expr::NumberLiteral(n) => format!("{:?}", n.value),
        _ => "...".to_owned(),
    }
}

/// Infer the concrete type of a literal expression.
pub(crate) fn infer_expr_literal_type(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::NumberLiteral(n) => match &n.value {
            ast::Number::Int(_) => Some("int"),
            ast::Number::Float(_) => Some("float"),
            ast::Number::Complex { .. } => Some("complex"),
        },
        Expr::StringLiteral(_) => Some("str"),
        Expr::BytesLiteral(_) => Some("bytes"),
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::NoneLiteral(_) => Some("None"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Type compatibility
// ---------------------------------------------------------------------------

/// Check if `actual` is assignable to `expected` with no class context:
/// `Any`, `object`, the numeric tower, and `X | Y` unions.
///
/// Delegates to the ONE subtyping implementation
/// (`subtyping::SubtypingContext::is_subtype`, [TYPEINF-SUBTYPING],
/// [NARROWPLAN-SUBTYPING]) over an empty context — rules that know the
/// module's class hierarchy seed `subtyping::module_context` instead.
pub(crate) fn is_type_compatible(actual: &str, expected: &str) -> bool {
    static EMPTY: std::sync::LazyLock<crate::subtyping::SubtypingContext> =
        std::sync::LazyLock::new(crate::subtyping::SubtypingContext::default);
    EMPTY.is_subtype(actual, expected)
}

// ---------------------------------------------------------------------------
// Literal helpers
// ---------------------------------------------------------------------------

/// Extract the content between `Literal[` (or `L[`) and the matching `]`.
pub(crate) fn extract_literal_inner(ann: &str) -> Option<&str> {
    // Support both `Literal[` and `L[`.
    let start_bracket = if let Some(pos) = ann.find("Literal[") {
        pos + "Literal[".len()
    } else if ann.starts_with("L[") {
        2
    } else {
        return None;
    };

    let mut depth = 1i32;
    let bytes = ann.as_bytes();
    let mut idx = start_bracket;
    while idx < bytes.len() {
        match bytes.get(idx) {
            Some(b'[') => depth += 1,
            Some(b']') => {
                depth -= 1;
                if depth == 0 {
                    return ann.get(start_bracket..idx);
                }
            }
            Some(_) | None => {}
        }
        idx += 1;
    }
    None
}

/// Extract the callee name from a RHS text like `ClassName(...)` or `ClassName[T](...)`.
pub(crate) fn extract_callee_name(rhs_text: &str) -> Option<&str> {
    // Handle `ClassName[T](...)` by stripping everything from `[` onwards first.
    let before_bracket = rhs_text.split('[').next()?;
    let before_paren = before_bracket.split('(').next()?;
    let name = before_paren.trim();
    if name.is_empty() {
        return None;
    }
    // Class names start with uppercase (heuristic).
    if !name.starts_with(|c: char| c.is_ascii_uppercase()) {
        return None;
    }
    Some(name)
}

// ---------------------------------------------------------------------------
// Identifier / typevar matching
// ---------------------------------------------------------------------------

pub(crate) fn contains_typevar_reference(text: &str, name: &str) -> bool {
    let name_bytes = name.as_bytes();
    let text_bytes = text.as_bytes();
    let name_len = name_bytes.len();

    if name_len > text_bytes.len() {
        return false;
    }

    for start in 0..=(text_bytes.len() - name_len) {
        let Some(slice) = text_bytes.get(start..start + name_len) else {
            continue;
        };
        if slice != name_bytes {
            continue;
        }
        if start > 0
            && text_bytes
                .get(start - 1)
                .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_')
        {
            continue;
        }
        let end = start + name_len;
        if end < text_bytes.len()
            && text_bytes
                .get(end)
                .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_')
        {
            continue;
        }
        return true;
    }
    false
}

/// Generic parameter names of a class definition: PEP 695 type parameters
/// plus `Protocol[...]` / `Generic[...]` base subscript arguments.
pub(crate) fn class_generic_param_names(cls: &ruff_python_ast::StmtClassDef) -> Vec<String> {
    let mut names: Vec<String> = cls
        .type_params
        .as_ref()
        .map(|tp| {
            tp.type_params
                .iter()
                .map(|p| p.name().to_string())
                .collect()
        })
        .unwrap_or_default();
    for base in cls.bases() {
        let Expr::Subscript(sub) = base else { continue };
        let base_name = ann_str(&sub.value);
        if base_name != "Protocol" && base_name != "Generic" {
            continue;
        }
        let args = basilisk_parser::subscript_elements(sub);
        names.extend(args.iter().filter_map(|a| match a {
            Expr::Name(n) => Some(n.id.to_string()),
            _ => None,
        }));
    }
    names
}

/// A `*args` or `**kwargs` parameter slot in a parsed signature.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum StarParam {
    /// The signature has no such parameter.
    #[default]
    Absent,
    /// Present without an annotation (implicitly `Any`).
    Untyped,
    /// Present with an annotation.
    Typed(String),
}

impl StarParam {
    /// `true` when the parameter exists in the signature.
    pub(crate) fn is_present(&self) -> bool {
        !matches!(self, StarParam::Absent)
    }

    /// The annotation text; `None` for absent or untyped (gradual `Any`).
    pub(crate) fn ty(&self) -> Option<&str> {
        match self {
            StarParam::Typed(ty) => Some(ty),
            StarParam::Absent | StarParam::Untyped => None,
        }
    }

    /// Build from an optional annotation of a present parameter.
    pub(crate) fn from_annotation(annotation: Option<String>) -> StarParam {
        annotation.map_or(StarParam::Untyped, StarParam::Typed)
    }
}

// ---------------------------------------------------------------------------
// Return-type verifiability (shared by E0011 and E0013)
// ---------------------------------------------------------------------------

/// Returns true when a return target depends on the returned expression's
/// **value**, which the kind-only return inference does not have — at the top
/// level or nested inside a union, container, optional, callable, or type-form.
///
/// Verifying a `Literal[v]` target requires the value of the returned
/// expression, but `return True` infers `Bool`, not `Literal[True]`. Such a
/// check is unreliable, so it is skipped.
///
/// A *nominal* target is NOT in this category any more. It used to be: every
/// `InferredType::Named` was treated as unverifiable, which silenced the whole
/// return check for `-> MyClass` and `-> MyAlias`
/// ([#378](https://github.com/Nimblesite/Basilisk/issues/378)). Names now
/// arrive through the annotation cascade
/// ([TYPEINF-ANNOTATION-RESOLUTION](../../../../docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION)),
/// which yields `Named` only for a resolved same-file nominal class and the
/// gradual `Unknown` for anything it cannot resolve — and `Unknown` suppresses
/// through ordinary assignability, with no rule-level skip needed.
///
/// Both E0011 and E0013 gate their assignability check on this so the two
/// sibling rules stay in lock-step.
pub(crate) fn is_value_dependent_target(ty: &InferredType) -> bool {
    match ty {
        InferredType::Literal(_) => true,
        InferredType::Optional(inner)
        | InferredType::List(inner)
        | InferredType::Set(inner)
        | InferredType::TypeForm(inner) => is_value_dependent_target(inner),
        InferredType::Dict(key, value) => {
            is_value_dependent_target(key) || is_value_dependent_target(value)
        }
        InferredType::Union(types) | InferredType::Tuple(types) => {
            types.iter().any(is_value_dependent_target)
        }
        InferredType::Callable(info) => {
            is_value_dependent_target(&info.return_type)
                || info.param_types.iter().any(is_value_dependent_target)
        }
        _ => false,
    }
}
