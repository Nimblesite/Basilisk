//! Implements [TYPEINF-SUBTYPING]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-SUBTYPING and
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-SUBTYPING
//!
//! The shared subtyping context — nominal class relationships, Protocol
//! member tables, `TypedDict` schemas, generic variance, and `Callable`
//! kinds, in ONE place.
//!
//! Two layers already exist and stay authoritative for what they cover:
//! [`crate::types::InferredType::is_assignable_to`] for inferred types
//! ([TYPEINF-SUBTYPING-IMPL]) and [`name_subtype`] here for the
//! annotation-text numeric tower the conformance rules use. This module adds
//! the *context-dependent* relations those layers cannot answer alone —
//! MRO walks, structural Protocol satisfaction, `TypedDict` field
//! compatibility, and declared variance.
//!
//! # [`SubtypingContext`] has no production caller yet — deliberately
//!
//! [`name_subtype`] below IS wired (eight rules delegate to it).
//! [`SubtypingContext`] is not, and that is the REQUIRED order, not an
//! oversight: [NARROWPLAN-SUBTYPING] mandates that rule-local subtype helpers
//! are replaced "only after parity tests pin their current accepted/rejected
//! cases". Landing the context plus its parity tests in one change and
//! migrating rules onto it in the next is what that instruction asks for —
//! migrating in the same change would move behaviour and its pins together,
//! which is exactly the drift the parity tests exist to prevent. The rules
//! consume this at the Integration stage ([NARROWPLAN-INTEGRATION]), whose
//! checklist item is "migrate assignment, return, call, and `assert_type`
//! rules incrementally, deleting the replaced local logic in the same
//! change". Until then this is a pure, fully-tested core.
//!
//! **Lint posture — do not "fix" this by narrowing visibility.** The
//! workspace denies `dead_code`. This module satisfies it because it is
//! `pub mod subtyping` at the crate root, so every item is reachable from
//! outside the crate and therefore live. Demoting the module or any item to
//! `pub(crate)` before the Integration-stage wiring lands would make
//! `dead_code` fire on a deliberate placeholder — and the fix for THAT is to
//! wire the rules up, never to add an `#[allow]`/`#[expect]`.

use std::collections::{HashMap, HashSet};

/// The context-free, annotation-text subtype core: identity plus the full
/// builtin numeric tower `bool <: int <: float <: complex`
/// ([TYPEINF-SUBTYPING-NOMINAL], typing-spec float/complex promotions).
///
/// This is THE single home of the text-level tower;
/// `rules::shared::is_numeric_subtype` and the rule-local helpers delegate
/// here so every rule agrees on it.
#[must_use]
pub fn name_subtype(sub: &str, sup: &str) -> bool {
    sub == sup
        || matches!(
            (sub, sup),
            ("bool", "int" | "float" | "complex")
                | ("int", "float" | "complex")
                | ("float", "complex")
        )
}

/// Declared variance of one generic type parameter
/// ([TYPEINF-SUBTYPING-GENERIC]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variance {
    /// `G[A] <: G[B]` when `A <: B` — read-only positions.
    Covariant,
    /// `G[A] <: G[B]` when `B <: A` — write-only positions.
    Contravariant,
    /// `G[A] <: G[B]` only when `A == B` — mutable containers, the default.
    Invariant,
}

/// One `TypedDict` field's schema entry ([TYPEINF-SUBTYPING-TYPEDDICT]).
#[derive(Debug, Clone, PartialEq)]
pub struct TypedDictField {
    /// The declared value type, as annotation text.
    pub ty: String,
    /// `false` for `NotRequired` fields.
    pub required: bool,
    /// `ReadOnly` fields check covariantly; mutable fields invariantly.
    pub read_only: bool,
}

/// The shared subtyping context: everything context-dependent that
/// [`name_subtype`] and `is_assignable_to` cannot answer alone.
#[derive(Debug, Clone, Default)]
pub struct SubtypingContext {
    /// Class name → direct base names (nominal edges).
    bases: HashMap<String, Vec<String>>,
    /// Class name → member name → declared type text.
    members: HashMap<String, HashMap<String, String>>,
    /// Names registered as `Protocol` classes.
    protocols: HashSet<String>,
    /// `TypedDict` name → field schemas.
    typeddicts: HashMap<String, HashMap<String, TypedDictField>>,
    /// Generic class name → per-parameter declared variance.
    variance: HashMap<String, Vec<Variance>>,
}

impl SubtypingContext {
    /// Register a class and its direct bases (nominal edges).
    pub fn register_class(&mut self, name: &str, bases: &[String]) {
        let _ = self.bases.insert(name.to_owned(), bases.to_vec());
    }

    /// Register one member (attribute or method) of a class.
    pub fn register_member(&mut self, class: &str, member: &str, ty: &str) {
        let _ = self
            .members
            .entry(class.to_owned())
            .or_default()
            .insert(member.to_owned(), ty.to_owned());
    }

    /// Mark a registered class as a `Protocol`.
    pub fn register_protocol(&mut self, name: &str) {
        let _ = self.protocols.insert(name.to_owned());
    }

    /// Register a `TypedDict` schema.
    pub fn register_typeddict(&mut self, name: &str, fields: HashMap<String, TypedDictField>) {
        let _ = self.typeddicts.insert(name.to_owned(), fields);
    }

    /// Register a generic class's declared per-parameter variance.
    pub fn register_variance(&mut self, class: &str, variance: Vec<Variance>) {
        let _ = self.variance.insert(class.to_owned(), variance);
    }

    /// The central entry: is annotation text `sub` usable where `sup` is
    /// expected, under this context?
    ///
    /// Gradual posture first ([TYPEINF-SUBTYPING-UNION]): `Any` either side
    /// and the `object` top accept. Then the tower, union decomposition,
    /// the nominal walk, structural Protocol satisfaction, and `TypedDict`
    /// schemas ([TYPEINF-SUBTYPING-NOMINAL/-PROTOCOL/-TYPEDDICT]).
    #[must_use]
    pub fn is_subtype(&self, sub: &str, sup: &str) -> bool {
        let sub = sub.trim();
        let sup = sup.trim();
        if sub == "Any" || sup == "Any" || sup == "object" || name_subtype(sub, sup) {
            return true;
        }
        if sub.contains('|') {
            return sub.split('|').all(|alt| self.is_subtype(alt, sup));
        }
        if sup.contains('|') {
            return sup.split('|').any(|alt| self.is_subtype(sub, alt));
        }
        self.is_nominal_subclass(sub, sup)
            || self.satisfies_protocol(sub, sup)
            || self.typeddict_assignable(sub, sup)
    }

    /// Nominal subtyping: `sup` appears among `sub`'s transitive bases
    /// ([TYPEINF-SUBTYPING-NOMINAL]). Cycle-guarded.
    #[must_use]
    pub fn is_nominal_subclass(&self, sub: &str, sup: &str) -> bool {
        self.mro(sub).iter().any(|ancestor| ancestor == sup)
    }

    /// The class's linearized ancestry (itself first) — breadth-first over
    /// the registered bases, each class visited once.
    fn mro(&self, class: &str) -> Vec<String> {
        let mut order = vec![class.to_owned()];
        let mut index = 0;
        while let Some(current) = order.get(index).cloned() {
            for base in self.bases.get(&current).into_iter().flatten() {
                if !order.contains(base) {
                    order.push(base.clone());
                }
            }
            index += 1;
        }
        order
    }

    /// A member's declared type, found on the class or anywhere up its MRO
    /// (inherited members satisfy protocols, [TYPEINF-SUBTYPING-PROTOCOL]).
    fn member_type(&self, class: &str, member: &str) -> Option<&str> {
        self.mro(class).iter().find_map(|ancestor| {
            self.members
                .get(ancestor)
                .and_then(|members| members.get(member))
                .map(String::as_str)
        })
    }

    /// Structural Protocol satisfaction ([TYPEINF-SUBTYPING-PROTOCOL]):
    /// `sub` provides every member `protocol` declares (own or inherited),
    /// each with a compatible (covariant) type. No inheritance required.
    #[must_use]
    pub fn satisfies_protocol(&self, sub: &str, protocol: &str) -> bool {
        if !self.protocols.contains(protocol) {
            return false;
        }
        let required: Vec<(&String, &String)> = self
            .mro(protocol)
            .iter()
            .filter_map(|ancestor| self.members.get(ancestor))
            .flatten()
            .collect::<HashMap<_, _>>()
            .into_iter()
            .collect();
        !required.is_empty()
            && required.iter().all(|(member, wanted)| {
                self.member_type(sub, member)
                    .is_some_and(|found| self.is_subtype(found, wanted))
            })
    }

    /// `TypedDict`-to-`TypedDict` structural assignability
    /// ([TYPEINF-SUBTYPING-TYPEDDICT]): every required target field exists
    /// in the source; `ReadOnly` fields check covariantly, mutable fields
    /// invariantly; `NotRequired` target fields may be absent.
    #[must_use]
    pub fn typeddict_assignable(&self, source: &str, target: &str) -> bool {
        let (Some(source_fields), Some(target_fields)) =
            (self.typeddicts.get(source), self.typeddicts.get(target))
        else {
            return false;
        };
        target_fields
            .iter()
            .all(|(name, wanted)| match source_fields.get(name) {
                None => !wanted.required,
                Some(found) if wanted.read_only => self.is_subtype(&found.ty, &wanted.ty),
                Some(found) => found.ty == wanted.ty,
            })
    }

    /// Positional generic-argument compatibility under the class's declared
    /// variance ([TYPEINF-SUBTYPING-GENERIC]). Unregistered classes default
    /// every position to invariant.
    #[must_use]
    pub fn generic_args_compatible(
        &self,
        class: &str,
        sub_args: &[&str],
        sup_args: &[&str],
    ) -> bool {
        if sub_args.len() != sup_args.len() {
            return false;
        }
        let declared = self.variance.get(class);
        sub_args
            .iter()
            .zip(sup_args)
            .enumerate()
            .all(|(position, (sub, sup))| {
                let variance = declared
                    .and_then(|list| list.get(position))
                    .copied()
                    .unwrap_or(Variance::Invariant);
                match variance {
                    Variance::Covariant => self.is_subtype(sub, sup),
                    Variance::Contravariant => self.is_subtype(sup, sub),
                    Variance::Invariant => self.is_subtype(sub, sup) && self.is_subtype(sup, sub),
                }
            })
    }

    /// `Callable` assignability ([TYPEINF-SUBTYPING-CALLABLE]):
    /// contravariant parameters, covariant return. An empty parameter list
    /// on either side is the gradual `Callable[..., R]`, which skips the
    /// parameter check — matching `is_assignable_to`'s treatment.
    #[must_use]
    pub fn callable_assignable(
        &self,
        source_params: &[&str],
        source_return: &str,
        target_params: &[&str],
        target_return: &str,
    ) -> bool {
        let params_ok = source_params.is_empty()
            || target_params.is_empty()
            || (source_params.len() == target_params.len()
                && source_params
                    .iter()
                    .zip(target_params)
                    .all(|(source, target)| self.is_subtype(target, source)));
        params_ok && self.is_subtype(source_return, target_return)
    }
}
