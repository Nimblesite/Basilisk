//! Implements [`assignment_compatibility`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Signature model for structural callable/protocol subtyping: parameter and
//! signature types, plus extraction from `ruff` AST class/function definitions.
//!
//! Every parameter and return annotation is LOWERED through the module's
//! binding table into a resolved [`TypeNode`] at extraction time
//! ([ASTREBUILD-LAW], [RESOLV-CANONICAL-RELATION]) — the model never stores
//! rendered annotation text, so an aliased import or a reformatted spelling
//! produces an identical signature model.

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{BindingTable, TypeNode};
use ruff_python_ast::{Expr, Stmt};

/// One callable parameter.
#[derive(Debug, Clone)]
pub(super) struct Param {
    pub(super) name: String,
    /// The lowered annotation; `None` when unannotated (gradual).
    pub(super) ty: Option<TypeNode>,
    pub(super) has_default: bool,
    /// `true` for positional-or-keyword ("standard") parameters.
    pub(super) is_standard: bool,
}

/// A `*args` or `**kwargs` slot with its annotation lowered to a resolved
/// [`TypeNode`] — the semantic replacement for the text-carrying shared
/// `StarParam` ([ASTREBUILD-LAW]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) enum SigStar {
    /// The signature has no such parameter.
    #[default]
    Absent,
    /// Present without an annotation (implicitly `Any`).
    Untyped,
    /// Present with a lowered annotation.
    Typed(TypeNode),
}

impl SigStar {
    /// `true` when the parameter exists in the signature.
    pub(super) fn is_present(&self) -> bool {
        !matches!(self, SigStar::Absent)
    }

    /// The lowered annotation; `None` for absent or untyped (gradual `Any`).
    pub(super) fn ty(&self) -> Option<&TypeNode> {
        match self {
            SigStar::Typed(ty) => Some(ty),
            SigStar::Absent | SigStar::Untyped => None,
        }
    }
}

/// A parsed callable signature.
#[derive(Debug, Clone, Default)]
pub(super) struct Sig {
    /// Positional parameters (positional-only first, then standard).
    pub(super) positional: Vec<Param>,
    pub(super) kwonly: Vec<Param>,
    /// The `*args` parameter slot.
    pub(super) vararg: SigStar,
    /// The `**kwargs` parameter slot.
    pub(super) kwarg: SigStar,
    /// The lowered return annotation; `None` when unannotated (gradual).
    pub(super) ret: Option<TypeNode>,
    /// `true` when the parameter list is gradual (`...`): `positional` then
    /// holds the required `Concatenate` prefix and `kwonly` any retained
    /// keyword-only parameters.
    pub(super) gradual: bool,
}

/// The resolved signatures of a type expression.
pub(super) enum TypeSigs {
    /// Involves a `ParamSpec`, an unspecializable generic, or another
    /// non-evaluable form — every verdict over it abstains
    /// ([ASTREBUILD-PHASE-RESOLVER]).
    Unknown,
    /// Concrete overload set.
    Sigs(Vec<Sig>),
}

/// A class indexed for structural comparison: its method signatures,
/// attribute names, base classes, and generic parameters.
pub(super) struct ClassEntry {
    /// Method name → overload signatures.
    pub(super) methods: HashMap<String, Vec<Sig>>,
    /// Annotated attribute names declared in the class body.
    #[expect(
        dead_code,
        reason = "read only by the inert protocol_members scaffolding ([ASTREBUILD-PHASE-RESOLVER])"
    )]
    pub(super) attrs: HashSet<String>,
    /// Base class identifiers (subscripts stripped). A base that is not a
    /// plain name yields an empty string, which matches no indexed class and
    /// is therefore treated as an unresolvable base by consumers.
    #[expect(
        dead_code,
        reason = "read only by the inert protocol_members scaffolding ([ASTREBUILD-PHASE-RESOLVER])"
    )]
    pub(super) bases: Vec<String>,
    /// Generic parameter names.
    pub(super) generic_params: Vec<String>,
}

/// Build a [`ClassEntry`] from a class definition, lowering every method
/// annotation through `bindings`.
pub(super) fn class_entry(
    cls: &ruff_python_ast::StmtClassDef,
    bindings: &BindingTable,
) -> ClassEntry {
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
        .map(|(name, defs)| (name, overload_sigs(&defs, bindings)))
        .collect();

    let bases = cls.bases().iter().map(base_name).collect();

    ClassEntry {
        methods: method_sigs,
        attrs,
        bases,
        generic_params: crate::rules::shared::class_generic_param_names(cls),
    }
}

/// The identifier a base-class expression names: `Base` or `Base[...]`.
/// Anything else — an attribute path, a call — yields an empty string, which
/// never names an indexed class ([ASTREBUILD-LAW]: identifiers come from the
/// AST node, never from sliced source text).
fn base_name(base: &Expr) -> String {
    let target = match base {
        Expr::Subscript(sub) => sub.value.as_ref(),
        other => other,
    };
    match target {
        Expr::Name(name) => name.id.to_string(),
        _ => String::new(),
    }
}

/// The signatures of every definition of a method.
fn overload_sigs(defs: &[&ruff_python_ast::StmtFunctionDef], bindings: &BindingTable) -> Vec<Sig> {
    defs.iter()
        .map(|f| sig_from_function(f, bindings))
        .collect()
}

/// The slot for a present `*`/`**` parameter.
fn star_slot(bindings: &BindingTable, annotation: Option<&Expr>) -> SigStar {
    annotation.map_or(SigStar::Untyped, |ann| {
        SigStar::Typed(TypeNode::lower(bindings, ann))
    })
}

/// Build a [`Sig`] from a method definition (drops `self`), lowering every
/// annotation through `bindings`.
pub(super) fn sig_from_function(
    func: &ruff_python_ast::StmtFunctionDef,
    bindings: &BindingTable,
) -> Sig {
    let params = &func.parameters;
    let to_param = |pwd: &ruff_python_ast::ParameterWithDefault, is_standard: bool| Param {
        name: pwd.parameter.name.to_string(),
        ty: pwd
            .parameter
            .annotation
            .as_deref()
            .map(|ann| TypeNode::lower(bindings, ann)),
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
    let vararg = params.vararg.as_ref().map_or(SigStar::Absent, |v| {
        star_slot(bindings, v.annotation.as_deref())
    });
    let kwarg = params.kwarg.as_ref().map_or(SigStar::Absent, |k| {
        star_slot(bindings, k.annotation.as_deref())
    });
    let ret = func
        .returns
        .as_deref()
        .map(|r| TypeNode::lower(bindings, r));

    Sig {
        positional,
        kwonly,
        vararg,
        kwarg,
        ret,
        gradual: false,
    }
}
