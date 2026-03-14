//! AST visitor that collects function definitions and module-level information.

const ENUM_BASES: &[&str] = &["Enum", "IntEnum", "StrEnum", "Flag", "IntFlag", "ReprEnum"];

mod annotations;
mod assigns;
mod calls_and_reveal;
mod class_info;
mod class_info_ext;
mod core;
mod dataclass;
mod enum_checks;
mod final_readonly;
mod final_readonly_ext;
mod function_info;
mod generics;
mod historical;
mod module_level;
mod protocol;
mod protocol_ext;
mod type_alias;
mod typeddict;
mod typeddict_ext;
mod typevar;
mod unhashable;
mod yield_exprs;

use basilisk_parser::ParsedModule;

use crate::scope::ResolvedModule;

pub(crate) fn collect(module: &ParsedModule) -> ResolvedModule {
    let mut functions = Vec::new();
    let mut classes = Vec::new();
    let mut module_vars = Vec::new();
    let mut imports = Vec::new();
    let mut match_stmts = Vec::new();

    core::collect_from_body(
        &module.ast.body,
        &mut functions,
        &mut classes,
        &mut module_vars,
        &mut imports,
        &mut match_stmts,
        true,
    );

    dataclass::apply_dataclass_transform(&module.ast.body, &mut classes, &functions);

    let calls = calls_and_reveal::collect_calls_from_stmts(&module.ast.body);
    let typevar_calls = typevar::collect_typevar_calls(&module.ast.body);
    reclassify_generic_params(&mut classes, &typevar_calls);

    let violations = collect_violations(module, &classes, &functions);

    build_resolved_module(
        module, functions, classes, module_vars, imports, match_stmts,
        calls, typevar_calls, violations,
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

/// Intermediate container for all violation collections.
struct Violations {
    reveal_type_calls: Vec<crate::scope::RevealTypeCallInfo>,
    assert_type_calls: Vec<crate::scope::AssertTypeCallInfo>,
    typeddict_calls: Vec<crate::scope::TypedDictCallInfo>,
    newtype_calls: Vec<crate::scope::NewTypeCallInfo>,
    namedtuple_defs: Vec<crate::scope::NamedTupleDefInfo>,
    multiple_unbounded_tuple_spans: Vec<crate::scope::Span>,
    module_bare_assignments: Vec<crate::scope::ModuleBareAssignment>,
    module_attr_assignments: Vec<crate::scope::ModuleAttrAssignment>,
    final_violations: Vec<crate::scope::FinalViolationInfo>,
    float_param_int_attr_accesses: Vec<crate::scope::FloatParamIntAttrAccess>,
    literal_string_enum_mismatches: Vec<crate::scope::LiteralStringEnumMismatch>,
    readonly_violations: Vec<crate::scope::ReadOnlyViolationInfo>,
    protocol_self_violations: Vec<crate::scope::ProtocolSelfViolation>,
    protocol_instantiation_violations: Vec<crate::scope::ProtocolInstantiationViolation>,
    isinstance_typeddict_violations: Vec<crate::scope::Span>,
    typeddict_key_violations: Vec<crate::scope::TypedDictKeyViolation>,
    generic_subscript_sites: Vec<crate::scope::GenericSubscriptSite>,
    type_alias_defs: Vec<crate::scope::TypeAliasDefInfo>,
    unhashable_hash_call_violations: Vec<crate::scope::UnhashableHashCallViolation>,
    protocol_runtime_checkable_violations: Vec<crate::scope::ProtocolRtcViolation>,
    generator_violations: Vec<crate::scope::GeneratorViolation>,
}

fn collect_violations(
    module: &ParsedModule,
    classes: &[crate::scope::ClassInfo],
    functions: &[crate::scope::FunctionInfo],
) -> Violations {
    let stmts = &module.ast.body;
    let source = &module.source;

    let typeddict_class_names: std::collections::HashSet<&str> = classes
        .iter()
        .filter(|c| c.is_typed_dict)
        .map(|c| c.name.as_str())
        .collect();
    let mut isinstance_typeddict_violations =
        typeddict_ext::collect_isinstance_typeddict_violations(stmts, &typeddict_class_names);
    isinstance_typeddict_violations
        .extend(typevar::collect_typevar_bound_typeddict_violations(stmts));

    Violations {
        reveal_type_calls: calls_and_reveal::collect_reveal_type_calls(stmts),
        assert_type_calls: calls_and_reveal::collect_assert_type_calls_from_stmts(stmts, &[], source),
        typeddict_calls: typeddict::collect_typeddict_calls(stmts),
        newtype_calls: type_alias::collect_newtype_calls(stmts),
        namedtuple_defs: type_alias::collect_namedtuple_defs(stmts, source),
        multiple_unbounded_tuple_spans: annotations::collect_multiple_unbounded_tuple_spans(stmts),
        module_bare_assignments: assigns::collect_module_bare_assignments(stmts),
        module_attr_assignments: assigns::collect_module_attr_assignments(stmts),
        final_violations: final_readonly_ext::collect_final_violations(stmts, classes, source),
        float_param_int_attr_accesses: module_level::collect_float_param_int_attr_accesses(stmts, source),
        literal_string_enum_mismatches: enum_checks::collect_literal_string_enum_mismatches(stmts, source),
        readonly_violations: final_readonly::collect_readonly_violations(stmts, classes),
        protocol_self_violations: protocol::collect_protocol_self_violations(stmts, classes, functions, source),
        protocol_instantiation_violations: protocol::collect_protocol_instantiation_violations(stmts, classes),
        isinstance_typeddict_violations,
        typeddict_key_violations: typeddict::collect_typeddict_key_violations(stmts, classes, source),
        generic_subscript_sites: generics::collect_generic_subscript_sites(stmts),
        type_alias_defs: type_alias::collect_type_alias_defs(stmts),
        unhashable_hash_call_violations: unhashable::collect_unhashable_hash_calls(stmts, classes),
        protocol_runtime_checkable_violations: protocol_ext::collect_protocol_rtc_violations(stmts, classes),
        generator_violations: module_level::collect_generator_violations(functions, source),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_resolved_module(
    module: &ParsedModule,
    functions: Vec<crate::scope::FunctionInfo>,
    classes: Vec<crate::scope::ClassInfo>,
    module_vars: Vec<crate::scope::VariableInfo>,
    imports: Vec<crate::scope::ImportInfo>,
    match_stmts: Vec<crate::scope::MatchStmtInfo>,
    calls: Vec<crate::scope::CallSite>,
    typevar_calls: Vec<crate::scope::TypeVarCallInfo>,
    violations: Violations,
) -> ResolvedModule {
    let stmts = &module.ast.body;
    ResolvedModule {
        functions,
        classes,
        module_vars,
        imports,
        match_stmts,
        calls,
        typevar_calls,
        reveal_type_calls: violations.reveal_type_calls,
        assert_type_calls: violations.assert_type_calls,
        typeddict_calls: violations.typeddict_calls,
        newtype_calls: violations.newtype_calls,
        multiple_unbounded_tuple_spans: violations.multiple_unbounded_tuple_spans,
        final_violations: violations.final_violations,
        module_bare_assignments: violations.module_bare_assignments,
        module_attr_assignments: violations.module_attr_assignments,
        module_attr_accesses: module_level::collect_module_attr_accesses(stmts),
        module_order_comparisons: module_level::collect_module_order_comparisons(stmts),
        readonly_violations: violations.readonly_violations,
        annotated_direct_call_spans: module_level::collect_annotated_direct_calls(stmts),
        imported_final_names: final_readonly::collect_imported_final_names(stmts, &module.path),
        type_alias_type_calls: type_alias::collect_type_alias_type_calls(stmts),
        type_statements: type_alias::collect_type_statements(stmts),
        annotated_too_few_args: Vec::new(),
        namedtuple_defs: violations.namedtuple_defs,
        float_param_int_attr_accesses: violations.float_param_int_attr_accesses,
        literal_string_enum_mismatches: violations.literal_string_enum_mismatches,
        enum_value_type_violations: enum_checks::collect_enum_value_type_violations(stmts, &module.source),
        local_classvar_violations: Vec::new(),
        pep695_bound_violations: generics::collect_pep695_bound_violations(stmts),
        historical_positional_violations: historical::collect_historical_positional_violations(stmts),
        invalid_string_annotations: annotations::collect_invalid_annotations(stmts),
        protocol_self_violations: violations.protocol_self_violations,
        protocol_instantiation_violations: violations.protocol_instantiation_violations,
        isinstance_typeddict_violations: violations.isinstance_typeddict_violations,
        typeddict_key_violations: violations.typeddict_key_violations,
        type_alias_defs: violations.type_alias_defs,
        generic_subscript_sites: violations.generic_subscript_sites,
        literal_augmented_assign_violations: Vec::new(),
        tuple_index_violations: Vec::new(),
        bounded_typevar_attr_violations: crate::bounded_typevar::collect(stmts),
        protocol_class_object_violations: Vec::new(),
        unhashable_hash_call_violations: violations.unhashable_hash_call_violations,
        protocol_runtime_checkable_violations: violations.protocol_runtime_checkable_violations,
        generator_violations: violations.generator_violations,
        unbound_typevar_usages: Vec::new(),
        path: module.path.clone(),
        source: module.source.clone(),
    }
}

// ---------------------------------------------------------------------------
// Historical positional-only parameter violation detection
// ---------------------------------------------------------------------------
