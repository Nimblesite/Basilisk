//! Implements [`version_target_syntax`] from [CHKARCH-VERSION-TARGET]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-VERSION-TARGET
//! `version_target_syntax`: PEP 695 syntax used below the configured target version.
//!
//! `type X = ...` aliases and `class Foo[T]` / `def f[T]()` type-parameter
//! lists are Python 3.12+ syntax (PEP 695). When the configured
//! `python_version` targets anything older, the file cannot even be parsed by
//! the target interpreter, so this fires as an error (issue #93).
//!
//! ```python
//! # python_version = "3.11"
//! type Alias = int          # E0155 — `type` statement requires 3.12+
//! class Box[T]: ...         # E0155 — PEP 695 type params require 3.12+
//! def first[T](x: T) -> T:  # E0155 — PEP 695 type params require 3.12+
//! ```

use basilisk_resolver::{GenericDefKind, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::{CheckContext, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "version_target_syntax",
    docs_url: "https://www.basilisk-python.dev/errors/version_target_syntax",
};

/// The first Python version with native PEP 695 syntax.
const PEP695_MIN_VERSION: (u32, u32) = (3, 12);

/// Emits `version_target_syntax` for PEP 695 syntax on sub-3.12 targets.
// Implements [CHKARCH-VERSION-NARROWING] — rejects PEP 695 syntax when the
// configured target version cannot parse it.
pub(crate) struct Pep695BelowTargetViolation;

impl Rule for Pep695BelowTargetViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(target) = ctx.target_version else {
            return;
        };
        if target >= PEP695_MIN_VERSION {
            return;
        }
        for alias in &module.pep695_scoping.aliases {
            diagnostics.push(make_diagnostic(
                &format!(
                    "PEP 695 `type {}` alias statement requires Python 3.12+",
                    alias.name
                ),
                alias.name_span,
                target,
                &module.path,
            ));
        }
        for def in &module.pep695_scoping.defs {
            let kind = match def.kind {
                GenericDefKind::Class => "class",
                GenericDefKind::Function => "def",
            };
            diagnostics.push(make_diagnostic(
                &format!(
                    "PEP 695 type parameter list on `{kind} {}` requires Python 3.12+",
                    def.name
                ),
                def.name_span,
                target,
                &module.path,
            ));
        }
    }
}

/// Build the E0155 diagnostic with target-version remediation.
fn make_diagnostic(
    message: &str,
    span: basilisk_resolver::Span,
    target: (u32, u32),
    path: &str,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        message.to_owned(),
        span,
        path,
        Some(format!(
            "the configured target is Python {}.{} — use `TypeVar`/`TypeAlias` from \
             `typing` (or `typing_extensions`), or raise `python_version` to 3.12+",
            target.0, target.1
        )),
        Some(
            "PEP 695 syntax is a parse error on interpreters older than 3.12, so code \
             using it cannot run on the configured target"
                .to_owned(),
        ),
    )
}
