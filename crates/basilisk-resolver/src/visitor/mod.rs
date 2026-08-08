//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! AST visitor that collects function definitions and module-level information.

mod annotations;
mod assert_narrow;
mod assigns;
mod calls_and_reveal;
mod class_info;
mod class_info_ext;
mod core;
mod dataclass;
mod final_readonly;
mod final_readonly_ext;
mod function_info;
mod generics;
mod historical;
mod module_level;
mod narrowing;
mod pep695_scoping;
mod protocol;
mod type_alias;
mod typeddict;
mod typeddict_ext;
mod typeddict_schema;
mod typevar;
mod unhashable;
pub(crate) mod walks;
pub(crate) mod walrus;
mod yield_exprs;

use basilisk_parser::ParsedModule;

use crate::scope::ResolvedModule;

pub(crate) fn collect(module: &ParsedModule) -> ResolvedModule {
    let mut functions = Vec::new();
    let mut classes = Vec::new();
    let mut module_vars = Vec::new();
    let mut imports = Vec::new();
    let mut match_stmts = Vec::new();

    // Which `typing` symbol a name denotes is decided by resolving it through
    // the module's own imports, never by its spelling — `KW_ONLY as KO`,
    // `dataclasses.KW_ONLY`, and a locally-defined `KW_ONLY` all answer
    // correctly. Implements [RESOLV-CANONICAL-BINDING].
    let bindings = basilisk_canonical::BindingTable::from_module(&module.ast.body);

    let mut sinks = core::CollectSinks {
        functions: &mut functions,
        classes: &mut classes,
        module_vars: &mut module_vars,
        imports: &mut imports,
        match_stmts: &mut match_stmts,
    };
    core::collect_from_body(&bindings, &module.ast.body, &mut sinks, true);

    let calls = calls_and_reveal::collect_calls_from_stmts(&module.ast.body);
    // Which callee is a type-parameter factory is decided by resolving the
    // callee node through the same bindings, so `TypeVar as TV` and
    // `typing.TypeVar` answer identically and a locally-defined `TypeVar` does
    // not answer at all.
    let typevar_calls = typevar::collect_typevar_calls(&bindings, &module.ast.body);
    reclassify_generic_params(&mut classes, &typevar_calls);

    let collected = collect_analysis_results(module, &bindings, &classes);
    build_resolved_module(
        module,
        bindings,
        functions,
        classes,
        module_vars,
        imports,
        match_stmts,
        calls,
        typevar_calls,
        collected,
    )
}

/// Reclassify generic params that are not actual `TypeVar`s.
fn reclassify_generic_params(
    classes: &mut [crate::scope::ClassInfo],
    typevar_calls: &[crate::scope::TypeVarCallInfo],
) {
    let typevar_names: std::collections::HashSet<&str> =
        typevar_calls.iter().map(|tv| tv.name.as_str()).collect();
    for cls in classes {
        let mut non_tv = std::mem::take(&mut cls.generic_non_typevar_args);
        cls.generic_params.retain(|p| {
            if typevar_names.contains(p.name.as_str()) {
                true
            } else {
                non_tv.push(p.span);
                false
            }
        });
        cls.generic_non_typevar_args = non_tv;
    }
}

/// Intermediate container for collected analysis results.
struct AnalysisResults {
    reveal_type_calls: Vec<crate::scope::RevealTypeCallInfo>,
    assert_type_calls: Vec<crate::scope::AssertTypeCallInfo>,
    typeddict_calls: Vec<crate::scope::TypedDictCallInfo>,
    newtype_calls: Vec<crate::scope::NewTypeCallInfo>,
    namedtuple_defs: Vec<crate::scope::NamedTupleDefInfo>,
    multiple_unbounded_tuple_spans: Vec<crate::scope::Span>,
    module_bare_assignments: Vec<crate::scope::ModuleBareAssignment>,
    module_attr_assignments: Vec<crate::scope::ModuleAttrAssignment>,
    final_issues: Vec<crate::scope::FinalViolationInfo>,
    float_param_int_attr_accesses: Vec<crate::scope::FloatParamIntAttrAccess>,
    literal_string_enum_mismatches: Vec<crate::scope::LiteralStringEnumMismatch>,
    readonly_issues: Vec<crate::scope::ReadOnlyViolationInfo>,
    protocol_self_issues: Vec<crate::scope::ProtocolSelfViolation>,
    protocol_instantiation_issues: Vec<crate::scope::ProtocolInstantiationViolation>,
    isinstance_typeddict_spans: Vec<crate::scope::Span>,
    typeddict_key_issues: Vec<crate::scope::TypedDictKeyViolation>,
    generic_subscript_sites: Vec<crate::scope::GenericSubscriptSite>,
    type_alias_defs: Vec<crate::scope::TypeAliasDefInfo>,
    unhashable_hash_calls: Vec<crate::scope::UnhashableHashCallViolation>,
    protocol_rtc_issues: Vec<crate::scope::ProtocolRtcViolation>,
    generator_issues: Vec<crate::scope::GeneratorViolation>,
}

fn collect_analysis_results(
    module: &ParsedModule,
    bindings: &basilisk_canonical::BindingTable,
    classes: &[crate::scope::ClassInfo],
) -> AnalysisResults {
    let stmts = &module.ast.body;
    let source = &module.source;

    let typeddict_class_names: std::collections::HashSet<&str> = classes
        .iter()
        .filter(|c| c.is_typed_dict)
        .map(|c| c.name.as_str())
        .collect();
    let mut isinstance_typeddict_spans = typeddict_ext::collect_isinstance_typeddict_violations(
        bindings,
        stmts,
        &typeddict_class_names,
    );
    isinstance_typeddict_spans.extend(typevar::collect_typevar_bound_typeddict_violations(
        bindings, stmts,
    ));

    // Fields initialised to an empty collection below had their collectors
    // deleted as text-matched logic. The rules downstream of them are
    // registered and INERT — never silently satisfied — pending
    // [ASTREBUILD-PHASE-RESOLVER]; see docs/plans/CHECKER-AST-RECONSTRUCTION-PLAN.md.
    AnalysisResults {
        reveal_type_calls: calls_and_reveal::collect_reveal_type_calls(stmts),
        assert_type_calls: calls_and_reveal::collect_assert_type_calls_from_stmts(stmts, source),
        typeddict_calls: Vec::new(),
        newtype_calls: Vec::new(),
        namedtuple_defs: Vec::new(),
        multiple_unbounded_tuple_spans: annotations::collect_multiple_unbounded_tuple_spans(stmts),
        module_bare_assignments: assigns::collect_module_bare_assignments(stmts),
        module_attr_assignments: assigns::collect_module_attr_assignments(stmts),
        final_issues: final_readonly_ext::collect_final_violations(bindings, stmts, classes),
        float_param_int_attr_accesses: Vec::new(),
        literal_string_enum_mismatches: Vec::new(),
        readonly_issues: final_readonly::collect_readonly_violations(stmts, classes, source),
        protocol_self_issues: Vec::new(),
        protocol_instantiation_issues: Vec::new(),
        isinstance_typeddict_spans,
        typeddict_key_issues: typeddict::collect_typeddict_key_violations(stmts, classes, source),
        generic_subscript_sites: generics::collect_generic_subscript_sites(stmts),
        type_alias_defs: type_alias::collect_type_alias_defs(stmts),
        unhashable_hash_calls: unhashable::collect_unhashable_hash_calls(stmts, classes),
        protocol_rtc_issues: Vec::new(),
        generator_issues: Vec::new(),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "module building requires many context params"
)]
fn build_resolved_module(
    module: &ParsedModule,
    bindings: basilisk_canonical::BindingTable,
    functions: Vec<crate::scope::FunctionInfo>,
    classes: Vec<crate::scope::ClassInfo>,
    module_vars: Vec<crate::scope::VariableInfo>,
    imports: Vec<crate::scope::ImportInfo>,
    match_stmts: Vec<crate::scope::MatchStmtInfo>,
    calls: Vec<crate::scope::CallSite>,
    typevar_calls: Vec<crate::scope::TypeVarCallInfo>,
    results: AnalysisResults,
) -> ResolvedModule {
    let stmts = &module.ast.body;
    // `TypeAliasType` validation was deleted with its text-matched recogniser
    // and depended on the equally-deleted `TypeVar` census. Inert pending
    // [ASTREBUILD-PHASE-RESOLVER].
    let type_alias_type_violations = Vec::new();
    // The `key=` lambda tuple-index collector identified its callees through a
    // spelling whitelist (`sorted`/`min`/`max`/`sort`) and was deleted with it.
    // Inert pending [ASTREBUILD-PHASE-RESOLVER].
    let tuple_index_violations: Vec<crate::scope::TupleIndexViolation> = Vec::new();
    ResolvedModule {
        bindings: crate::scope::ModuleBindings::new(bindings),
        functions,
        classes,
        module_vars,
        imports,
        match_stmts,
        // Implements the module-scope binding census behind `names_undefined`'s
        // module-level passes (issues #397, #398): binding name -> site count.
        module_bindings: assigns::collect_all_assigns(stmts).into_iter().fold(
            std::collections::HashMap::new(),
            |mut counts, name| {
                *counts.entry(name).or_insert(0) += 1;
                counts
            },
        ),
        cast_calls: Vec::new(),
        calls,
        typevar_calls,
        reveal_type_calls: results.reveal_type_calls,
        assert_type_calls: results.assert_type_calls,
        typeddict_calls: results.typeddict_calls,
        newtype_calls: results.newtype_calls,
        multiple_unbounded_tuple_spans: results.multiple_unbounded_tuple_spans,
        final_violations: results.final_issues,
        module_bare_assignments: results.module_bare_assignments,
        module_attr_assignments: results.module_attr_assignments,
        module_attr_accesses: module_level::collect_module_attr_accesses(stmts),
        module_order_comparisons: module_level::collect_module_order_comparisons(stmts),
        readonly_violations: results.readonly_issues,
        annotated_direct_call_spans: Vec::new(),
        imported_final_names: final_readonly::collect_imported_final_names(stmts, &module.path),
        imported_final_methods: std::collections::HashMap::new(),
        type_alias_type_calls: Vec::new(),
        type_alias_type_violations,
        type_statements: type_alias::collect_type_statements(stmts),
        annotated_too_few_args: Vec::new(),
        namedtuple_defs: results.namedtuple_defs,
        float_param_int_attr_accesses: results.float_param_int_attr_accesses,
        literal_string_enum_mismatches: results.literal_string_enum_mismatches,
        enum_value_type_violations: Vec::new(),
        local_classvar_violations: Vec::new(),
        pep695_bound_violations: generics::collect_pep695_bound_violations(stmts),
        historical_positional_violations: historical::collect_historical_positional_violations(
            stmts,
        ),
        invalid_string_annotations: annotations::collect_invalid_annotations(stmts),
        protocol_self_violations: results.protocol_self_issues,
        protocol_instantiation_violations: results.protocol_instantiation_issues,
        isinstance_typeddict_violations: results.isinstance_typeddict_spans,
        typeddict_key_violations: results.typeddict_key_issues,
        type_alias_defs: results.type_alias_defs,
        generic_subscript_sites: results.generic_subscript_sites,
        literal_augmented_assign_violations: Vec::new(),
        tuple_index_violations,
        bounded_typevar_attr_violations: crate::bounded_typevar::collect(stmts),
        protocol_class_object_violations: Vec::new(),
        unhashable_hash_call_violations: results.unhashable_hash_calls,
        protocol_runtime_checkable_violations: results.protocol_rtc_issues,
        generator_violations: results.generator_issues,
        unbound_typevar_usages: Vec::new(),
        imported_symbols: std::collections::HashMap::new(),
        builtin_classes: std::sync::Arc::new(std::collections::HashMap::new()),
        imported_modules: std::collections::HashMap::new(),
        pep695_scoping: pep695_scoping::collect_pep695_scoping(stmts, &module.source),
        path: module.path.clone(),
        source: module.source.clone(),
        comment_ranges: module
            .comment_ranges
            .iter()
            .map(|&(start, end)| crate::scope::Span::new(start, end))
            .collect(),
        lazy_ast: super::scope::LazyAst::default(),
    }
}
