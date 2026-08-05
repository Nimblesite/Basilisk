//! Implements [`callables_protocol_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Module context, function signatures, and protocol info for `callables_protocol_2`.

use ruff_python_ast::{self as ast, Stmt};

// Re-export shared helpers so sibling modules can use `context::ann_str` etc.
pub(super) use crate::rules::shared::{ann_str, expr_name};

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
    pub(super) return_type: String,
    /// `true` when the signature's `**kwargs` were expanded into
    /// `kw_only_params`. [`callables_protocol_2`]
    pub(super) had_unpack_kwargs: bool,
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
    /// Overload signatures for `__call__`, if any.
    pub(super) overload_sigs: Vec<FuncSig>,
    /// Protocol attributes with names and type annotations.
    pub(super) attrs: Vec<ProtocolAttr>,
}

/// A declared attribute on a protocol.
#[derive(Debug, Clone)]
pub(super) struct ProtocolAttr {
    /// Attribute name.
    pub(super) name: String,
    /// Type annotation text (e.g. `int`, `str`).
    pub(super) ann: String,
}

// Ensure ProtocolAttr fields are considered used for dead-code analysis.
// These are infrastructure for protocol attribute type checking.
const _: () = {
    fn _assert_fields_used(attr: &ProtocolAttr) {
        let _ = &attr.name;
        let _ = &attr.ann;
    }
};

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
        let protocols = Vec::new();
        let mut non_protocol_classes = Vec::new();
        for stmt in stmts {
            match stmt {
                Stmt::FunctionDef(func) => {
                    functions.push(extract_func_sig(func, false));
                }
                Stmt::ClassDef(cls) => {
                    if has_call_method(cls) {
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

/// Returns `true` if the class body contains a `__call__` method.
pub(super) fn has_call_method(cls: &ast::StmtClassDef) -> bool {
    cls.body
        .iter()
        .any(|stmt| matches!(stmt, Stmt::FunctionDef(f) if f.name.as_str() == "__call__"))
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
        had_unpack_kwargs: false,
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

/// Extract the base name from a type string (strips `[...]` subscript).
pub(super) fn extract_base_name(s: &str) -> String {
    s.find('[')
        .map_or_else(|| s.trim().to_owned(), |i| s[..i].trim().to_owned())
}
