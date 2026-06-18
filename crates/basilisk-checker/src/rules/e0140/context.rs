//! Implements [BSK-E0140] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Module context, function signatures, and protocol info for BSK-E0140.

use ruff_python_ast::{self as ast, Expr, Stmt};

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
    /// `true` if this signature originally declared `**kwargs: Unpack[TypedDict]`
    /// that has since been expanded into `kw_only_params`. Distinguishes a
    /// callable that genuinely accepts the TypedDict's keys via `**kwargs` from
    /// one with only fixed parameters: per the typing spec a destination
    /// `**kwargs: Unpack[TD]` requires the source to also provide `**kwargs`.
    /// [BSK-E0140]
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

/// A field in a `TypedDict` class.
#[derive(Debug, Clone)]
pub(super) struct TypedDictField {
    /// Field name.
    pub(super) name: String,
    /// Inner type annotation (unwrapped from Required/NotRequired).
    pub(super) type_ann: String,
    /// Whether the field is required.
    pub(super) is_required: bool,
}

/// Collected `TypedDict` definition.
#[derive(Debug, Clone)]
pub(super) struct TypedDictDef {
    /// `TypedDict` class name.
    pub(super) name: String,
    /// All fields (including inherited ones).
    pub(super) fields: Vec<TypedDictField>,
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
        let mut typeddicts = Vec::new();
        for stmt in stmts {
            match stmt {
                Stmt::FunctionDef(func) => {
                    functions.push(extract_func_sig(func, false));
                }
                Stmt::ClassDef(cls) => {
                    if is_protocol_class(cls) {
                        protocols.push(extract_protocol_info(cls));
                    } else if is_typeddict_class(cls, &typeddicts) {
                        typeddicts.push(extract_typeddict(cls, &typeddicts));
                    } else if has_call_method(cls) {
                        non_protocol_classes.push(cls.name.to_string());
                    }
                }
                _ => {}
            }
        }
        // Expand Unpack[TypedDict] kwargs into effective kw-only params, for both
        // top-level functions and protocol `__call__` signatures, so the two
        // compare structurally regardless of which side declared `Unpack`.
        expand_unpack_kwargs(&mut functions, &typeddicts);
        expand_unpack_kwargs_in_protocols(&mut protocols, &typeddicts);
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
    let mut overload_sigs = Vec::new();
    let mut has_extra_attrs = false;
    let mut attrs = Vec::new();
    for body_stmt in &cls.body {
        match body_stmt {
            Stmt::FunctionDef(func) if func.name.as_str() == "__call__" => {
                if is_overload_decorated(func) {
                    overload_sigs.push(extract_func_sig(func, true));
                } else {
                    call_sig = Some(extract_func_sig(func, true));
                }
            }
            Stmt::AnnAssign(ann) => {
                if let Some(attr_name) = expr_name(&ann.target) {
                    let is_dunder = is_standard_dunder(attr_name);
                    if !is_dunder {
                        has_extra_attrs = true;
                    }
                    attrs.push(ProtocolAttr {
                        name: attr_name.to_owned(),
                        ann: ann_str(&ann.annotation),
                    });
                }
            }
            _ => {}
        }
    }
    ProtocolInfo {
        name: cls.name.to_string(),
        call_sig,
        has_extra_attrs,
        overload_sigs,
        attrs,
    }
}

/// Returns `true` for standard dunder attributes that all functions/objects have.
fn is_standard_dunder(name: &str) -> bool {
    matches!(
        name,
        "__name__" | "__module__" | "__qualname__" | "__annotations__" | "__doc__"
    )
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

// ---------------------------------------------------------------------------
// TypedDict support
// ---------------------------------------------------------------------------

/// Returns `true` if the class is a `TypedDict` (directly or via inheritance).
fn is_typeddict_class(cls: &ast::StmtClassDef, known: &[TypedDictDef]) -> bool {
    cls.arguments.as_ref().is_some_and(|args| {
        args.args.iter().any(|arg| {
            if let Expr::Name(n) = arg {
                let name = n.id.as_str();
                name == "TypedDict" || known.iter().any(|td| td.name == name)
            } else {
                false
            }
        })
    })
}

/// Extract a [`TypedDictDef`] from a `TypedDict` class definition.
fn extract_typeddict(cls: &ast::StmtClassDef, known: &[TypedDictDef]) -> TypedDictDef {
    let mut fields = Vec::new();
    // Collect inherited fields from base TypedDicts
    if let Some(args) = &cls.arguments {
        for base in &args.args {
            if let Expr::Name(n) = base {
                if let Some(base_td) = known.iter().find(|td| td.name == n.id.as_str()) {
                    fields.extend(base_td.fields.iter().cloned());
                }
            }
        }
    }
    // Collect own fields
    for stmt in &cls.body {
        if let Stmt::AnnAssign(ann) = stmt {
            if let Some(field_name) = expr_name(&ann.target) {
                let (type_ann, is_required) = unwrap_required_annotation(&ann.annotation);
                fields.push(TypedDictField {
                    name: field_name.to_owned(),
                    type_ann,
                    is_required,
                });
            }
        }
    }
    TypedDictDef {
        name: cls.name.to_string(),
        fields,
    }
}

/// Unwrap `Required` / `NotRequired` / `ReadOnly` qualifiers, returning the inner
/// type text and whether the field is required. `ReadOnly` (PEP 705) has no effect
/// on a `**kwargs: Unpack[TD]` signature, so it is stripped transparently. Total
/// TypedDicts default to required. [BSK-E0140]
fn unwrap_required_annotation(expr: &Expr) -> (String, bool) {
    if let Expr::Subscript(sub) = expr {
        if let Expr::Name(n) = sub.value.as_ref() {
            match n.id.as_str() {
                "Required" => return (unwrap_required_annotation(&sub.slice).0, true),
                "NotRequired" => return (unwrap_required_annotation(&sub.slice).0, false),
                "ReadOnly" => return unwrap_required_annotation(&sub.slice),
                _ => {}
            }
        }
    }
    // Default: Required (total=True)
    (ann_str(expr), true)
}

/// Expand `**kwargs: Unpack[TD]` into effective kw-only params for every function.
fn expand_unpack_kwargs(functions: &mut [FuncSig], typeddicts: &[TypedDictDef]) {
    for func in functions.iter_mut() {
        expand_unpack_in_sig(func, typeddicts);
    }
}

/// Expand `**kwargs: Unpack[TD]` in protocol `__call__` and overload signatures,
/// mirroring [`expand_unpack_kwargs`] so a protocol target compares structurally
/// against a function whose kwargs were already expanded. [BSK-E0140]
fn expand_unpack_kwargs_in_protocols(protocols: &mut [ProtocolInfo], typeddicts: &[TypedDictDef]) {
    for proto in protocols.iter_mut() {
        if let Some(call_sig) = proto.call_sig.as_mut() {
            expand_unpack_in_sig(call_sig, typeddicts);
        }
        for overload_sig in &mut proto.overload_sigs {
            expand_unpack_in_sig(overload_sig, typeddicts);
        }
    }
}

/// Expand a single signature's `**kwargs: Unpack[TD]` into kw-only params and record
/// [`FuncSig::had_unpack_kwargs`]. No-op when the signature has no `**kwargs` or the
/// annotation is not `Unpack[TypedDict]`. [BSK-E0140]
fn expand_unpack_in_sig(sig: &mut FuncSig, typeddicts: &[TypedDictDef]) {
    if !sig.has_kwargs {
        return;
    }
    let Some(td_name) = extract_unpack_type(&sig.kwargs_type) else {
        return;
    };
    let Some(td) = typeddicts.iter().find(|td| td.name == td_name) else {
        return;
    };
    // Replace kwargs with expanded kw-only params from the TypedDict.
    sig.has_kwargs = false;
    sig.kwargs_type = String::new();
    sig.had_unpack_kwargs = true;
    for field in &td.fields {
        sig.kw_only_params.push(ParamInfo {
            name: field.name.clone(),
            type_annotation: field.type_ann.clone(),
            has_default: !field.is_required,
            is_positional_only: false,
        });
    }
}

/// Extract the `TypedDict` name from an `Unpack[TD]` annotation string.
fn extract_unpack_type(ann: &str) -> Option<&str> {
    let inner = ann.strip_prefix("Unpack[")?.strip_suffix(']')?;
    Some(inner.trim())
}
