//! Implements [`enums_definition`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `enums_definition`: access to an enum member that does not exist for the target.
//!
//! Enum members may be defined conditionally on a statically-known check such as
//! the Python version:
//!
//! ```python
//! class Color(Enum):
//!     RED = 1
//!     if sys.version_info >= (4, 0):
//!         BLUE = 3      # absent when checking for 3.12
//!
//! Color.BLUE            # error — BLUE does not exist at the target version
//! ```
//!
//! This rule flags access to a member that was defined only under an `if`-guard
//! that is statically false at the configured target. It is intentionally narrow
//! — it never touches unconditional members, inherited attributes, or functional
//! `Enum(...)` calls — so it cannot fire on valid code.

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{evaluate, parse_static_condition, BranchTruth, ResolvedModule};
use ruff_python_ast::{Expr, Stmt};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::shared::parse_module;
use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "enums_definition",
    docs_url: "https://www.basilisk-python.dev/errors/enums_definition",
};

/// Emits `enums_definition` for access to a version-excluded enum member.
pub(crate) struct EnumMemberAccess;

impl Rule for EnumMemberAccess {
    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let enum_names: HashSet<&str> = module
            .classes
            .iter()
            .filter(|cls| cls.is_enum)
            .map(|cls| cls.name.as_str())
            .collect();
        if enum_names.is_empty() {
            return;
        }

        let Some(parsed) = parse_module(module) else {
            return;
        };
        let Some(target_version) = ctx.target_version else {
            return;
        };

        // enum class name -> member names whose guard is statically false at the target.
        let mut excluded: HashMap<String, HashSet<String>> = HashMap::new();
        collect_excluded_members(
            &module.bindings,
            &parsed.ast.body,
            &enum_names,
            target_version,
            &mut excluded,
        );
        if excluded.is_empty() {
            return;
        }

        for access in &module.module_attr_accesses {
            if let Some(members) = excluded.get(access.object_name.as_str()) {
                if members.contains(&access.attr_name) {
                    diagnostics.push(make_diagnostic(
                        &access.object_name,
                        &access.attr_name,
                        access.span,
                        &module.path,
                    ));
                }
            }
        }
    }
}

/// Walk the module, recording for each enum class the member names defined only
/// under a statically-false `if`-guard.
fn collect_excluded_members(
    bindings: &BindingTable,
    stmts: &[Stmt],
    enum_names: &HashSet<&str>,
    target: (u32, u32),
    out: &mut HashMap<String, HashSet<String>>,
) {
    for stmt in stmts {
        if let Stmt::ClassDef(cls) = stmt {
            if enum_names.contains(cls.name.as_str()) {
                let mut members = HashSet::new();
                collect_dead_branch_members(bindings, &cls.body, target, &mut members);
                if !members.is_empty() {
                    let _ = out.insert(cls.name.to_string(), members);
                }
            }
            // Nested classes may also be enums.
            collect_excluded_members(bindings, &cls.body, enum_names, target, out);
        }
    }
}

/// Collect names assigned inside an `if`-guard that is statically false at the
/// target (and recurse into live branches for nested guards).
fn collect_dead_branch_members(stmts: &[Stmt], target: (u32, u32), out: &mut HashSet<String>) {
    for stmt in stmts {
        let Stmt::If(if_stmt) = stmt else {
            continue;
        };
        match evaluate(&parse_static_condition(&if_stmt.test), target) {
            BranchTruth::AlwaysFalse => collect_assigned_names(&if_stmt.body, out),
            _ => collect_dead_branch_members(&if_stmt.body, target, out),
        }
    }
}

/// Collect the simple-name assignment targets in a statement list.
fn collect_assigned_names(stmts: &[Stmt], out: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    if let Expr::Name(name) = target {
                        let _ = out.insert(name.id.to_string());
                    }
                }
            }
            Stmt::AnnAssign(ann) => {
                if let Expr::Name(name) = ann.target.as_ref() {
                    let _ = out.insert(name.id.to_string());
                }
            }
            _ => {}
        }
    }
}

fn make_diagnostic(
    enum_name: &str,
    member: &str,
    span: basilisk_resolver::Span,
    path: &str,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Enum `{enum_name}` has no member `{member}` for the target Python version — it is \
             defined only under a version guard that is not satisfied"
        ),
        span,
        path,
        Some(format!(
            "`{member}` exists only for a later Python version; guard the access, or lower the \
             target version"
        )),
        Some(
            "Conditionally-defined enum members do not exist when their `sys.version_info` guard \
             is false at the configured target"
                .to_owned(),
        ),
    )
}
