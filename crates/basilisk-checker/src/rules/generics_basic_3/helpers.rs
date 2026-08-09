//! Implements [`generics_basic_3`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! All internal types, parsing, and checking logic for `generics_basic_3`.
//!
//! Annotations are resolved through the module's binding table into
//! [`TypeNode`]s and related with [`assignable`] ([ASTREBUILD-LAW]) — never
//! compared as source text. Source text appears only inside diagnostic
//! messages.

use std::collections::HashMap;

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{assignable, BindingTable, Span, TypeNode, TypingForm};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::{expr_name, infer_expr_literal_type};
use crate::span_util::slice_span;

pub(super) const CODE: ErrorCode = ErrorCode {
    code: "generics_basic_3",
    docs_url: "https://www.basilisk-python.dev/errors/generics_basic_3",
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

// ##########################################################################
// # DELETED — `is_subtype_of`. DO NOT RESTORE IT. DO NOT REINTRODUCE IT    #
// # HERE OR ANYWHERE ELSE UNDER ANOTHER NAME.                              #
// #                                                                        #
// # This was a VENDORED COPY of the string-keyed subtyping layer deleted   #
// # from `crate::subtyping`. Its own doc comment admitted the plan to      #
// # merge it into `SubtypingContext` — the module that no longer exists    #
// # precisely because it was structurally incorrect. A second copy of a    #
// # deleted defect is the defect coming back.                              #
// #                                                                        #
// # Every input was a SPELLING:                                            #
// #   fn is_subtype_of(subtype: &str, supertype: &str,                     #
// #                    class_bases: &HashMap<String, Vec<String>>)         #
// #   matches!((subtype, supertype), ("bool", "int"))                      #
// #   bases.iter().any(|b| b == supertype)                                 #
// #                                                                        #
// # So `bool <: int` held only for those two literal spellings — never for #
// # `from builtins import int as Whole` — and the inheritance walk         #
// # compared base-class NAME TEXT, so a base reached under an alias was    #
// # invisible and two unrelated classes sharing a name were conflated.     #
// #                                                                        #
// # The replacement resolves each base through the binding table to a      #
// # canonical symbol and walks RESOLVED identities. The call site above is #
// # LEFT AS A PANICKING CALL ON PURPOSE — it is the map of what must be    #
// # rebuilt.                                                               #
// #                                                                        #
// # Pinned by: tests/nominal_spelling_surgery_pin_tests.rs                 #
// ##########################################################################

/// DELETED — panics. The signature survives only so its call site in
/// `group_of` stays visible as the rebuild map; see the banner above.
pub(super) fn is_subtype_of(
    _subtype: &str,
    _supertype: &str,
    _class_bases: &HashMap<String, Vec<String>>,
) -> bool {
    panic!(
        "basilisk-checker: `generics_basic_3::is_subtype_of` was DELETED because it was \
         a VENDORED COPY of the string-keyed subtyping layer — a numeric tower written \
         as `matches!((subtype, supertype), (\"bool\", \"int\"))` and an inheritance \
         walk comparing base-class NAME TEXT. It panics because the real \
         implementation — walking resolved base symbols from the binding table — DOES \
         NOT EXIST YET. Do not restore it, do not re-vendor it under another name, and \
         do not answer `true`/`false` in its place."
    )
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

/// A `Mapping`-typed variable's key parameter: the resolved node for verdicts
/// and the annotation's source text for messages only.
pub(super) struct MappingInfo {
    key: TypeNode,
    key_text: String,
}

/// Module-level knowledge needed to check calls.
pub(super) struct ModuleContext<'a> {
    /// The module's binding table, for lowering annotations in nested scopes.
    bindings: &'a BindingTable,
    /// The module source, for diagnostic message rendering only.
    source: &'a str,
    /// All constrained `TypeVars` defined at module level.
    pub(super) constrained_tvars: HashMap<String, ConstrainedTypeVar>,
    /// Functions that have at least one constrained-TypeVar parameter.
    pub(super) constrained_funcs: Vec<ConstrainedFunc>,
    /// Variables whose annotation is a simple name reference: name -> the
    /// referenced type name. Structured annotations are outside the
    /// constrained-`TypeVar` matcher's model and abstain
    /// ([ASTREBUILD-PHASE-RESOLVER]).
    pub(super) var_types: HashMap<String, String>,
    /// Variables with a resolved mapping type: name -> key parameter.
    pub(super) mapping_vars: HashMap<String, MappingInfo>,
    /// Class inheritance: maps class name -> list of base class names.
    /// Used for resolving subclass-to-constraint matching in `TypeVar` checks.
    pub(super) class_bases: HashMap<String, Vec<String>>,
}

impl<'a> ModuleContext<'a> {
    /// Build a `ModuleContext` from the top-level AST statements.
    pub(super) fn from_ast(stmts: &[Stmt], bindings: &'a BindingTable, source: &'a str) -> Self {
        let constrained_tvars: HashMap<String, ConstrainedTypeVar> = HashMap::new();
        let mut constrained_funcs: Vec<ConstrainedFunc> = Vec::new();
        let mut var_types: HashMap<String, String> = HashMap::new();
        let mut mapping_vars: HashMap<String, MappingInfo> = HashMap::new();

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
                        if let Some(type_name) = expr_name(&ann.annotation) {
                            let _ = var_types.insert(var_name.to_owned(), type_name.to_owned());
                        }
                        if let Some(info) = mapping_key_info(bindings, source, &ann.annotation) {
                            let _ = mapping_vars.insert(var_name.to_owned(), info);
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
                    let bases: Vec<String> = args.args.iter().filter_map(base_class_name).collect();
                    if !bases.is_empty() {
                        let _ = class_bases.insert(cls.name.to_string(), bases);
                    }
                }
            }
        }

        Self {
            bindings,
            source,
            constrained_tvars,
            constrained_funcs,
            var_types,
            mapping_vars,
            class_bases,
        }
    }
}

/// The nominal name of a base class expression, from the AST: a bare `Name`,
/// or the subscripted name of `Base[...]`. Structured bases without a simple
/// name carry no nominal identity for the walk and are skipped.
fn base_class_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Subscript(sub) => match sub.value.as_ref() {
            Expr::Name(name) => Some(name.id.to_string()),
            _ => None,
        },
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Scope context
// ---------------------------------------------------------------------------

/// A lookup scope: function-local variable/mapping types overlaid on the
/// module context. Locals shadow module entries; nothing is copied, so
/// building one per function is O(parameters), not O(module).
pub(super) struct ScopeContext<'a> {
    module: &'a ModuleContext<'a>,
    /// Simple-name types of the current function's parameters.
    local_types: HashMap<String, String>,
    /// Mapping-typed locals: name -> key parameter.
    local_mappings: HashMap<String, MappingInfo>,
}

impl<'a> ScopeContext<'a> {
    /// Module-level scope: no locals. `HashMap::new()` does not allocate.
    pub(super) fn module_scope(module: &'a ModuleContext<'a>) -> Self {
        Self {
            module,
            local_types: HashMap::new(),
            local_mappings: HashMap::new(),
        }
    }

    /// Function scope: the function's annotated parameters shadow module vars.
    pub(super) fn function_scope(
        module: &'a ModuleContext<'a>,
        func: &ast::StmtFunctionDef,
    ) -> Self {
        let mut local_types = HashMap::new();
        let mut local_mappings = HashMap::new();
        for param in func
            .parameters
            .args
            .iter()
            .chain(func.parameters.posonlyargs.iter())
        {
            if let Some(ann) = &param.parameter.annotation {
                if let Some(info) = mapping_key_info(module.bindings, module.source, ann) {
                    let _ = local_mappings.insert(param.parameter.name.to_string(), info);
                }
                if let Some(type_name) = expr_name(ann) {
                    let _ =
                        local_types.insert(param.parameter.name.to_string(), type_name.to_owned());
                }
            }
        }
        Self {
            module,
            local_types,
            local_mappings,
        }
    }

    pub(super) fn module(&self) -> &'a ModuleContext<'a> {
        self.module
    }

    fn var_type(&self, name: &str) -> Option<&str> {
        self.local_types
            .get(name)
            .or_else(|| self.module.var_types.get(name))
            .map(String::as_str)
    }

    fn mapping_var(&self, name: &str) -> Option<&MappingInfo> {
        self.local_mappings
            .get(name)
            .or_else(|| self.module.mapping_vars.get(name))
    }
}

// ---------------------------------------------------------------------------
// TypeVar constraint parsing
// ---------------------------------------------------------------------------

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
// Mapping annotation resolution
// ---------------------------------------------------------------------------

/// Resolve a mapping annotation `M[K, V]` to its key parameter.
///
/// The subscript base must resolve, through the binding table, to a form
/// whose first parameter is the key type: the builtin `dict` (or its
/// `typing.Dict` alias) or the abstract `Mapping`/`MutableMapping` protocols.
/// A user-defined mapping subclass would require MRO-level type-parameter
/// mapping, which this layer does not model — it abstains
/// ([ASTREBUILD-PHASE-RESOLVER]).
fn mapping_key_info(bindings: &BindingTable, source: &str, ann: &Expr) -> Option<MappingInfo> {
    let Expr::Subscript(sub) = ann else {
        return None;
    };
    let is_mapping_form = matches!(
        bindings.form_of_with_builtins(&sub.value),
        Some(
            TypingForm::DictClass
                | TypingForm::DictAlias
                | TypingForm::Mapping
                | TypingForm::MutableMapping
        )
    );
    if !is_mapping_form {
        return None;
    }
    let Expr::Tuple(args) = sub.slice.as_ref() else {
        return None;
    };
    let key_expr = args.elts.first()?;
    if args.elts.len() < 2 {
        return None;
    }
    Some(MappingInfo {
        key: TypeNode::lower(bindings, key_expr),
        key_text: slice_span(source, Span::from(key_expr.range()))
            .unwrap_or("<key type>")
            .trim()
            .to_owned(),
    })
}

// ---------------------------------------------------------------------------
// Call-site checking (constrained TypeVar)
// ---------------------------------------------------------------------------

/// Check a single call expression for constrained-`TypeVar` group mismatches.
pub(super) fn check_call(
    call: &ast::ExprCall,
    scope: &ScopeContext<'_>,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    let ctx = scope.module();
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
        let Some(arg_type_str) = infer_arg_type(arg, scope) else {
            continue;
        };
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
///
/// The verdict relates the subscript key's literal type to the resolved key
/// parameter with [`assignable`]; a relation the layer cannot decide
/// abstains and no diagnostic is emitted.
pub(super) fn check_subscript(
    sub: &ast::ExprSubscript,
    scope: &ScopeContext<'_>,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    let Some(obj_name) = expr_name(&sub.value) else {
        return;
    };
    let Some(mapping) = scope.mapping_var(obj_name) else {
        return;
    };
    let idx_node = TypeNode::of_literal_expr(&sub.slice);
    if assignable(&idx_node, &mapping.key) == Some(false) {
        let key_ty = &mapping.key_text;
        let idx_text = slice_span(scope.module().source, Span::from(sub.slice.range()))
            .unwrap_or("<key>")
            .trim();
        let span = Span::from(sub.range());
        diag.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Invalid subscript key `{idx_text}` for `{obj_name}` \
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
                Some("Generic metaclasses are not supported by the Python type system".to_owned()),
                Some("PEP 484: generic metaclass instances are not supported".to_owned()),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Type inference helpers
// ---------------------------------------------------------------------------

/// Infer the type name of an argument expression, using the scope's type maps.
fn infer_arg_type(arg: &Expr, scope: &ScopeContext<'_>) -> Option<String> {
    match arg {
        Expr::Name(n) => scope.var_type(n.id.as_str()).map(str::to_owned),
        _ => infer_expr_literal_type(arg).map(str::to_owned),
    }
}

/// Build a span for a call expression.
pub(super) fn call_span(call: &ast::ExprCall) -> Span {
    Span::from(call.range())
}
