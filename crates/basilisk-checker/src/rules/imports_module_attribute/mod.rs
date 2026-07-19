//! Implements [`imports_module_attribute`] from [CHKARCH-DIAG] / [CHKARCH-DIAG-STUB-MEMBER]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-stub-member
//! `imports_module_attribute`: Access to a module attribute the local stub does not declare.
//!
//! When `import X` resolves to an authoritative user/local or selected Typeshed
//! stub, Basilisk sees the declarations and re-exports in that stub. `X.attr`
//! where `attr` is not declared is an error.
//!
//! The escape hatch is the module-level `def __getattr__(name: str) -> Any: ...`
//! that the "Create local type stub" quick fix ships by default: keep it and
//! every attribute is allowed (the module stays `Any`); remove it and declare
//! specific symbols, and undeclared access is flagged.
//!
//! ```python
//! import cowsay            # resolves to .basilisk/stubs/cowsay.pyi
//! cowsay.get_output_string(...)  # E0154 if the stub declares neither this nor __getattr__
//! ```
//!
//! Scope: plain imports backed by a user stub or the active step-3 Typeshed
//! source. Untyped and inline third-party imports remain outside this rule.

use basilisk_resolver::{ImportedModuleApi, ResolvedModule, Span};
use ruff_python_ast::visitor::{walk_expr, Visitor};
use ruff_python_ast::Expr;
use ruff_text_size::Ranged as _;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "test-only code: indexing acceptable in unit tests"
)]
mod tests;

const CODE: ErrorCode = ErrorCode {
    code: "imports_module_attribute",
    docs_url: "https://www.basilisk-python.dev/errors/imports_module_attribute",
};

/// Dunder attributes that exist on every module object regardless of the stub,
/// so accessing them is never a missing-attribute error.
const MODULE_DUNDERS: &[&str] = &[
    "__name__",
    "__doc__",
    "__file__",
    "__dict__",
    "__loader__",
    "__spec__",
    "__package__",
    "__path__",
    "__builtins__",
    "__cached__",
    "__getattr__",
    "__all__",
];

/// Emits `imports_module_attribute` for `module.attr` where a user stub does not declare `attr`.
pub(crate) struct ModuleAttributeUndefined;

impl Rule for ModuleAttributeUndefined {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Only authoritative stub-backed imports populate `imported_modules`.
        if module.imported_modules.is_empty() {
            return;
        }
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };
        let mut visitor = AttrVisitor {
            accesses: Vec::new(),
        };
        for stmt in &parsed.ast.body {
            visitor.visit_stmt(stmt);
        }
        for access in &visitor.accesses {
            check_access(access, module, diagnostics);
        }
    }
}

/// A `name.attr` access site collected from the AST.
struct AttrAccess {
    /// The bare name on the left of the dot (a candidate module binding).
    base: String,
    /// The attribute name being accessed.
    attr: String,
    /// Span of the whole `name.attr` expression.
    span: Span,
}

/// Walks every expression (including inside function/class bodies) and records
/// `name.attr` accesses where the base is a bare `Name`.
struct AttrVisitor {
    accesses: Vec<AttrAccess>,
}

impl<'a> Visitor<'a> for AttrVisitor {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Attribute(attr) = expr {
            if let Expr::Name(name) = attr.value.as_ref() {
                self.accesses.push(AttrAccess {
                    base: name.id.to_string(),
                    attr: attr.attr.to_string(),
                    span: Span::new(expr.range().start().to_u32(), expr.range().end().to_u32()),
                });
            }
        }
        walk_expr(self, expr);
    }
}

/// Emit `imports_module_attribute` if `access` targets an undeclared attribute of a user stub.
fn check_access(access: &AttrAccess, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let Some(api) = module.imported_modules.get(&access.base) else {
        return; // not a user-stub-backed module binding
    };
    if api.has_getattr {
        return; // explicit opt-out: `def __getattr__` permits any attribute
    }
    if api.member_names.contains(&access.attr) {
        return; // declared in the stub
    }
    if MODULE_DUNDERS.contains(&access.attr.as_str()) {
        return; // always present on a module object
    }
    diagnostics.push(make_diagnostic(access, api, &module.path));
}

/// Build the `imports_module_attribute` diagnostic.
fn make_diagnostic(access: &AttrAccess, api: &ImportedModuleApi, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE,
        format!(
            "Module `{}` has no attribute `{}`",
            access.base, access.attr
        ),
        access.span,
        path,
        Some(format!(
            "declare `{0}` in the local stub `{1}`, or fix the typo. To allow any \
             attribute, keep `def __getattr__(name: str) -> Any: ...` in the stub.",
            access.attr,
            api.stub_path.display()
        )),
        Some(
            "A local stub is authoritative — Basilisk only sees the names it declares. \
             https://typing.python.org/en/latest/guides/writing_stubs.html"
                .to_owned(),
        ),
    )
}
