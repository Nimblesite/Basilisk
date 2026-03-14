//! Type structures and parsing utilities for BSK-E0140.

use ruff_python_ast::{self as ast, Expr, Stmt};

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// Signature of a function collected from the AST.
#[derive(Debug, Clone)]
pub(super) struct FuncSig {
    pub(super) name: String,
    pub(super) positional_params: Vec<ParamInfo>,
    pub(super) has_varargs: bool,
    pub(super) varargs_type: String,
    pub(super) has_kwargs: bool,
    pub(super) kwargs_type: String,
    pub(super) kw_only_params: Vec<ParamInfo>,
    #[expect(
        dead_code,
        reason = "return_type will be used for future type checking"
    )]
    pub(super) return_type: String,
}

/// Information about a single function parameter.
#[derive(Debug, Clone)]
pub(super) struct ParamInfo {
    pub(super) name: String,
    pub(super) type_annotation: String,
    pub(super) has_default: bool,
    pub(super) is_positional_only: bool,
}

/// Information about a Protocol class's call signature.
#[derive(Debug, Clone)]
pub(super) struct ProtocolInfo {
    pub(super) name: String,
    pub(super) call_sig: Option<FuncSig>,
    pub(super) has_extra_attrs: bool,
}

/// Callable type annotation parsed form.
#[expect(dead_code, reason = "struct will be used for future type checking")]
pub(super) struct CallableTypeInfo {
    pub(super) param_types: Option<Vec<String>>,
    pub(super) concatenate_prefix: Vec<String>,
    pub(super) is_open_ended: bool,
    #[expect(
        dead_code,
        reason = "return_type will be used for future type checking"
    )]
    pub(super) return_type: String,
}

/// Module-level context containing all collected functions, protocols, and classes.
pub(super) struct ModuleContext {
    pub(super) functions: Vec<FuncSig>,
    pub(super) protocols: Vec<ProtocolInfo>,
    pub(super) non_protocol_classes: Vec<String>,
}

impl ModuleContext {
    /// Build context from a list of top-level AST statements.
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

    /// Find a function by name.
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

/// Returns `true` if the class inherits from `Protocol` (plain or subscripted).
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

/// Extract protocol info (call signature and extra attributes) from a class.
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
pub(super) fn is_overload_decorated(func: &ast::StmtFunctionDef) -> bool {
    func.decorator_list
        .iter()
        .any(|dec| matches!(&dec.expression, Expr::Name(n) if n.id.as_str() == "overload"))
}

/// Extract a function's signature from its AST node.
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

/// Build a `ParamInfo` from an AST parameter with default.
pub(super) fn mk_param(param: &ast::ParameterWithDefault, is_pos_only: bool) -> ParamInfo {
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

// ---------------------------------------------------------------------------
// String utilities
// ---------------------------------------------------------------------------

/// Extract the base name (before `[`) from an annotation string.
pub(super) fn extract_base_name(s: &str) -> String {
    s.find('[')
        .map_or_else(|| s.trim().to_owned(), |i| s[..i].trim().to_owned())
}

/// Convert an expression to its annotation string form.
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

/// Extract a name string from a `Name` expression.
pub(super) fn expr_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Callable type parsing
// ---------------------------------------------------------------------------

/// Parse a `Callable[..., ret]` annotation into a structured form.
pub(super) fn parse_callable_type(s: &str) -> Option<CallableTypeInfo> {
    if !s.starts_with("Callable[") {
        return None;
    }
    let inner = &s["Callable[".len()..s.len().checked_sub(1)?];
    let (first, ret) = split_top_comma(inner)?;
    let first = first.trim();
    let ret = ret.trim().to_owned();
    if first == "..." {
        return Some(CallableTypeInfo {
            param_types: None,
            concatenate_prefix: Vec::new(),
            is_open_ended: true,
            return_type: ret,
        });
    }
    if first.starts_with("Concatenate[") {
        let ci = &first["Concatenate[".len()..first.len().checked_sub(1)?];
        let parts = split_all_commas(ci);
        let mut prefix = Vec::new();
        let mut open = false;
        for p in &parts {
            let p = p.trim();
            if p == "..." {
                open = true;
            } else {
                prefix.push(p.to_owned());
            }
        }
        return Some(CallableTypeInfo {
            param_types: None,
            concatenate_prefix: prefix,
            is_open_ended: open,
            return_type: ret,
        });
    }
    if first.starts_with('[') && first.ends_with(']') {
        let li = &first[1..first.len() - 1];
        let types = if li.trim().is_empty() {
            Vec::new()
        } else {
            split_all_commas(li)
                .iter()
                .map(|s| s.trim().to_owned())
                .collect()
        };
        return Some(CallableTypeInfo {
            param_types: Some(types),
            concatenate_prefix: Vec::new(),
            is_open_ended: false,
            return_type: ret,
        });
    }
    None
}

/// Split at the first top-level comma.
pub(super) fn split_top_comma(s: &str) -> Option<(&str, &str)> {
    let mut d: usize = 0;
    for (i, c) in s.char_indices() {
        match c {
            '[' | '(' => d += 1,
            ']' | ')' => d = d.saturating_sub(1),
            ',' if d == 0 => return Some((&s[..i], &s[i + 1..])),
            _ => {}
        }
    }
    None
}

/// Split at every top-level comma.
pub(super) fn split_all_commas(s: &str) -> Vec<&str> {
    let mut d: usize = 0;
    let mut parts = Vec::new();
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '[' | '(' => d += 1,
            ']' | ')' => d = d.saturating_sub(1),
            ',' if d == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Returns `true` when `target` and `source` types are compatible.
pub(super) fn types_compat(target: &str, source: &str) -> bool {
    if target == source {
        return true;
    }
    if target == "Any" || source == "Any" {
        return true;
    }
    if target.is_empty() || source.is_empty() {
        return true;
    }
    if target == "int" && source == "float" {
        return true;
    }
    if target == "float" && source == "int" {
        return true;
    }
    if target == "bool" && source == "int" {
        return true;
    }
    if target.contains(" | ") {
        return target.split(" | ").any(|m| m.trim() == source);
    }
    let builtins = [
        "int", "str", "float", "bool", "bytes", "None", "complex", "object",
    ];
    if builtins.contains(&target) && builtins.contains(&source) {
        return false;
    }
    true
}
