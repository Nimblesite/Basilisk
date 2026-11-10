//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Lightweight `Named` trait + collection helpers.
//!
//! Many resolver types share a `pub name: String` field, and 20+ sites across
//! the checker, LSP and resolver build `Vec<&str>` / `HashSet<&str>` of those
//! names with the same `iter().map(|x| x.name.as_str()).collect()` boilerplate.
//! `Named` plus [`collect_names`] / [`collect_name_set`] replace that pattern
//! with a single call.

use std::collections::{HashMap, HashSet};

use super::{
    AttributeInfo, ClassInfo, FunctionInfo, GenericParamInfo, ParameterInfo, TypeAliasDefInfo,
    TypeVarCallInfo, VariableInfo,
};

/// Anything that exposes a `&str` name.
pub trait Named {
    /// Borrow the name as a string slice.
    fn name_str(&self) -> &str;
}

macro_rules! impl_named_for_string_field {
    ($($t:ty),* $(,)?) => {
        $(
            impl Named for $t {
                #[inline]
                fn name_str(&self) -> &str {
                    self.name.as_str()
                }
            }
        )*
    };
}

impl_named_for_string_field!(
    AttributeInfo,
    ClassInfo,
    FunctionInfo,
    GenericParamInfo,
    ParameterInfo,
    TypeAliasDefInfo,
    TypeVarCallInfo,
    VariableInfo,
);

/// Collect the names of every `Named` item into a `Vec<&str>`, preserving order.
pub fn collect_names<T: Named>(items: &[T]) -> Vec<&str> {
    items.iter().map(Named::name_str).collect()
}

/// Collect the names of every `Named` item into a `HashSet<&str>`.
pub fn collect_name_set<T: Named>(items: &[T]) -> HashSet<&str> {
    items.iter().map(Named::name_str).collect()
}

/// Collect the names of items matching `pred` into a `Vec<&str>`.
pub fn collect_names_where<T: Named, F: FnMut(&&T) -> bool>(items: &[T], pred: F) -> Vec<&str> {
    items.iter().filter(pred).map(Named::name_str).collect()
}

/// Collect the names of items matching `pred` into a `HashSet<&str>`.
pub fn collect_name_set_where<T: Named, F: FnMut(&&T) -> bool>(
    items: &[T],
    pred: F,
) -> HashSet<&str> {
    items.iter().filter(pred).map(Named::name_str).collect()
}

/// Build a `name -> &item` lookup. Equivalent to
/// `items.iter().map(|x| (x.name_str(), x)).collect()` — a pattern repeated
/// dozens of times to look classes/functions up by name during rule checks.
pub fn name_lookup<T: Named>(items: &[T]) -> HashMap<&str, &T> {
    items.iter().map(|item| (item.name_str(), item)).collect()
}
