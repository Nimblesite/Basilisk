//! Implements [TYPEINF-TARGET-TYPELEVEL] — lowering Ruff AST annotation
//! expressions into the type-level term language.
//! See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-TYPELEVEL
//!
//! This is the bridge from PEP 695 `type` statements to [`TypeTerm`]s the
//! evaluator and acceptance conditions understand. Lowering is total and
//! gradual: any expression shape outside the type sublanguage lowers to
//! `Ground(Unknown)` — shape *validity* is a separate rule's concern
//! (`aliases_type_statement`), never the engine's.

use std::collections::HashSet;

use ruff_python_ast::{
    ExceptHandler, Expr, ExprSubscript, ModModule, Operator, Stmt, StmtTypeAlias,
};
use ruff_text_size::{Ranged as _, TextRange};

use crate::types::InferredType;

use super::term::{AliasDef, TypeTerm};

/// One lowered PEP 695 `type` statement.
#[derive(Debug, Clone, PartialEq)]
pub struct LoweredAlias {
    /// The alias name.
    pub name: String,
    /// The source range of the alias name token (for diagnostics).
    pub name_range: TextRange,
    /// The lowered definition (parameters replaced by [`TypeTerm::Param`]).
    pub def: AliasDef,
}

/// Lower every PEP 695 `type` statement in `module` (at any nesting depth)
/// into [`LoweredAlias`] definitions, in source order. Duplicate names are
/// all returned; a caller registering them in order gets last-binding-wins
/// (modulo [`super::AliasEnv::insert`]'s acceptance gate, which skips
/// rejected definitions).
#[must_use]
pub fn lower_module_aliases(module: &ModModule) -> Vec<LoweredAlias> {
    let mut stmts: Vec<&StmtTypeAlias> = Vec::new();
    collect_type_aliases(&module.body, &mut stmts);
    let alias_names: HashSet<String> = stmts
        .iter()
        .filter_map(|stmt| simple_name(&stmt.name))
        .collect();
    stmts
        .iter()
        .filter_map(|stmt| lower_alias(stmt, &alias_names))
        .collect()
}

/// Recursively collect `type` statements from every statement body
/// (module, class, function, and compound-statement scope alike — scope
/// *legality* is rule business).
fn collect_type_aliases<'a>(body: &'a [Stmt], out: &mut Vec<&'a StmtTypeAlias>) {
    for stmt in body {
        match stmt {
            Stmt::TypeAlias(alias) => out.push(alias),
            Stmt::ClassDef(class) => collect_type_aliases(&class.body, out),
            Stmt::FunctionDef(func) => collect_type_aliases(&func.body, out),
            Stmt::If(if_stmt) => {
                collect_type_aliases(&if_stmt.body, out);
                for clause in &if_stmt.elif_else_clauses {
                    collect_type_aliases(&clause.body, out);
                }
            }
            Stmt::For(for_stmt) => {
                collect_type_aliases(&for_stmt.body, out);
                collect_type_aliases(&for_stmt.orelse, out);
            }
            Stmt::While(while_stmt) => {
                collect_type_aliases(&while_stmt.body, out);
                collect_type_aliases(&while_stmt.orelse, out);
            }
            Stmt::With(with_stmt) => collect_type_aliases(&with_stmt.body, out),
            Stmt::Try(try_stmt) => {
                collect_type_aliases(&try_stmt.body, out);
                for ExceptHandler::ExceptHandler(handler) in &try_stmt.handlers {
                    collect_type_aliases(&handler.body, out);
                }
                collect_type_aliases(&try_stmt.orelse, out);
                collect_type_aliases(&try_stmt.finalbody, out);
            }
            Stmt::Match(match_stmt) => {
                for case in &match_stmt.cases {
                    collect_type_aliases(&case.body, out);
                }
            }
            _ => {}
        }
    }
}

/// Lower one `type Name[P..] = rhs` statement.
fn lower_alias(stmt: &StmtTypeAlias, alias_names: &HashSet<String>) -> Option<LoweredAlias> {
    let name = simple_name(&stmt.name)?;
    let params: Vec<String> = stmt
        .type_params
        .as_deref()
        .map(|type_params| {
            type_params
                .type_params
                .iter()
                .map(|param| param.name().to_string())
                .collect()
        })
        .unwrap_or_default();
    let ctx = LowerCtx {
        params: &params,
        aliases: alias_names,
    };
    let body = ctx.lower(&stmt.value);
    Some(LoweredAlias {
        name,
        name_range: stmt.name.range(),
        def: AliasDef {
            arity: params.len(),
            body,
        },
    })
}

/// Lowering context: the enclosing alias's parameters and the module's
/// alias name set (module-local names lower to [`TypeTerm::Alias`]
/// references; everything else grounds out).
#[derive(Debug)]
pub struct LowerCtx<'a> {
    /// Enclosing type-parameter names, in declaration order.
    pub params: &'a [String],
    /// Names of `type` aliases defined in this module.
    pub aliases: &'a HashSet<String>,
}

impl LowerCtx<'_> {
    /// Lower one annotation expression to a [`TypeTerm`].
    #[must_use]
    pub fn lower(&self, expr: &Expr) -> TypeTerm {
        match expr {
            Expr::Name(name) => self.lower_name(name.id.as_str()),
            Expr::Subscript(sub) => self.lower_subscript(sub),
            Expr::BinOp(bin) if bin.op == Operator::BitOr => {
                let mut arms = Vec::new();
                self.lower_union_arm(&bin.left, &mut arms);
                self.lower_union_arm(&bin.right, &mut arms);
                TypeTerm::Union(arms)
            }
            // String annotation: a forward reference — parse and lower the
            // inner expression ([TYPEINF-ANNOTATION-RESOLUTION]).
            Expr::StringLiteral(literal) => self.lower_forward_ref(literal.value.to_str()),
            Expr::NoneLiteral(_) => TypeTerm::Ground(InferredType::None_),
            Expr::Attribute(_) => ground_from_text(&dotted_text(expr).unwrap_or_default()),
            Expr::Starred(starred) => self.lower(&starred.value),
            // Outside the type sublanguage (literals, calls, lambdas, ..):
            // gradual ground. Shape validity is `aliases_type_statement`'s
            // concern, not the engine's.
            _ => TypeTerm::Ground(InferredType::Unknown),
        }
    }

    /// A bare name: parameter → `Param`, module alias → `Alias` reference,
    /// anything else → ground type via the annotation parser.
    fn lower_name(&self, id: &str) -> TypeTerm {
        if let Some(index) = self.params.iter().position(|param| param == id) {
            return TypeTerm::Param(index);
        }
        if self.aliases.contains(id) {
            return TypeTerm::Alias(id.to_owned(), Vec::new());
        }
        ground_from_text(id)
    }

    /// A subscript `base[args]`: builtin containers get their dedicated
    /// constructors, module aliases become applications, and any other
    /// base is a [`TypeTerm::Named`] constructor head.
    ///
    /// `Union[..]`, `Optional[..]`, and `Annotated[..]` (bare or
    /// `typing.`-qualified) are *transparent* type operators — semantically
    /// identical to their `|`-spellings — so they lower to [`TypeTerm::Union`]
    /// (or the underlying type), NEVER to a `Named` constructor: they must
    /// not guard recursion (`type X = Union[int, X]` is as circular as
    /// `type X = int | X`).
    fn lower_subscript(&self, sub: &ExprSubscript) -> TypeTerm {
        let args = self.lower_subscript_args(sub);
        let Some(base_name) = dotted_text(&sub.value) else {
            return TypeTerm::Ground(InferredType::Unknown);
        };
        match (base_name.as_str(), args.len()) {
            ("Union" | "typing.Union", _) => TypeTerm::Union(args),
            ("Optional" | "typing.Optional", 1) => match args.into_iter().next() {
                Some(inner) => TypeTerm::Union(vec![inner, TypeTerm::Ground(InferredType::None_)]),
                None => TypeTerm::Ground(InferredType::Unknown),
            },
            ("Annotated" | "typing.Annotated", _) => args
                .into_iter()
                .next()
                .unwrap_or(TypeTerm::Ground(InferredType::Unknown)),
            ("list" | "List", 1) => match args.into_iter().next() {
                Some(element) => TypeTerm::List(Box::new(element)),
                None => TypeTerm::Ground(InferredType::Unknown),
            },
            ("set" | "frozenset" | "Set" | "FrozenSet", 1) => match args.into_iter().next() {
                Some(element) => TypeTerm::Set(Box::new(element)),
                None => TypeTerm::Ground(InferredType::Unknown),
            },
            ("dict" | "Dict", 2) => {
                let mut iter = args.into_iter();
                match (iter.next(), iter.next()) {
                    (Some(key), Some(value)) => TypeTerm::Dict(Box::new(key), Box::new(value)),
                    _ => TypeTerm::Ground(InferredType::Unknown),
                }
            }
            ("tuple" | "Tuple", _) => TypeTerm::Tuple(args),
            (name, _) if self.aliases.contains(name) => TypeTerm::Alias(name.to_owned(), args),
            (name, _) => TypeTerm::Named(name.to_owned(), args),
        }
    }

    /// Subscript arguments: a tuple slice contributes each element;
    /// `...` (as in `tuple[X, ...]` / `Callable[..., R]`) contributes
    /// nothing structural and is dropped.
    fn lower_subscript_args(&self, sub: &ExprSubscript) -> Vec<TypeTerm> {
        basilisk_parser::subscript_elements(sub)
            .into_iter()
            .filter(|element| !matches!(element, Expr::EllipsisLiteral(_)))
            .map(|element| self.lower(element))
            .collect()
    }

    /// Flatten nested `X | Y | Z` into one union arm list.
    fn lower_union_arm(&self, expr: &Expr, arms: &mut Vec<TypeTerm>) {
        match expr {
            Expr::BinOp(bin) if bin.op == Operator::BitOr => {
                self.lower_union_arm(&bin.left, arms);
                self.lower_union_arm(&bin.right, arms);
            }
            other => arms.push(self.lower(other)),
        }
    }

    /// Parse a string forward reference and lower its expression; an
    /// unparseable string grounds out gradually.
    fn lower_forward_ref(&self, text: &str) -> TypeTerm {
        match ruff_python_parser::parse_expression(text.trim()) {
            Ok(parsed) => self.lower(parsed.expr()),
            Err(_) => TypeTerm::Ground(InferredType::Unknown),
        }
    }
}

/// Ground a leaf via the annotation parser (`int` → `Int`, unknown names →
/// `Named`), keeping one source of truth for leaf spelling.
fn ground_from_text(text: &str) -> TypeTerm {
    TypeTerm::Ground(InferredType::from_annotation(text))
}

/// The dotted text of a `Name` / `Attribute` chain (`typing.Sequence`),
/// or `None` for any other shape.
fn dotted_text(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attr) => Some(format!("{}.{}", dotted_text(&attr.value)?, attr.attr)),
        _ => None,
    }
}

/// The simple name of a `Name` expression.
fn simple_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::accept::{classify, Acceptance};
    use super::*;

    fn lower_all(source: &str) -> Vec<LoweredAlias> {
        ruff_python_parser::parse_module(source)
            .map(|parsed| lower_module_aliases(parsed.syntax()))
            .unwrap_or_default()
    }

    fn classify_source_alias(source: &str, name: &str) -> Option<Acceptance> {
        let aliases = lower_all(source);
        aliases
            .iter()
            .find(|alias| alias.name == name)
            .map(|alias| classify(name, &alias.def))
    }

    /// The #371 boundary cases lower and classify as accepted: guarded
    /// recursion through every constructor, in both spellings.
    #[test]
    fn issue_371_recursive_aliases_lower_as_accepted() {
        for (source, name) in [
            ("type J = list[J]\n", "J"),
            ("type J = int | list[J]\n", "J"),
            ("type J = dict[str, J]\n", "J"),
            (
                "type JsonValue = None | bool | int | float | str | list[JsonValue] | dict[str, JsonValue]\n",
                "JsonValue",
            ),
            ("type R = str | int | tuple[\"R\", ...]\n", "R"),
            ("type T[X] = X | list[T[X]]\n", "T"),
        ] {
            assert_eq!(
                classify_source_alias(source, name),
                Some(Acceptance::Accepted),
                "{source}"
            );
        }
    }

    /// The conformance-mandated rejections still classify as unguarded.
    #[test]
    fn conformance_circular_aliases_lower_as_unguarded() {
        for (source, name) in [
            ("type R3 = R3\n", "R3"),
            ("type R4[T] = T | R4[str]\n", "R4"),
            ("type X = int | X\n", "X"),
        ] {
            assert_eq!(
                classify_source_alias(source, name),
                Some(Acceptance::Unguarded),
                "{source}"
            );
        }
    }

    /// `Union[..]`, `Optional[..]`, and `Annotated[..]` are transparent type
    /// operators, not constructors: recursion through them is exactly as
    /// unguarded as through their `|`-spellings, while recursion through a
    /// real constructor INSIDE them stays accepted.
    #[test]
    fn transparent_special_forms_do_not_guard_recursion() {
        for (source, name, expected) in [
            ("type X = Union[int, X]\n", "X", Acceptance::Unguarded),
            (
                "type X = typing.Union[int, X]\n",
                "X",
                Acceptance::Unguarded,
            ),
            ("type Y = Optional[Y]\n", "Y", Acceptance::Unguarded),
            ("type Y = typing.Optional[Y]\n", "Y", Acceptance::Unguarded),
            (
                "type Z = Annotated[Z, \"meta\"]\n",
                "Z",
                Acceptance::Unguarded,
            ),
            ("type A = Union[int, list[A]]\n", "A", Acceptance::Accepted),
            ("type B = Optional[list[B]]\n", "B", Acceptance::Accepted),
            (
                "type C = Annotated[list[C], \"meta\"]\n",
                "C",
                Acceptance::Accepted,
            ),
        ] {
            assert_eq!(
                classify_source_alias(source, name),
                Some(expected),
                "{source}"
            );
        }
    }

    /// Every compound-statement body is walked for `type` statements —
    /// deleting any [`collect_type_aliases`] arm loses an alias here.
    #[test]
    fn aliases_are_collected_from_every_compound_statement_body() {
        let source = "\
if cond:
    type A1 = int
elif cond:
    type A2 = int
else:
    type A3 = int
for item in items:
    type B1 = int
else:
    type B2 = int
while cond:
    type C1 = int
else:
    type C2 = int
with ctx:
    type D1 = int
try:
    type E1 = int
except Exception:
    type E2 = int
else:
    type E3 = int
finally:
    type E4 = int
match value:
    case 1:
        type F1 = int
class Holder:
    type G1 = int
def scope():
    type H1 = int
";
        let aliases = lower_all(source);
        let names: Vec<&str> = aliases.iter().map(|alias| alias.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "A1", "A2", "A3", "B1", "B2", "C1", "C2", "D1", "E1", "E2", "E3", "E4", "F1", "G1",
                "H1"
            ]
        );
    }

    /// Growing recursion lowers as non-regular (the Paterson/Coverage
    /// analogue rejects it; the escape hatch can still admit it).
    #[test]
    fn growing_recursion_lowers_as_non_regular() {
        assert_eq!(
            classify_source_alias("type R[T] = set[R[list[T]]]\n", "R"),
            Some(Acceptance::NonRegular)
        );
    }

    /// Parameters lower positionally; string forward references lower
    /// through a real parse (`"B"` reaches the parameter, not ground).
    #[test]
    fn parameters_and_forward_refs_lower_structurally() {
        let aliases = lower_all("type Pair[A, B] = dict[A, \"B\"]\n");
        let bodies: Vec<(usize, &TypeTerm)> = aliases
            .iter()
            .map(|alias| (alias.def.arity, &alias.def.body))
            .collect();
        assert_eq!(
            bodies,
            [(
                2,
                &TypeTerm::Dict(Box::new(TypeTerm::Param(0)), Box::new(TypeTerm::Param(1)))
            )]
        );
    }

    /// Class-scope aliases are collected; non-type RHS grounds gradually.
    #[test]
    fn class_scope_and_non_type_rhs_lower_totally() {
        let aliases =
            lower_all("class C:\n    type Inner = list[Inner]\ntype Weird = (lambda: int)()\n");
        let summary: Vec<(&str, &TypeTerm)> = aliases
            .iter()
            .map(|alias| (alias.name.as_str(), &alias.def.body))
            .collect();
        assert_eq!(
            summary,
            [
                (
                    "Inner",
                    &TypeTerm::List(Box::new(TypeTerm::Alias("Inner".to_owned(), Vec::new())))
                ),
                ("Weird", &TypeTerm::Ground(InferredType::Unknown)),
            ]
        );
    }
}
