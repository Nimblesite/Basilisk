//! `constructors_call_init`: Constructor call errors via `__init__` method.
//!
//! Detects several categories of constructor call errors when a class defines
//! or inherits `__init__`:
//!
//! 1. **Specialized generic argument mismatch** (L21): Calling `Class[int](1.0)`
//!    when `__init__` expects `x: T` and `T=int`, but `1.0` is `float`.
//!
//! 2. **Explicit self annotation mismatch** (L56): `__init__` annotates `self`
//!    as `Class4[int]` but the constructor is called as `Class4[str]()`.
//!
//! 3. **Class-scoped `TypeVar`s in self annotation** (L107): Using class-scoped
//!    type variables in a reordered `self` annotation is invalid.
//!
//! 4. **No custom `__init__` with arguments** (L130): Classes inheriting only
//!    from `object` (no custom `__init__` or `__new__`) cannot accept arguments.

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};

use super::Rule;

mod helpers;

use helpers::{check_init_method_args, CODE};

/// Emits `constructors_call_init` for constructor call errors involving `__init__`.
pub(crate) struct ConstructorCallError;

impl Rule for ConstructorCallError {
    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        super::check_with_own_types(self, module, ctx, diagnostics);
    }

    fn check_with_types(
        &self,
        module: &ResolvedModule,
        types: &super::shared::module_types::ModuleTypes<'_>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;
        let path = &module.path;

        // Build class info map.
        let class_map: HashMap<&str, &basilisk_resolver::ClassInfo> =
            basilisk_resolver::name_lookup(&module.classes);

        // Build method map: (class_name, method_name) -> Vec<&FunctionInfo>
        let method_map = super::shared::method_name_map(&module.functions);

        // Collect module-level TypeVar names for class-scoped TypeVar detection.
        let typevar_names: Vec<&str> = basilisk_resolver::collect_names(&module.typevar_calls);

        // Check 4: Class-scoped `TypeVar`s in self annotation of __init__.
        check_class_scoped_typevars_in_self(
            module,
            source,
            &class_map,
            &method_map,
            &typevar_names,
            diagnostics,
        );

        // Every call in every expression position, from the module's one
        // shared walk ([NARROWPLAN-CALLSITES]).
        let Some(oracle) = types.oracle() else {
            return;
        };
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };
        let index = super::shared::ExprIndex::build(&parsed.ast);
        let ctx = Ctx {
            module,
            path,
            class_map: &class_map,
            method_map: &method_map,
            typevar_names: &typevar_names,
            index: &index,
        };
        for call in oracle.calls() {
            check_constructor_call(call, &ctx, diagnostics);
        }
    }
}

/// Check 4: Detect class-scoped `TypeVar`s used in `self` annotation of `__init__`
/// in a different order from the class's generic params.
fn check_class_scoped_typevars_in_self(
    _module: &ResolvedModule,
    _source: &str,
    _class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    _method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    _typevar_names: &[&str],
    _diagnostics: &mut Vec<Diagnostic>,
) {
    // ######################################################################
    // # DELETED BODY. DO NOT RESTORE IT AND DO NOT RETURN WITHOUT          #
    // # CHECKING IN ITS PLACE.                                             #
    // #                                                                    #
    // # A second hand-written parser over the `self` annotation's text:    #
    // #                                                                    #
    // #   let resolved = resolve_string_annotation(ann_text.trim());       #
    // #   let bracket_start = resolved.find('[')?;                         #
    // #   let bracket_end   = resolved.rfind(']')?;                        #
    // #   let ann_class_name = resolved[..bracket_start].trim();           #
    // #   let ann_args = resolved[bracket_start+1..bracket_end]            #
    // #                      .split(',').map(str::trim);                   #
    // #                                                                    #
    // # It then compared those STRINGS against the class's declared type   #
    // # parameter names to decide whether the `self` annotation reorders   #
    // # them. Both the parse and the comparison are spelling: a type       #
    // # parameter reached under any other name, an annotation split across #
    // # lines, or a nested subscript containing a comma each broke it.     #
    // #                                                                    #
    // # A `self` annotation is an `Expr`. Its type arguments are the       #
    // # subscript's slice elements, and each resolves to a TypeVar symbol. #
    // #                                                                    #
    // # Pinned by: tests/no_type_spelling_surgery_tests.rs                 #
    // ######################################################################
    panic!(
        "basilisk-checker: `check_class_scoped_typevars_in_self` was DELETED because \
         it hand-parsed the `self` annotation from TEXT (`find('[')`, `rfind(']')`, \
         `split(',')`) and compared the resulting STRINGS against type-parameter \
         names. It panics because the real implementation — reading the annotation's \
         `Expr::Subscript` and resolving each slice element to a TypeVar symbol — \
         DOES NOT EXIST YET. Do not restore the parser and do not return without \
         checking in its place."
    )
}

/// Bundle of state threaded through all E0111 statement/expression walkers.
struct Ctx<'a> {
    module: &'a ResolvedModule,
    path: &'a str,
    class_map: &'a HashMap<&'a str, &'a basilisk_resolver::ClassInfo>,
    method_map: &'a HashMap<(&'a str, &'a str), Vec<&'a basilisk_resolver::FunctionInfo>>,
    typevar_names: &'a [&'a str],
    index: &'a super::shared::ExprIndex<'a>,
}

/// Check a single call expression for constructor call errors.
fn check_constructor_call(
    call: &ruff_python_ast::ExprCall,
    ctx: &Ctx<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Expr;
    let Ctx {
        path,
        class_map,
        method_map,
        ..
    } = *ctx;

    match call.func.as_ref() {
        // Case A: Simple class call like `Class11(1)` or `Class3(Class2(None))`
        Expr::Name(name_node) => {
            let class_name = name_node.id.as_str();
            let Some(class_info) = class_map.get(class_name) else {
                return;
            };

            // Check 5: No custom __init__ with arguments.
            check_no_init_with_args(
                call,
                class_name,
                class_info,
                class_map,
                method_map,
                ctx.module,
                path,
                diagnostics,
            );

            // Check 6: Unknown kwargs to dataclass/transform class constructors.
            check_dataclass_unknown_kwargs(
                call,
                class_name,
                class_info,
                class_map,
                path,
                diagnostics,
            );
        }
        // Case B: Specialized call like `Class1[int](1.0)` or `Class4[str]()`
        Expr::Subscript(sub) => check_subscript_constructor(call, sub, ctx, diagnostics),
        _ => {}
    }
}

/// Handles the `ClassName[TypeArgs](args)` form of constructor call.
fn check_subscript_constructor(
    call: &ruff_python_ast::ExprCall,
    sub: &ruff_python_ast::ExprSubscript,
    ctx: &Ctx<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Expr;
    let Ctx {
        module,
        class_map,
        method_map,
        typevar_names,
        index,
        ..
    } = *ctx;

    let Expr::Name(class_name_node) = sub.value.as_ref() else {
        return;
    };
    let class_name = class_name_node.id.as_str();
    let Some(class_info) = class_map.get(class_name) else {
        return;
    };
    if class_info.generic_params.is_empty() {
        return;
    }

    if let Some(init_funcs) = method_map.get(&(class_name, "__init__")) {
        for init_func in init_funcs {
            check_init_method_args(
                init_func,
                call,
                class_name,
                class_info,
                typevar_names,
                module,
                index,
                diagnostics,
            );
        }
    }
}

/// Check 5: Classes without custom `__init__` that receive arguments.
// ##########################################################################
// # DELETED BODY — `check_no_init_with_args`. DO NOT RESTORE IT AND DO NOT #
// # REPLACE IT WITH A PLACEHOLDER THAT RETURNS WITHOUT CHECKING.           #
// #                                                                        #
// # The rule fires when a class that inherits only from `object` is called #
// # with arguments. Every gate that decided "inherits only from `object`"  #
// # read SOURCE TEXT:                                                      #
// #                                                                        #
// #   let base_name = base.split('[').next().unwrap_or(base);              #
// #   class_map.get(base_name)                    // dataclass base?       #
// #   method_map.contains_key(&(base_name, "__init_subclass__"))           #
// #                                                                        #
// # plus `has_custom_init_in_bases` and `has_unresolved_base`, both of     #
// # which were themselves built on the same split-at-a-bracket base names  #
// # and on `base_name != "object"`.                                        #
// #                                                                        #
// # A subscripted base written with a space, a base reached under an       #
// # alias, and two unrelated classes sharing a rendered name each produced #
// # the wrong gate — and the gate is what decides whether a diagnostic is  #
// # emitted at all. So this rule both fired on valid code and stayed       #
// # silent on invalid code, depending only on how the source was spelled.  #
// #                                                                        #
// # "Which classes does this one inherit from, and do any of them define   #
// # a constructor?" is a question about RESOLVED class symbols.            #
// #                                                                        #
// # Pinned by: tests/no_type_spelling_surgery_tests.rs                     #
// ##########################################################################

/// DELETED — panics. The signature survives only so its caller stays visible
/// as the rebuild map; see the banner above.
fn check_no_init_with_args(
    _call: &ruff_python_ast::ExprCall,
    _class_name: &str,
    _class_info: &basilisk_resolver::ClassInfo,
    _class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    _method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    _module: &ResolvedModule,
    _path: &str,
    _diagnostics: &mut Vec<Diagnostic>,
) {
    panic!(
        "basilisk-checker: `check_no_init_with_args` was DELETED because every gate \
         deciding whether a class inherits only from `object` read SOURCE TEXT — base \
         heads split at `[`, class and method lookup keyed on the resulting strings, \
         and the top type recognised as the literal `\"object\"`. It panics because \
         the real implementation — resolving each base expression to a class symbol \
         through the binding table — DOES NOT EXIST YET. Do not restore the splits and \
         do not return without checking in its place."
    )
}

/// Check 6: Detect unknown keyword arguments passed to dataclass/transform constructors.
///
/// Dataclass constructors accept kwargs matching their field names. Passing an
/// unknown kwarg (not matching any field) is an error.
fn check_dataclass_unknown_kwargs(
    call: &ruff_python_ast::ExprCall,
    class_name: &str,
    class_info: &basilisk_resolver::ClassInfo,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_text_size::Ranged as _;

    if !class_info.is_dataclass {
        return;
    }

    if call.arguments.keywords.is_empty() {
        return;
    }

    // Collect fields from this class and all base classes (for inherited fields).
    let mut known_fields: std::collections::HashSet<&str> = std::collections::HashSet::new();
    collect_dataclass_fields(class_info, class_map, &mut known_fields);

    if known_fields.is_empty() {
        return;
    }

    for kw in &call.arguments.keywords {
        let Some(arg_name) = kw.arg.as_ref() else {
            continue; // **kwargs unpacking
        };
        if !known_fields.contains(arg_name.as_str()) {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!("Unknown field `{arg_name}` in constructor of dataclass `{class_name}`"),
                crate::span_util::text_range_to_span(kw.range()),
                path,
                Some(format!(
                    "`{class_name}` has fields: {}",
                    known_fields.iter().copied().collect::<Vec<_>>().join(", ")
                )),
                None,
            ));
        }
    }
}

/// Recursively collect all dataclass field names including inherited ones.
fn collect_dataclass_fields<'a>(
    class_info: &'a basilisk_resolver::ClassInfo,
    class_map: &HashMap<&str, &'a basilisk_resolver::ClassInfo>,
    fields: &mut std::collections::HashSet<&'a str>,
) {
    let mut visited = std::collections::HashSet::new();
    let _ = visited.insert(class_info.name.as_str());
    dataclass_fields_walk(class_info, class_map, fields, &mut visited);
}

/// Recursive body of [`collect_dataclass_fields`]; `visited` breaks base-name
/// cycles (GitHub #278).
fn dataclass_fields_walk<'a>(
    class_info: &'a basilisk_resolver::ClassInfo,
    class_map: &HashMap<&str, &'a basilisk_resolver::ClassInfo>,
    fields: &mut std::collections::HashSet<&'a str>,
    visited: &mut std::collections::HashSet<&'a str>,
) {
    for attr in &class_info.attributes {
        let _ = fields.insert(attr.name.as_str());
    }
    for base_name in &class_info.bases {
        if visited.insert(base_name.as_str()) {
            if let Some(base_info) = class_map.get(base_name.as_str()) {
                dataclass_fields_walk(base_info, class_map, fields, visited);
            }
        }
    }
}
