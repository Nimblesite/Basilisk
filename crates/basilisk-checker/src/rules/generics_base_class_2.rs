//! Implements [`generics_base_class_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_base_class_2`: Inconsistent `TypeVar` ordering across base classes.
//!
//! When a class inherits from multiple generic bases that share a common
//! generic ancestor, the `TypeVar` argument orderings must be consistent
//! (PEP 484: the type variable ordering implied by every path to a shared
//! generic ancestor must agree).
//!
//! ```python
//! class Grandparent(Generic[T1, T2]): ...
//! class Parent(Grandparent[T1, T2]): ...
//! class BadChild(Parent[T1, T2], Grandparent[T2, T1]): ...  # E
//! ```
//!
//! `BadChild` inherits `Grandparent` twice — once via `Parent[T1, T2]`
//! (which maps to `Grandparent[T1, T2]`) and once directly as
//! `Grandparent[T2, T1]`.  The orderings conflict.
//!
//! Base classes and their type arguments are read from the AST — a base is an
//! element of the class definition's argument list and a type argument is an
//! element of the subscript slice — never from the source text ([ASTREBUILD-LAW]).

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;
use ruff_python_ast::{self as ast, Expr, Stmt};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_base_class_2",
    docs_url: "https://www.basilisk-python.dev/errors/generics_base_class_2",
};

/// Emits `generics_base_class_2` when base classes impose inconsistent `TypeVar` orderings.
pub(crate) struct InconsistentTypeVarOrder;

impl Rule for InconsistentTypeVarOrder {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };

        let tables = ClassTables {
            info: basilisk_resolver::name_lookup(&module.classes),
            defs: class_defs(&parsed.ast.body),
        };

        for class in &module.classes {
            check_class(class, &tables, &module.path, diagnostics);
        }
    }
}

/// Lookup tables for the module's classes: the resolver's view (generic
/// parameters, name spans) and the parsed definitions (base expressions).
struct ClassTables<'a> {
    info: HashMap<&'a str, &'a basilisk_resolver::ClassInfo>,
    defs: HashMap<&'a str, &'a ast::StmtClassDef>,
}

/// Map each module-level class name to its parsed definition.
fn class_defs(stmts: &[Stmt]) -> HashMap<&str, &ast::StmtClassDef> {
    stmts
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::ClassDef(class) => Some((class.name.as_str(), class)),
            _ => None,
        })
        .collect()
}

/// A base class reference: its name and the type-argument names it supplies.
struct BaseSubscript {
    name: String,
    type_args: Vec<String>,
}

/// The base class references of a parsed class definition.
///
/// Keyword arguments (`metaclass=...`, `total=False`) live in
/// `arguments.keywords` and never appear here. A subscripted base whose type
/// arguments are not all simple names cannot be compared by `TypeVar`
/// identity, so the rule abstains for it ([ASTREBUILD-PHASE-RESOLVER]).
fn base_subscripts(class_def: &ast::StmtClassDef) -> Vec<BaseSubscript> {
    let Some(arguments) = class_def.arguments.as_deref() else {
        return Vec::new();
    };
    arguments.args.iter().filter_map(base_subscript).collect()
}

/// One base expression as a [`BaseSubscript`], when its shape is comparable.
fn base_subscript(base: &Expr) -> Option<BaseSubscript> {
    match base {
        Expr::Name(name) => Some(BaseSubscript {
            name: name.id.to_string(),
            type_args: Vec::new(),
        }),
        Expr::Subscript(sub) => {
            let Expr::Name(base_name) = sub.value.as_ref() else {
                return None;
            };
            let elements: Vec<&Expr> = match sub.slice.as_ref() {
                Expr::Tuple(tuple) => tuple.elts.iter().collect(),
                single => vec![single],
            };
            let type_args: Option<Vec<String>> = elements
                .iter()
                .map(|element| match element {
                    Expr::Name(name) => Some(name.id.to_string()),
                    _ => None,
                })
                .collect();
            type_args.map(|type_args| BaseSubscript {
                name: base_name.id.to_string(),
                type_args,
            })
        }
        _ => None,
    }
}

/// For a given class, check if any direct base leads to a shared ancestor
/// with conflicting `TypeVar` orderings.
fn check_class(
    class: &basilisk_resolver::ClassInfo,
    tables: &ClassTables<'_>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(class_def) = tables.defs.get(class.name.as_str()) else {
        return;
    };
    let bases = base_subscripts(class_def);
    if bases.len() < 2 {
        return;
    }

    // For each direct base, compute the implied ancestor type arg mappings.
    // ancestor_name -> list of (type args as resolved through the chain, origin base index)
    let mut ancestor_args: HashMap<String, Vec<(Vec<String>, usize)>> = HashMap::new();

    for (idx, base) in bases.iter().enumerate() {
        // The direct base itself is an "ancestor".
        if !base.type_args.is_empty() {
            ancestor_args
                .entry(base.name.clone())
                .or_default()
                .push((base.type_args.clone(), idx));
        }

        // If this base is a class defined in this module, propagate through its bases.
        propagate_ancestors(
            &base.name,
            &base.type_args,
            tables,
            idx,
            &mut ancestor_args,
            0,
        );
    }

    // Check for conflicts: same ancestor reached with different type arg orderings.
    for (ancestor_name, mappings) in &ancestor_args {
        if mappings.len() < 2 {
            continue;
        }
        let Some(first_mapping) = mappings.first() else {
            continue;
        };
        let first_args = &first_mapping.0;
        for other in mappings.get(1..).unwrap_or_default() {
            if other.0 != *first_args {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Inconsistent TypeVar ordering for `{}` in base classes of `{}`",
                        ancestor_name, class.name
                    ),
                    class.name_span,
                    path,
                    Some(
                        "All paths to a shared generic ancestor must use the same TypeVar ordering"
                            .to_owned(),
                    ),
                    Some(
                        "PEP 484: type variable ordering must be consistent across base classes"
                            .to_owned(),
                    ),
                ));
                return; // One diagnostic per class is enough.
            }
        }
    }
}

/// Propagate type arg mappings up the class hierarchy.
///
/// Given that a child inherits `Parent[T1, T2]` and `Parent` is defined as
/// `class Parent(Grandparent[T1, T2])`, we substitute `Parent`'s type params
/// with the actual type args to get `Grandparent[T1, T2]` as seen by the child.
fn propagate_ancestors(
    parent_name: &str,
    child_args_for_parent: &[String],
    tables: &ClassTables<'_>,
    origin_idx: usize,
    ancestor_args: &mut HashMap<String, Vec<(Vec<String>, usize)>>,
    depth: usize,
) {
    // Prevent infinite recursion in circular hierarchies.
    if depth > 10 {
        return;
    }
    let (Some(parent_info), Some(parent_def)) =
        (tables.info.get(parent_name), tables.defs.get(parent_name))
    else {
        return;
    };

    // Build a substitution map: parent's generic param name -> child's type arg.
    let substitution: HashMap<&str, &str> = parent_info
        .generic_params
        .iter()
        .map(|param| param.name.as_str())
        .zip(child_args_for_parent.iter().map(String::as_str))
        .collect();

    for parent_base in &base_subscripts(parent_def) {
        if parent_base.type_args.is_empty() {
            continue;
        }

        // Substitute type args.
        let resolved_args: Vec<String> = parent_base
            .type_args
            .iter()
            .map(|arg| {
                substitution
                    .get(arg.as_str())
                    .map_or_else(|| arg.clone(), |s| (*s).to_owned())
            })
            .collect();

        ancestor_args
            .entry(parent_base.name.clone())
            .or_default()
            .push((resolved_args.clone(), origin_idx));

        // Recurse upward.
        propagate_ancestors(
            &parent_base.name,
            &resolved_args,
            tables,
            origin_idx,
            ancestor_args,
            depth + 1,
        );
    }
}
