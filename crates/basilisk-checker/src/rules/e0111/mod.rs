//! BSK-E0111: Constructor call errors via `__init__` method.
//!
//! Detects several categories of constructor call errors when a class defines
//! or inherits `__init__`:
//!
//! 1. **Specialized generic argument mismatch** (L21): Calling `Class[int](1.0)`
//!    when `__init__` expects `x: T` and `T=int`, but `1.0` is `float`.
//!
//! 2. **Self type incompatibility** (L42): Passing a base-class instance where
//!    `Self` in `__init__` demands a subclass instance.
//!
//! 3. **Explicit self annotation mismatch** (L56): `__init__` annotates `self`
//!    as `Class4[int]` but the constructor is called as `Class4[str]()`.
//!
//! 4. **Class-scoped `TypeVar`s in self annotation** (L107): Using class-scoped
//!    type variables in a reordered `self` annotation is invalid.
//!
//! 5. **No custom `__init__` with arguments** (L130): Classes inheriting only
//!    from `object` (no custom `__init__` or `__new__`) cannot accept arguments.

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::shared::{infer_expr_literal_type, is_type_compatible};

use super::Rule;

mod helpers;

use helpers::{
    check_init_method_args, check_self_type_incompatibility, extract_type_args_text,
    has_custom_init_in_bases, is_namedtuple_class, resolve_string_annotation, CODE,
};

/// Emits BSK-E0111 for constructor call errors involving `__init__`.
pub(crate) struct ConstructorCallError;

impl Rule for ConstructorCallError {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Build class info map.
        let class_map: HashMap<&str, &basilisk_resolver::ClassInfo> = module
            .classes
            .iter()
            .map(|c| (c.name.as_str(), c))
            .collect();

        // Build method map: (class_name, method_name) -> Vec<&FunctionInfo>
        let mut method_map: HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>> =
            HashMap::new();
        for func in &module.functions {
            if let Some(ref class_name) = func.class_name {
                method_map
                    .entry((class_name.as_str(), func.name.as_str()))
                    .or_default()
                    .push(func);
            }
        }

        // Collect module-level TypeVar names for class-scoped TypeVar detection.
        let typevar_names: Vec<&str> = module
            .typevar_calls
            .iter()
            .map(|tv| tv.name.as_str())
            .collect();

        // Check 4: Class-scoped `TypeVar`s in self annotation of __init__.
        check_class_scoped_typevars_in_self(
            module,
            source,
            &class_map,
            &method_map,
            &typevar_names,
            diagnostics,
        );

        // Re-parse source to walk call expressions.
        let Ok(parsed) = basilisk_parser::parse_source(source.clone(), path.clone()) else {
            return;
        };

        for stmt in &parsed.ast.body {
            check_stmt(
                stmt,
                source,
                path,
                &class_map,
                &method_map,
                &typevar_names,
                diagnostics,
            );
        }
    }
}

/// Check 4: Detect class-scoped `TypeVar`s used in `self` annotation of `__init__`
/// in a different order from the class's generic params.
fn check_class_scoped_typevars_in_self(
    module: &ResolvedModule,
    source: &str,
    _class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    typevar_names: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for class in &module.classes {
        if class.generic_params.is_empty() {
            continue;
        }

        let class_param_names: Vec<&str> = class
            .generic_params
            .iter()
            .map(|p| p.name.as_str())
            .collect();

        let Some(init_funcs) = method_map.get(&(class.name.as_str(), "__init__")) else {
            continue;
        };

        for init_func in init_funcs {
            // Skip overload decorators — only check the implementation.
            if init_func.decorators.iter().any(|d| d == "overload") {
                continue;
            }

            let Some(self_param) = init_func.parameters.first() else {
                continue;
            };

            let Some(ann_span) = self_param.annotation_span else {
                continue;
            };

            let Some(ann_text) = ann_span.slice_source(source) else {
                continue;
            };

            let resolved = resolve_string_annotation(ann_text.trim());

            // Extract type args from annotation like "Class8[T2, T1]"
            let Some(bracket_start) = resolved.find('[') else {
                continue;
            };
            let Some(bracket_end) = resolved.rfind(']') else {
                continue;
            };

            let ann_class_name = resolved[..bracket_start].trim();
            if ann_class_name != class.name {
                continue;
            }

            let args_str = &resolved[bracket_start + 1..bracket_end];
            let ann_args: Vec<&str> = args_str.split(',').map(str::trim).collect();

            // Check if all annotation args are class-scoped TypeVars.
            let all_class_scoped = ann_args
                .iter()
                .all(|arg| class_param_names.contains(arg) && typevar_names.contains(arg));

            if !all_class_scoped {
                continue;
            }

            // Check if the order differs from the class generic params.
            if ann_args.len() == class_param_names.len()
                && ann_args
                    .iter()
                    .zip(class_param_names.iter())
                    .any(|(a, b)| a != b)
            {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Class-scoped type variables should not be used in the `self` \
                         annotation of `__init__` in class `{}`",
                        class.name
                    ),
                    span: init_func.def_span,
                    path: module.path.clone(),
                    help: Some(
                        "Use function-scoped type variables instead of class-scoped ones \
                         in the `self` annotation"
                            .to_owned(),
                    ),
                    note: Some(format!(
                        "Class `{}` declares generic params `[{}]` but `self` annotation \
                         uses `[{}]`",
                        class.name,
                        class_param_names.join(", "),
                        ann_args.join(", ")
                    )),
                    provenance: None,
                });
            }
        }
    }
}

/// Walk a statement looking for constructor call expressions.
fn check_stmt(
    stmt: &ruff_python_ast::Stmt,
    source: &str,
    path: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    typevar_names: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Stmt;

    match stmt {
        Stmt::Expr(node) => {
            check_expr_recursive(
                &node.value,
                source,
                path,
                class_map,
                method_map,
                typevar_names,
                diagnostics,
            );
        }
        Stmt::Assign(node) => {
            check_expr_recursive(
                &node.value,
                source,
                path,
                class_map,
                method_map,
                typevar_names,
                diagnostics,
            );
        }
        Stmt::AnnAssign(node) => {
            if let Some(val) = node.value.as_deref() {
                check_expr_recursive(
                    val,
                    source,
                    path,
                    class_map,
                    method_map,
                    typevar_names,
                    diagnostics,
                );
            }
        }
        Stmt::Try(try_stmt) => {
            for body in [&try_stmt.body, &try_stmt.orelse, &try_stmt.finalbody] {
                for s in body {
                    check_stmt(
                        s,
                        source,
                        path,
                        class_map,
                        method_map,
                        typevar_names,
                        diagnostics,
                    );
                }
            }
            for handler in &try_stmt.handlers {
                let ruff_python_ast::ExceptHandler::ExceptHandler(h) = handler;
                for s in &h.body {
                    check_stmt(
                        s,
                        source,
                        path,
                        class_map,
                        method_map,
                        typevar_names,
                        diagnostics,
                    );
                }
            }
        }
        _ => {}
    }
}

/// Recursively check expressions for constructor call errors.
fn check_expr_recursive(
    expr: &ruff_python_ast::Expr,
    source: &str,
    path: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    typevar_names: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Expr;

    if let Expr::Call(call) = expr {
        // Recurse into arguments first.
        for arg in &call.arguments.args {
            check_expr_recursive(
                arg,
                source,
                path,
                class_map,
                method_map,
                typevar_names,
                diagnostics,
            );
        }

        check_constructor_call(
            call,
            source,
            path,
            class_map,
            method_map,
            typevar_names,
            diagnostics,
        );
    }
}

/// Check a single call expression for constructor call errors.
fn check_constructor_call(
    call: &ruff_python_ast::ExprCall,
    source: &str,
    path: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    typevar_names: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Expr;

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

            // Check 7: NamedTuple constructor arg count and arg types.
            check_namedtuple_constructor(
                call,
                class_name,
                class_info,
                class_map,
                source,
                path,
                diagnostics,
            );

            // Check 2: Self type incompatibility through inheritance.
            check_self_type_incompatibility(
                call,
                class_name,
                class_info,
                source,
                class_map,
                method_map,
                path,
                diagnostics,
            );
        }
        // Case B: Specialized call like `Class1[int](1.0)` or `Class4[str]()`
        Expr::Subscript(sub) => {
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

            let type_args = extract_type_args_text(&sub.slice, source);

            // Build substitution map.
            let mut substitutions: HashMap<&str, &str> = HashMap::new();
            for (idx, param) in class_info.generic_params.iter().enumerate() {
                if let Some(arg) = type_args.get(idx) {
                    let _ = substitutions.insert(param.name.as_str(), arg.as_str());
                }
            }

            // Check 1: Argument type mismatch after substitution (__init__).
            if let Some(init_funcs) = method_map.get(&(class_name, "__init__")) {
                for init_func in init_funcs {
                    if init_func.decorators.iter().any(|d| d == "overload") {
                        continue;
                    }
                    check_init_method_args(
                        init_func,
                        &substitutions,
                        call,
                        class_name,
                        &type_args,
                        source,
                        path,
                        class_info,
                        typevar_names,
                        diagnostics,
                    );
                }
            }

            // Check generic NamedTuple constructor arg types with substitution.
            check_generic_nt_arg_types(
                call,
                class_name,
                class_info,
                class_map,
                &substitutions,
                source,
                path,
                diagnostics,
            );
        }
        _ => {}
    }
}

/// Check 5: Classes without custom `__init__` that receive arguments.
fn check_no_init_with_args(
    call: &ruff_python_ast::ExprCall,
    class_name: &str,
    class_info: &basilisk_resolver::ClassInfo,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_text_size::Ranged as _;

    // If there are no positional arguments, nothing to check.
    if call.arguments.args.is_empty() && call.arguments.keywords.is_empty() {
        return;
    }

    // NamedTuple classes have a synthesized __new__; do not flag them here.
    if is_namedtuple_class(class_info, class_map) {
        return;
    }

    // Dataclass-decorated classes have synthesized __init__.
    if class_info.is_dataclass {
        return;
    }

    // TypedDict classes have synthesized constructors.
    if class_info.is_typed_dict {
        return;
    }

    // Check if the class itself defines __init__ or __new__.
    if method_map.contains_key(&(class_name, "__init__"))
        || method_map.contains_key(&(class_name, "__new__"))
    {
        return;
    }

    // Check if any base class (other than object) defines __init__ or __new__.
    if has_custom_init_in_bases(class_info, class_map, method_map) {
        return;
    }

    // If any base class is a dataclass (including via @dataclass_transform),
    // the subclass inherits the synthesized __init__.
    for base in &class_info.bases {
        let base_name = base.split('[').next().unwrap_or(base);
        if let Some(base_info) = class_map.get(base_name) {
            if base_info.is_dataclass || base_info.is_typed_dict {
                return;
            }
        }
    }

    // Classes whose bases define __init_subclass__ often accept keyword
    // arguments in their constructors; skip to avoid false positives.
    for base in &class_info.bases {
        let base_name = base.split('[').next().unwrap_or(base);
        if method_map.contains_key(&(base_name, "__init_subclass__")) {
            return;
        }
    }

    let range = call.range();
    let span = Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    };

    diagnostics.push(Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Class `{class_name}` does not define `__init__` or `__new__` and inherits \
             only from `object`; constructor does not accept arguments"
        ),
        span,
        path: path.to_owned(),
        help: Some(format!(
            "Define an `__init__` method on `{class_name}` or one of its base classes"
        )),
        note: None,
        provenance: None,
    });
}

/// Check 7: `NamedTuple` constructor arg count and type validation.
///
/// Class-based `NamedTuple` subclasses have synthesized `__new__` accepting one
/// positional argument per annotated field. Fields with defaults are optional.
/// Extra positional args, unknown keyword args, and type mismatches are errors.
fn check_namedtuple_constructor(
    call: &ruff_python_ast::ExprCall,
    class_name: &str,
    class_info: &basilisk_resolver::ClassInfo,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !is_namedtuple_class(class_info, class_map) {
        return;
    }

    let fields: Vec<&basilisk_resolver::AttributeInfo> = class_info
        .attributes
        .iter()
        .filter(|a| a.has_annotation)
        .collect();

    // Check arg count (too many, too few) — returns true if an error was emitted.
    if check_nt_arg_count(call, class_name, &fields, path, diagnostics) {
        return;
    }

    check_nt_unknown_kwargs(call, class_name, &fields, path, diagnostics);
    check_nt_arg_types(call, class_name, &fields, source, path, diagnostics);
    check_nt_kwarg_types(call, class_name, &fields, source, path, diagnostics);
}

/// Check `NamedTuple` constructor arg count. Returns `true` if a diagnostic was emitted.
fn check_nt_arg_count(
    call: &ruff_python_ast::ExprCall,
    class_name: &str,
    fields: &[&basilisk_resolver::AttributeInfo],
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    use ruff_text_size::Ranged as _;

    let total_fields = fields.len();
    let fields_with_defaults = fields.iter().filter(|a| a.has_value).count();
    let required_fields = total_fields - fields_with_defaults;
    let positional_args = call.arguments.args.len();
    let keyword_args = call.arguments.keywords.len();

    if positional_args > total_fields {
        let range = call.range();
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "`{class_name}` accepts at most {total_fields} positional \
                 argument{}, but {positional_args} {} given",
                if total_fields == 1 { "" } else { "s" },
                if positional_args == 1 { "was" } else { "were" },
            ),
            span: Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            },
            path: path.to_owned(),
            help: Some(format!(
                "`{class_name}` has {total_fields} field{}: {}",
                if total_fields == 1 { "" } else { "s" },
                fields
                    .iter()
                    .map(|f| f.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            )),
            note: None,
            provenance: None,
        });
        return true;
    }

    let covered = positional_args + keyword_args;
    if covered < required_fields {
        let range = call.range();
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "`{class_name}` requires at least {required_fields} argument{}, \
                 but {covered} {} given",
                if required_fields == 1 { "" } else { "s" },
                if covered == 1 { "was" } else { "were" },
            ),
            span: Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            },
            path: path.to_owned(),
            help: Some(format!(
                "Required fields: {}",
                fields
                    .iter()
                    .filter(|f| !f.has_value)
                    .map(|f| f.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            )),
            note: None,
            provenance: None,
        });
        return true;
    }

    false
}

/// Check for unknown keyword arguments in `NamedTuple` constructor.
fn check_nt_unknown_kwargs(
    call: &ruff_python_ast::ExprCall,
    class_name: &str,
    fields: &[&basilisk_resolver::AttributeInfo],
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_text_size::Ranged as _;

    let field_names: std::collections::HashSet<&str> =
        fields.iter().map(|f| f.name.as_str()).collect();
    for kw in &call.arguments.keywords {
        if let Some(ref arg_name) = kw.arg {
            if !field_names.contains(arg_name.as_str()) {
                let range = call.range();
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!("`{class_name}` has no field `{arg_name}`"),
                    span: Span {
                        start: range.start().to_u32(),
                        end: range.end().to_u32(),
                    },
                    path: path.to_owned(),
                    help: Some(format!(
                        "Valid fields: {}",
                        fields
                            .iter()
                            .map(|f| f.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                    )),
                    note: None,
                    provenance: None,
                });
            }
        }
    }
}

/// Check positional argument types in a `NamedTuple` constructor call.
fn check_nt_arg_types(
    call: &ruff_python_ast::ExprCall,
    class_name: &str,
    fields: &[&basilisk_resolver::AttributeInfo],
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_text_size::Ranged as _;

    for (idx, arg) in call.arguments.args.iter().enumerate() {
        let Some(field) = fields.get(idx) else {
            break;
        };
        let ann = field
            .annotation_span
            .and_then(|sp| crate::span_util::slice_span(source, sp));
        let Some(ann) = ann else {
            continue;
        };
        let ann = ann.trim();
        let arg_type = infer_expr_literal_type(arg).map(str::to_owned);
        let Some(arg_type_name) = arg_type else {
            continue;
        };
        if !is_type_compatible(&arg_type_name, ann) {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Argument {idx} to `{class_name}()` has type `{arg_type_name}`, \
                     expected `{ann}` for field `{}`",
                    field.name
                ),
                span: crate::span_util::text_range_to_span(arg.range()),
                path: path.to_owned(),
                help: None,
                note: None,
                provenance: None,
            });
        }
    }
}

/// Check keyword argument types in a `NamedTuple` constructor call.
fn check_nt_kwarg_types(
    call: &ruff_python_ast::ExprCall,
    class_name: &str,
    fields: &[&basilisk_resolver::AttributeInfo],
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_text_size::Ranged as _;

    for kw in &call.arguments.keywords {
        let Some(ref arg_name) = kw.arg else {
            continue;
        };
        let Some(field) = fields.iter().find(|f| f.name == arg_name.as_str()) else {
            continue;
        };
        let ann = field
            .annotation_span
            .and_then(|sp| crate::span_util::slice_span(source, sp));
        let Some(ann) = ann else {
            continue;
        };
        let ann = ann.trim();
        let arg_type = infer_expr_literal_type(&kw.value).map(str::to_owned);
        let Some(arg_type_name) = arg_type else {
            continue;
        };
        if !is_type_compatible(&arg_type_name, ann) {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Keyword argument `{arg_name}` to `{class_name}()` has type \
                     `{arg_type_name}`, expected `{ann}`",
                ),
                span: crate::span_util::text_range_to_span(kw.range()),
                path: path.to_owned(),
                help: None,
                note: None,
                provenance: None,
            });
        }
    }
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
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Unknown field `{arg_name}` in constructor of dataclass `{class_name}`"
                ),
                span: crate::span_util::text_range_to_span(kw.range()),
                path: path.to_owned(),
                help: Some(format!(
                    "`{class_name}` has fields: {}",
                    known_fields.iter().copied().collect::<Vec<_>>().join(", ")
                )),
                note: None,
                provenance: None,
            });
        }
    }
}

/// Recursively collect all dataclass field names including inherited ones.
fn collect_dataclass_fields<'a>(
    class_info: &'a basilisk_resolver::ClassInfo,
    class_map: &HashMap<&str, &'a basilisk_resolver::ClassInfo>,
    fields: &mut std::collections::HashSet<&'a str>,
) {
    for attr in &class_info.attributes {
        let _ = fields.insert(attr.name.as_str());
    }
    for base_name in &class_info.bases {
        if let Some(base_info) = class_map.get(base_name.as_str()) {
            collect_dataclass_fields(base_info, class_map, fields);
        }
    }
}

/// Check generic `NamedTuple` constructor arg types with type substitutions.
///
/// For `Property[str]("", 3.1)` where `Property(NamedTuple, Generic[T])` has
/// field `value: T`, substitutes `T=str` and checks that `3.1` is not `str`.
#[expect(
    clippy::too_many_arguments,
    reason = "constructor check requires full context"
)]
fn check_generic_nt_arg_types(
    call: &ruff_python_ast::ExprCall,
    class_name: &str,
    class_info: &basilisk_resolver::ClassInfo,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    substitutions: &HashMap<&str, &str>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_text_size::Ranged as _;

    if !is_namedtuple_class(class_info, class_map) || substitutions.is_empty() {
        return;
    }

    let fields: Vec<&basilisk_resolver::AttributeInfo> = class_info
        .attributes
        .iter()
        .filter(|a| a.has_annotation)
        .collect();

    for (idx, arg) in call.arguments.args.iter().enumerate() {
        let Some(field) = fields.get(idx) else {
            break;
        };
        let ann = field
            .annotation_span
            .and_then(|sp| crate::span_util::slice_span(source, sp));
        let Some(ann) = ann else {
            continue;
        };
        let ann = ann.trim();
        // Apply type substitutions (e.g. T → str).
        let resolved_ann = substitutions.get(ann).unwrap_or(&ann);
        let arg_type = infer_expr_literal_type(arg).map(str::to_owned);
        let Some(arg_type_name) = arg_type else {
            continue;
        };
        if !is_type_compatible(&arg_type_name, resolved_ann) {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Argument {idx} to `{class_name}[...]()` has type `{arg_type_name}`, \
                     expected `{resolved_ann}` for field `{}`",
                    field.name
                ),
                span: crate::span_util::text_range_to_span(arg.range()),
                path: path.to_owned(),
                help: None,
                note: None,
                provenance: None,
            });
        }
    }
}
