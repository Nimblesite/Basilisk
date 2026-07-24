//! Import and redundant-alias extraction for stub modules.

use ruff_python_ast::{self as ast, StmtImport, StmtImportFrom};

use crate::types::StarReexport;

use super::StubExtractor;

impl StubExtractor {
    // Implements [STUBRES-PYI-REEXPORTS] — `import x as x` marks `x` as
    // re-exported, while every import also records module bindings used by
    // `submodule.__all__` references.
    pub(super) fn visit_import(&mut self, import: &StmtImport) {
        for alias in &import.names {
            let imported = alias.name.to_string();
            let local_name = alias.asname.as_ref().map_or_else(
                || imported.split('.').next().unwrap_or(&imported).to_owned(),
                ToString::to_string,
            );
            let module = if alias.asname.is_some() {
                imported.clone()
            } else {
                local_name.clone()
            };
            let _ = self
                .module_bindings
                .insert(local_name, StarReexport { module, level: 0 });
            if alias
                .asname
                .as_ref()
                .is_some_and(|asname| asname.as_str() == alias.name.as_str())
            {
                self.reexported_names.push(alias.name.to_string());
            }
        }
    }

    pub(super) fn visit_import_from(&mut self, import: &StmtImportFrom) {
        if import.names.iter().any(|alias| alias.name.as_str() == "*") {
            self.star_reexports.push(StarReexport {
                module: import
                    .module
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                level: import.level,
            });
            return;
        }
        for alias in &import.names {
            self.record_imported_module_binding(import, alias);
            if alias
                .asname
                .as_ref()
                .is_some_and(|asname| asname.as_str() == alias.name.as_str())
            {
                self.reexported_names.push(alias.name.to_string());
            }
        }
    }

    fn record_imported_module_binding(&mut self, import: &StmtImportFrom, alias: &ast::Alias) {
        let imported_name = alias.name.as_str();
        let local_name = alias
            .asname
            .as_ref()
            .map_or_else(|| imported_name.to_owned(), ToString::to_string);
        let parent = import
            .module
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
        let module = if imported_name == "__all__" {
            parent
        } else if parent.is_empty() {
            imported_name.to_owned()
        } else {
            format!("{parent}.{imported_name}")
        };
        let _ = self.module_bindings.insert(
            local_name,
            StarReexport {
                module,
                level: import.level,
            },
        );
    }
}
