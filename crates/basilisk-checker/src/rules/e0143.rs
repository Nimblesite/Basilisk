//! BSK-E0143: `NamedTuple` usage violations.
//!
//! Detects invalid usage of `NamedTuple` instances:
//!
//! 1. **Out-of-bounds index access**: `p[3]` on a 3-field `NamedTuple` (valid: 0..2 or -3..-1).
//! 2. **Attribute assignment**: `p.x = 3` — `NamedTuple` fields are read-only.
//! 3. **Subscript assignment**: `p[0] = 3` — `NamedTuple` elements are read-only.
//! 4. **Attribute deletion**: `del p.x` — `NamedTuple` fields cannot be deleted.
//! 5. **Subscript deletion**: `del p[0]` — `NamedTuple` elements cannot be deleted.
//! 6. **Wrong-count tuple unpack**: `x, y = p` when `p` has 3 fields.
//!
//! ```python
//! class Point(NamedTuple):
//!     x: int
//!     y: int
//!     units: str = "meters"
//!
//! p = Point(1, 2)
//! print(p[3])     # E: out-of-bounds index
//! print(p[-4])    # E: out-of-bounds negative index
//! p.x = 3        # E: NamedTuple fields are read-only
//! p[0] = 3       # E: NamedTuple elements are read-only
//! del p.x        # E: NamedTuple fields cannot be deleted
//! del p[0]       # E: NamedTuple elements cannot be deleted
//! x, y = p       # E: too few values to unpack (expected 3)
//! ```

use std::collections::HashMap;

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, error_diagnostic_owned};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0143",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0143",
};

/// Emits BSK-E0143 for invalid `NamedTuple` usage.
pub(crate) struct NamedTupleUsageViolation;

impl Rule for NamedTupleUsageViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };

        let ctx = ModuleContext::from_ast(&parsed.ast.body);
        if ctx.namedtuple_classes.is_empty() {
            return;
        }

        check_stmts(&parsed.ast.body, &ctx, &module.path, diagnostics);
    }
}

/// Context built from module-level AST: which classes are `NamedTuples` and which
/// variables hold `NamedTuple` instances.
struct ModuleContext {
    /// Map from class name -> field count.
    namedtuple_classes: HashMap<String, usize>,
    /// Map from variable name -> class name.
    var_to_nt_class: HashMap<String, String>,
}

impl ModuleContext {
    fn from_ast(stmts: &[Stmt]) -> Self {
        let mut namedtuple_classes: HashMap<String, usize> = HashMap::new();
        let mut var_to_nt_class: HashMap<String, String> = HashMap::new();

        // First pass: collect direct NamedTuple class definitions.
        for stmt in stmts {
            if let Stmt::ClassDef(cls) = stmt {
                if is_namedtuple_class(cls) {
                    let field_count = count_annotated_fields(cls);
                    let _ = namedtuple_classes.insert(cls.name.to_string(), field_count);
                }
            }
        }

        // Second pass: collect subclasses of known NamedTuple classes.
        for stmt in stmts {
            if let Stmt::ClassDef(cls) = stmt {
                if !namedtuple_classes.contains_key(cls.name.as_str()) {
                    if let Some(base_count) = namedtuple_base_count(cls, &namedtuple_classes) {
                        let own_fields = count_annotated_fields(cls);
                        let _ = namedtuple_classes
                            .insert(cls.name.to_string(), base_count + own_fields);
                    }
                }
            }
        }

        // Third pass: map variables to their NamedTuple class via constructor calls.
        for stmt in stmts {
            if let Stmt::Assign(assign) = stmt {
                if assign.targets.len() == 1 {
                    if let Some(var_name) = assign.targets.first().and_then(expr_simple_name) {
                        if let Some(class_name) = call_class_name(&assign.value) {
                            if namedtuple_classes.contains_key(class_name) {
                                let _ = var_to_nt_class
                                    .insert(var_name.to_owned(), class_name.to_owned());
                            }
                        }
                    }
                }
            }
        }

        Self {
            namedtuple_classes,
            var_to_nt_class,
        }
    }

    /// Return the field count for the `NamedTuple` class assigned to `var_name`, if known.
    fn nt_field_count(&self, var_name: &str) -> Option<usize> {
        let class_name = self.var_to_nt_class.get(var_name)?;
        self.namedtuple_classes.get(class_name).copied()
    }
}

/// Returns true if the class directly inherits from `NamedTuple`.
fn is_namedtuple_class(cls: &ast::StmtClassDef) -> bool {
    cls.arguments.as_ref().is_some_and(|args| {
        args.args.iter().any(|base| {
            matches!(base, Expr::Name(n) if n.id.as_str() == "NamedTuple")
                || matches!(base, Expr::Attribute(a) if a.attr.as_str() == "NamedTuple")
        })
    })
}

/// Count annotated field definitions (`x: T` or `x: T = default`) in a class body.
fn count_annotated_fields(cls: &ast::StmtClassDef) -> usize {
    cls.body
        .iter()
        .filter(|stmt| matches!(stmt, Stmt::AnnAssign(_)))
        .count()
}

/// If `cls` inherits from a known `NamedTuple` class, return that base class's field count.
fn namedtuple_base_count(
    cls: &ast::StmtClassDef,
    namedtuple_classes: &HashMap<String, usize>,
) -> Option<usize> {
    cls.arguments.as_ref()?.args.iter().find_map(|base| {
        let base_name = expr_simple_name(base)?;
        namedtuple_classes.get(base_name).copied()
    })
}

/// Extract a simple name from an expression.
fn expr_simple_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

/// If `expr` is a call like `ClassName(...)`, return the class name.
fn call_class_name(expr: &Expr) -> Option<&str> {
    if let Expr::Call(call) = expr {
        return expr_simple_name(&call.func);
    }
    None
}

/// Convert a ruff `TextRange` to a Basilisk `Span`.
fn to_span(range: ruff_text_size::TextRange) -> Span {
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}

/// Parse a literal integer index from an expression (handles negation).
fn parse_literal_int(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::NumberLiteral(n) => {
            if let ast::Number::Int(i) = &n.value {
                i.as_i64()
            } else {
                None
            }
        }
        Expr::UnaryOp(u) if u.op == ast::UnaryOp::USub => {
            if let Expr::NumberLiteral(n) = u.operand.as_ref() {
                if let ast::Number::Int(i) = &n.value {
                    i.as_i64().map(|v| -v)
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Check all module-level statements.
fn check_stmts(stmts: &[Stmt], ctx: &ModuleContext, path: &str, diag: &mut Vec<Diagnostic>) {
    for stmt in stmts {
        check_stmt(stmt, ctx, path, diag);
    }
}

fn check_stmt(stmt: &Stmt, ctx: &ModuleContext, path: &str, diag: &mut Vec<Diagnostic>) {
    match stmt {
        // `p.x = 3` or `p[0] = 3` — assignment to NamedTuple field/element.
        Stmt::Assign(assign) => {
            for target in &assign.targets {
                check_mutation_target(target, Mutation::Assign, ctx, path, diag);
            }
            // Also check for wrong-count tuple unpack: `x, y = p`.
            check_tuple_unpack(assign, ctx, path, diag);
        }

        // `del p.x` or `del p[0]`.
        Stmt::Delete(del) => {
            for target in &del.targets {
                check_mutation_target(target, Mutation::Delete, ctx, path, diag);
            }
        }

        // Expressions like `print(p[3])` — recurse into call arguments.
        Stmt::Expr(expr_stmt) => {
            check_expr_recursive(&expr_stmt.value, ctx, path, diag);
        }

        _ => {}
    }
}

/// Check an assignment for wrong-count tuple unpack: `x, y = p` when `p` has 3 fields.
fn check_tuple_unpack(
    assign: &ast::StmtAssign,
    ctx: &ModuleContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    if assign.targets.len() != 1 {
        return;
    }
    let Some(target) = assign.targets.first() else {
        return;
    };

    // The target must be a tuple of simple names.
    let target_count = match target {
        Expr::Tuple(t) => t.elts.len(),
        _ => return,
    };

    // The value must be a simple name bound to a NamedTuple.
    let Some(obj_name) = expr_simple_name(&assign.value) else {
        return;
    };
    let Some(field_count) = ctx.nt_field_count(obj_name) else {
        return;
    };

    if target_count != field_count {
        let (kind, detail) = if target_count < field_count {
            (
                "too few values to unpack",
                format!(
                    "`{obj_name}` has {field_count} field(s) but only {target_count} \
                     variable(s) provided"
                ),
            )
        } else {
            (
                "too many values to unpack",
                format!(
                    "`{obj_name}` has {field_count} field(s) but {target_count} \
                     variable(s) provided"
                ),
            )
        };

        diag.push(error_diagnostic_owned(
            CODE.clone(),
            format!("NamedTuple unpack mismatch: {kind}: {detail}"),
            to_span(assign.range()),
            path,
            Some(format!(
                "Use exactly {field_count} variable(s) when unpacking `{obj_name}`"
            )),
            Some(
                "NamedTuple is a fixed-length tuple; unpack must match the field count exactly"
                    .to_owned(),
            ),
        ));
    }
}

/// What kind of mutation is being attempted on the `NamedTuple` target.
#[derive(Clone, Copy)]
enum Mutation {
    Assign,
    Delete,
}

impl Mutation {
    fn attribute_message(self, field: &str, obj: &str) -> String {
        match self {
            Self::Assign => format!(
                "Cannot assign to attribute `{field}` on NamedTuple instance `{obj}`: \
                 NamedTuple fields are read-only"
            ),
            Self::Delete => format!(
                "Cannot delete attribute `{field}` on NamedTuple instance `{obj}`: \
                 NamedTuple fields are read-only"
            ),
        }
    }

    fn attribute_hint(self) -> &'static str {
        match self {
            Self::Assign => "NamedTuple instances are immutable; fields cannot be reassigned",
            Self::Delete => "NamedTuple instances are immutable; fields cannot be deleted",
        }
    }

    fn element_message(self, obj: &str) -> String {
        match self {
            Self::Assign => format!(
                "Cannot assign to element of NamedTuple instance `{obj}`: \
                 NamedTuple elements are read-only"
            ),
            Self::Delete => format!(
                "Cannot delete element of NamedTuple instance `{obj}`: \
                 NamedTuple elements are read-only"
            ),
        }
    }

    fn element_hint(self) -> &'static str {
        match self {
            Self::Assign => "NamedTuple instances are immutable; elements cannot be reassigned",
            Self::Delete => "NamedTuple instances are immutable; elements cannot be deleted",
        }
    }
}

/// Check a mutation target (`del` or assignment) for `NamedTuple` violations.
fn check_mutation_target(
    target: &Expr,
    mutation: Mutation,
    ctx: &ModuleContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    match target {
        // `del p.x` or `p.x = 3`
        Expr::Attribute(attr) => {
            if let Some(obj_name) = expr_simple_name(&attr.value) {
                if ctx.nt_field_count(obj_name).is_some() {
                    diag.push(error_diagnostic_owned(
                        CODE.clone(),
                        mutation.attribute_message(&attr.attr, obj_name),
                        to_span(attr.range()),
                        path,
                        Some(mutation.attribute_hint().to_owned()),
                        None,
                    ));
                }
            }
        }

        // `del p[0]` or `p[0] = 3`
        Expr::Subscript(sub) => {
            if let Some(obj_name) = expr_simple_name(&sub.value) {
                if let Some(field_count) = ctx.nt_field_count(obj_name) {
                    if let Some(idx) = parse_literal_int(&sub.slice) {
                        if is_out_of_bounds(idx, field_count) {
                            diag.push(out_of_bounds_diag(
                                obj_name,
                                idx,
                                field_count,
                                to_span(sub.range()),
                                path,
                            ));
                            return;
                        }
                    }
                    diag.push(error_diagnostic_owned(
                        CODE.clone(),
                        mutation.element_message(obj_name),
                        to_span(sub.range()),
                        path,
                        Some(mutation.element_hint().to_owned()),
                        None,
                    ));
                }
            }
        }

        _ => {}
    }
}

/// Recursively check expressions for `NamedTuple` subscript out-of-bounds access.
fn check_expr_recursive(expr: &Expr, ctx: &ModuleContext, path: &str, diag: &mut Vec<Diagnostic>) {
    match expr {
        // `p[3]` — subscript with a literal integer index.
        Expr::Subscript(sub) => {
            if let Some(obj_name) = expr_simple_name(&sub.value) {
                if let Some(field_count) = ctx.nt_field_count(obj_name) {
                    if let Some(idx) = parse_literal_int(&sub.slice) {
                        if is_out_of_bounds(idx, field_count) {
                            diag.push(out_of_bounds_diag(
                                obj_name,
                                idx,
                                field_count,
                                to_span(sub.range()),
                                path,
                            ));
                        }
                    }
                }
            }
            // Recurse into the value and slice.
            check_expr_recursive(&sub.value, ctx, path, diag);
            check_expr_recursive(&sub.slice, ctx, path, diag);
        }

        // Recurse into call arguments.
        Expr::Call(call) => {
            for arg in &call.arguments.args {
                check_expr_recursive(arg, ctx, path, diag);
            }
            for kw in &call.arguments.keywords {
                check_expr_recursive(&kw.value, ctx, path, diag);
            }
        }

        _ => {}
    }
}

/// Returns true when `idx` is out of range for a tuple with `field_count` elements.
#[expect(
    clippy::cast_possible_wrap,
    reason = "field_count is always small enough for i64"
)]
#[expect(
    clippy::as_conversions,
    reason = "field_count is always small enough for i64; no safe alternative for signed cast"
)]
fn is_out_of_bounds(idx: i64, field_count: usize) -> bool {
    let len = field_count as i64;
    idx >= len || idx < -len
}

/// Build an out-of-bounds index diagnostic.
fn out_of_bounds_diag(
    obj_name: &str,
    idx: i64,
    field_count: usize,
    span: Span,
    path: &str,
) -> Diagnostic {
    let max_idx = field_count.saturating_sub(1);
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "NamedTuple index {idx} out of range for `{obj_name}` with {field_count} \
             field(s) (valid range: -{field_count}..{max_idx})"
        ),
        span,
        path,
        Some(format!(
            "Valid indices for `{obj_name}` are -{field_count}..{max_idx} (inclusive)"
        )),
        Some(
            "NamedTuple is a subtype of tuple; index access obeys the same bounds".to_owned(),
        ),
    )
}
