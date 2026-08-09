// ############################################################################
// # DELETED IMPLEMENTATION — PANIC-ONLY SHELL. DO NOT PUT LOGIC BACK HERE.   #
// #                                                                          #
// # Every body in this file was DELETED because it decided subtyping from    #
// # the SPELLING of annotation text: `is_subtype(&str, &str)` over class     #
// # names harvested from rendered annotations, `sup == "object"` for the top #
// # type, literal `"int"`/`"str"`/`"float"` matching for the numeric tower,  #
// # `|` splitting to decompose unions, `starts_with("tuple[")` to recognise  #
// # a tuple, and `strip_prefix` to settle enum membership.                   #
// #                                                                          #
// # None of that is a type judgment. It is string surgery wearing one, and   #
// # it is the defect that got Basilisk withdrawn from the python/typing      #
// # conformance results.                                                     #
// #                                                                          #
// # THE SIGNATURES SURVIVE ONLY AS A MAP. Each body panics because the real  #
// # implementation DOES NOT EXIST YET. That is mandatory, not a placeholder  #
// # awaiting convenience:                                                    #
// #                                                                          #
// #   * DO NOT return `false` "for now" — that silently blesses every        #
// #     mismatch and reports full coverage while checking nothing.           #
// #   * DO NOT return `true` "for now" — that invents a diagnostic for       #
// #     every legal subclass assignment.                                     #
// #   * DO NOT reintroduce the string tables under a new name, in a rule     #
// #     module, or as a "temporary" local helper.                            #
// #                                                                          #
// # The replacement is the resolved semantic model: `basilisk-resolver`      #
// # bindings and the `basilisk-canonical` binding table, with a class        #
// # hierarchy built from RESOLVED base symbols — never from base-class       #
// # source text. When a caller is rebuilt, it stops calling this file; this  #
// # file is never repaired.                                                  #
// #                                                                          #
// # Pinned by:                                                               #
// #   crates/basilisk-checker/tests/nominal_spelling_surgery_pin_tests.rs    #
// #   crates/basilisk-checker/tests/no_type_spelling_surgery_tests.rs        #
// ############################################################################

//! The DELETED string-keyed subtyping layer, reduced to loudly panicking
//! signatures so its call sites remain visible as the rebuild map.

use std::collections::HashMap;

/// Panic message shared by every deleted body in this module.
macro_rules! deleted {
    ($what:literal) => {
        panic!(concat!(
            "basilisk-checker: `",
            $what,
            "` was DELETED because it decided subtyping from the SPELLING of \
             annotation text rather than from resolved symbols. It panics \
             because the real implementation — a hierarchy built from resolved \
             base symbols on the binding table — DOES NOT EXIST YET. Do not \
             restore the deleted body and do not substitute a default answer: \
             rebuild this caller on the resolved semantic model, or make it \
             abstain."
        ))
    };
}

/// Declared variance of one generic type parameter. Inert data — the deleted
/// logic is what read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variance {
    /// `G[A] <: G[B]` when `A <: B` — read-only positions.
    Covariant,
    /// `G[A] <: G[B]` when `B <: A` — write-only positions.
    Contravariant,
    /// `G[A] <: G[B]` only when `A == B` — mutable containers, the default.
    Invariant,
}

/// One `TypedDict` field's schema entry. Inert data.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedDictField {
    /// The declared value type. Held as annotation TEXT — itself part of why
    /// this layer could only ever compare spellings.
    pub ty: String,
    /// `false` for `NotRequired` fields.
    pub required: bool,
    /// `ReadOnly` fields checked covariantly; mutable fields invariantly.
    pub read_only: bool,
}

/// DELETED — was a hierarchy keyed on rendered type NAMES. The fields are gone
/// with the bodies that read them; nothing may be registered into it and
/// nothing may be asked of it.
#[derive(Debug, Clone, Default)]
pub struct SubtypingContext;

impl SubtypingContext {
    /// DELETED — panics; see the banner at the head of this file.
    pub fn register_class(&mut self, _name: &str, _bases: &[String]) {
        deleted!("SubtypingContext::register_class")
    }

    /// DELETED — panics; see the banner at the head of this file.
    pub fn register_member(&mut self, _class: &str, _member: &str, _ty: &str) {
        deleted!("SubtypingContext::register_member")
    }

    /// DELETED — panics; see the banner at the head of this file.
    pub fn register_protocol(&mut self, _name: &str) {
        deleted!("SubtypingContext::register_protocol")
    }

    /// DELETED — panics; see the banner at the head of this file.
    pub fn register_typeddict(&mut self, _name: &str, _fields: HashMap<String, TypedDictField>) {
        deleted!("SubtypingContext::register_typeddict")
    }

    /// DELETED — panics; see the banner at the head of this file.
    pub fn register_variance(&mut self, _class: &str, _variance: Vec<Variance>) {
        deleted!("SubtypingContext::register_variance")
    }

    /// DELETED — panics. Took two annotation STRINGS and compared them.
    #[must_use]
    pub fn is_subtype(&self, _sub: &str, _sup: &str) -> bool {
        deleted!("SubtypingContext::is_subtype")
    }

    /// DELETED — panics. Walked an MRO of NAME strings.
    #[must_use]
    pub fn is_nominal_subclass(&self, _sub: &str, _sup: &str) -> bool {
        deleted!("SubtypingContext::is_nominal_subclass")
    }

    /// DELETED — panics. Matched protocol members by name and type TEXT.
    #[must_use]
    pub fn satisfies_protocol(&self, _sub: &str, _protocol: &str) -> bool {
        deleted!("SubtypingContext::satisfies_protocol")
    }

    /// DELETED — panics. Compared `TypedDict` schemas by rendered field text.
    #[must_use]
    pub fn typeddict_assignable(&self, _source: &str, _target: &str) -> bool {
        deleted!("SubtypingContext::typeddict_assignable")
    }

    /// DELETED — panics. Took type arguments as `&[&str]`.
    #[must_use]
    pub fn generic_args_compatible(
        &self,
        _class: &str,
        _sub_args: &[&str],
        _sup_args: &[&str],
    ) -> bool {
        deleted!("SubtypingContext::generic_args_compatible")
    }

    /// DELETED — panics. Took parameter and return types as strings.
    #[must_use]
    pub fn callable_assignable(
        &self,
        _source_params: &[&str],
        _source_return: &str,
        _target_params: &[&str],
        _target_return: &str,
    ) -> bool {
        deleted!("SubtypingContext::callable_assignable")
    }
}

/// DELETED — panics. Settled subtyping between two NAME STRINGS: a hard-coded
/// `("bool", "int")` numeric tower, `sup == "object"`, and prefix tests on
/// rendered generics. Subtyping is a relation between resolved types.
#[must_use]
pub fn name_subtype(_sub: &str, _sup: &str) -> bool {
    deleted!("name_subtype")
}

/// DELETED — panics. Built the context above by harvesting base-class SOURCE
/// TEXT out of a module. The replacement resolves each base to the symbol it
/// denotes through the binding table.
#[must_use]
pub fn module_context(_module: &basilisk_resolver::ResolvedModule) -> SubtypingContext {
    deleted!("module_context")
}
