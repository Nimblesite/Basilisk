//! Implements [`protocols_modules`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-optional
//! `protocols_modules`: Module assigned to incompatible protocol type.
//!
//! When a module object is assigned to a variable typed as a `Protocol`, the
//! module's public interface must be compatible with the protocol.  This rule
//! detects assignments of the form:
//!
//! ```python
//! import some_module
//!
//! class MyProtocol(Protocol):
//!     timeout: str
//!
//! x: MyProtocol = some_module  # E — some_module.timeout is int, not str
//! ```
//!
//! This is a simplified check: if the annotation names a class that inherits
//! from `Protocol` and the RHS is a module name, the assignment is flagged
//! when the module is known to be incompatible.
//!
//! Specification: <https://typing.readthedocs.io/en/latest/spec/protocol.html#modules-as-implementations-of-protocols>

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diag_help_note, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

use crate::rules::shared::is_type_compatible;

const CODE: ErrorCode = ErrorCode {
    code: "protocols_modules",
    docs_url: "https://www.basilisk-python.dev/errors/protocols_modules",
};

/// Emits `protocols_modules` when a module is assigned to a protocol-typed variable
/// but the module does not satisfy the protocol.
pub(crate) struct ModuleProtocolIncompatible;

impl Rule for ModuleProtocolIncompatible {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;
        let path = &module.path;

        // Step 1: Find protocol classes (classes with `Protocol` in their bases).
        let protocol_classes: HashMap<&str, &basilisk_resolver::ClassInfo> = module
            .classes
            .iter()
            .filter(|cls| cls.bases.iter().any(|b| b == "Protocol"))
            .map(|cls| (cls.name.as_str(), cls))
            .collect();

        if protocol_classes.is_empty() {
            return;
        }

        // Step 2: Build a map of protocol method return types.
        // Key: (class_name, method_name) -> return_type_text
        let mut protocol_method_returns: HashMap<(&str, &str), String> = HashMap::new();
        for func in &module.functions {
            let Some(ref class_name) = func.class_name else {
                continue;
            };
            if !protocol_classes.contains_key(class_name.as_str()) {
                continue;
            }
            let return_type = extract_return_type(func, source);
            let _ = protocol_method_returns
                .insert((class_name.as_str(), func.name.as_str()), return_type);
        }

        // Step 3: Collect imported module names.
        let imported_modules: HashSet<&str> = module
            .imports
            .iter()
            .filter(|imp| imp.kind == basilisk_resolver::ImportKind::Plain)
            .map(|imp| imp.module.as_str())
            .collect();

        if imported_modules.is_empty() {
            return;
        }

        // Step 4: Re-parse and walk annotated assignments at module level.
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };

        // Step 5: Load module information from companion files.
        let module_dir = std::path::Path::new(path)
            .parent()
            .map(std::path::Path::to_path_buf);

        for stmt in &parsed.ast.body {
            check_annotated_assign(
                stmt,
                source,
                path,
                &protocol_classes,
                &protocol_method_returns,
                &imported_modules,
                module_dir.as_ref(),
                diagnostics,
            );
        }
    }
}

/// Extract return type string from a function info.
fn extract_return_type(func: &basilisk_resolver::FunctionInfo, source: &str) -> String {
    match &func.return_annotation {
        basilisk_resolver::ReturnAnnotationKind::NoneType => "None".to_owned(),
        basilisk_resolver::ReturnAnnotationKind::Other => {
            if let Some(ann_span) = func.return_annotation_span {
                slice_span(source, ann_span)
                    .map_or_else(|| "object".to_owned(), |s| s.trim().to_owned())
            } else {
                "object".to_owned()
            }
        }
        _ => "object".to_owned(),
    }
}

/// Information about a companion module's interface.
struct ModuleInterface {
    /// Attribute names -> inferred type string.
    attributes: HashMap<String, String>,
    /// Method names -> return type string.
    methods: HashMap<String, String>,
}

/// Load a companion module's interface by parsing its source file.
fn load_module_interface(
    module_name: &str,
    module_dir: Option<&std::path::PathBuf>,
) -> Option<ModuleInterface> {
    let dir = module_dir?;
    let file_path = dir.join(format!("{module_name}.py"));

    let source = std::fs::read_to_string(&file_path).ok()?;
    let parsed =
        basilisk_parser::parse_source(source.clone(), file_path.to_string_lossy().into_owned())
            .ok()?;

    let resolved = basilisk_resolver::resolve(&parsed).ok()?;

    let mut attributes = HashMap::new();
    let mut methods = HashMap::new();

    // Collect module-level variable types.
    for var in &resolved.module_vars {
        let inferred_type = infer_rhs_type(&var.rhs_kind);
        let _ = attributes.insert(var.name.clone(), inferred_type);
    }

    // Collect module-level function signatures.
    for func in &resolved.functions {
        if func.class_name.is_some() {
            continue; // Skip methods of classes defined in the module.
        }
        let return_type = extract_return_type(func, &source);
        let _ = methods.insert(func.name.clone(), return_type);
    }

    Some(ModuleInterface {
        attributes,
        methods,
    })
}

/// Infer a simple type string from the RHS kind.
fn infer_rhs_type(kind: &basilisk_resolver::RhsKind) -> String {
    match kind {
        basilisk_resolver::RhsKind::IntLiteral => "int".to_owned(),
        basilisk_resolver::RhsKind::FloatLiteral => "float".to_owned(),
        basilisk_resolver::RhsKind::StrLiteral => "str".to_owned(),
        basilisk_resolver::RhsKind::BoolLiteral => "bool".to_owned(),
        basilisk_resolver::RhsKind::BytesLiteral => "bytes".to_owned(),
        basilisk_resolver::RhsKind::NoneValue => "None".to_owned(),
        _ => "object".to_owned(),
    }
}

/// Check an annotated assignment for module-protocol incompatibility.
#[expect(
    clippy::too_many_arguments,
    reason = "protocol conformance check needs all context params"
)]
fn check_annotated_assign(
    stmt: &ruff_python_ast::Stmt,
    source: &str,
    path: &str,
    protocol_classes: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    protocol_method_returns: &HashMap<(&str, &str), String>,
    imported_modules: &HashSet<&str>,
    module_dir: Option<&std::path::PathBuf>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::{Expr, Stmt};
    use ruff_text_size::Ranged as _;

    let Stmt::AnnAssign(ann_assign) = stmt else {
        return;
    };

    // The annotation must be a simple name referencing a protocol class.
    let Expr::Name(ann_name) = ann_assign.annotation.as_ref() else {
        return;
    };
    let protocol_name = ann_name.id.as_str();
    let Some(protocol_info) = protocol_classes.get(protocol_name) else {
        return;
    };

    // The RHS must be present and a simple name referencing an imported module.
    let Some(value) = &ann_assign.value else {
        return;
    };
    let Expr::Name(rhs_name) = value.as_ref() else {
        return;
    };
    let module_name = rhs_name.id.as_str();
    if !imported_modules.contains(module_name) {
        return;
    }

    // Load the companion module's interface.
    let range = ann_assign.range();
    let span = Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    };

    let Some(mod_interface) = load_module_interface(module_name, module_dir) else {
        // Module interface could not be loaded (e.g. stdlib module like `sys`).
        // We cannot verify compatibility, so flag the assignment.
        diagnostics.push(error_diag_help_note(
            CODE.clone(),
            format!(
                "Module `{module_name}` assigned to protocol `{protocol_name}` \
                 but compatibility cannot be verified"
            ),
            span,
            path,
            format!(
                "Ensure module `{module_name}` provides all members required \
                 by `{protocol_name}` with compatible types"
            ),
            "A module can be used where a protocol is expected only if its \
             public interface is compatible with the protocol",
        ));
        return;
    };

    // Check compatibility.
    let incompatibility = check_protocol_compatibility(
        protocol_info,
        &mod_interface,
        source,
        protocol_name,
        protocol_method_returns,
    );

    if let Some(reason) = incompatibility {
        diagnostics.push(error_diag_help_note(
            CODE.clone(),
            format!(
                "Module `{module_name}` is not compatible with protocol \
                 `{protocol_name}`: {reason}"
            ),
            span,
            path,
            format!(
                "Ensure module `{module_name}` provides all members required \
                 by `{protocol_name}` with compatible types"
            ),
            "A module can be used where a protocol is expected only if its \
             public interface is compatible with the protocol",
        ));
    }
}

/// Check if a module interface satisfies a protocol.
/// Returns `Some(reason)` if incompatible, `None` if compatible.
fn check_protocol_compatibility(
    protocol: &basilisk_resolver::ClassInfo,
    mod_interface: &ModuleInterface,
    source: &str,
    protocol_name: &str,
    protocol_method_returns: &HashMap<(&str, &str), String>,
) -> Option<String> {
    // Check each protocol attribute.
    for attr in &protocol.attributes {
        let attr_name = attr.name.as_str();

        // Get the protocol's declared type for this attribute.
        let protocol_type = if let Some(ann_span) = attr.annotation_span {
            slice_span(source, ann_span)
                .map_or_else(|| "object".to_owned(), |s| s.trim().to_owned())
        } else {
            "object".to_owned()
        };

        // Check if the module has this attribute.
        if let Some(mod_type) = mod_interface.attributes.get(attr_name) {
            // Type must be compatible.
            if !is_type_compatible(mod_type, &protocol_type) {
                return Some(format!(
                    "attribute `{attr_name}` has type `{mod_type}` \
                     but protocol expects `{protocol_type}`"
                ));
            }
        } else {
            // Attribute missing — check methods (callable attributes are methods).
            if mod_interface.methods.contains_key(attr_name) {
                continue;
            }
            return Some(format!(
                "missing attribute `{attr_name}` required by protocol"
            ));
        }
    }

    // Check each protocol method.
    for method_name in &protocol.method_names {
        let method_name_str = method_name.as_str();

        if !mod_interface.methods.contains_key(method_name_str) {
            return Some(format!(
                "missing method `{method_name_str}` required by protocol"
            ));
        }

        // Check return type compatibility.
        if let Some(protocol_return) =
            protocol_method_returns.get(&(protocol_name, method_name_str))
        {
            if let Some(mod_return) = mod_interface.methods.get(method_name_str) {
                if !is_type_compatible(mod_return, protocol_return) {
                    return Some(format!(
                        "method `{method_name_str}` returns `{mod_return}` \
                         but protocol expects `{protocol_return}`"
                    ));
                }
            }
        }
    }

    None
}
