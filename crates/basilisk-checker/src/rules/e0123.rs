//! BSK-E0123: `super()` call on abstract protocol method with no default implementation.
//!
//! When a class explicitly implements a `Protocol` and one of its methods
//! calls `super().method_name()`, the parent protocol method must provide a
//! default implementation.  If the parent method is abstract (its body is
//! only `...` or `pass`), calling `super()` on it is an error because there
//! is no concrete implementation to dispatch to.
//!
//! ```python
//! from typing import Protocol
//! from abc import abstractmethod
//!
//! class PColor(Protocol):
//!     @abstractmethod
//!     def draw(self) -> str:
//!         ...
//!
//! class BadColor(PColor):
//!     def draw(self) -> str:
//!         return super().draw()  # E — no default implementation
//! ```

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, FunctionInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0123",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0123",
};

/// Emits BSK-E0123 when a method calls `super().method()` on an abstract
/// protocol method that has no default implementation.
pub(crate) struct SuperCallOnAbstractProtocolMethod;

impl Rule for SuperCallOnAbstractProtocolMethod {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let class_map: HashMap<&str, &ClassInfo> = module
            .classes
            .iter()
            .map(|c| (c.name.as_str(), c))
            .collect();

        let method_map: HashMap<(&str, &str), &FunctionInfo> = module
            .functions
            .iter()
            .filter_map(|func| {
                func.class_name
                    .as_deref()
                    .map(|cls| ((cls, func.name.as_str()), func))
            })
            .collect();

        for class in &module.classes {
            check_class(
                class,
                &class_map,
                &method_map,
                &module.source,
                &module.path,
                diagnostics,
            );
        }
    }
}

/// Check all methods of a class for `super()` calls to abstract protocol methods.
fn check_class(
    class: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
    method_map: &HashMap<(&str, &str), &FunctionInfo>,
    source: &str,
    path: &str,
    out: &mut Vec<Diagnostic>,
) {
    // Find protocol bases (bases that themselves inherit from Protocol).
    let protocol_bases: Vec<&ClassInfo> = class
        .bases
        .iter()
        .filter_map(|base_name| class_map.get(base_name.as_str()).copied())
        .filter(|base| is_protocol_class(base))
        .collect();

    if protocol_bases.is_empty() {
        return;
    }

    // For each method in this class, check if it calls super().method_name()
    // where method_name is abstract in a protocol base.
    for method_name in &class.method_names {
        let Some(func) = method_map.get(&(class.name.as_str(), method_name.as_str())) else {
            continue;
        };

        for ret_stmt in &func.return_stmts {
            if !ret_stmt.value_is_call {
                continue;
            }

            let Some(stmt_text) =
                source.get(ret_stmt.span.start as usize..ret_stmt.span.end as usize)
            else {
                continue;
            };

            // Check for super().method_name() pattern in the return text.
            let Some(called_method) = extract_super_call_method(stmt_text) else {
                continue;
            };

            // Check if the called method is abstract in any protocol base.
            for protocol_base in &protocol_bases {
                let Some(base_func) = method_map.get(&(protocol_base.name.as_str(), called_method))
                else {
                    continue;
                };

                // The base method is abstract if it has @abstractmethod and a stub body.
                let is_abstract =
                    is_abstract_method(protocol_base, called_method) && base_func.is_stub_body;

                if is_abstract {
                    out.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Method `{called_method}` in `{}` calls `super().{called_method}()` \
                             but `{}` declares it as abstract with no default implementation",
                            class.name, protocol_base.name
                        ),
                        span: ret_stmt.span,
                        path: path.to_owned(),
                        help: Some(format!(
                            "Provide a concrete implementation instead of calling \
                             `super().{called_method}()`"
                        )),
                        note: Some(
                            "Abstract protocol methods with stub bodies (`...` or `pass`) \
                             have no default implementation to call via `super()`"
                                .to_owned(),
                        ),
                    });
                }
            }
        }
    }
}

/// Returns `true` when the class directly lists `Protocol` among its bases.
fn is_protocol_class(class: &ClassInfo) -> bool {
    class.bases.iter().any(|base| base == "Protocol")
}

/// Returns `true` when the method has an `@abstractmethod` decorator in the class.
fn is_abstract_method(class: &ClassInfo, method_name: &str) -> bool {
    class.method_decorators.iter().any(|(name, decorators)| {
        name == method_name
            && decorators
                .iter()
                .any(|d| d == "abstractmethod" || d.ends_with(".abstractmethod"))
    })
}

/// Extract the method name from a `super().method_name(...)` pattern in source text.
///
/// Returns `Some("method_name")` if the text contains `super().method_name(`,
/// `None` otherwise.
fn extract_super_call_method(text: &str) -> Option<&str> {
    let super_idx = text.find("super()")?;
    let after_super = &text[super_idx + "super()".len()..];
    let after_dot = after_super.strip_prefix('.')?;

    // Find the method name: everything up to the next '('.
    let paren_idx = after_dot.find('(')?;
    let method_name = after_dot[..paren_idx].trim();

    if method_name.is_empty() {
        return None;
    }

    // Validate it looks like an identifier.
    if method_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        Some(method_name)
    } else {
        None
    }
}
