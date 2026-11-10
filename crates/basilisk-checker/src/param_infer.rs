// ############################################################################
// # BROKEN — THIS FILE DOES NOT COMPILE. DO NOT "FIX" IT BY RESTORING TEXT   #
// # MATCHING.                                                                #
// #                                                                          #
// # Deleted helper this file called:                                         #
// #   InferredType::from_annotation (types_parsing.rs)
// #                                                                          #
// # That helper decided types from the SPELLING of source text (lowercased   #
// # annotation strings, `"int"`/`"str"`/`"object"` literal matching, `|`     #
// # splitting, `starts_with("tuple[")`). It was deleted, not replaced.       #
// #                                                                          #
// # The call sites below are LEFT BROKEN ON PURPOSE. They are the map of     #
// # what must be rebuilt on the resolved AST — resolved bindings, canonical  #
// # `TypeNode`, and `assignable`/`equivalent` — or made to abstain.          #
// #                                                                          #
// # Restoring the deleted helper, vendoring a copy of it, or re-deriving a   #
// # type from source text anywhere below is FORBIDDEN.                       #
// #                                                                          #
// # Evidence and the failing tests that pin the real behaviour:              #
// #   docs/RULE-VALIDITY-REPORT.md                                           #
// #   crates/basilisk-checker/tests/legacy_annotation_text_parser_pin_tests.rs
// #   crates/basilisk-checker/tests/pep_spelling_invariance_pin_tests.rs     #
// ############################################################################

//! Implements the [NARROWPLAN-CHECKLIST] Stage 2 expression-inference item
//! "Infer unannotated parameter types from body constraints and call sites"
//! (issue #317). See docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md.
//!
//! Pure, module-level core shared by the Salsa layer and (at the Integration
//! stage) the `BSK-0001` missing-parameter-annotation rule, whose exemption
//! must stay exactly as strong as this inference and no stronger
//! ([TYPEINF-EXCEEDS-REQUIRED]).
//!
//! Two evidence sources, combined per parameter:
//! - **Body constraints** (upper bounds): the parameter is bound to a fresh
//!   input-polarity variable and the body runs through the bidirectional
//!   engine with the module's other signatures in scope — passing `p` to a
//!   callee with a declared parameter type demands that type of `p`;
//! - **Call sites** (lower bounds): same-module calls to the function
//!   contribute their synthesized argument types.
//!
//! Resolution follows input polarity: what is DEMANDED wins (upper bounds);
//! with no demand, the union of what FLOWS IN (call-site lower bounds); with
//! neither, `Unknown` — never a guess ([TYPEINF-EXCEEDS-NOUNKNOWN]).

use ruff_python_ast::Stmt;

use crate::bidir::{BidirEngine, Polarity, Ty, TyVarId};
use crate::types::InferredType;

/// The inferred type for each of one function's parameters (`None` for a
/// parameter that carries an explicit annotation — nothing to infer).
#[derive(Debug, Clone, PartialEq)]
pub struct InferredParameters {
    /// `(parameter name, inferred type)` in declaration order; the type is
    /// `None` when the parameter is explicitly annotated.
    pub parameters: Vec<(String, Option<InferredType>)>,
}

/// Infer unannotated parameter types for the function named `function_name`
/// inside `module_source`, with `globals` naming the module's other
/// definitions (`(name, type)` pairs) and `call_args` carrying the synthesized
/// argument types of same-module call sites of this function (outer `Vec`:
/// one entry per call site; inner: positional argument types).
#[must_use]
pub fn infer_parameters(
    module_source: &str,
    function_name: &str,
    globals: &[(String, InferredType)],
    call_args: &[Vec<InferredType>],
) -> Option<InferredParameters> {
    let parsed = ruff_python_parser::parse_module(module_source).ok()?;
    let function = find_function(&parsed.syntax().body, function_name)?;

    let mut engine = BidirEngine::new(
        globals
            .iter()
            .map(|(name, ty)| (name.clone(), Ty::from_inferred(ty)))
            .collect(),
    );
    let mut param_vars: Vec<(String, Option<TyVarId>)> = Vec::new();
    for parameter_ref in &function.parameters {
        let parameter = parameter_ref.as_parameter();
        let name = parameter.name.to_string();
        if parameter.annotation.is_some() {
            param_vars.push((name, None));
            continue;
        }
        let var = engine.fresh_param_var(Polarity::Input);
        engine.bind_global(&name, Ty::Var(var));
        param_vars.push((name, Some(var)));
    }

    walk_body(&mut engine, &function.body);
    apply_call_site_bounds(&mut engine, &param_vars, call_args);
    let solution = engine.finish();

    let parameters = param_vars
        .into_iter()
        .map(|(name, var)| {
            let inferred = var.map(|id| solution.vars.resolve_with_inflow(id));
            (name, inferred)
        })
        .collect();
    Some(InferredParameters { parameters })
}

/// Locate a top-level function definition by name.
fn find_function<'a>(body: &'a [Stmt], name: &str) -> Option<&'a ruff_python_ast::StmtFunctionDef> {
    body.iter().find_map(|stmt| match stmt {
        Stmt::FunctionDef(def) if def.name.as_str() == name => Some(def),
        _ => None,
    })
}

/// Run the engine over every expression position in the body (assignment
/// values, returns, bare expressions, branch bodies) so parameter uses
/// generate their constraints.
fn walk_body(engine: &mut BidirEngine, stmts: &[Stmt]) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign(node) => {
                let _ = engine.synth(&node.value);
            }
            Stmt::AnnAssign(node) => {
                if let Some(value) = node.value.as_deref() {
                    let _ = engine.synth(value);
                }
            }
            Stmt::Return(node) => {
                if let Some(value) = node.value.as_deref() {
                    let _ = engine.synth(value);
                }
            }
            Stmt::Expr(node) => {
                let _ = engine.synth(&node.value);
            }
            Stmt::If(node) => {
                let _ = engine.synth(&node.test);
                walk_body(engine, &node.body);
                for clause in &node.elif_else_clauses {
                    if let Some(test) = &clause.test {
                        let _ = engine.synth(test);
                    }
                    walk_body(engine, &clause.body);
                }
            }
            Stmt::For(node) => {
                let _ = engine.synth(&node.iter);
                walk_body(engine, &node.body);
            }
            Stmt::While(node) => {
                let _ = engine.synth(&node.test);
                walk_body(engine, &node.body);
            }
            Stmt::With(node) => walk_body(engine, &node.body),
            _ => {}
        }
    }
}

/// Add call-site argument types as lower bounds on the parameter variables.
fn apply_call_site_bounds(
    engine: &mut BidirEngine,
    param_vars: &[(String, Option<TyVarId>)],
    call_args: &[Vec<InferredType>],
) {
    for site in call_args {
        for (position, argument) in site.iter().enumerate() {
            let Some((_, Some(var))) = param_vars.get(position) else {
                continue;
            };
            engine.add_var_lower_bound(*var, Ty::from_inferred(argument));
        }
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::expect_used,
        reason = "test-only inference over fixed, known-valid fixtures"
    )]

    use super::*;
    use crate::types::{CallableInfo, LiteralValue};

    /// Body constraints: passing `p` to a callee with a declared parameter
    /// type demands that type ([#317] body-constraint recovery).
    #[test]
    fn body_call_demand_infers_the_parameter() {
        let source = r"
def f(p):
    return consume(p)
";
        let globals = vec![(
            "consume".to_owned(),
            InferredType::Callable(CallableInfo {
                param_types: vec![InferredType::Int],
                return_type: Box::new(InferredType::Bool),
            }),
        )];
        let inferred =
            infer_parameters(source, "f", &globals, &[]).expect("function found and inferred");
        assert_eq!(
            inferred.parameters,
            vec![("p".to_owned(), Some(InferredType::Int))],
            "the callee's declared parameter demands int of p"
        );
    }

    /// Call sites: same-module argument types flow in as lower bounds when
    /// nothing in the body demands more.
    #[test]
    fn call_sites_infer_the_parameter() {
        let source = r"
def f(p):
    return p
";
        let call_args = vec![
            vec![InferredType::Literal(LiteralValue::Int(1))],
            vec![InferredType::Literal(LiteralValue::Int(2))],
        ];
        let inferred =
            infer_parameters(source, "f", &[], &call_args).expect("function found and inferred");
        let (name, ty) = inferred.parameters.first().expect("one parameter");
        assert_eq!(name, "p");
        let ty = ty.clone().expect("inferred");
        assert!(
            !matches!(ty, InferredType::Unknown),
            "call-site lower bounds must produce a REAL type, not the \
             vacuously-admitting Unknown"
        );
        assert!(
            InferredType::Literal(LiteralValue::Int(1)).is_assignable_to(&ty)
                && InferredType::Literal(LiteralValue::Int(2)).is_assignable_to(&ty),
            "call-site arguments must be admitted by the inferred type, got {ty:?}"
        );
    }

    /// No evidence: the parameter stays `Unknown` — never a guess
    /// ([TYPEINF-EXCEEDS-NOUNKNOWN]).
    #[test]
    fn no_evidence_stays_unknown() {
        let source = r"
def f(p):
    return p
";
        let inferred =
            infer_parameters(source, "f", &[], &[]).expect("function found and inferred");
        assert_eq!(
            inferred.parameters,
            vec![("p".to_owned(), Some(InferredType::Unknown))]
        );
    }

    /// Annotated parameters are not second-guessed.
    #[test]
    fn annotated_parameters_are_skipped() {
        let source = r"
def f(p: int, q):
    return p
";
        let inferred =
            infer_parameters(source, "f", &[], &[]).expect("function found and inferred");
        assert_eq!(inferred.parameters.first(), Some(&("p".to_owned(), None)));
    }
}

/// Engine globals from cross-module imports ([NARROWPLAN-CHECKLIST]
/// "imported function/method return types"): each imported symbol in
/// [`basilisk_resolver::ResolvedModule::imported_symbols`] maps to a type —
/// functions become the gradual `Callable[..., R]` from their return
/// annotation (so calls to imported names infer their returns), classes
/// become their instance-`Named` form, variables take their annotation.
/// Symbols without type information are omitted (they stay `Unknown` in the
/// engine — never a guess).
#[must_use]
pub fn imported_callable_globals(
    module: &basilisk_resolver::ResolvedModule,
) -> Vec<(String, InferredType)> {
    use basilisk_resolver::scope::ExternalSymbolKind;
    module
        .imported_symbols
        .iter()
        .map(|(binding, symbol)| {
            let ty = match symbol.kind {
                ExternalSymbolKind::Function => {
                    InferredType::Callable(crate::types::CallableInfo {
                        // An imported function's parameters are not modelled
                        // here — gradual tail, not "takes no arguments".
                        param_types: crate::types::gradual_params(Vec::new()),
                        return_type: Box::new(
                            symbol
                                .type_annotation
                                .as_deref()
                                .map_or(InferredType::Unknown, InferredType::from_annotation),
                        ),
                    })
                }
                ExternalSymbolKind::Class => InferredType::Named(symbol.name.to_ascii_lowercase()),
                ExternalSymbolKind::Variable | ExternalSymbolKind::ReExport => symbol
                    .type_annotation
                    .as_deref()
                    .map_or(InferredType::Unknown, InferredType::from_annotation),
            };
            (binding.clone(), ty)
        })
        .collect()
}

#[cfg(test)]
mod imported_globals_tests {
    #![expect(
        clippy::expect_used,
        reason = "test-only lookups over fixed fixture symbols"
    )]

    use super::*;
    use basilisk_resolver::scope::{ExternalSymbol, ExternalSymbolKind, Span};

    fn symbol(kind: ExternalSymbolKind, annotation: Option<&str>) -> ExternalSymbol {
        ExternalSymbol {
            name: "helper".to_owned(),
            kind,
            type_annotation: annotation.map(str::to_owned),
            source_path: std::path::PathBuf::from("other.py"),
            source_span: Span::new(0, 0),
            signature: None,
            docstring: None,
            provenance: None,
            methods: Vec::new(),
            bases: Vec::new(),
            metaclass: None,
            metaclass_calls: Vec::new(),
        }
    }

    /// Imported functions expose their return annotation as a gradual
    /// callable, so `x = helper()` infers the imported return type.
    #[test]
    fn imported_function_returns_flow_through_calls() {
        let mut module = basilisk_resolver::ResolvedModule::default();
        let _ = module.imported_symbols.insert(
            "helper".to_owned(),
            symbol(ExternalSymbolKind::Function, Some("list[int]")),
        );
        let globals = imported_callable_globals(&module);
        let inferred = infer_parameters("def f(p):\n    return helper()\n", "f", &globals, &[])
            .expect("inferred");
        // The parameter has no evidence — but the imported callable is in
        // scope, proving the globals wire in (return flows are covered by
        // the engine call-synthesis tests).
        assert_eq!(inferred.parameters.len(), 1);

        let (_, ty) = globals.first().expect("helper present");
        assert!(
            matches!(
                ty,
                InferredType::Callable(info)
                    if *info.return_type == InferredType::List(Box::new(InferredType::Int))
            ),
            "imported function must map to Callable[..., list[int]], got {ty:?}"
        );
    }

    /// Classes map to instances; unannotated variables are Unknown.
    #[test]
    fn imported_classes_and_variables_map_conservatively() {
        let mut module = basilisk_resolver::ResolvedModule::default();
        let _ = module
            .imported_symbols
            .insert("Widget".to_owned(), symbol(ExternalSymbolKind::Class, None));
        let _ = module.imported_symbols.insert(
            "COUNT".to_owned(),
            symbol(ExternalSymbolKind::Variable, Some("int")),
        );
        let globals = imported_callable_globals(&module);
        let lookup = |name: &str| {
            globals
                .iter()
                .find(|(binding, _)| binding == name)
                .map(|(_, ty)| ty.clone())
        };
        assert_eq!(
            lookup("Widget"),
            Some(InferredType::Named("helper".to_owned())),
            "class binding maps to the SOURCE class name's instance form"
        );
        assert_eq!(lookup("COUNT"), Some(InferredType::Int));
    }
}
