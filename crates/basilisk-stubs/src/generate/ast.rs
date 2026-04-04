//! AST-based stub generation.
//!
//! Parses `.py` source files with `ruff_python_parser` and extracts
//! function signatures, class definitions, and module-level variables
//! to produce `.pyi` stub content.  No subprocess needed.

use std::path::Path;

use ruff_python_ast::{self as ast, Stmt};

use super::{GeneratedStub, StubGenError, StubGenMode};

/// Generate stubs from a Python source file using AST analysis.
///
/// # Errors
///
/// Returns `StubGenError::Io` if the file cannot be read.
/// Returns `StubGenError::Parse` if the file cannot be parsed.
pub fn generate_ast_stubs(
    module_name: &str,
    source_path: &Path,
) -> Result<GeneratedStub, StubGenError> {
    let source = std::fs::read_to_string(source_path)?;
    generate_ast_stubs_from_source(module_name, &source)
}

/// Generate stubs from Python source text (for testing).
///
/// # Errors
///
/// Returns `StubGenError::Parse` if the source cannot be parsed.
pub fn generate_ast_stubs_from_source(
    module_name: &str,
    source: &str,
) -> Result<GeneratedStub, StubGenError> {
    let parsed = ruff_python_parser::parse_module(source)
        .map_err(|err| StubGenError::Parse(format!("{err}")))?;

    let module_ast = parsed.into_syntax();

    let mut lines = Vec::new();
    lines.push(format!(
        "# Auto-generated stub for `{module_name}` (AST analysis)"
    ));
    lines.push("# Tier 3: best-effort, may be inaccurate".to_owned());
    lines.push(String::new());
    lines.push("from typing import Any".to_owned());
    lines.push(String::new());

    // Determine which names are exported via __all__.
    let all_names = extract_all_names(&module_ast.body);

    for stmt in &module_ast.body {
        match stmt {
            Stmt::FunctionDef(func) => {
                let name = func.name.as_str();
                if should_export(name, all_names.as_ref()) {
                    lines.push(format_function_def(func, source));
                }
            }
            Stmt::ClassDef(class) => {
                let name = class.name.as_str();
                if should_export(name, all_names.as_ref()) {
                    lines.push(format_class_def(class));
                }
            }
            Stmt::AnnAssign(ann) => {
                if let Some(line) = format_ann_assign(ann, source) {
                    lines.push(line);
                }
            }
            Stmt::Assign(assign) => {
                if let Some(line) = format_assign(assign) {
                    lines.push(line);
                }
            }
            _ => {}
        }
    }

    lines.push(String::new());

    Ok(GeneratedStub {
        module_name: module_name.to_owned(),
        pyi_content: lines.join("\n"),
        mode: StubGenMode::Ast,
    })
}

/// Extract names from `__all__` if present.
fn extract_all_names(stmts: &[Stmt]) -> Option<Vec<String>> {
    for stmt in stmts {
        if let Stmt::Assign(assign) = stmt {
            for target in &assign.targets {
                if let ast::Expr::Name(name) = target {
                    if name.id.as_str() == "__all__" {
                        if let ast::Expr::List(list) = assign.value.as_ref() {
                            let names: Vec<String> = list
                                .elts
                                .iter()
                                .filter_map(|elt| {
                                    if let ast::Expr::StringLiteral(s) = elt {
                                        Some(s.value.to_string())
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            return Some(names);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Check if a name should be exported (public or in `__all__`).
fn should_export(name: &str, all_names: Option<&Vec<String>>) -> bool {
    if let Some(names) = all_names {
        names.iter().any(|n| n == name)
    } else {
        !name.starts_with('_')
    }
}

/// Format a function definition as a `.pyi` stub line.
fn format_function_def(func: &ast::StmtFunctionDef, source: &str) -> String {
    let name = func.name.as_str();
    let async_prefix = if func.is_async { "async " } else { "" };

    let mut params = Vec::new();
    let parameters = &func.parameters;

    for param in parameters.posonlyargs.iter().chain(parameters.args.iter()) {
        let pname = param.parameter.name.as_str();
        let ann = param
            .parameter
            .annotation
            .as_ref()
            .and_then(|a| slice_expr(a, source));
        if let Some(a) = ann {
            params.push(format!("{pname}: {a}"));
        } else {
            params.push(pname.to_owned());
        }
    }

    if let Some(vararg) = &parameters.vararg {
        params.push(format!("*{}", vararg.name.as_str()));
    }

    for param in &parameters.kwonlyargs {
        let pname = param.parameter.name.as_str();
        let ann = param
            .parameter
            .annotation
            .as_ref()
            .and_then(|a| slice_expr(a, source));
        if let Some(a) = ann {
            params.push(format!("{pname}: {a}"));
        } else {
            params.push(pname.to_owned());
        }
    }

    if let Some(kwarg) = &parameters.kwarg {
        params.push(format!("**{}", kwarg.name.as_str()));
    }

    let ret = func
        .returns
        .as_ref()
        .and_then(|r| slice_expr(r, source))
        .unwrap_or_else(|| "Any".to_owned());

    format!("{async_prefix}def {name}({}) -> {ret}: ...", params.join(", "))
}

/// Format a class definition as a `.pyi` stub.
fn format_class_def(class: &ast::StmtClassDef) -> String {
    let name = class.name.as_str();
    format!("class {name}: ...")
}

/// Format an annotated assignment as a `.pyi` stub line.
fn format_ann_assign(ann: &ast::StmtAnnAssign, source: &str) -> Option<String> {
    let target = if let ast::Expr::Name(name) = ann.target.as_ref() {
        name.id.as_str()
    } else {
        return None;
    };

    if target.starts_with('_') {
        return None;
    }

    let annotation = slice_expr(&ann.annotation, source)?;
    Some(format!("{target}: {annotation}"))
}

/// Format a simple assignment (infer type from literal).
fn format_assign(assign: &ast::StmtAssign) -> Option<String> {
    let target = assign.targets.first()?;
    let name = if let ast::Expr::Name(n) = target {
        n.id.as_str()
    } else {
        return None;
    };

    if name.starts_with('_') || name == "__all__" {
        return None;
    }

    // Only emit for all-caps constants (likely module-level constants).
    if name.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
        Some(format!("{name}: Any"))
    } else {
        None
    }
}

/// Extract source text for an expression node using its text range.
fn slice_expr(expr: &ast::Expr, source: &str) -> Option<String> {
    use ruff_text_size::Ranged;
    let range = expr.range();
    let start: usize = range.start().into();
    let end: usize = range.end().into();
    if start < source.len() && end <= source.len() && start < end {
        Some(source[start..end].to_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_from_annotated_source() {
        let source = r#"
def greet(name: str) -> str:
    return f"Hello {name}"

class Dog:
    name: str
    def bark(self) -> str:
        return "woof"

VERSION: str = "1.0.0"
"#;

        let result = generate_ast_stubs_from_source("mymodule", source).unwrap();
        assert!(result.pyi_content.contains("def greet(name: str) -> str: ..."));
        assert!(result.pyi_content.contains("class Dog: ..."));
        assert!(result.pyi_content.contains("VERSION: str"));
        assert_eq!(result.mode, StubGenMode::Ast);
    }

    #[test]
    fn generate_skips_private_names() {
        let source = r#"
def _private(): pass
def public(): pass
class _Internal: pass
"#;

        let result = generate_ast_stubs_from_source("mymodule", source).unwrap();
        assert!(!result.pyi_content.contains("_private"));
        assert!(!result.pyi_content.contains("_Internal"));
        assert!(result.pyi_content.contains("def public"));
    }

    #[test]
    fn generate_respects_all() {
        let source = r#"
__all__ = ["exported"]

def exported(): pass
def not_exported(): pass
"#;

        let result = generate_ast_stubs_from_source("mymodule", source).unwrap();
        assert!(result.pyi_content.contains("def exported"));
        assert!(!result.pyi_content.contains("not_exported"));
    }

    #[test]
    fn generate_unannotated_gets_any() {
        let source = "def foo(x, y): pass\n";
        let result = generate_ast_stubs_from_source("mymodule", source).unwrap();
        assert!(result.pyi_content.contains("def foo(x, y) -> Any: ..."));
    }

    #[test]
    fn generate_async_function() {
        let source = "async def fetch(url: str) -> bytes: ...\n";
        let result = generate_ast_stubs_from_source("mymodule", source).unwrap();
        assert!(result.pyi_content.contains("async def fetch(url: str) -> bytes: ..."));
    }
}
