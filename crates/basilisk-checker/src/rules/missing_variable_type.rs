//! Implements [BSK-0003] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-MISSING
//! BSK-0003: Missing variable type annotation.
//!
//! Fires when a module-level variable has no type annotation.  This house rule
//! is off by default — the default configuration contains only PEP-tagged rules — and
//! a project opts in via configuration. When enabled, every module-level
//! binding must carry an explicit annotation so that Basilisk can verify
//! downstream usage and generate accurate stubs.

use basilisk_resolver::{ResolvedModule, RhsKind, VariableInfo};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-0003",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-0003",
};

/// Emits BSK-0003 for every unannotated module-level variable.
pub(crate) struct MissingVariableType;

impl Rule for MissingVariableType {
    fn opt_in_spec(&self) -> Option<crate::rule_tags::OptInSpec> {
        Some(crate::rule_tags::OptInSpec {
            code: CODE.code,
            tags: &["strictness"],
        })
    }

    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        module
            .module_vars
            .iter()
            .filter(|var| !var.has_annotation && is_unresolvable(&var.rhs_kind))
            .for_each(|var| diagnostics.push(make_diagnostic(var, &module.path)));
    }
}

/// Returns `true` for RHS kinds whose element/value type cannot be inferred
/// from the literal alone.
fn is_unresolvable(rhs: &RhsKind) -> bool {
    matches!(
        rhs,
        RhsKind::EmptyList | RhsKind::EmptyDict | RhsKind::NoneValue
    )
}

fn make_diagnostic(var: &VariableInfo, path: &str) -> Diagnostic {
    let (message, help) = match &var.rhs_kind {
        RhsKind::EmptyList => (
            format!(
                "Missing type annotation for `{}` — cannot infer element type from empty list `[]`",
                var.name
            ),
            format!("{}: list[<type>] = []", var.name),
        ),
        RhsKind::EmptyDict => (
            format!(
                "Missing type annotation for `{}` — cannot infer key/value types from empty dict `{{}}`",
                var.name
            ),
            format!("{}: dict[<key>, <value>] = {{}}", var.name),
        ),
        RhsKind::NoneValue => (
            format!(
                "Missing type annotation for `{}` — cannot infer type from `None`",
                var.name
            ),
            format!("{}: <type> | None = None", var.name),
        ),
        _ => (
            format!(
                "Missing type annotation for module-level variable `{}`",
                var.name
            ),
            format!("{}: <type> = ...", var.name),
        ),
    };

    error_diagnostic_owned(
        CODE.clone(),
        message,
        var.name_span,
        path,
        Some(format!("Add a type annotation: `{help}`")),
        Some(
            "In Basilisk, all module-level variables require explicit type annotations".to_owned(),
        ),
    )
}

#[cfg(test)]
mod tests {
    use basilisk_resolver::{RhsKind, Span, VariableInfo};

    use super::{is_unresolvable, make_diagnostic, MissingVariableType, Rule};

    fn unannotated(name: &str, rhs: RhsKind) -> VariableInfo {
        VariableInfo {
            name: name.to_owned(),
            name_span: Span { start: 0, end: 1 },
            has_annotation: false,
            rhs_kind: rhs,
            annotation_span: None,
            rhs_span: None,
        }
    }

    /// [CHKARCH-CONFIG-MODEL]: BSK-0003 is an opt-in `strictness` house rule,
    /// never a PEP rule. Provenance is read from `opt_in_spec`, so a `None`
    /// here would silently promote the rule to always-on — guard it directly.
    #[test]
    fn opt_in_spec_marks_a_strictness_house_rule() {
        let spec = MissingVariableType.opt_in_spec();
        assert!(spec.is_some(), "BSK-0003 must stay opt-in");
        if let Some(spec) = spec {
            assert_eq!(spec.code, "BSK-0003");
            assert_eq!(spec.tags, &["strictness"]);
        }
    }

    /// Only empty collections and `None` leave the element/value type
    /// unknowable from the literal alone; every other RHS is resolvable.
    #[test]
    fn only_empty_collections_and_none_are_unresolvable() {
        assert!(is_unresolvable(&RhsKind::EmptyList));
        assert!(is_unresolvable(&RhsKind::EmptyDict));
        assert!(is_unresolvable(&RhsKind::NoneValue));
        assert!(!is_unresolvable(&RhsKind::IntLiteral));
        assert!(!is_unresolvable(&RhsKind::Other));
    }

    /// Each unresolvable RHS renders its own tailored guidance — one match arm
    /// per kind — so dropping an arm changes the user-facing message.
    #[test]
    fn make_diagnostic_renders_a_message_per_rhs_kind() {
        assert!(
            make_diagnostic(&unannotated("xs", RhsKind::EmptyList), "m.py")
                .message
                .contains("empty list")
        );
        assert!(
            make_diagnostic(&unannotated("d", RhsKind::EmptyDict), "m.py")
                .message
                .contains("empty dict")
        );
        assert!(
            make_diagnostic(&unannotated("n", RhsKind::NoneValue), "m.py")
                .message
                .contains("`None`")
        );
        // The catch-all arm covers every other kind with the generic message.
        assert!(make_diagnostic(&unannotated("v", RhsKind::Other), "m.py")
            .message
            .contains("module-level variable"));
    }
}
