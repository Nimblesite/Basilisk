//! BSK-E0115: Use of deprecated class, function, or method.
//!
//! PEP 702 introduces `@deprecated` from `typing` / `typing_extensions`.
//! Using a deprecated entity (calling, importing, accessing) should produce
//! a diagnostic so that developers migrate away from the deprecated API.
//!
//! ```python
//! from typing_extensions import deprecated
//!
//! @deprecated("Use new_func instead")
//! def old_func() -> None: ...
//!
//! old_func()  # BSK-E0115
//! ```

mod collect;
mod decorators;
mod types;
mod visit;

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

use collect::{
    collect_deprecated_definitions, collect_imported_deprecated,
    collect_imported_deprecated_members, collect_var_types,
};
use types::DeprecatedUsageContext;
use visit::visit_stmt_for_usage;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0115",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0115",
};

/// Emits BSK-E0115 for usage of `@deprecated` decorated entities.
pub(crate) struct DeprecatedUsage;

impl Rule for DeprecatedUsage {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };

        // Collect deprecated names defined in THIS module.
        let mut local_deprecated: HashMap<String, collect::DeprecatedInfo> = HashMap::new();
        collect_deprecated_definitions(&parsed.ast.body, &mut local_deprecated, None);

        // Collect deprecated names from imported sibling modules.
        let mut imported_deprecated: HashMap<String, collect::DeprecatedInfo> = HashMap::new();
        // Also track module aliases: `import X as alias` -> alias maps to module X
        let mut module_aliases: HashMap<String, String> = HashMap::new();
        // Track deprecated from-imports with their spans (for import-site diagnostics).
        let mut from_import_deprecated: Vec<(String, Span)> = Vec::new();
        collect_imported_deprecated(
            &parsed.ast.body,
            &module.path,
            &mut imported_deprecated,
            &mut module_aliases,
            &mut from_import_deprecated,
        );

        // Emit diagnostics for deprecated from-imports (e.g. `from X import Ham`).
        // PEP 702 requires a diagnostic at the import site when a deprecated name is imported.
        for (local_name, span) in &from_import_deprecated {
            if let Some(info) = imported_deprecated.get(local_name.as_str()) {
                diagnostics.push(make_diagnostic(
                    *span,
                    &info.kind,
                    local_name,
                    info.message.as_deref(),
                    &module.path,
                ));
            }
        }

        // Merge all deprecated names.
        let mut all_deprecated = local_deprecated;
        for (name, info) in imported_deprecated {
            let _ = all_deprecated.insert(name, info);
        }

        if all_deprecated.is_empty() && module_aliases.is_empty() {
            return;
        }

        // Collect deprecated method/attribute info from imported module classes.
        let deprecated_members =
            collect_imported_deprecated_members(&parsed.ast.body, &module.path);

        // Build a variable-to-type map from simple assignments, e.g.:
        //   spam = library.Spam()   -> spam -> VarType { module_alias: "library", class_name: "Spam" }
        //   invocable = Invocable() -> invocable -> VarType { module_alias: "", class_name: "Invocable" }
        let var_types = collect_var_types(&parsed.ast.body);

        // Walk the AST to find usages of deprecated names.
        let def_spans: HashSet<u32> = all_deprecated
            .values()
            .map(|info| info.def_span.start)
            .collect();
        let ctx = DeprecatedUsageContext {
            deprecated: &all_deprecated,
            module_aliases: &module_aliases,
            deprecated_members: &deprecated_members,
            var_types: &var_types,
            path: &module.path,
            _def_spans: &def_spans,
        };
        for stmt in &parsed.ast.body {
            visit_stmt_for_usage(stmt, &ctx, diagnostics);
        }
    }
}

/// Build a `BSK-E0115` diagnostic for a deprecated entity usage.
pub(super) fn make_diagnostic(
    span: Span,
    kind: &str,
    name: &str,
    message: Option<&str>,
    path: &str,
) -> Diagnostic {
    let primary = format!("Use of deprecated {kind} `{name}`");
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: primary,
        span,
        path: path.to_owned(),
        help: message.map(|m| format!("Deprecated: {m}")),
        note: Some("Marked with `@deprecated` per PEP 702".to_owned()),
    }
}
