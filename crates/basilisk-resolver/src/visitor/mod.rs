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

    // Post-process: apply @dataclass_transform factory semantics.
    dataclass::apply_dataclass_transform(&module.ast.body, &mut classes, &functions);

    let calls = calls_and_reveal::collect_calls_from_stmts(&module.ast.body);
    let typevar_calls = typevar::collect_typevar_calls(&module.ast.body);

    // Post-process: reclassify generic params that are not actual TypeVars.
    let typevar_names: std::collections::HashSet<&str> =
        typevar_calls.iter().map(|tv| tv.name.as_str()).collect();
    for cls in &mut classes {
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
    let reveal_type_calls = calls_and_reveal::collect_reveal_type_calls(&module.ast.body);
    let assert_type_calls = calls_and_reveal::collect_assert_type_calls_from_stmts(
        &module.ast.body,
        &[],
        &module.source,
    );
    let typeddict_calls = typeddict::collect_typeddict_calls(&module.ast.body);
    let newtype_calls = type_alias::collect_newtype_calls(&module.ast.body);
    let namedtuple_defs = type_alias::collect_namedtuple_defs(&module.ast.body, &module.source);
    let multiple_unbounded_tuple_spans =
        annotations::collect_multiple_unbounded_tuple_spans(&module.ast.body);

    let module_bare_assignments = assigns::collect_module_bare_assignments(&module.ast.body);
    let module_attr_assignments = assigns::collect_module_attr_assignments(&module.ast.body);
    let final_violations =
        final_readonly_ext::collect_final_violations(&module.ast.body, &classes, &module.source);
    let float_param_int_attr_accesses =
        module_level::collect_float_param_int_attr_accesses(&module.ast.body, &module.source);
    let literal_string_enum_mismatches =
        enum_checks::collect_literal_string_enum_mismatches(&module.ast.body, &module.source);
    let readonly_violations =
        final_readonly::collect_readonly_violations(&module.ast.body, &classes);
    let protocol_self_violations = protocol::collect_protocol_self_violations(
        &module.ast.body,
        &classes,
        &functions,
        &module.source,
    );
    let protocol_instantiation_violations =
        protocol::collect_protocol_instantiation_violations(&module.ast.body, &classes);
    let typeddict_class_names: std::collections::HashSet<&str> = classes
        .iter()
        .filter(|c| c.is_typed_dict)
        .map(|c| c.name.as_str())
        .collect();
    let mut isinstance_typeddict_violations =
        typeddict_ext::collect_isinstance_typeddict_violations(
            &module.ast.body,
            &typeddict_class_names,
        );
    isinstance_typeddict_violations.extend(typevar::collect_typevar_bound_typeddict_violations(
        &module.ast.body,
    ));
    let typeddict_key_violations =
        typeddict::collect_typeddict_key_violations(&module.ast.body, &classes, &module.source);
    let generic_subscript_sites = generics::collect_generic_subscript_sites(&module.ast.body);
    let type_alias_defs = type_alias::collect_type_alias_defs(&module.ast.body);
    let unhashable_hash_call_violations =
        unhashable::collect_unhashable_hash_calls(&module.ast.body, &classes);
    let protocol_runtime_checkable_violations =
        protocol_ext::collect_protocol_rtc_violations(&module.ast.body, &classes);

    let generator_violations =
        module_level::collect_generator_violations(&functions, &module.source);
    let unbound_typevar_usages = Vec::new();
    ResolvedModule {
        functions,
        classes,
        module_vars,
        imports,
        match_stmts,
        calls,
        typevar_calls,
        reveal_type_calls,
        assert_type_calls,
        typeddict_calls,
        newtype_calls,
        multiple_unbounded_tuple_spans,
        final_violations,
        module_bare_assignments,
        module_attr_assignments,
        module_attr_accesses: module_level::collect_module_attr_accesses(&module.ast.body),
        module_order_comparisons: module_level::collect_module_order_comparisons(&module.ast.body),
        readonly_violations,
        annotated_direct_call_spans: module_level::collect_annotated_direct_calls(&module.ast.body),
        imported_final_names: final_readonly::collect_imported_final_names(
            &module.ast.body,
            &module.path,
        ),
        type_alias_type_calls: type_alias::collect_type_alias_type_calls(&module.ast.body),
        type_statements: type_alias::collect_type_statements(&module.ast.body),
        annotated_too_few_args: Vec::new(),
        namedtuple_defs,
        float_param_int_attr_accesses,
        literal_string_enum_mismatches,
        enum_value_type_violations: enum_checks::collect_enum_value_type_violations(
            &module.ast.body,
            &module.source,
        ),
        local_classvar_violations: Vec::new(),
        pep695_bound_violations: generics::collect_pep695_bound_violations(&module.ast.body),
        historical_positional_violations: historical::collect_historical_positional_violations(
            &module.ast.body,
        ),
        invalid_string_annotations: annotations::collect_invalid_annotations(&module.ast.body),
        protocol_self_violations,
        protocol_instantiation_violations,
        isinstance_typeddict_violations,
        typeddict_key_violations,
        type_alias_defs,
        generic_subscript_sites,
        literal_augmented_assign_violations: Vec::new(),
        tuple_index_violations: Vec::new(),
        bounded_typevar_attr_violations: crate::bounded_typevar::collect(&module.ast.body),
        protocol_class_object_violations: Vec::new(),
        unhashable_hash_call_violations,
        protocol_runtime_checkable_violations,
        generator_violations,
        unbound_typevar_usages,
        path: module.path.clone(),
        source: module.source.clone(),
    }
}

// ---------------------------------------------------------------------------
// Historical positional-only parameter violation detection
// ---------------------------------------------------------------------------
