//! Implements [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
//!
//! Type checking rules.
//!
//! Each rule is a zero-size struct implementing [`Rule`]. Rules are
//! registered in [`run_all`] and executed in order against a resolved module.
//!
//! Module declarations are listed explicitly (not via a `rule_modules!` macro)
//! so `cargo mutants` can discover them — it parses Rust with `syn` and does
//! not expand macros, which means macro-declared modules are invisible to it.

pub(crate) mod aliases_implicit;
pub(crate) mod aliases_newtype;
pub(crate) mod aliases_recursive;
pub(crate) mod aliases_type_statement;
pub(crate) mod aliases_typealiastype;
pub(crate) mod annotations_forward_refs;
pub(crate) mod annotations_generators;
pub(crate) mod annotations_generators_2;
pub(crate) mod annotations_generators_helpers;
pub(crate) mod annotations_typeexpr;
pub(crate) mod assignment_compatibility;
pub(crate) mod callables_annotation;
pub(crate) mod callables_kwargs;
pub(crate) mod callables_protocol;
pub(crate) mod callables_protocol_2;
pub(crate) mod callables_subtyping;
pub(crate) mod calls_argument_count;
pub(crate) mod calls_argument_type;
pub(crate) mod classes_classvar;
pub(crate) mod classes_override;
pub(crate) mod classes_override_2;
pub(crate) mod classes_override_3;
pub(crate) mod constructors_call_init;
pub(crate) mod constructors_call_new;
pub(crate) mod constructors_call_type;
pub(crate) mod constructors_callable;
pub(crate) mod dataclasses_frozen;
pub(crate) mod dataclasses_hash;
pub(crate) mod dataclasses_inheritance;
pub(crate) mod dataclasses_kwonly;
pub(crate) mod dataclasses_match_args;
pub(crate) mod dataclasses_order;
pub(crate) mod dataclasses_postinit;
pub(crate) mod dataclasses_slots;
pub(crate) mod dataclasses_transform_class;
pub(crate) mod dataclasses_transform_meta;
pub(crate) mod dataclasses_usage;
pub(crate) mod dict_key_hashable;
pub(crate) mod directives_assert_type;
pub(crate) mod directives_assert_type_2;
pub(crate) mod directives_cast;
pub(crate) mod directives_deprecated;
pub(crate) mod directives_disjoint_base;
pub(crate) mod directives_reveal_type;
pub(crate) mod directives_version_platform;
pub(crate) mod enums_behaviors;
pub(crate) mod enums_expansion;
pub(crate) mod enums_member_access;
pub(crate) mod enums_member_values;
pub(crate) mod enums_members;
pub(crate) mod enums_members_2;
pub(crate) mod explicit_any;
pub(crate) mod generics_base_class;
pub(crate) mod generics_base_class_2;
pub(crate) mod generics_base_class_3;
pub(crate) mod generics_basic;
pub(crate) mod generics_basic_2;
pub(crate) mod generics_basic_3;
pub(crate) mod generics_defaults;
pub(crate) mod generics_defaults_2;
pub(crate) mod generics_defaults_referential;
pub(crate) mod generics_defaults_referential_2;
pub(crate) mod generics_defaults_referential_2_helpers;
pub(crate) mod generics_defaults_specialization;
pub(crate) mod generics_scoping;
pub(crate) mod generics_self_attributes;
pub(crate) mod generics_self_basic;
pub(crate) mod generics_self_protocols;
pub(crate) mod generics_self_usage;
pub(crate) mod generics_syntax_compatibility;
pub(crate) mod generics_syntax_declarations;
pub(crate) mod generics_syntax_declarations_2;
pub(crate) mod generics_syntax_scoping;
pub(crate) mod generics_type_erasure;
pub(crate) mod generics_typevartuple_args;
pub(crate) mod generics_typevartuple_basic;
pub(crate) mod generics_typevartuple_basic_2;
pub(crate) mod generics_typevartuple_basic_3;
pub(crate) mod generics_typevartuple_callable;
pub(crate) mod generics_typevartuple_specialization;
pub(crate) mod generics_typevartuple_specialization_2;
pub(crate) mod generics_typevartuple_unpack;
pub(crate) mod generics_upper_bound_2;
pub(crate) mod generics_variance;
pub(crate) mod generics_variance_inference;
pub(crate) mod guards;
pub(crate) mod historical_positional;
pub(crate) mod imports_missing_name;
pub(crate) mod imports_module_attribute;
pub(crate) mod imports_unresolved;
pub(crate) mod lambda_missing_annotations;
pub(crate) mod literals_literalstring;
pub(crate) mod literals_literalstring_helpers;
pub(crate) mod literals_parameterizations;
pub(crate) mod literals_parameterizations_2;
pub(crate) mod literals_semantics;
pub(crate) mod literals_semantics_2;
pub(crate) mod match_exhaustiveness;
pub(crate) mod missing_attribute_annotation;
pub(crate) mod missing_override_decorator;
pub(crate) mod missing_parameter_annotation;
pub(crate) mod missing_return_annotation;
pub(crate) mod missing_type_stubs;
pub(crate) mod missing_vararg_annotation;
pub(crate) mod missing_variable_type;
pub(crate) mod namedtuples_define_class;
pub(crate) mod namedtuples_define_functional;
pub(crate) mod namedtuples_type_compat;
pub(crate) mod namedtuples_usage;
pub(crate) mod names_unbound;
pub(crate) mod names_undefined;
pub(crate) mod narrowing_typeguard;
pub(crate) mod narrowing_typeis;
pub(crate) mod narrowing_typeis_2;
pub(crate) mod overloads_basic;
pub(crate) mod overloads_consistency;
pub(crate) mod overloads_consistency_2;
pub(crate) mod overloads_consistency_3;
pub(crate) mod overloads_definitions;
pub(crate) mod overloads_evaluation;
pub(crate) mod protocols_class_objects;
pub(crate) mod protocols_class_objects_2;
pub(crate) mod protocols_definition;
pub(crate) mod protocols_definition_2;
pub(crate) mod protocols_explicit;
pub(crate) mod protocols_explicit_2;
pub(crate) mod protocols_explicit_3;
pub(crate) mod protocols_generic;
pub(crate) mod protocols_merging;
pub(crate) mod protocols_modules;
pub(crate) mod protocols_runtime_checkable;
pub(crate) mod protocols_runtime_checkable_2;
pub(crate) mod protocols_subtyping;
pub(crate) mod protocols_variance;
pub(crate) mod protocols_variance_2;
pub(crate) mod qualifiers_annotated;
pub(crate) mod qualifiers_annotated_2;
pub(crate) mod qualifiers_final_annotation;
pub(crate) mod qualifiers_final_annotation_2;
pub(crate) mod qualifiers_final_decorator;
pub(crate) mod redundant_annotation;
pub(crate) mod returns_compatibility;
pub(crate) mod returns_compatibility_2;
pub(crate) mod shared;

pub(crate) use shared::module_types::ModuleTypes;
pub(crate) mod specialtypes_never;
pub(crate) mod specialtypes_never_2;
pub(crate) mod specialtypes_promotions;
pub(crate) mod specialtypes_type;
pub(crate) mod stale_lock_file;
pub(crate) mod suppression_active_specific;
pub(crate) mod suppression_blanket;
pub(crate) mod suppression_malformed;
pub(crate) mod suppression_unused;
pub(crate) mod tuples_index;
pub(crate) mod tuples_index_2;
pub(crate) mod tuples_type_compat;
pub(crate) mod tuples_type_form;
pub(crate) mod tuples_type_form_2;
pub(crate) mod typeddicts_alt_syntax;
pub(crate) mod typeddicts_class_syntax;
pub(crate) mod typeddicts_class_syntax_2;
pub(crate) mod typeddicts_extra_items;
pub(crate) mod typeddicts_inheritance;
pub(crate) mod typeddicts_operations;
pub(crate) mod typeddicts_readonly;
pub(crate) mod typeddicts_required;
pub(crate) mod typeddicts_usage;
pub(crate) mod undeclared_dependency_import;
pub(crate) mod unused_dependency;
pub(crate) mod version_target_syntax;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

pub use crate::context::CheckContext;

/// A single type checking rule.
pub(crate) trait Rule {
    /// Run the rule against a resolved module and push any diagnostics.
    ///
    /// `ctx` carries the configured target version/platform
    /// ([CHKARCH-VERSION-TARGET]) so rules never hardcode a Python version.
    fn check(&self, module: &ResolvedModule, ctx: &CheckContext, diagnostics: &mut Vec<Diagnostic>);

    /// Run the rule with the module's SHARED type context — the annotation
    /// cascade, the inference oracle, and the nominal subtyping table.
    ///
    /// Each of those costs a full walk of the module, so the driver builds them
    /// once and passes them here; a rule that builds its own pays the walk
    /// again, and a dozen such rules made the walks the dominant cost of
    /// checking a file ([CHKARCH-TESTING-BENCH]). Rules that reason
    /// about types override this; every other rule ignores the argument through
    /// the default. [NARROWPLAN-INTEGRATION]
    fn check_with_types(
        &self,
        module: &ResolvedModule,
        types: &shared::module_types::ModuleTypes<'_>,
        ctx: &CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let _ = types;
        self.check(module, ctx, diagnostics);
    }

    /// This rule's opt-in tag declaration, or `None` for a core PEP rule.
    ///
    /// Returning `Some(..)` marks the rule as Basilisk-original: off by default,
    /// selected only when the configuration opts into one of its tags. This is
    /// the single source of rule provenance — there is no central rule list, and
    /// the `BSK-` code prefix is cosmetic. See [`crate::rule_tags::OptInSpec`].
    /// [CHKTAG-PROVENANCE]
    fn opt_in_spec(&self) -> Option<crate::rule_tags::OptInSpec> {
        None
    }
}

/// All registered Phase 1 rules.
#[expect(
    clippy::too_many_lines,
    reason = "rule registry listing is inherently long"
)]
fn all_rules() -> &'static [&'static dyn Rule] {
    &[
        &missing_parameter_annotation::MissingParameterAnnotation,
        &missing_return_annotation::MissingReturnAnnotation,
        &missing_variable_type::MissingVariableType,
        &missing_vararg_annotation::MissingVarArgAnnotation,
        &missing_attribute_annotation::MissingAttributeAnnotation,
        &imports_unresolved::ImportFromUntypedModule,
        &imports_missing_name::MissingImportedName,
        &returns_compatibility::ReturnTypeMismatch,
        &calls_argument_type::ArgumentTypeMismatch,
        &returns_compatibility_2::ReturnTypeMismatch,
        &assignment_compatibility::AssignmentTypeMismatch,
        &callables_annotation::InvalidTypeArgCount,
        &classes_override::IncompatibleOverride,
        &classes_override_2::IncompatibleVariableOverride,
        &names_undefined::UndefinedVariable,
        &names_unbound::UnboundVariable,
        &overloads_definitions::MissingOverloadImpl,
        &overloads_consistency::OverlappingOverloads,
        &dict_key_hashable::UnhashableDictKey,
        &match_exhaustiveness::NonExhaustiveMatch,
        &annotations_typeexpr::InvalidTypeForm,
        &missing_override_decorator::MissingOverrideDecorator,
        &generics_basic::TypeVarSingleConstraint,
        &generics_base_class::DuplicateTypeVarInGeneric,
        &typeddicts_class_syntax::TypedDictMethodNotAllowed,
        &generics_defaults::NonDefaultAfterDefault,
        &directives_cast::InvalidCastCall,
        &typeddicts_class_syntax_2::InvalidTypedDictBase,
        &directives_reveal_type::InvalidRevealTypeCall,
        &qualifiers_final_decorator::FinalViolation,
        &typeddicts_required::RequiredNotRequiredContext,
        &classes_classvar::ClassVarInvalidContext,
        &typeddicts_alt_syntax::InvalidTypedDictCall,
        &typeddicts_inheritance::InvalidTypedDictInheritance,
        &directives_assert_type::InvalidAssertTypeCall,
        &enums_behaviors::EnumWithMembersFinal,
        &calls_argument_count::TooFewArguments,
        &generics_syntax_compatibility::Pep695TraditionalTypeVarMix,
        &generics_basic_2::NonTypeVarInGeneric,
        &qualifiers_final_annotation::FinalInvalidPosition,
        &qualifiers_annotated::AnnotatedInvalidFirstArg,
        &enums_members::EnumMemberAnnotated,
        &enums_member_access::EnumMemberAccess,
        &annotations_forward_refs::InvalidTypeAnnotation,
        &aliases_implicit::TypeAliasInvalidRhs,
        &tuples_type_form::MultipleUnboundedTupleTypes,
        &aliases_newtype::InvalidNewType,
        &literals_parameterizations::InvalidLiteralParam,
        &dataclasses_frozen::FrozenDataclassAssignment,
        &directives_assert_type_2::AssertTypeMismatch,
        &qualifiers_final_annotation_2::FinalAnnotationViolation,
        &generics_typevartuple_basic::TypeVarInvalidKwargs,
        &typeddicts_readonly::ReadOnlyTypedDictMutation,
        &aliases_type_statement::TypeStatementInvalidRhs,
        &qualifiers_annotated_2::AnnotatedTooFewArguments,
        &dataclasses_match_args::MatchArgsFalseAccess,
        &dataclasses_order::CrossTypeDataclassOrderComparison,
        &enums_expansion::AssertTypeEnumLiteralMismatch,
        &specialtypes_never::NoReturnFallThrough,
        &dataclasses_hash::NonHashableDataclassAssignment,
        &namedtuples_define_functional::InvalidNamedTupleCall,
        &specialtypes_promotions::FloatParamIntAttrAccess,
        &enums_member_values::EnumValueTypeMismatch,
        &enums_members_2::EnumNonMemberInLiteral,
        &literals_parameterizations_2::LiteralStringEnumMismatch,
        &dataclasses_kwonly::DataclassKwOnlyViolation,
        &specialtypes_never_2::NeverTypeCompatibility,
        &historical_positional::HistoricalPositionalViolation,
        &overloads_basic::NoMatchingOverload,
        &namedtuples_type_compat::NamedTupleTupleCompat,
        &constructors_call_new::ConstructorCallNewMismatch,
        &generics_self_attributes::SelfTypeAttributeIncompatible,
        &overloads_evaluation::OverloadUnionExpansionFailure,
        &generics_self_protocols::ProtocolSelfViolation,
        &generics_self_basic::SelfTypeViolation,
        &protocols_modules::ModuleProtocolIncompatible,
        &generics_typevartuple_unpack::TypeVarTupleUnpackViolation,
        &generics_typevartuple_callable::TypeVarTupleCallableMismatch,
        &generics_typevartuple_basic_2::TypeVarTupleUnpackRequired,
        &generics_typevartuple_basic_3::TypeVarTupleInvalidParams,
        &generics_typevartuple_args::TypeVarTupleArgCountMismatch,
        &generics_typevartuple_specialization::MultipleTypeVarTuplesInGeneric,
        &typeddicts_usage::TypedDictRuntimeViolation,
        &generics_syntax_declarations::Pep695InvalidBound,
        &tuples_type_form_2::InvalidTupleTypeSyntax,
        &generics_defaults_2::TypeVarDefaultIncompatible,
        &generics_defaults_specialization::TooFewTypeArguments,
        &typeddicts_operations::TypedDictKeyValidation,
        &generics_self_usage::SelfInvalidLocation,
        &dataclasses_postinit::InitVarViolation,
        &dataclasses_usage::DataclassFieldDefaultFactoryMismatch,
        &protocols_definition::ProtocolNewSelfAttrViolation,
        &protocols_merging::NonProtocolBaseInProtocol,
        &protocols_explicit::ProtocolInstantiation,
        &literals_semantics::LiteralAugmentedAssign,
        &narrowing_typeguard::TypeGuardNoNarrowingParam,
        &generics_defaults_referential::TypeVarDefaultReferential,
        &tuples_index::TupleIndexOutOfBounds,
        &aliases_recursive::CyclicalTypeAliasReference,
        &generics_syntax_declarations_2::BoundedTypeVarAttrAccess,
        &protocols_class_objects::ProtocolClassObject,
        &generics_variance::VarianceIncompatibleBase,
        &dataclasses_slots::DataclassSlotsViolation,
        &generics_upper_bound_2::TypeVarBoundCallViolation,
        &protocols_variance::ProtocolVarianceViolation,
        &constructors_call_init::ConstructorCallError,
        &narrowing_typeis::TypeGuardCallableReturnMismatch,
        &narrowing_typeis_2::TypeIsInconsistentNarrowing,
        &protocols_runtime_checkable::ProtocolRuntimeCheckableViolation,
        &directives_deprecated::DeprecatedUsage,
        &directives_disjoint_base::DisjointBaseViolation,
        &namedtuples_define_class::NamedTupleDefError,
        &generics_scoping::UnboundTypeVarScope,
        &protocols_explicit_2::SuperAbstractCall,
        &protocols_runtime_checkable_2::ProtocolUnsafeOverlap,
        &annotations_generators::GeneratorReturnTypeViolation,
        &protocols_definition_2::ProtocolAssignmentConformance,
        &callables_protocol::CallableCallSiteViolation,
        &protocols_explicit_3::SuperCallOnAbstractProtocolMethod,
        &protocols_subtyping::ProtocolTupleElementMismatch,
        &generics_type_erasure::InstanceAttrOnClass,
        &literals_literalstring::LiteralStringAssignment,
        &tuples_index_2::TupleIndexOutOfRange,
        &generics_defaults_referential_2::TypeVarDefaultReferential,
        &literals_semantics_2::LiteralValueIncompatible,
        &generics_variance_inference::TypeVarScopeViolation,
        &annotations_generators_2::GeneratorTypeMismatch,
        &generics_base_class_2::InconsistentTypeVarOrder,
        &protocols_variance_2::ProtocolVarianceMismatch,
        &generics_base_class_3::InvariantGenericArgMismatch,
        &callables_subtyping::CallableSubtypingViolation,
        &protocols_generic::GenericProtocolViolation,
        &dataclasses_transform_meta::DataclassTransformMetaViolation,
        &generics_typevartuple_specialization_2::TypeVarTupleSpecializationViolation,
        &callables_protocol_2::CallableAssignmentViolation,
        &callables_kwargs::UnpackKwargsViolation,
        &dataclasses_transform_class::DataclassTransformClassViolation,
        &namedtuples_usage::NamedTupleUsageViolation,
        &constructors_call_type::TypeCallConstructorViolation,
        &specialtypes_type::TypeBracketViolation,
        &protocols_class_objects_2::ProtocolClassObjectViolation,
        &tuples_type_compat::TupleStarredUnpackCompatibility,
        &generics_basic_3::GenericTypeArgViolation,
        &generics_syntax_scoping::Pep695TypeParamScopingViolation,
        &directives_version_platform::DeadBranchVariable,
        &aliases_typealiastype::TypeAliasTypeViolation,
        &missing_type_stubs::MissingTypeStubs,
        &constructors_callable::ConstructorCallableMisuse,
        &imports_module_attribute::ModuleAttributeUndefined,
        &version_target_syntax::Pep695BelowTargetViolation,
        &typeddicts_extra_items::TypedDictExtraItemsViolation,
        &dataclasses_inheritance::DataclassFieldOrder,
        &overloads_consistency_2::OverloadDecoratorConsistency,
        &classes_override_3::OverrideWithoutBaseMethod,
        &overloads_consistency_3::OverloadImplConsistency,
        &undeclared_dependency_import::UndeclaredDependencyImport,
        &unused_dependency::UnusedDependency,
        &stale_lock_file::StaleLockFile,
        &suppression_active_specific::ActiveSpecificSuppression,
        &suppression_blanket::ActiveBlanketSuppression,
        &suppression_unused::UnusedSuppression,
        &suppression_malformed::MalformedSuppression,
        &explicit_any::ExplicitAny,
        &lambda_missing_annotations::LambdaMissingAnnotations,
        &redundant_annotation::RedundantAnnotationWarning,
    ]
}

/// Number of live entries in the rule registry.
///
/// Kept crate-private: public consumers use [`crate::rule_catalog`], while the
/// generated catalog parity test uses this count as a drift guard.
#[cfg(test)]
pub(crate) fn registered_rule_count() -> usize {
    all_rules().len()
}

/// Run all registered Phase 1 rules against a resolved module.
#[must_use]
pub fn run_all(module: &ResolvedModule, ctx: &CheckContext) -> Vec<Diagnostic> {
    // Most diagnostics are one-per primary source construct. Reserving for the
    // largest construct family avoids repeated growth and moves on files with
    // thousands of homogeneous errors without summing unrelated families and
    // over-allocating clean modules.
    let expected = [
        module.functions.len(),
        module.classes.len(),
        module.module_vars.len(),
        module.imports.len(),
        module.calls.len(),
        module.type_statements.len(),
    ]
    .into_iter()
    .max()
    .unwrap_or(0);
    // One type context for the whole module: every rule that reasons about
    // types shares the cascade, the oracle, and the class table instead of
    // rebuilding them ([CHKARCH-TESTING-BENCH], [NARROWPLAN-INTEGRATION]).
    let types = shared::module_types::ModuleTypes::build(module);
    all_rules()
        .iter()
        .fold(Vec::with_capacity(expected), |mut acc, rule| {
            rule.check_with_types(module, &types, ctx, &mut acc);
            acc
        })
}

/// Standalone entry point for a rule that reads the module's type context: a
/// single-rule test, or any caller outside the driver. Builds exactly what
/// [`run_all`] would otherwise share, so the two paths judge identically.
pub(crate) fn check_with_own_types<R: Rule + ?Sized>(
    rule: &R,
    module: &ResolvedModule,
    ctx: &CheckContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let types = shared::module_types::ModuleTypes::build(module);
    rule.check_with_types(module, &types, ctx, diagnostics);
}

/// Each Basilisk-original rule's self-declared [`crate::rule_tags::OptInSpec`],
/// gathered from the live registry so rule provenance can never drift from a
/// hand-maintained list. Consumed by the tagging layer. [CHKTAG-PROVENANCE]
pub(crate) fn opt_in_specs() -> Vec<crate::rule_tags::OptInSpec> {
    all_rules()
        .iter()
        .filter_map(|rule| rule.opt_in_spec())
        .collect()
}
