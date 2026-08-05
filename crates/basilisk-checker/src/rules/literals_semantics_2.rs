//! Implements [`literals_semantics_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `literals_semantics_2`: Literal value assignment incompatibility.
//!
//! Detects two classes of Literal-related assignment errors inside function bodies:
//!
//! 1. **`Literal[0]` vs `Literal[False]` non-equivalence (PEP 586)**:
//!    `Literal[0]` and `Literal[False]` are distinct types despite `0 == False` in
//!    Python.  Assigning a `Literal[0]`-typed parameter to a `Literal[False]` local
//!    (or vice versa) is a type error.
//!
//! 2. **Augmented assignment widens a Literal type**:
//!    `a += 3` where `a` is typed `Literal[3, 4, 5]` produces an `int` result,
//!    which is not assignable back to `Literal[3, 4, 5]`.
//!
//! Every verdict is structural over the parsed `ruff` AST
//! ([LINESCANPLAN-AST-MIGRATION], issue #408): `Literal` resolves through the
//! import cascade under any spelling, literal values compare as parsed values
//! (`0x14` equals `20`; `0` never equals `False` because the AST keeps int
//! and bool distinct), and assignments are `AnnAssign`/`AugAssign` nodes,
//! never reconstructed source lines.
//!
//! ```python
//! def func(a: Literal[0], b: Literal[False]):
//!     x1: Literal[False] = a  # E — int 0 ≠ bool False in Literal
//!     x2: Literal[0] = b      # E — bool False ≠ int 0 in Literal
//!
//! def func2(a: Literal[3, 4, 5]):
//!     a += 3  # E — result type is `int`, not `Literal[3, 4, 5]`
//! ```

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::{Expr, Operator, Stmt, UnaryOp};
use ruff_text_size::Ranged;

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::ann_str;
use crate::rules::shared::typing_form::{subscript_args, subscript_of};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "literals_semantics_2",
    docs_url: "https://www.basilisk-python.dev/errors/literals_semantics_2",
};

/// Emits `literals_semantics_2` for Literal value assignment incompatibilities.
pub(crate) struct LiteralValueIncompatible;

impl Rule for LiteralValueIncompatible {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = module.lazy_ast.get_or_parse(&module.source, &module.path) else {
            return;
        };
        let Some(resolver) = AnnotationResolver::for_module(module) else {
            return;
        };
        walk_functions(&parsed.ast.body, &resolver, &module.path, diagnostics);
    }
}

/// Recursively visit every function definition, however nested.
fn walk_functions(
    body: &[Stmt],
    resolver: &AnnotationResolver<'_>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in body {
        match stmt {
            Stmt::FunctionDef(func_def) => {
                check_function(func_def, resolver, path, diagnostics);
                walk_functions(&func_def.body, resolver, path, diagnostics);
            }
            Stmt::ClassDef(class_def) => {
                walk_functions(&class_def.body, resolver, path, diagnostics);
            }
            _ => {}
        }
    }
}

/// Check one function's body for Literal assignment violations.
fn check_function(
    func_def: &ruff_python_ast::StmtFunctionDef,
    resolver: &AnnotationResolver<'_>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Parameters whose annotation is `Literal[...]`, mapped to their values.
    let mut param_literals: HashMap<&str, (&Expr, Vec<String>)> = HashMap::new();
    for param in func_def.parameters.iter_non_variadic_params() {
        let Some(annotation) = param.annotation() else {
            continue;
        };
        let Some(values) = literal_values(resolver, annotation) else {
            continue;
        };
        let _ = param_literals.insert(param.name().as_str(), (annotation, values));
    }
    if param_literals.is_empty() {
        return;
    }

    check_body(&func_def.body, resolver, &param_literals, path, diagnostics);
}

/// Walk statements of a function body (including nested blocks that are not
/// new scopes) for the two violation shapes.
fn check_body(
    body: &[Stmt],
    resolver: &AnnotationResolver<'_>,
    param_literals: &HashMap<&str, (&Expr, Vec<String>)>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in body {
        match stmt {
            Stmt::AnnAssign(assign) => {
                check_annotated_assignment(assign, resolver, param_literals, path, diagnostics);
            }
            Stmt::AugAssign(assign) => {
                check_augmented_assignment(assign, param_literals, path, diagnostics);
            }
            Stmt::If(if_stmt) => {
                check_body(&if_stmt.body, resolver, param_literals, path, diagnostics);
                for clause in &if_stmt.elif_else_clauses {
                    check_body(&clause.body, resolver, param_literals, path, diagnostics);
                }
            }
            Stmt::For(for_stmt) => {
                check_body(&for_stmt.body, resolver, param_literals, path, diagnostics);
                check_body(
                    &for_stmt.orelse,
                    resolver,
                    param_literals,
                    path,
                    diagnostics,
                );
            }
            Stmt::While(while_stmt) => {
                check_body(
                    &while_stmt.body,
                    resolver,
                    param_literals,
                    path,
                    diagnostics,
                );
                check_body(
                    &while_stmt.orelse,
                    resolver,
                    param_literals,
                    path,
                    diagnostics,
                );
            }
            Stmt::With(with_stmt) => {
                check_body(&with_stmt.body, resolver, param_literals, path, diagnostics);
            }
            _ => {}
        }
    }
}

/// `x: Literal[V] = param` where the parameter's Literal values are not all
/// members of the target's Literal values.
fn check_annotated_assignment(
    assign: &ruff_python_ast::StmtAnnAssign,
    resolver: &AnnotationResolver<'_>,
    param_literals: &HashMap<&str, (&Expr, Vec<String>)>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Expr::Name(target) = assign.target.as_ref() else {
        return;
    };
    let Some(target_values) = literal_values(resolver, &assign.annotation) else {
        return;
    };
    let Some(Expr::Name(rhs)) = assign.value.as_deref() else {
        return;
    };
    let Some((param_ann, source_values)) = param_literals.get(rhs.id.as_str()) else {
        return;
    };

    let assignable = source_values
        .iter()
        .all(|value| target_values.contains(value));
    if assignable {
        return;
    }

    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Type mismatch: `{}` is annotated `{}` but assigned \
             parameter `{}` with type `{}`",
            target.id.as_str(),
            ann_str(&assign.annotation),
            rhs.id.as_str(),
            ann_str(param_ann)
        ),
        node_span(target.range()),
        path,
        Some("`Literal[0]` and `Literal[False]` are not equivalent (PEP 586)".to_owned()),
        Some("int and bool Literal values are distinct even when numerically equal".to_owned()),
    ));
}

/// `param op= expr` where the parameter is Literal-typed: the arithmetic
/// result widens past the declared Literal, so the re-assignment is invalid.
fn check_augmented_assignment(
    assign: &ruff_python_ast::StmtAugAssign,
    param_literals: &HashMap<&str, (&Expr, Vec<String>)>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Expr::Name(target) = assign.target.as_ref() else {
        return;
    };
    let Some((param_ann, _)) = param_literals.get(target.id.as_str()) else {
        return;
    };
    let op = augmented_op_text(assign.op);
    let param_ann = ann_str(param_ann);

    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Augmented assignment `{} {op} ...` is incompatible with \
             declared type `{param_ann}`",
            target.id.as_str()
        ),
        node_span(target.range()),
        path,
        Some(format!(
            "The result of `{op}` on `{param_ann}` widens to `int`, which is not \
             assignable back to `{param_ann}`"
        )),
        Some(
            "Augmented assignment re-assigns the target, so the result type must be \
             compatible with the declared Literal type"
                .to_owned(),
        ),
    ));
}

/// The `op=` spelling of an augmented-assignment operator.
const fn augmented_op_text(op: Operator) -> &'static str {
    match op {
        Operator::Add => "+=",
        Operator::Sub => "-=",
        Operator::Mult => "*=",
        Operator::MatMult => "@=",
        Operator::Div => "/=",
        Operator::Mod => "%=",
        Operator::Pow => "**=",
        Operator::LShift => "<<=",
        Operator::RShift => ">>=",
        Operator::BitOr => "|=",
        Operator::BitXor => "^=",
        Operator::BitAnd => "&=",
        Operator::FloorDiv => "//=",
    }
}

/// The values of a `Literal[...]` annotation as canonical comparison keys, or
/// `None` when the annotation is not a `Literal` subscript. Int and bool keys
/// never collide because the AST keeps the node kinds distinct.
fn literal_values(resolver: &AnnotationResolver<'_>, annotation: &Expr) -> Option<Vec<String>> {
    let slice = subscript_of(resolver, annotation, "Literal")?;
    Some(
        subscript_args(slice)
            .into_iter()
            .map(literal_value_key)
            .collect(),
    )
}

/// A canonical comparison key for one Literal value expression.
///
/// Parsed values, not source spellings: `0x14`, `0o24`, and `20` all key as
/// `int:20`, while `True`/`False` key as `bool:` — so `Literal[0]` and
/// `Literal[False]` can never compare equal (PEP 586).
fn literal_value_key(value: &Expr) -> String {
    match value {
        Expr::BooleanLiteral(lit) => format!("bool:{}", lit.value),
        Expr::NumberLiteral(lit) => format!("num:{:?}", lit.value),
        Expr::NoneLiteral(_) => "none".to_owned(),
        Expr::StringLiteral(lit) => format!("str:{}", lit.value.to_str()),
        Expr::BytesLiteral(lit) => {
            let bytes: Vec<u8> = lit.value.bytes().collect();
            format!("bytes:{bytes:?}")
        }
        Expr::UnaryOp(unary) if unary.op == UnaryOp::USub => {
            format!("neg:{}", literal_value_key(&unary.operand))
        }
        other => format!("expr:{}", ann_str(other)),
    }
}

/// The resolver span of an AST node's range.
fn node_span(range: ruff_text_size::TextRange) -> Span {
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}
