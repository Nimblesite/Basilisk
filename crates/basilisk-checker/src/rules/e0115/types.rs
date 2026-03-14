//! Shared data types for BSK-E0115.

use std::collections::{HashMap, HashSet};

use super::collect::DeprecatedInfo;

/// Inferred type of a variable assigned to a class constructor call.
#[derive(Debug, Clone)]
pub(super) struct VarType {
    /// Module alias used to access the class (e.g. "library"), or "" for a local class.
    pub(super) module_alias: String,
    /// The class name (e.g. "Spam" or "Invocable").
    pub(super) class_name: String,
}

/// Contextual data for visiting statements and detecting deprecated usages.
pub(super) struct DeprecatedUsageContext<'a> {
    pub(super) deprecated: &'a HashMap<String, DeprecatedInfo>,
    pub(super) module_aliases: &'a HashMap<String, String>,
    pub(super) deprecated_members: &'a HashMap<String, HashMap<String, DeprecatedInfo>>,
    pub(super) var_types: &'a HashMap<String, VarType>,
    pub(super) path: &'a str,
    pub(super) _def_spans: &'a HashSet<u32>,
}
