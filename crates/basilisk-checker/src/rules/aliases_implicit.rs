//! Implements [`aliases_implicit`] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
//! `aliases_implicit`: Invalid right-hand side for a `TypeAlias` annotation.
//!
//! PEP 613 requires that the RHS of an explicit `TypeAlias` annotation must be
//! a valid type expression, and the typing spec's aliases chapter gives plain
//! assignments the same split: an assignment whose RHS is a type expression is
//! an implicit alias; any other assignment makes an ordinary variable, and a
//! variable is not valid in a type expression. All verdicts are structural,
//! over the parsed `ruff` AST ([LINESCANPLAN-AST-MIGRATION], issue #408):
//!
//! - Literals, collection displays, comprehensions, lambdas, conditionals,
//!   boolean operators, f-strings, and calls that are not recognised type
//!   constructors (`TypeVar`, `ParamSpec`, `TypeVarTuple`, `NewType`,
//!   `TypedDict`, `NamedTuple`, `TypeAliasType`, `type(...)`) all make
//!   runtime values, not types.
//! - Alias parameterization is checked against the alias's own type
//!   parameters: arity, `ParamSpec` argument shape (PEP 612), and `TypeVar`
//!   bounds through the module's subtyping context ([NARROWPLAN-SUBTYPING]).
//!
//! ```python
//! from typing import TypeAlias
//! BadTypeAlias2: TypeAlias = [int, str]   # E — list literal
//! BadTypeAlias10: TypeAlias = True         # E — bool literal
//! ```

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::visitor::{walk_expr, Visitor};
use ruff_python_ast::{Expr, Operator};

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic, error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::{
    annotation_is_type_alias, is_type_expression, runtime_value_names, type_constructor_names,
    ExprIndex, StringPolicy, TypeExprJudge,
};
use crate::span_util::slice_span;
use crate::subtyping::SubtypingContext;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "aliases_implicit",
    docs_url: "https://www.basilisk-python.dev/errors/aliases_implicit",
};

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic(
        CODE.clone(),
        message,
        span,
        path,
        Some("The RHS of a `TypeAlias` annotation must be a valid type expression"),
        Some("PEP 613: `x: TypeAlias = T` requires T to be a type, not a literal or expression"),
    )
}

/// Emits `aliases_implicit` when a `TypeAlias`-annotated variable has an invalid RHS type expression.
pub(crate) struct TypeAliasInvalidRhs;

impl Rule for TypeAliasInvalidRhs {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // The module's own AST and binding tables; a module that does not
        // parse is reported by the parser itself.
        let Some(parsed) = module.lazy_ast.get_or_parse(&module.source, &module.path) else {
            return;
        };
        let Some(resolver) = AnnotationResolver::for_module(module) else {
            return;
        };
        let index = ExprIndex::build(&parsed.ast);
        let runtime_vars = runtime_value_names(module, &resolver, &index);

        check_explicit_alias_values(module, &resolver, &index, &runtime_vars, diagnostics);

        let alias_map = build_alias_info_map(module, &resolver, &index, &runtime_vars);
        check_alias_parameterization(module, &index, &alias_map, diagnostics);
        check_union_alias_instantiation(module, &alias_map, diagnostics);
        check_runtime_name_annotations(module, &index, &runtime_vars, diagnostics);
    }
}

/// Validate the RHS of every explicit `TypeAlias`-annotated variable.
///
/// The annotation is resolved through the shared cascade, so `TypeAlias`,
/// `typing.TypeAlias`, and `from typing import TypeAlias as TA` all count.
/// PEP 613 alias values are evaluated eagerly, so a string is a forward
/// reference only at the top level (`X: TypeAlias = "int | str"` is valid;
/// `X: TypeAlias = "int" | str` is a runtime error).
fn check_explicit_alias_values(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
    runtime_vars: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let judge = TypeExprJudge {
        non_type: &|name| runtime_vars.contains(name),
        strings: StringPolicy::EagerForwardRef,
    };
    for var in &module.module_vars {
        if !annotation_is_type_alias(resolver, var.annotation_span) {
            continue;
        }
        let Some(rhs) = var.rhs_span.and_then(|span| index.expr(span)) else {
            continue;
        };
        if !is_type_expression(rhs, &judge) {
            diagnostics.push(make_diagnostic(
                format!(
                    "Invalid type expression as right-hand side of `TypeAlias` for `{}`",
                    var.name
                ),
                var.name_span,
                &module.path,
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Type alias parameterization checking
// ---------------------------------------------------------------------------

/// One declared type parameter of an alias, in order of first appearance in
/// the alias's RHS.
struct TypeParam {
    name: String,
    bound: Option<String>,
    is_paramspec: bool,
    is_typevartuple: bool,
}

/// Information about a type alias definition.
struct AliasInfo {
    /// The alias's type parameters in RHS order.
    params: Vec<TypeParam>,
    /// Whether the alias RHS is a top-level `|` union.
    is_union: bool,
}

/// Every `Name` referenced anywhere in an expression, in source order —
/// including subscript arguments, where an alias's type parameters live.
fn ordered_name_refs(expr: &Expr) -> Vec<&str> {
    struct NameCollector<'ast> {
        names: Vec<&'ast str>,
    }
    impl<'ast> Visitor<'ast> for NameCollector<'ast> {
        fn visit_expr(&mut self, expr: &'ast Expr) {
            if let Expr::Name(name) = expr {
                self.names.push(name.id.as_str());
            }
            walk_expr(self, expr);
        }
    }
    let mut collector = NameCollector { names: Vec::new() };
    collector.visit_expr(expr);
    collector.names
}

/// The alias's type parameters: every distinct declared `TypeVar` /
/// `ParamSpec` / `TypeVarTuple` referenced in the RHS, in first-appearance
/// order, each carrying its declared bound.
fn alias_type_params(module: &ResolvedModule, rhs: &Expr) -> Vec<TypeParam> {
    let declared: HashMap<&str, (Option<&str>, bool, bool)> = module
        .typevar_calls
        .iter()
        .map(|tv| {
            (
                tv.name.as_str(),
                (
                    tv.bound_type_name.as_deref(),
                    tv.is_paramspec,
                    tv.is_typevartuple,
                ),
            )
        })
        .collect();
    let mut seen = HashSet::new();
    let mut params = Vec::new();
    for name in ordered_name_refs(rhs) {
        if let Some((bound, is_paramspec, is_typevartuple)) = declared.get(name) {
            if seen.insert(name) {
                params.push(TypeParam {
                    name: name.to_owned(),
                    bound: bound.map(str::to_owned),
                    is_paramspec: *is_paramspec,
                    is_typevartuple: *is_typevartuple,
                });
            }
        }
    }
    params
}

/// Build a map from alias name to its [`AliasInfo`], covering explicit
/// `TypeAlias`-annotated variables and implicit aliases — unannotated
/// assignments whose RHS is a type expression. Alias-hood comes from the
/// binding's structure, never from the spelling of its name.
fn build_alias_info_map(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
    runtime_vars: &HashSet<String>,
) -> HashMap<String, AliasInfo> {
    let constructors = type_constructor_names(module);
    let implicit_judge = TypeExprJudge {
        non_type: &|name| runtime_vars.contains(name),
        strings: StringPolicy::RejectValue,
    };
    let mut map = HashMap::new();
    for var in &module.module_vars {
        let Some(rhs) = var.rhs_span.and_then(|span| index.expr(span)) else {
            continue;
        };
        let explicit = annotation_is_type_alias(resolver, var.annotation_span);
        if !explicit {
            let is_implicit = !var.has_annotation
                && !runtime_vars.contains(&var.name)
                && !constructors.contains(var.name.as_str())
                && is_type_expression(rhs, &implicit_judge);
            if !is_implicit {
                continue;
            }
        }
        let _ = map.insert(
            var.name.clone(),
            AliasInfo {
                params: alias_type_params(module, rhs),
                is_union: matches!(rhs, Expr::BinOp(binop) if binop.op == Operator::BitOr),
            },
        );
    }
    map
}

/// Check alias parameterization across function parameter and module
/// variable annotations.
fn check_alias_parameterization(
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    alias_map: &HashMap<String, AliasInfo>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // A TypeVar used as a type argument defers its bound to the use site.
    let typevar_names: HashSet<&str> = module
        .typevar_calls
        .iter()
        .map(|tv| tv.name.as_str())
        .collect();
    // Bound verdicts route through the module-seeded context
    // ([NARROWPLAN-SUBTYPING]).
    let subtyping = crate::subtyping::module_context(module);
    let known_class_names: HashSet<&str> = module.classes.iter().map(|c| c.name.as_str()).collect();
    let checker = ParameterizationChecker {
        module,
        index,
        alias_map,
        typevar_names,
        subtyping,
        known_class_names,
    };
    for func in &module.functions {
        for param in &func.parameters {
            if let Some(span) = param.annotation_span {
                checker.check_annotation(span, diagnostics);
            }
        }
    }
    for var in &module.module_vars {
        if let Some(span) = var.annotation_span {
            checker.check_annotation(span, diagnostics);
        }
    }
}

struct ParameterizationChecker<'m, 'ast> {
    module: &'m ResolvedModule,
    index: &'m ExprIndex<'ast>,
    alias_map: &'m HashMap<String, AliasInfo>,
    typevar_names: HashSet<&'m str>,
    subtyping: SubtypingContext,
    known_class_names: HashSet<&'m str>,
}

impl ParameterizationChecker<'_, '_> {
    /// Check one annotation node: it must be a subscript of a known alias.
    fn check_annotation(&self, span: Span, diagnostics: &mut Vec<Diagnostic>) {
        let Some(Expr::Subscript(subscript)) = self.index.expr(span) else {
            return;
        };
        let Expr::Name(base) = &*subscript.value else {
            return;
        };
        let Some(info) = self.alias_map.get(base.id.as_str()) else {
            return;
        };
        let args: Vec<&Expr> = match &*subscript.slice {
            Expr::Tuple(tuple) => tuple.elts.iter().collect(),
            single => vec![single],
        };
        let base_name = base.id.as_str();
        if info.params.is_empty() {
            self.report_not_generic(base_name, span, diagnostics);
            return;
        }
        // A TypeVarTuple parameter absorbs any number of type arguments
        // (PEP 646), so the alias has no upper arity to enforce.
        let variadic = info.params.iter().any(|param| param.is_typevartuple);
        if !variadic && args.len() > info.params.len() {
            self.report_too_many(base_name, info, args.len(), span, diagnostics);
            return;
        }
        self.check_paramspec_args(base_name, info, &args, span, diagnostics);
        self.check_bounds(base_name, info, &args, span, diagnostics);
    }

    fn report_not_generic(&self, base: &str, span: Span, diagnostics: &mut Vec<Diagnostic>) {
        let annotation_text = slice_span(&self.module.source, span).unwrap_or(base);
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!("Type alias `{base}` is not generic and cannot be parameterized"),
            span,
            &self.module.path,
            Some(format!(
                "Remove the type arguments from `{annotation_text}`"
            )),
            Some(format!(
                "`{base}` does not use any TypeVar parameters in its definition"
            )),
        ));
    }

    fn report_too_many(
        &self,
        base: &str,
        info: &AliasInfo,
        arg_count: usize,
        span: Span,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Too many type arguments for `{base}`: expected {}, got {arg_count}",
                info.params.len()
            ),
            span,
            &self.module.path,
            Some(format!(
                "`{base}` accepts {} type parameter(s)",
                info.params.len()
            )),
            None,
        ));
    }

    /// PEP 612: the argument at a `ParamSpec` parameter's position must be a
    /// parameter-list expression — `[...]`, `...`, or another `ParamSpec`.
    /// When the alias's ONLY parameter is the `ParamSpec`, a single argument
    /// is implicitly wrapped in a list and any shape is valid.
    fn check_paramspec_args(
        &self,
        base: &str,
        info: &AliasInfo,
        args: &[&Expr],
        span: Span,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if info.params.len() < 2 {
            return;
        }
        for (position, param) in info.params.iter().enumerate() {
            if !param.is_paramspec {
                continue;
            }
            let Some(arg) = args.get(position) else {
                continue;
            };
            if !self.is_paramspec_argument(arg) {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!("Invalid type argument for `ParamSpec` parameter in `{base}`"),
                    span,
                    &self.module.path,
                    Some(
                        "ParamSpec arguments must be a list of parameter types \
                         (e.g. `[int, str]`) or `...`"
                            .to_owned(),
                    ),
                    None,
                ));
            }
        }
    }

    fn is_paramspec_argument(&self, arg: &Expr) -> bool {
        match arg {
            Expr::List(_) | Expr::EllipsisLiteral(_) => true,
            Expr::Name(name) => self
                .module
                .typevar_calls
                .iter()
                .any(|tv| tv.is_paramspec && tv.name == name.id.as_str()),
            _ => false,
        }
    }

    /// Check `TypeVar` bounds. A violation is reported only when the
    /// subtyping context has positive knowledge of BOTH sides — builtin
    /// tower names or module-local classes — because `is_subtype` answers
    /// `false` for names it cannot see (imported bases, typeshed classes),
    /// and inventing errors from ignorance breaks the gradual guarantee.
    fn check_bounds(
        &self,
        base: &str,
        info: &AliasInfo,
        args: &[&Expr],
        span: Span,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (position, param) in info.params.iter().enumerate() {
            let Some(bound) = param.bound.as_deref() else {
                continue;
            };
            let Some(arg_name) = args.get(position).and_then(|arg| simple_type_name(arg)) else {
                continue;
            };
            if self.typevar_names.contains(arg_name) {
                continue; // Defers its own bound to the use site.
            }
            if !self.name_is_known(arg_name) || !self.name_is_known(bound) {
                continue;
            }
            if !self.subtyping.is_subtype(arg_name, bound) {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Type argument `{arg_name}` does not satisfy \
                         bound `{bound}` of TypeVar `{}` in `{base}`",
                        param.name
                    ),
                    span,
                    &self.module.path,
                    Some(format!(
                        "TypeVar `{}` requires a type that is a \
                         subtype of `{bound}`",
                        param.name
                    )),
                    None,
                ));
            }
        }
    }

    /// Positive knowledge: the builtin types the tower models, or a class
    /// this module defines (registered in the subtyping context).
    fn name_is_known(&self, name: &str) -> bool {
        matches!(
            name,
            "int" | "float" | "complex" | "bool" | "str" | "bytes" | "object" | "None"
        ) || self.known_class_names.contains(name)
    }
}

/// The simple name a type argument denotes, when it has one.
fn simple_type_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(name) => Some(name.id.as_str()),
        Expr::NoneLiteral(_) => Some("None"),
        _ => None,
    }
}

/// Check for calls to union type aliases (e.g. `ListOrSetAlias()`).
///
/// Union aliases like `X = list | set` cannot be instantiated because the
/// runtime doesn't know which branch to construct.
fn check_union_alias_instantiation(
    module: &ResolvedModule,
    alias_map: &HashMap<String, AliasInfo>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for call in &module.calls {
        let Some(info) = alias_map.get(&call.callee) else {
            continue;
        };
        if info.is_union {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!("Cannot instantiate union type alias `{}`", call.callee),
                call.span,
                &module.path,
                Some(format!(
                    "`{}` is a union of types; instantiate one of the union members directly",
                    call.callee
                )),
                None,
            ));
        }
    }
}

/// Check annotations that reference runtime (non-type) names.
///
/// When a module-level name holds a runtime value (`BadTypeAlias1 = eval(...)`)
/// and is used as a type annotation — on a function parameter or a module
/// variable — this is an error because the name does not resolve to a type.
fn check_runtime_name_annotations(
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    runtime_vars: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let param_spans = module
        .functions
        .iter()
        .flat_map(|func| &func.parameters)
        .filter_map(|param| param.annotation_span);
    let module_var_spans = module
        .module_vars
        .iter()
        .filter_map(|var| var.annotation_span);

    for span in param_spans.chain(module_var_spans) {
        let Some(Expr::Name(name)) = index.expr(span) else {
            continue;
        };
        let name = name.id.as_str();
        if runtime_vars.contains(name) {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Variable `{name}` is not a valid type and \
                     cannot be used as an annotation"
                ),
                span,
                &module.path,
                Some(format!(
                    "`{name}` is assigned a runtime value, not a type expression"
                )),
                Some(
                    "Only type expressions (classes, type aliases, typing constructs) \
                     are valid annotations"
                        .to_owned(),
                ),
            ));
        }
    }
}
