//! Implements [`generics_defaults_referential_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! `generics_defaults_referential_2`: ```TypeVar``` default referential violations.
//!
//! PEP 696 defines rules for when a `TypeVar` default references another
//! `TypeVar`:
//!
//! 1. **Ordering**: The referenced `TypeVar` must appear *before* the referencing
//!    `TypeVar` in `Generic[...]`.
//! 2. **Scope**: A `TypeVar` default must not reference `TypeVar`ar from an outer
//!    class scope.
//! 3. **Bound/constraint compatibility**: When `TypeVar` `T2` defaults to
//!    `TypeVar` `T1`, `T1`'s bound must be a subtype of `T2`'s bound, and
//!    `T2`'s constraints (if any) must be a superset of `T1`'s constraints.
//!
//! ```python
//! from typing import TypeVar, Generic
//!
//! S1 = TypeVar("S1")
//! S2 = TypeVar("S2", default=S1)
//!
//! Start2T = TypeVar("Start2T", default="StopT")
//! Stop2T = TypeVar("Stop2T", default=int)
//! class slice2(Generic[Start2T, Stop2T]): ...   # E: bad ordering
//!
//! class Foo3(Generic[S1]):
//!     class Bar2(Generic[S2]): ...              # E: outer scope
//!
//! Y1 = TypeVar("Y1", bound=int)
//! Invalid2 = TypeVar("Invalid2", float, str, default=Y1)  # E
//! ```

use std::collections::{HashMap, HashSet};

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use crate::rules::shared::{is_numeric_subtype, split_top_level_commas};

use super::generics_defaults_referential_2_helpers::{
    find_matching_bracket, literal_type_mismatch, parse_typevar_info_from_source,
    resolve_generic_params, TypeVarInfo,
};
use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_defaults_referential_2",
    docs_url: "https://www.basilisk-python.dev/errors/generics_defaults_referential_2",
};

/// Emits `generics_defaults_referential_2` for `TypeVar` default referential violations.
pub(crate) struct TypeVarDefaultReferential;

impl Rule for TypeVarDefaultReferential {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let typevar_names: HashSet<&str> =
            basilisk_resolver::collect_name_set(&module.typevar_calls);

        let typevar_info_list = parse_typevar_info_from_source(&module.source, &typevar_names);

        // Build lookup from name to info
        let info_map: HashMap<&str, &TypeVarInfo> = typevar_info_list
            .iter()
            .map(|info| (info.name.as_str(), info))
            .collect();

        // Build lookup from name to resolver TypeVarCallInfo for spans
        let span_map: HashMap<&str, &basilisk_resolver::TypeVarCallInfo> =
            basilisk_resolver::name_lookup(&module.typevar_calls);

        // Check 1: Forward references in Generic[...] params (ordering)
        check_ordering(module, &info_map, &typevar_names, diagnostics);

        // Check 2: Outer scope references
        check_outer_scope(module, &info_map, &typevar_names, diagnostics);

        // Check 3: Bound/constraint compatibility
        check_bound_constraint_compat(
            &typevar_info_list,
            &info_map,
            &span_map,
            &typevar_names,
            &module.path,
            diagnostics,
        );

        // Check 4: Subscripted generic class calls with literal arg mismatches
        check_subscripted_class_calls(module, &info_map, diagnostics);
    }
}

/// Check that `TypeVar` defaults don't referenc`TypeVars`rs that appear later
/// in the Generic[...] parameter list, or that are not in the list at all.
fn check_ordering(
    module: &ResolvedModule,
    info_map: &HashMap<&str, &TypeVarInfo>,
    typevar_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for class in &module.classes {
        if class.generic_params.is_empty() {
            continue;
        }

        let param_names: Vec<&str> = basilisk_resolver::collect_names(&class.generic_params);

        for (idx, param) in class.generic_params.iter().enumerate() {
            let Some(info) = info_map.get(param.name.as_str()) else {
                continue;
            };
            let Some(ref default_name) = info.default_typevar_name else {
                continue;
            };

            if !typevar_names.contains(default_name.as_str()) {
                continue;
            }

            let ref_pos = param_names.iter().position(|&n| n == default_name);

            let is_violation = match ref_pos {
                Some(pos) => pos > idx, // appears after this param
                None => true,           // not in this class's Generic at all
            };

            if is_violation {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "TypeVar `{}` has default `{}` which is not properly ordered \
                         in `Generic[...]` for `{}`",
                        param.name, default_name, class.name
                    ),
                    class.name_span,
                    &module.path,
                    Some(
                        "The referenced TypeVar must appear before the TypeVar \
                         that defaults to it in the same Generic parameter list"
                            .to_owned(),
                    ),
                    Some(
                        "PEP 696: a TypeVar default must reference a TypeVar that \
                         appears earlier in the same Generic parameter list"
                            .to_owned(),
                    ),
                ));
            }
        }
    }
}

/// Check that `TypeVar` defaults don't referenc`TypeVars`rs from an outer class scope.
///
/// Since the resolver doesn't track nested classes, we detect nested
/// `class ... (Generic[...])` patterns by scanning the source for indented
/// class definitions inside outer classes.
fn check_outer_scope(
    module: &ResolvedModule,
    info_map: &HashMap<&str, &TypeVarInfo>,
    typevar_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Collect outer class TypeVar params
    let mut outer_class_params: HashSet<&str> = HashSet::new();
    for class in &module.classes {
        for param in &class.generic_params {
            let _ = outer_class_params.insert(param.name.as_str());
        }
    }

    // Scan source for nested class definitions with Generic[...]
    // Pattern: indented `class Name(Generic[...]):` inside a class body
    let lines: Vec<&str> = module.source.lines().collect();
    let mut inside_class = false;
    let mut class_indent = 0usize;

    for (line_idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();

        // Detect outer class start
        if trimmed.starts_with("class ") && indent == 0 {
            inside_class = true;
            class_indent = indent;
            continue;
        }

        // Detect nested class inside an outer class
        if inside_class && indent > class_indent && trimmed.starts_with("class ") {
            // Extract Generic params from nested class
            if let Some(generic_start) = trimmed.find("Generic[") {
                let after = &trimmed[generic_start + 8..];
                if let Some(bracket_end) = after.find(']') {
                    let params_str = &after[..bracket_end];
                    let nested_params: Vec<&str> = params_str.split(',').map(str::trim).collect();

                    for nested_param in &nested_params {
                        let Some(info) = info_map.get(nested_param) else {
                            continue;
                        };
                        let Some(ref default_name) = info.default_typevar_name else {
                            continue;
                        };

                        if !typevar_names.contains(default_name.as_str()) {
                            continue;
                        }

                        // Is the default referencing an outer class's param?
                        let in_nested = nested_params.contains(&default_name.as_str());
                        if !in_nested && outer_class_params.contains(default_name.as_str()) {
                            // Compute byte offset for this line
                            let byte_offset: u32 =
                                u32::try_from(lines.get(..line_idx).map_or(0, |slice| {
                                    slice.iter().map(|l| l.len() + 1).sum::<usize>()
                                }))
                                .unwrap_or(u32::MAX);
                            let line_len = u32::try_from(line.len()).unwrap_or(u32::MAX);

                            diagnostics.push(error_diagnostic_owned(
                                CODE.clone(),
                                format!(
                                    "TypeVar `{nested_param}` has default `{default_name}` which references an \
                                     outer-scope TypeVar",
                                ),
                                basilisk_resolver::Span {
                                    start: byte_offset,
                                    end: byte_offset + line_len,
                                },
                                &module.path,
                                Some(
                                    "TypeVar defaults cannot reference TypeVars from an \
                                     enclosing class scope"
                                        .to_owned(),
                                ),
                                Some(
                                    "PEP 696: using a type parameter from an outer scope \
                                     as a default is not supported"
                                        .to_owned(),
                                ),
                            ));
                        }
                    }
                }
            }
        }

        // Reset inside_class when we return to top-level
        if inside_class && indent == 0 && !trimmed.is_empty() && !trimmed.starts_with('#') {
            inside_class = false;
        }
    }
}

/// Check that when `TypeVar` T2 defaults t`TypeVar`ar T1:
/// - T1's bound is a subtype of T2's bound (if T2 has a bound)
/// - T2's constraints are a superset of T1's constraints (if T2 has constraints)
fn check_bound_constraint_compat(
    typevar_info_list: &[TypeVarInfo],
    info_map: &HashMap<&str, &TypeVarInfo>,
    span_map: &HashMap<&str, &basilisk_resolver::TypeVarCallInfo>,
    typevar_names: &HashSet<&str>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for info in typevar_info_list {
        let Some(ref default_name) = info.default_typevar_name else {
            continue;
        };
        if !typevar_names.contains(default_name.as_str()) {
            continue;
        }
        let Some(ref_info) = info_map.get(default_name.as_str()) else {
            continue;
        };
        let Some(tv) = span_map.get(info.name.as_str()) else {
            continue;
        };
        check_one_bound_compat(info, ref_info, default_name, tv.span, path, diagnostics);
    }
}

/// Check bound and constraint compatibility for a single `TypeVar` pair.
fn check_one_bound_compat(
    info: &TypeVarInfo,
    ref_info: &TypeVarInfo,
    default_name: &str,
    span: basilisk_resolver::Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(ref info_bound) = info.bound_name {
        if let Some(ref ref_bound) = ref_info.bound_name {
            if !is_numeric_subtype(ref_bound, info_bound) {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "TypeVar `{}` has default `{}` with bound `{}` which is not a \
                         subtype of `{}`'s bound `{}`",
                        info.name, default_name, ref_bound, info.name, info_bound
                    ),
                    span,
                    path,
                    Some(
                        "The referenced TypeVar's bound must be a subtype of this TypeVar's bound"
                            .to_owned(),
                    ),
                    Some(
                        "PEP 696: T1's bound must be a subtype of T2's bound when T2 defaults to T1"
                            .to_owned(),
                    ),
                ));
            }
        }
    }
    if info.constraint_names.is_empty() {
        return;
    }
    check_constraint_compat(info, ref_info, default_name, span, path, diagnostics);
}

/// Check constraint-specific compatibility (called when `info` has constraints).
fn check_constraint_compat(
    info: &TypeVarInfo,
    ref_info: &TypeVarInfo,
    default_name: &str,
    span: basilisk_resolver::Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(ref ref_bound) = ref_info.bound_name {
        let compatible = info.constraint_names.iter().any(|c| c == ref_bound);
        if !compatible {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "TypeVar `{}` has default `{}` with upper bound `{}` which is \
                     incompatible with constraints `{}`",
                    info.name,
                    default_name,
                    ref_bound,
                    info.constraint_names.join(", ")
                ),
                span,
                path,
                Some(
                    "The referenced TypeVar's bound must be compatible with this TypeVar's constraints"
                        .to_owned(),
                ),
                Some(
                    "PEP 696: the upper bound of the default TypeVar must be compatible with the constrained TypeVar's constraint types"
                        .to_owned(),
                ),
            ));
        }
    }
    if ref_info.constraint_names.is_empty() {
        return;
    }
    let info_set: HashSet<&str> = info.constraint_names.iter().map(String::as_str).collect();
    let ref_set: HashSet<&str> = ref_info
        .constraint_names
        .iter()
        .map(String::as_str)
        .collect();
    if !ref_set.is_subset(&info_set) {
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "TypeVar `{}`'s constraints `{{{}}}` are not a superset of \
                 default TypeVar `{}`'s constraints `{{{}}}`",
                info.name,
                info.constraint_names.join(", "),
                default_name,
                ref_info.constraint_names.join(", ")
            ),
            span,
            path,
            Some(
                "The constrained TypeVar's constraints must include all of the default TypeVar's constraints"
                    .to_owned(),
            ),
            Some(
                "PEP 696: the constraints of T2 must be a superset of the constraints of T1 when T2 defaults to T1"
                    .to_owned(),
            ),
        ));
    }
}

/// Check subscripted generic class calls where literal arguments mismatch
/// the resolved parameter types (including `TypeVar` defaults).
///
/// Detects patterns like `Foo[int](1, "")` where `Foo.__init__` expects
/// `(a: int, b: int)` after resolving defaults, but receives a `str` literal.
fn check_subscripted_class_calls(
    module: &ResolvedModule,
    info_map: &HashMap<&str, &TypeVarInfo>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let class_map: HashMap<&str, &basilisk_resolver::ClassInfo> =
        basilisk_resolver::name_lookup(&module.classes);
    let init_map: HashMap<&str, &basilisk_resolver::FunctionInfo> = module
        .functions
        .iter()
        .filter(|f| f.name == "__init__")
        .filter_map(|f| f.class_name.as_deref().map(|cn| (cn, f)))
        .collect();

    for (line_idx, line) in module.source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        for (class_name, class_info) in &class_map {
            check_subscripted_class_on_line(
                module,
                info_map,
                diagnostics,
                &init_map,
                line_idx,
                line,
                trimmed,
                class_name,
                class_info,
            );
        }
    }
}

/// Check a single class pattern on a single source line.
#[expect(
    clippy::too_many_arguments,
    reason = "all args needed for line-level check"
)]
fn check_subscripted_class_on_line(
    module: &ResolvedModule,
    info_map: &HashMap<&str, &TypeVarInfo>,
    diagnostics: &mut Vec<Diagnostic>,
    init_map: &HashMap<&str, &basilisk_resolver::FunctionInfo>,
    line_idx: usize,
    line: &str,
    trimmed: &str,
    class_name: &&str,
    class_info: &basilisk_resolver::ClassInfo,
) {
    if class_info.generic_params.is_empty() {
        return;
    }
    let pattern = format!("{class_name}[");
    let Some(start) = trimmed.find(&pattern) else {
        return;
    };
    let after_name = &trimmed[start + pattern.len()..];
    let Some(bracket_end) = find_matching_bracket(after_name, '[', ']') else {
        return;
    };
    let type_args_str = &after_name[..bracket_end];
    let after_bracket = &after_name[bracket_end + 1..];
    if !after_bracket.starts_with('(') {
        return;
    }
    let call_args_str = &after_bracket[1..];
    let Some(paren_end) = find_matching_bracket(call_args_str, '(', ')') else {
        return;
    };
    let call_args_str = &call_args_str[..paren_end];
    let type_args: Vec<&str> = split_top_level_commas(type_args_str)
        .iter()
        .map(|s| s.trim())
        .collect();
    let resolved_types = resolve_generic_params(&class_info.generic_params, &type_args, info_map);
    let Some(init_fn) = init_map.get(class_name) else {
        return;
    };
    let init_params: Vec<_> = init_fn
        .parameters
        .iter()
        .filter(|p| p.name != "self")
        .collect();
    let call_args = split_top_level_commas(call_args_str);
    for (arg_idx, call_arg) in call_args.iter().enumerate() {
        let call_arg = call_arg.trim();
        let Some(param) = init_params.get(arg_idx) else {
            break;
        };
        let Some(ann_span) = param.annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(&module.source, ann_span) else {
            continue;
        };
        let resolved_type = resolved_types
            .get(ann_text)
            .map_or(ann_text, String::as_str);
        if let Some(mismatch) = literal_type_mismatch(call_arg, resolved_type) {
            let byte_offset: u32 = u32::try_from(
                module
                    .source
                    .lines()
                    .take(line_idx)
                    .map(|l| l.len() + 1)
                    .sum::<usize>(),
            )
            .unwrap_or(u32::MAX);
            let line_len = u32::try_from(line.len()).unwrap_or(u32::MAX);
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "{class_name}[{type_args_str}].__init__ parameter `{}` expects \
                     `{resolved_type}` but received {mismatch}",
                    param.name
                ),
                basilisk_resolver::Span { start: byte_offset, end: byte_offset + line_len },
                &module.path,
                Some(format!(
                    "Pass a value of type `{resolved_type}` for parameter `{}`",
                    param.name
                )),
                Some(
                    "PEP 696: TypeVar defaults are resolved when the class is subscripted with fewer type arguments"
                        .to_owned(),
                ),
            ));
        }
    }
}
