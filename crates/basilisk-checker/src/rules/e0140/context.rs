//! Module context, function signatures, and protocol info for BSK-E0140.

use ruff_python_ast::{self as ast, Expr, Stmt};

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// Describes a single function parameter.
#[derive(Debug, Clone)]
pub(super) struct ParamInfo {
    /// Parameter name.
    pub(super) name: String,
    /// The annotation text (empty string if unannotated).
    pub(super) type_annotation: String,
    /// Whether the parameter has a default value.
    pub(super) has_default: bool,
    /// Whether the parameter is positional-only (defined before `/`).
    pub(super) is_positional_only: bool,
}

/// Signature extracted from a function definition.
#[derive(Debug, Clone)]
pub(super) struct FuncSig {
    /// Function name.
    pub(super) name: String,
    /// Positional (and positional-or-keyword) parameters, in order.
    pub(super) positional_params: Vec<ParamInfo>,
    /// Whether the function accepts `*args`.
    pub(super) has_varargs: bool,
    /// Annotation text for `*args` (empty if untyped).
    pub(super) varargs_type: String,
    /// Whether the function accepts `**kwargs`.
    pub(super) has_kwargs: bool,
    /// Annotation text for `**kwargs` (empty if untyped).
    pub(super) kwargs_type: String,
    /// Keyword-only parameters.
    pub(super) kw_only_params: Vec<ParamInfo>,
    /// Return type annotation text (empty if unannotated).
    #[expect(
        dead_code,
        reason = "return_type will be used for future type checking"
    )]
    pub(super) return_type: String,
}

/// Information extracted from a `Protocol` class.
#[derive(Debug, Clone)]
pub(super) struct ProtocolInfo {
    /// Protocol class name.
    pub(super) name: String,
    /// The signature of `__call__`, if present.
    pub(super) call_sig: Option<FuncSig>,
    /// Whether the protocol has non-dunder annotated attributes.
    pub(super) has_extra_attrs: bool,
}

/// Module-level context: collected functions and protocols.
pub(super) struct ModuleContext {
    /// Top-level function signatures.
    pub(super) functions: Vec<FuncSig>,
    /// Protocol classes.
    pub(super) protocols: Vec<ProtocolInfo>,
    /// Non-protocol classes that have a `__call__` method.
    pub(super) non_protocol_classes: Vec<String>,
}

impl ModuleContext {
    /// Build a [`ModuleContext`] by scanning the top-level AST statements.
    pub(super) fn from_ast(stmts: &[Stmt]) -> Self {
        let mut functions = Vec::new();
        let mut protocols = Vec::new();
        let mut non_protocol_classes = Vec::new();
        for stmt in stmts {
            match stmt {
                Stmt::FunctionDef(func) => {
                    functions.push(extract_func_sig(func, false));
                }
                Stmt::ClassDef(cls) => {
                    if is_protocol_class(cls) {
                        protocols.push(extract_protocol_info(cls));
                    } else if has_call_method(cls) {
                        non_protocol_classes.push(cls.name.to_string());
                    }
                }
                _ => {}
            }
        }
        Self {
            functions,
            protocols,
            non_protocol_classes,
        }
    }

    /// Find a top-level function by name.
    pub(super) fn find_func(&self, name: &str) -> Option<&FuncSig> {
        self.functions.iter().find(|f| f.name == name)
    }

    /// Find a protocol by name.
    pub(super) fn find_protocol(&self, name: &str) -> Option<&ProtocolInfo> {
        self.protocols.iter().find(|p| p.name == name)
    }

    /// Returns `true` if `name` is a non-protocol class with a `__call__` method.
    pub(super) fn is_non_protocol_class(&self, name: &str) -> bool {
        self.non_protocol_classes.iter().any(|n| n == name)
    }
}

// ---------------------------------------------------------------------------
// AST extraction helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the class inherits from `Protocol` (directly or generically).
pub(super) fn is_protocol_class(cls: &ast::StmtClassDef) -> bool {
    cls.arguments.as_ref().is_some_and(|args| {
        args.args.iter().any(|arg| match arg {
            Expr::Name(name) => name.id.as_str() == "Protocol",
            Expr::Subscript(sub) => {
                matches!(sub.value.as_ref(), Expr::Name(n) if n.id.as_str() == "Protocol")
            }
            _ => false,
        })
    })
}

/// Returns `true` if the class body contains a `__call__` method.
pub(super) fn has_call_method(cls: &ast::StmtClassDef) -> bool {
    cls.body
        .iter()
        .any(|stmt| matches!(stmt, Stmt::FunctionDef(f) if f.name.as_str() == "__call__"))
}

/// Extract a [`ProtocolInfo`] from a protocol class definition.
pub(super) fn extract_protocol_info(cls: &ast::StmtClassDef) -> ProtocolInfo {
    let mut call_sig = None;
    let mut has_extra_attrs = false;
    for body_stmt in &cls.body {
        match body_stmt {
            Stmt::FunctionDef(func) => {
                if func.name.as_str() == "__call__" && !is_overload_decorated(func) {
                    call_sig = Some(extract_func_sig(func, true));
                }
            }
            Stmt::AnnAssign(ann) => {
                if let Some(attr_name) = expr_name(&ann.target) {
                    if !matches!(
                        attr_name,
                        "__name__" | "__module__" | "__qualname__" | "__annotations__" | "__doc__"
                    ) {
                        has_extra_attrs = true;
                    }
                }
            }
            _ => {}
        }
    }
    ProtocolInfo {
        name: cls.name.to_string(),
        call_sig,
        has_extra_attrs,
    }
}

/// Returns `true` if the function is decorated with `@overload`.
fn is_overload_decorated(func: &ast::StmtFunctionDef) -> bool {
    func.decorator_list
        .iter()
        .any(|dec| matches!(&dec.expression, Expr::Name(n) if n.id.as_str() == "overload"))
}

/// Extract a [`FuncSig`] from a function definition.
///
/// If `skip_self` is `true`, the first `self` parameter is omitted.
pub(super) fn extract_func_sig(func: &ast::StmtFunctionDef, skip_self: bool) -> FuncSig {
    let params = &func.parameters;
    let mut positional_params = Vec::new();
    let mut kw_only_params = Vec::new();
    for (idx, param) in params.posonlyargs.iter().enumerate() {
        if skip_self && idx == 0 && param.parameter.name.as_str() == "self" {
            continue;
        }
        positional_params.push(mk_param(param, true));
    }
    for (idx, param) in params.args.iter().enumerate() {
        if skip_self
            && positional_params.is_empty()
            && idx == 0
            && param.parameter.name.as_str() == "self"
        {
            continue;
        }
        positional_params.push(mk_param(param, false));
    }
    for param in &params.kwonlyargs {
        kw_only_params.push(mk_param(param, false));
    }
    let has_varargs = params.vararg.is_some();
    let varargs_type = params
        .vararg
        .as_ref()
        .and_then(|v| v.annotation.as_ref().map(|a| ann_str(a)))
        .unwrap_or_default();
    let has_kwargs = params.kwarg.is_some();
    let kwargs_type = params
        .kwarg
        .as_ref()
        .and_then(|k| k.annotation.as_ref().map(|a| ann_str(a)))
        .unwrap_or_default();
    let return_type = func
        .returns
        .as_ref()
        .map(|r| ann_str(r))
        .unwrap_or_default();
    FuncSig {
        name: func.name.to_string(),
        positional_params,
        has_varargs,
        varargs_type,
        has_kwargs,
        kwargs_type,
        kw_only_params,
        return_type,
    }
}

/// Build a [`ParamInfo`] from a parameter-with-default AST node.
fn mk_param(param: &ast::ParameterWithDefault, is_pos_only: bool) -> ParamInfo {
    ParamInfo {
        name: param.parameter.name.to_string(),
        type_annotation: param
            .parameter
            .annotation
            .as_ref()
            .map(|a| ann_str(a))
            .unwrap_or_default(),
        has_default: param.default.is_some(),
        is_positional_only: is_pos_only,
    }
}

/// Extract the name string from a `Name` expression, if applicable.
pub(super) fn expr_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

/// Render an annotation expression to a string.
pub(super) fn ann_str(expr: &Expr) -> String {
    match expr {
        Expr::Name(n) => n.id.to_string(),
        Expr::Subscript(s) => format!("{}[{}]", ann_str(&s.value), ann_str(&s.slice)),
        Expr::Attribute(a) => format!("{}.{}", ann_str(&a.value), a.attr),
        Expr::Tuple(t) => t.elts.iter().map(ann_str).collect::<Vec<_>>().join(", "),
        Expr::BinOp(b) => format!("{} | {}", ann_str(&b.left), ann_str(&b.right)),
        Expr::NoneLiteral(_) => "None".to_owned(),
        Expr::List(l) => format!(
            "[{}]",
            l.elts.iter().map(ann_str).collect::<Vec<_>>().join(", ")
        ),
        Expr::NumberLiteral(n) => format!("{:?}", n.value),
        _ => "...".to_owned(),
    }
}

/// Extract the base name from a type string (strips `[...]` subscript).
pub(super) fn extract_base_name(s: &str) -> String {
    s.find('[')
        .map_or_else(|| s.trim().to_owned(), |i| s[..i].trim().to_owned())
}
