//! All internal types, parsing, and checking logic for BSK-E0148.

use std::collections::HashMap;

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::Span;

use crate::diagnostic::{Diagnostic, ErrorCode, error_diagnostic_owned};
// Re-export shared helpers so sibling modules can use `helpers::ann_str` etc.
pub(super) use crate::rules::shared::{ann_str, expr_name};
use crate::rules::shared::{infer_expr_literal_type, is_type_compatible, split_top_level_commas};

pub(super) const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0148",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0148",
};

// ---------------------------------------------------------------------------
// TypeVar data types
// ---------------------------------------------------------------------------

/// Constraint group for a `TypeVar`: the list of allowed types.
#[derive(Debug, Clone)]
pub(super) struct ConstrainedTypeVar {
    /// The `TypeVar` name (e.g. `"AnyStr"`).
    pub(super) name: String,
    /// The constraint types in order (e.g. `["str", "bytes"]`).
    pub(super) constraints: Vec<String>,
}

impl ConstrainedTypeVar {
    /// Returns the constraint group index (0-based) that `ty` belongs to, or
    /// `None` when `ty` is not a known constraint.
    pub(super) fn group_of(
        &self,
        ty: &str,
        class_bases: &HashMap<String, Vec<String>>,
    ) -> Option<usize> {
        self.constraints
            .iter()
            .enumerate()
            .find_map(|(idx, constraint)| {
                (ty == constraint.as_str() || is_subtype_of(ty, constraint, class_bases))
                    .then_some(idx)
            })
    }
}

/// Returns `true` when `subtype` is a well-known subtype of `supertype`,
/// or when class inheritance shows `subtype` inherits from `supertype`.
fn is_subtype_of(
    subtype: &str,
    supertype: &str,
    class_bases: &HashMap<String, Vec<String>>,
) -> bool {
    // Built-in subtype relationships.
    if matches!((subtype, supertype), ("bool", "int")) {
        return true;
    }
    // Check class inheritance chain.
    if let Some(bases) = class_bases.get(subtype) {
        if bases.iter().any(|b| b == supertype) {
            return true;
        }
        // Recursive: check if any base is a subtype of supertype.
        return bases
            .iter()
            .any(|b| is_subtype_of(b, supertype, class_bases));
    }
    false
}

/// A function signature with constrained `TypeVar` parameters.
#[derive(Debug, Clone)]
pub(super) struct ConstrainedFunc {
    /// The function name.
    pub(super) name: String,
    /// For each parameter index: which `ConstrainedTypeVar` it uses (by name).
    pub(super) param_tv: Vec<Option<String>>,
}

// ---------------------------------------------------------------------------
// Module context
// ---------------------------------------------------------------------------

/// Module-level knowledge needed to check calls.
pub(super) struct ModuleContext {
    /// All constrained `TypeVars` defined at module level.
    pub(super) constrained_tvars: HashMap<String, ConstrainedTypeVar>,
    /// Functions that have at least one constrained-TypeVar parameter.
    pub(super) constrained_funcs: Vec<ConstrainedFunc>,
    /// Variables with known types: name -> type annotation text.
    pub(super) var_types: HashMap<String, String>,
    /// Classes that represent Mapping types with known key types.
    /// Maps class name -> (`key_type_text`, `value_type_text`).
    pub(super) mapping_vars: HashMap<String, (String, String)>,
    /// Class inheritance: maps class name -> list of base class names.
    /// Used for resolving subclass-to-constraint matching in `TypeVar` checks.
    pub(super) class_bases: HashMap<String, Vec<String>>,
}

impl ModuleContext {
    /// Build a `ModuleContext` from the top-level AST statements.
    pub(super) fn from_ast(stmts: &[Stmt]) -> Self {
        let mut constrained_tvars: HashMap<String, ConstrainedTypeVar> = HashMap::new();
        let mut constrained_funcs: Vec<ConstrainedFunc> = Vec::new();
        let mut var_types: HashMap<String, String> = HashMap::new();
        let mut mapping_vars: HashMap<String, (String, String)> = HashMap::new();

        // Pass 1: collect TypeVar definitions.
        for stmt in stmts {
            if let Stmt::Assign(assign) = stmt {
                if assign.targets.len() == 1 {
                    if let Some(lhs_name) = assign.targets.first().and_then(expr_name) {
                        if let Some(ctv) = try_parse_constrained_typevar(lhs_name, &assign.value) {
                            let _ = constrained_tvars.insert(lhs_name.to_owned(), ctv);
                        }
                    }
                }
            }
        }

        // Pass 2: collect function signatures and variable annotations.
        for stmt in stmts {
            match stmt {
                Stmt::FunctionDef(func) => {
                    if let Some(cfunc) = try_parse_constrained_func(func, &constrained_tvars) {
                        constrained_funcs.push(cfunc);
                    }
                }
                Stmt::AnnAssign(ann) => {
                    if let Some(var_name) = expr_name(&ann.target) {
                        let ann_text = ann_str(&ann.annotation);
                        let _ = var_types.insert(var_name.to_owned(), ann_text.clone());
                        if let Some((key_ty, val_ty)) = parse_mapping_annotation(&ann_text) {
                            let _ = mapping_vars.insert(var_name.to_owned(), (key_ty, val_ty));
                        }
                    }
                }
                _ => {}
            }
        }

        // Pass 3: collect class inheritance for subtype resolution.
        let mut class_bases: HashMap<String, Vec<String>> = HashMap::new();
        for stmt in stmts {
            if let Stmt::ClassDef(cls) = stmt {
                if let Some(args) = &cls.arguments {
                    let bases: Vec<String> = args.args.iter().map(ann_str).collect();
                    if !bases.is_empty() {
                        let _ = class_bases.insert(cls.name.to_string(), bases);
                    }
                }
            }
        }

        Self {
            constrained_tvars,
            constrained_funcs,
            var_types,
            mapping_vars,
            class_bases,
        }
    }
}

// ---------------------------------------------------------------------------
// TypeVar constraint parsing
// ---------------------------------------------------------------------------

/// Try to parse `name = TypeVar("name", str, bytes)` into a `ConstrainedTypeVar`.
fn try_parse_constrained_typevar(lhs_name: &str, expr: &Expr) -> Option<ConstrainedTypeVar> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let callee = expr_name(&call.func)?;
    if callee != "TypeVar" {
        return None;
    }
    if call.arguments.args.len() < 3 {
        return None;
    }
    let constraints: Vec<String> = call
        .arguments
        .args
        .get(1..)
        .unwrap_or_default()
        .iter()
        .map(ann_str)
        .collect();
    if constraints.len() < 2 {
        return None;
    }
    Some(ConstrainedTypeVar {
        name: lhs_name.to_owned(),
        constraints,
    })
}

/// Try to extract constrained-TypeVar parameter info from a function definition.
fn try_parse_constrained_func(
    func: &ast::StmtFunctionDef,
    tvars: &HashMap<String, ConstrainedTypeVar>,
) -> Option<ConstrainedFunc> {
    let mut param_tv: Vec<Option<String>> = Vec::new();
    let mut has_constrained = false;

    for param in func
        .parameters
        .args
        .iter()
        .chain(func.parameters.posonlyargs.iter())
    {
        let tv_name = param
            .parameter
            .annotation
            .as_ref()
            .and_then(|a| expr_name(a))
            .and_then(|ann| tvars.get(ann).map(|tv| tv.name.clone()));
        if tv_name.is_some() {
            has_constrained = true;
        }
        param_tv.push(tv_name);
    }

    if !has_constrained {
        return None;
    }

    Some(ConstrainedFunc {
        name: func.name.to_string(),
        param_tv,
    })
}

// ---------------------------------------------------------------------------
// Mapping annotation parsing
// ---------------------------------------------------------------------------

/// Detect Mapping-like annotations with explicit key/value types.
///
/// Recognises `Name[K, V]` patterns. Returns `(key_type, value_type)` or `None`.
pub(super) fn parse_mapping_annotation(ann: &str) -> Option<(String, String)> {
    let ann = ann.trim();
    let bracket_pos = ann.find('[')?;
    let inner = ann.get(bracket_pos + 1..ann.rfind(']')?)?;
    let args = split_top_level_commas(inner);
    if args.len() < 2 {
        return None;
    }
    let key_ty = args.first()?.trim().to_owned();
    let val_ty = args.get(1)?.trim().to_owned();
    if key_ty.is_empty() || val_ty.is_empty() {
        return None;
    }
    Some((key_ty, val_ty))
}

/// Resolve a `Mapping` annotation, handling custom subclasses with reordered `Generic` params.
///
/// For `MyMap2[int, str]` where `MyMap2(Mapping[K, V], Generic[V, K])`,
/// resolves `Generic[V, K]` specialization order (`V=int, K=str`) then maps
/// `Mapping[K, V]` to `key=K=str, value=V=int`. Returns `(key_type, value_type)`.
pub(super) fn resolve_mapping_annotation(
    ann: &str,
    class_bases: &HashMap<String, Vec<String>>,
) -> Option<(String, String)> {
    // First try direct Mapping/dict annotation.
    let bracket = ann.find('[')?;
    let class_name = ann.get(..bracket)?.trim();

    // Direct Mapping/dict — use first two args directly.
    if matches!(
        class_name,
        "Mapping" | "Dict" | "dict" | "MutableMapping" | "OrderedDict" | "DefaultDict"
    ) {
        return parse_mapping_annotation(ann);
    }

    // Custom class — check if it inherits from Mapping via class_bases.
    let bases = class_bases.get(class_name)?;
    let mapping_base = bases
        .iter()
        .find(|b| b.starts_with("Mapping[") || b.starts_with("MutableMapping["))?;
    let generic_base = bases.iter().find(|b| b.starts_with("Generic["));

    // Parse the specialization args from the annotation.
    let inner = ann.get(bracket + 1..ann.rfind(']')?)?;
    let spec_args: Vec<String> = split_top_level_commas(inner)
        .into_iter()
        .map(|s| s.trim().to_owned())
        .collect();

    // Parse Generic param order if available.
    if let Some(generic) = generic_base {
        let gb = generic.find('[')?;
        let generic_inner = generic.get(gb + 1..generic.rfind(']')?)?;
        let generic_params: Vec<&str> = generic_inner.split(',').map(str::trim).collect();

        // Build substitution map: generic_param → specialized_arg
        let mut subs: HashMap<&str, &str> = HashMap::new();
        for (idx, param) in generic_params.iter().enumerate() {
            if let Some(arg) = spec_args.get(idx) {
                let _ = subs.insert(param, arg.as_str());
            }
        }

        // Parse Mapping[K, V] to find key/value param names.
        let mb = mapping_base.find('[')?;
        let mapping_inner = mapping_base.get(mb + 1..mapping_base.rfind(']')?)?;
        let mapping_params: Vec<&str> = mapping_inner.split(',').map(str::trim).collect();
        let key_param = mapping_params.first()?;
        let val_param = mapping_params.get(1)?;

        // Substitute.
        let key_ty = subs.get(key_param).unwrap_or(key_param);
        let val_ty = subs.get(val_param).unwrap_or(val_param);
        return Some(((*key_ty).to_owned(), (*val_ty).to_owned()));
    }

    // No Generic base — assume direct parameter order matches Mapping.
    parse_mapping_annotation(ann)
}

// ---------------------------------------------------------------------------
// Call-site checking (constrained TypeVar)
// ---------------------------------------------------------------------------

/// Check a single call expression for constrained-`TypeVar` group mismatches.
pub(super) fn check_call(
    call: &ast::ExprCall,
    ctx: &ModuleContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    let Some(callee_name) = expr_name(&call.func) else {
        return;
    };
    let Some(cfunc) = ctx.constrained_funcs.iter().find(|f| f.name == callee_name) else {
        return;
    };

    let mut tv_group: HashMap<&str, (usize, String)> = HashMap::new();

    for (arg_idx, arg) in call.arguments.args.iter().enumerate() {
        let Some(tv_name) = cfunc.param_tv.get(arg_idx).and_then(|o| o.as_deref()) else {
            continue;
        };
        let Some(constrained_tv) = ctx.constrained_tvars.get(tv_name) else {
            continue;
        };
        let Some(arg_type_str) = infer_arg_type(arg, &ctx.var_types) else {
            continue;
        };
        if arg_type_str == "Any" {
            continue;
        }
        let Some(group) = constrained_tv.group_of(&arg_type_str, &ctx.class_bases) else {
            continue;
        };

        match tv_group.get(tv_name) {
            None => {
                let _ = tv_group.insert(tv_name, (group, arg_type_str));
            }
            Some(&(existing_group, ref _existing_type)) => {
                if existing_group != group {
                    diag.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "Constraint mismatch for TypeVar `{tv_name}` in call to \
                             `{callee_name}`: argument types belong to different constraint groups"
                        ),
                        call_span(call),
                        path,
                        Some(format!(
                            "TypeVar `{tv_name}` is constrained to `{}`; all arguments bound to \
                             the same TypeVar must use the same constraint",
                            constrained_tv.constraints.join("` or `")
                        )),
                        Some(
                            "PEP 484: arguments for a constrained TypeVar must all match the \
                             same constraint alternative"
                                .to_owned(),
                        ),
                    ));
                    return; // One diagnostic per call.
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Subscript checking (Mapping key type)
// ---------------------------------------------------------------------------

/// Check a subscript expression for `Mapping` key type mismatches.
pub(super) fn check_subscript(
    sub: &ast::ExprSubscript,
    ctx: &ModuleContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    let Some(obj_name) = expr_name(&sub.value) else {
        return;
    };
    let Some((key_ty, _val_ty)) = ctx.mapping_vars.get(obj_name) else {
        return;
    };
    let Some(idx_ty) = infer_expr_literal_type(&sub.slice) else {
        return;
    };

    if !is_type_compatible(idx_ty, key_ty) {
        let span = Span::from(sub.range());
        diag.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Invalid subscript key type `{idx_ty}` for `{obj_name}` \
                 which expects key type `{key_ty}`"
            ),
            span,
            path,
            Some(format!(
                "`{obj_name}` is parameterized with key type `{key_ty}`; \
                 use a `{key_ty}` value as the subscript key"
            )),
            Some(
                "PEP 484: subscript key must be compatible with the declared key type parameter"
                    .to_owned(),
            ),
        ));
    }
}

// ---------------------------------------------------------------------------
// Class-def checking (generic metaclass)
// ---------------------------------------------------------------------------

/// Check a class definition for use of a parameterized generic as a metaclass.
pub(super) fn check_class_def(cls: &ast::StmtClassDef, path: &str, diag: &mut Vec<Diagnostic>) {
    let Some(args) = &cls.arguments else {
        return;
    };

    for kw in &args.keywords {
        let Some(kw_name) = &kw.arg else {
            continue;
        };
        if kw_name.as_str() != "metaclass" {
            continue;
        }
        if matches!(&kw.value, Expr::Subscript(_)) {
            let span = Span::from(cls.range());
            diag.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Class `{}` uses a parameterized generic type as its metaclass",
                    cls.name
                ),
                span,
                path,
                Some(
                    "Generic metaclasses are not supported by the Python type system".to_owned(),
                ),
                Some("PEP 484: generic metaclass instances are not supported".to_owned()),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Type inference helpers
// ---------------------------------------------------------------------------

/// Infer the type text of an argument expression, using the variable type map.
fn infer_arg_type<'a>(arg: &'a Expr, var_types: &'a HashMap<String, String>) -> Option<String> {
    match arg {
        Expr::Name(n) => var_types.get(n.id.as_str()).cloned(),
        _ => infer_expr_literal_type(arg).map(str::to_owned),
    }
}

/// Build a span for a call expression.
pub(super) fn call_span(call: &ast::ExprCall) -> Span {
    Span::from(call.range())
}
