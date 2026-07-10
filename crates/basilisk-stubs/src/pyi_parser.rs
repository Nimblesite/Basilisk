//! Implements [STUBRES-ENGINE]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-ENGINE
//! `.pyi` stub file parser.
//!
//! Parses `.pyi` files using `basilisk-parser` and extracts structured
//! type information into a [`StubModule`]. Handles `@overload` grouping,
//! class definitions with methods and attributes, and module-level
//! variable annotations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ruff_python_ast::{
    self as ast, Decorator, Expr, Parameters, Stmt, StmtAnnAssign, StmtAssign, StmtClassDef,
    StmtFunctionDef,
};

use crate::types::{
    StubClass, StubFunction, StubModule, StubParam, StubParamKind, StubSource, StubTier,
    StubVariable,
};

/// Errors that can occur during `.pyi` parsing.
#[derive(Debug, thiserror::Error)]
pub enum StubParseError {
    /// Failed to read the `.pyi` file from disk.
    #[error("failed to read stub file {path}: {source}")]
    Io {
        /// Path that could not be read.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// The `.pyi` file contained syntax errors.
    #[error("syntax error in stub file {path}: {message}")]
    Syntax {
        /// Path with the syntax error.
        path: PathBuf,
        /// Error description.
        message: String,
    },
}

/// Parse a `.pyi` file from disk and extract a [`StubModule`].
///
/// # Errors
///
/// Returns [`StubParseError::Io`] if the file cannot be read, or
/// [`StubParseError::Syntax`] if the file has parse errors.
pub fn parse_pyi_file(
    path: &Path,
    module_name: &str,
    source: StubSource,
    tier: StubTier,
) -> Result<StubModule, StubParseError> {
    // Routed through `read_tracked` so the result cache records stub reads too.
    // See [CHKCACHE-READSET-FS].
    let content = basilisk_common::fs::read_tracked(path).map_err(|err| StubParseError::Io {
        path: path.to_path_buf(),
        source: err,
    })?;
    parse_pyi_source(&content, path, module_name, source, tier)
}

/// Parse `.pyi` source text and extract a [`StubModule`].
///
/// # Errors
///
/// Returns [`StubParseError::Syntax`] if the source has parse errors.
// Implements [STUBRES-PYI] — reuses `basilisk-parser`; only signatures
// matter (def/class/annotations), bodies (`...`/`pass`) are ignored, and the
// `@overload` decorator is tracked (see `StubExtractor::visit_function`).
pub fn parse_pyi_source(
    content: &str,
    path: &Path,
    module_name: &str,
    source: StubSource,
    tier: StubTier,
) -> Result<StubModule, StubParseError> {
    let module_ast =
        basilisk_parser::parse_source(content.to_owned(), path.to_string_lossy().into_owned())
            .map_err(|err| StubParseError::Syntax {
                path: path.to_path_buf(),
                message: err.to_string(),
            })?
            .ast;
    let mut extractor = StubExtractor::new(module_name, path, source, tier);
    extractor.visit_body(&module_ast.body);
    Ok(extractor.into_module())
}

/// Walks the AST and collects stub information.
struct StubExtractor {
    module_name: String,
    path: PathBuf,
    source: StubSource,
    tier: StubTier,
    functions: HashMap<String, StubFunction>,
    overloads: HashMap<String, Vec<StubFunction>>,
    classes: HashMap<String, StubClass>,
    variables: HashMap<String, StubVariable>,
}

impl StubExtractor {
    fn new(module_name: &str, path: &Path, source: StubSource, tier: StubTier) -> Self {
        Self {
            module_name: module_name.to_owned(),
            path: path.to_path_buf(),
            source,
            tier,
            functions: HashMap::new(),
            overloads: HashMap::new(),
            classes: HashMap::new(),
            variables: HashMap::new(),
        }
    }

    fn into_module(self) -> StubModule {
        StubModule {
            module_name: self.module_name,
            path: self.path,
            source: self.source,
            tier: self.tier,
            functions: self.functions,
            overloads: self.overloads,
            classes: self.classes,
            variables: self.variables,
        }
    }

    /// Visit all top-level statements in a module body.
    fn visit_body(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::FunctionDef(func) => self.visit_function(func, None),
                Stmt::ClassDef(class) => self.visit_class(class),
                Stmt::AnnAssign(ann) => self.visit_ann_assign(ann),
                Stmt::Assign(assign) => self.visit_assign(assign),
                _ => {}
            }
        }
    }

    // Implements [STUBRES-PYI] — `@overload` is the one decorator that is
    // significant for stub extraction: overload variants are grouped separately.
    fn visit_function(&mut self, func: &StmtFunctionDef, class_name: Option<&str>) {
        let decorators = extract_decorator_names(&func.decorator_list);
        let is_overload = decorators.iter().any(|d| d == "overload");
        let params = extract_params(&func.parameters, class_name.is_some());
        let return_type = func.returns.as_ref().map(|ret| expr_to_annotation(ret));

        let stub_fn = StubFunction {
            name: func.name.to_string(),
            params,
            return_type,
            is_overload,
            is_async: func.is_async,
            decorators,
            class_name: class_name.map(str::to_owned),
        };

        let name = func.name.to_string();
        if is_overload {
            self.overloads.entry(name).or_default().push(stub_fn);
        } else {
            let _ = self.functions.insert(name, stub_fn);
        }
    }

    fn visit_class(&mut self, class: &StmtClassDef) {
        let bases = class
            .arguments
            .as_ref()
            .map(|args| args.args.iter().map(expr_to_annotation).collect())
            .unwrap_or_default();

        let mut methods = Vec::new();
        let mut attributes = Vec::new();

        for stmt in &class.body {
            match stmt {
                Stmt::FunctionDef(func) => {
                    let class_name = class.name.to_string();
                    let decorators = extract_decorator_names(&func.decorator_list);
                    let is_overload = decorators.iter().any(|d| d == "overload");
                    let params = extract_params(&func.parameters, true);
                    let return_type = func.returns.as_ref().map(|ret| expr_to_annotation(ret));

                    methods.push(StubFunction {
                        name: func.name.to_string(),
                        params,
                        return_type,
                        is_overload,
                        is_async: func.is_async,
                        decorators,
                        class_name: Some(class_name),
                    });
                }
                Stmt::AnnAssign(ann) => {
                    if let Some(name) = ann_assign_target_name(ann) {
                        attributes.push(StubVariable {
                            name,
                            annotation: Some(expr_to_annotation(&ann.annotation)),
                        });
                    }
                }
                _ => {}
            }
        }

        // Also register methods at the module level for the function,
        // including overloads tracking on the class stub.
        self.visit_function_methods_for_class(&class.name, &methods);

        let stub_class = StubClass {
            name: class.name.to_string(),
            bases,
            methods,
            attributes,
        };
        let _ = self.classes.insert(class.name.to_string(), stub_class);
    }

    /// Register class methods' overload groups on the extractor so they can be
    /// queried by qualified name (`ClassName.method_name`).
    fn visit_function_methods_for_class(&mut self, class_name: &str, methods: &[StubFunction]) {
        for method in methods {
            let qualified = format!("{class_name}.{}", method.name);
            if method.is_overload {
                self.overloads
                    .entry(qualified)
                    .or_default()
                    .push(method.clone());
            } else {
                let _ = self.functions.insert(qualified, method.clone());
            }
        }
    }

    fn visit_ann_assign(&mut self, ann: &StmtAnnAssign) {
        if let Some(name) = ann_assign_target_name(ann) {
            let _ = self.variables.insert(
                name.clone(),
                StubVariable {
                    name,
                    annotation: Some(expr_to_annotation(&ann.annotation)),
                },
            );
        }
    }

    fn visit_assign(&mut self, assign: &StmtAssign) {
        // Handle `__all__ = [...]` and type alias assignments like `X = int`.
        for target in &assign.targets {
            if let Expr::Name(name_expr) = target {
                let annotation = Some(expr_to_annotation(&assign.value));
                let _ = self.variables.insert(
                    name_expr.id.to_string(),
                    StubVariable {
                        name: name_expr.id.to_string(),
                        annotation,
                    },
                );
            }
        }
    }
}

/// Extract the target name from an annotated assignment (`x: int = ...`).
fn ann_assign_target_name(ann: &StmtAnnAssign) -> Option<String> {
    if let Expr::Name(name_expr) = ann.target.as_ref() {
        Some(name_expr.id.to_string())
    } else {
        None
    }
}

/// Extract decorator names from a list of decorators.
///
/// Handles simple names (`@overload`), dotted names (`@typing.overload`),
/// and call expressions (`@some_decorator()`).
fn extract_decorator_names(decorators: &[Decorator]) -> Vec<String> {
    decorators
        .iter()
        .map(|dec| expr_to_decorator_name(&dec.expression))
        .collect()
}

/// Convert a decorator expression to its string name.
fn expr_to_decorator_name(expr: &Expr) -> String {
    match expr {
        Expr::Name(name) => name.id.to_string(),
        Expr::Attribute(attr) => {
            let prefix = expr_to_decorator_name(&attr.value);
            format!("{prefix}.{}", attr.attr)
        }
        Expr::Call(call) => expr_to_decorator_name(&call.func),
        _ => String::new(),
    }
}

/// Convert a type annotation expression to its text representation.
///
/// Recursively handles subscripts (`list[int]`), unions (`int | str`),
/// attributes (`typing.Optional`), and other forms.
fn expr_to_annotation(expr: &Expr) -> String {
    match expr {
        Expr::Name(name) => name.id.to_string(),
        Expr::NoneLiteral(_) => "None".to_owned(),
        Expr::EllipsisLiteral(_) => "...".to_owned(),
        Expr::Attribute(attr) => {
            let prefix = expr_to_annotation(&attr.value);
            format!("{prefix}.{}", attr.attr)
        }
        Expr::Subscript(sub) => {
            let base = expr_to_annotation(&sub.value);
            let slice = expr_to_annotation(&sub.slice);
            format!("{base}[{slice}]")
        }
        Expr::Tuple(tuple) => {
            let elts: Vec<String> = tuple.elts.iter().map(expr_to_annotation).collect();
            elts.join(", ")
        }
        Expr::List(list) => {
            let elts: Vec<String> = list.elts.iter().map(expr_to_annotation).collect();
            format!("[{}]", elts.join(", "))
        }
        Expr::BinOp(binop) if matches!(binop.op, ast::Operator::BitOr) => {
            let left = expr_to_annotation(&binop.left);
            let right = expr_to_annotation(&binop.right);
            format!("{left} | {right}")
        }
        Expr::StringLiteral(s) => format!("\"{}\"", s.value),
        Expr::NumberLiteral(n) => match &n.value {
            ast::Number::Int(i) => i.to_string(),
            ast::Number::Float(f) => format!("{f}"),
            ast::Number::Complex { real, imag } => format!("{real}+{imag}j"),
        },
        Expr::BooleanLiteral(b) => {
            if b.value {
                "True".to_owned()
            } else {
                "False".to_owned()
            }
        }
        Expr::Starred(starred) => {
            let inner = expr_to_annotation(&starred.value);
            format!("*{inner}")
        }
        _ => "Unknown".to_owned(),
    }
}

/// Extract parameters from a function's parameter list.
///
/// When `is_method` is true, the first parameter (`self`/`cls`) is skipped.
fn extract_params(params: &Parameters, is_method: bool) -> Vec<StubParam> {
    let mut result = Vec::new();

    // Positional-only parameters (before `/`)
    let posonlyargs = &params.posonlyargs;
    for (idx, param) in posonlyargs.iter().enumerate() {
        if is_method && idx == 0 && result.is_empty() {
            continue;
        }
        result.push(param_to_stub_param(param, StubParamKind::PositionalOnly));
    }

    // Regular parameters
    let skip_first = is_method && result.is_empty();
    for (idx, param) in params.args.iter().enumerate() {
        if skip_first && idx == 0 {
            continue;
        }
        result.push(param_to_stub_param(param, StubParamKind::Regular));
    }

    // *args
    if let Some(vararg) = &params.vararg {
        result.push(StubParam {
            name: vararg.name.to_string(),
            annotation: vararg
                .annotation
                .as_ref()
                .map(|ann| expr_to_annotation(ann)),
            has_default: false,
            kind: StubParamKind::Vararg,
        });
    }

    // Keyword-only parameters (after `*` or `*args`)
    for param in &params.kwonlyargs {
        result.push(param_to_stub_param(param, StubParamKind::KeywordOnly));
    }

    // **kwargs
    if let Some(kwarg) = &params.kwarg {
        result.push(StubParam {
            name: kwarg.name.to_string(),
            annotation: kwarg.annotation.as_ref().map(|ann| expr_to_annotation(ann)),
            has_default: false,
            kind: StubParamKind::Kwarg,
        });
    }

    result
}

/// Convert a `ruff_python_ast::ParameterWithDefault` to a [`StubParam`].
fn param_to_stub_param(param: &ast::ParameterWithDefault, kind: StubParamKind) -> StubParam {
    StubParam {
        name: param.parameter.name.to_string(),
        annotation: param
            .parameter
            .annotation
            .as_ref()
            .map(|ann| expr_to_annotation(ann)),
        has_default: param.default.is_some(),
        kind,
    }
}
