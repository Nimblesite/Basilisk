//! Implements [BSK-E0014] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Signature model for structural callable/protocol subtyping: parameter and
//! signature types, plus extraction from `ruff` AST class/function definitions.

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, Stmt};

use crate::rules::shared::ann_str;

/// One callable parameter.
#[derive(Debug, Clone)]
pub(super) struct Param {
    pub(super) name: String,
    pub(super) ty: Option<String>,
    pub(super) has_default: bool,
    /// `true` for positional-or-keyword ("standard") parameters.
    pub(super) is_standard: bool,
}

/// A `*args` or `**kwargs` parameter slot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) enum StarParam {
    /// The signature has no such parameter.
    #[default]
    Absent,
    /// Present without an annotation (implicitly `Any`).
    Untyped,
    /// Present with an annotation.
    Typed(String),
}

impl StarParam {
    /// `true` when the parameter exists in the signature.
    pub(super) fn is_present(&self) -> bool {
        !matches!(self, StarParam::Absent)
    }

    /// The annotation text; `None` for absent or untyped (gradual `Any`).
    pub(super) fn ty(&self) -> Option<&str> {
        match self {
            StarParam::Typed(ty) => Some(ty),
            StarParam::Absent | StarParam::Untyped => None,
        }
    }

    fn from_annotation(annotation: Option<String>) -> StarParam {
        annotation.map_or(StarParam::Untyped, StarParam::Typed)
    }
}

/// A parsed callable signature.
#[derive(Debug, Clone, Default)]
pub(super) struct Sig {
    /// Positional parameters (positional-only first, then standard).
    pub(super) positional: Vec<Param>,
    pub(super) kwonly: Vec<Param>,
    /// The `*args` parameter slot.
    pub(super) vararg: StarParam,
    /// The `**kwargs` parameter slot.
    pub(super) kwarg: StarParam,
    pub(super) ret: Option<String>,
    /// `true` when the parameter list is gradual (`...`): `positional` then
    /// holds the required `Concatenate` prefix and `kwonly` any retained
    /// keyword-only parameters.
    pub(super) gradual: bool,
}

/// The resolved signatures of a type expression.
pub(super) enum TypeSigs {
    /// Involves a `ParamSpec` or another non-evaluable form — treat as compatible.
    Unknown,
    /// Concrete overload set.
    Sigs(Vec<Sig>),
}

/// An anonymous positional-only parameter of type `ty`.
pub(super) fn posonly_param(ty: String) -> Param {
    Param {
        name: String::new(),
        ty: Some(ty),
        has_default: false,
        is_standard: false,
    }
}

/// A class indexed for structural comparison: its method signatures,
/// attribute names, base classes, and generic parameters.
pub(super) struct ClassEntry {
    /// Method name → overload signatures.
    pub(super) methods: HashMap<String, Vec<Sig>>,
    /// Annotated attribute names declared in the class body.
    pub(super) attrs: HashSet<String>,
    /// Base class names (subscripts stripped, `Protocol`/`Generic` excluded).
    pub(super) bases: Vec<String>,
    pub(super) is_protocol: bool,
    /// Generic parameter names (PEP 695 or `Protocol[...]`/`Generic[...]`).
    pub(super) generic_params: Vec<String>,
}

/// Build a [`ClassEntry`] from a class definition.
pub(super) fn class_entry(cls: &ruff_python_ast::StmtClassDef) -> ClassEntry {
    let mut methods: HashMap<String, Vec<&ruff_python_ast::StmtFunctionDef>> = HashMap::new();
    let mut attrs = HashSet::new();
    for stmt in &cls.body {
        match stmt {
            Stmt::FunctionDef(func) => {
                methods.entry(func.name.to_string()).or_default().push(func);
            }
            Stmt::AnnAssign(ann) => {
                if let Expr::Name(target) = ann.target.as_ref() {
                    let _ = attrs.insert(target.id.to_string());
                }
            }
            _ => {}
        }
    }

    let method_sigs = methods
        .into_iter()
        .map(|(name, defs)| (name, overload_sigs(&defs)))
        .collect();

    let mut is_protocol = false;
    let mut bases = Vec::new();
    for base in cls.bases() {
        let base_name = match base {
            Expr::Subscript(sub) => ann_str(&sub.value),
            other => ann_str(other),
        };
        match base_name.as_str() {
            "Protocol" => is_protocol = true,
            "Generic" => {}
            _ => bases.push(base_name),
        }
    }

    ClassEntry {
        methods: method_sigs,
        attrs,
        bases,
        is_protocol,
        generic_params: generic_param_names(cls),
    }
}

/// Reduce a method's definitions to its effective overload set.
fn overload_sigs(defs: &[&ruff_python_ast::StmtFunctionDef]) -> Vec<Sig> {
    let is_overload = |f: &ruff_python_ast::StmtFunctionDef| {
        f.decorator_list
            .iter()
            .any(|d| matches!(&d.expression, Expr::Name(n) if n.id.as_str() == "overload"))
    };
    let has_overloads = defs.iter().any(|f| is_overload(f));
    defs.iter()
        .filter(|f| !has_overloads || is_overload(f))
        .map(|f| sig_from_function(f))
        .collect()
}

/// Generic parameter names of a class: PEP 695 params plus `Protocol[...]` /
/// `Generic[...]` subscript arguments.
fn generic_param_names(cls: &ruff_python_ast::StmtClassDef) -> Vec<String> {
    let mut names: Vec<String> = cls
        .type_params
        .as_ref()
        .map(|tp| {
            tp.type_params
                .iter()
                .map(|p| p.name().to_string())
                .collect()
        })
        .unwrap_or_default();
    for base in cls.bases() {
        let Expr::Subscript(sub) = base else { continue };
        let base_name = ann_str(&sub.value);
        if base_name != "Protocol" && base_name != "Generic" {
            continue;
        }
        let args: Vec<&Expr> = match sub.slice.as_ref() {
            Expr::Tuple(t) => t.elts.iter().collect(),
            other => vec![other],
        };
        names.extend(args.iter().filter_map(|a| match a {
            Expr::Name(n) => Some(n.id.to_string()),
            _ => None,
        }));
    }
    names
}

/// Build a [`Sig`] from a method definition (drops `self`).
pub(super) fn sig_from_function(func: &ruff_python_ast::StmtFunctionDef) -> Sig {
    let params = &func.parameters;
    let to_param = |pwd: &ruff_python_ast::ParameterWithDefault, is_standard: bool| Param {
        name: pwd.parameter.name.to_string(),
        ty: pwd.parameter.annotation.as_deref().map(ann_str),
        has_default: pwd.default.is_some(),
        is_standard,
    };
    let mut positional: Vec<Param> = params
        .posonlyargs
        .iter()
        .map(|p| to_param(p, false))
        .chain(params.args.iter().map(|p| to_param(p, true)))
        .collect();
    if !positional.is_empty() {
        let _ = positional.remove(0); // self
    }
    let kwonly = params
        .kwonlyargs
        .iter()
        .map(|p| to_param(p, false))
        .collect();
    let vararg = params.vararg.as_ref().map_or(StarParam::Absent, |v| {
        StarParam::from_annotation(v.annotation.as_deref().map(ann_str))
    });
    let kwarg = params.kwarg.as_ref().map_or(StarParam::Absent, |k| {
        StarParam::from_annotation(k.annotation.as_deref().map(ann_str))
    });
    let ret = func.returns.as_deref().map(ann_str);

    let mut sig = Sig {
        positional,
        kwonly,
        vararg,
        kwarg,
        ret,
        gradual: false,
    };
    // `*args: Any, **kwargs: Any` (literally annotated or unannotated) is
    // equivalent to `...` per the typing spec; other parameters are retained.
    let is_any = |param: &StarParam| param.is_present() && param.ty().is_none_or(|ty| ty == "Any");
    if is_any(&sig.vararg) && is_any(&sig.kwarg) {
        sig.gradual = true;
        sig.vararg = StarParam::Absent;
        sig.kwarg = StarParam::Absent;
    }
    sig
}
