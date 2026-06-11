//! Implements [BSK-E0142] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! `dataclass_transform` field-specifier `converter` support (PEP 681).
//!
//! When a field specifier call carries `converter=fn`, the type accepted by
//! the synthesized `__init__` parameter (and by attribute assignment) is the
//! type of the converter's first positional parameter — not the field's
//! declared type.  This module checks:
//!
//! 1. The converter accepts at least one positional argument.
//! 2. `default=` / `default_factory=` values are assignable to the converter input.
//! 3. Constructor positional arguments are assignable to each field's input type.
//! 4. Attribute assignments on instances are assignable to the field's input type.

use std::collections::HashMap;

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::rules::shared::{ann_str, infer_expr_literal_type, is_type_compatible, parse_module};
use crate::span_util::{slice_span, text_range_to_span};

use super::helpers::CODE;

/// A field of a transform subclass, with the type its synthesized `__init__`
/// parameter accepts.
struct FieldSpec {
    name: String,
    /// Type accepted by `__init__` and attribute assignment.  `None` when the
    /// converter input could not be resolved (assume anything is accepted).
    input_type: Option<String>,
}

/// Per-module context for converter checking.
struct ConverterCtx<'a> {
    module: &'a ResolvedModule,
    /// Field-specifier function names declared via `field_specifiers=(...)`.
    specifier_names: Vec<String>,
    /// Map from transform-subclass name to its ordered fields.
    class_fields: HashMap<String, Vec<FieldSpec>>,
}

/// Entry point: run all converter-related checks for classes inheriting from a
/// class-applied `@dataclass_transform` base.
pub(super) fn check_converters(
    module: &ResolvedModule,
    transform_subclasses: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(parsed) = parse_module(module) else {
        return;
    };
    let specifier_names = collect_field_specifier_names(&parsed.ast.body);
    if specifier_names.is_empty() {
        return;
    }

    let mut ctx = ConverterCtx {
        module,
        specifier_names,
        class_fields: HashMap::new(),
    };

    for stmt in &parsed.ast.body {
        let Stmt::ClassDef(cls) = stmt else {
            continue;
        };
        if !transform_subclasses.contains(&cls.name.as_str()) {
            continue;
        }
        let fields = check_class_fields(cls, &ctx, &parsed.ast.body, diagnostics);
        let _ = ctx.class_fields.insert(cls.name.to_string(), fields);
    }

    check_constructor_calls(&parsed.ast.body, &ctx, diagnostics);
    check_attr_assignments(&ctx, diagnostics);
}

/// Extract field-specifier names from `@dataclass_transform(field_specifiers=(a, b))`
/// decorators anywhere in the module (class, function, or metaclass form).
fn collect_field_specifier_names(stmts: &[Stmt]) -> Vec<String> {
    let mut names = Vec::new();
    let decorator_lists = stmts.iter().filter_map(|stmt| match stmt {
        Stmt::ClassDef(c) => Some(&c.decorator_list),
        Stmt::FunctionDef(f) => Some(&f.decorator_list),
        _ => None,
    });
    for decorators in decorator_lists {
        for dec in decorators {
            let Expr::Call(call) = &dec.expression else {
                continue;
            };
            if !matches!(call.func.as_ref(), Expr::Name(n) if n.id.as_str() == "dataclass_transform")
            {
                continue;
            }
            for kw in &call.arguments.keywords {
                if kw
                    .arg
                    .as_ref()
                    .is_none_or(|a| a.as_str() != "field_specifiers")
                {
                    continue;
                }
                if let Expr::Tuple(tuple) = &kw.value {
                    names.extend(tuple.elts.iter().filter_map(|e| match e {
                        Expr::Name(n) => Some(n.id.to_string()),
                        _ => None,
                    }));
                }
            }
        }
    }
    names
}

/// Check the field-specifier calls of one transform subclass and build its
/// ordered [`FieldSpec`] list.
fn check_class_fields(
    cls: &ruff_python_ast::StmtClassDef,
    ctx: &ConverterCtx<'_>,
    module_stmts: &[Stmt],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<FieldSpec> {
    let mut fields = Vec::new();
    for stmt in &cls.body {
        let Stmt::AnnAssign(ann) = stmt else {
            continue;
        };
        let Expr::Name(target) = ann.target.as_ref() else {
            continue;
        };
        let declared_type = ann_str(&ann.annotation);

        let Some(Expr::Call(call)) = ann.value.as_deref() else {
            fields.push(FieldSpec {
                name: target.id.to_string(),
                input_type: Some(declared_type),
            });
            continue;
        };
        let is_specifier = matches!(
            call.func.as_ref(),
            Expr::Name(n) if ctx.specifier_names.iter().any(|s| s == n.id.as_str())
        );
        if !is_specifier {
            fields.push(FieldSpec {
                name: target.id.to_string(),
                input_type: Some(declared_type),
            });
            continue;
        }

        let converter_input = check_specifier_call(call, ctx, module_stmts, diagnostics);
        fields.push(FieldSpec {
            name: target.id.to_string(),
            input_type: converter_input.or(Some(declared_type)),
        });
    }
    fields
}

/// Validate one field-specifier call's `converter`/`default`/`default_factory`
/// keywords.  Returns the resolved converter input type, if any.
fn check_specifier_call(
    call: &ruff_python_ast::ExprCall,
    ctx: &ConverterCtx<'_>,
    module_stmts: &[Stmt],
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let converter_kw = call
        .arguments
        .keywords
        .iter()
        .find(|kw| kw.arg.as_ref().is_some_and(|a| a.as_str() == "converter"))?;
    let Expr::Name(converter_name) = &converter_kw.value else {
        return None;
    };

    let input = converter_input_type(converter_name.id.as_str(), module_stmts);
    match &input {
        ConverterInput::NoPositional => {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Converter `{}` must accept at least one positional argument",
                    converter_name.id
                ),
                text_range_to_span(converter_kw.range()),
                &ctx.module.path,
                Some(
                    "A dataclass_transform field converter is called with the field value \
                     as a single positional argument"
                        .to_owned(),
                ),
                Some("See the typing spec: dataclasses.html#converters".to_owned()),
            ));
            None
        }
        ConverterInput::Unknown => None,
        ConverterInput::Type(input_type) => {
            check_default_kwargs(call, input_type, ctx, module_stmts, diagnostics);
            Some(input_type.clone())
        }
    }
}

/// Check `default=` and `default_factory=` values against the converter input type.
fn check_default_kwargs(
    call: &ruff_python_ast::ExprCall,
    input_type: &str,
    ctx: &ConverterCtx<'_>,
    module_stmts: &[Stmt],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for kw in &call.arguments.keywords {
        let Some(kw_name) = kw.arg.as_ref() else {
            continue;
        };
        let value_type = match kw_name.as_str() {
            "default" => infer_expr_literal_type(&kw.value).map(str::to_owned),
            "default_factory" => match &kw.value {
                Expr::Name(factory) => factory_return_type(factory.id.as_str(), module_stmts),
                _ => None,
            },
            _ => continue,
        };
        let Some(value_type) = value_type else {
            continue;
        };
        if !input_assignable(&value_type, input_type) {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "`{kw_name}` of type `{value_type}` is not assignable to the converter's \
                     input type `{input_type}`"
                ),
                text_range_to_span(kw.range()),
                &ctx.module.path,
                Some(format!(
                    "The default value is passed through the converter, which accepts \
                     `{input_type}`"
                )),
                None,
            ));
        }
    }
}

/// The resolved input type of a converter callable.
enum ConverterInput {
    /// First positional parameter type (or union of overload first-param types).
    Type(String),
    /// The callable cannot accept any positional argument.
    NoPositional,
    /// Not resolvable in this module (builtins, attributes, etc.).
    Unknown,
}

/// Resolve the type a converter accepts as its first positional argument.
fn converter_input_type(name: &str, module_stmts: &[Stmt]) -> ConverterInput {
    let mut first_param_types = Vec::new();
    let mut found_signature = false;
    let mut has_overloads = false;

    for stmt in module_stmts {
        match stmt {
            Stmt::FunctionDef(func) if func.name.as_str() == name => {
                let is_overload = is_overload_decorated(&func.decorator_list);
                if has_overloads && !is_overload {
                    continue;
                }
                if is_overload && !has_overloads {
                    has_overloads = true;
                    first_param_types.clear();
                }
                found_signature = true;
                match first_positional_type(&func.parameters, false) {
                    FirstParam::Type(t) => first_param_types.push(t),
                    FirstParam::Unannotated => return ConverterInput::Unknown,
                    FirstParam::Missing => {
                        if !is_overload {
                            return ConverterInput::NoPositional;
                        }
                    }
                }
            }
            Stmt::ClassDef(cls) if cls.name.as_str() == name => {
                return class_init_input_type(cls);
            }
            _ => {}
        }
    }

    match (found_signature, first_param_types.is_empty()) {
        (false, _) => ConverterInput::Unknown,
        (true, true) => ConverterInput::NoPositional,
        (true, false) => {
            first_param_types.dedup();
            ConverterInput::Type(first_param_types.join(" | "))
        }
    }
}

/// Resolve the converter input type for a class converter from its `__init__`
/// overload signatures (skipping `self`).
fn class_init_input_type(cls: &ruff_python_ast::StmtClassDef) -> ConverterInput {
    let mut first_param_types = Vec::new();
    let mut has_overloads = false;

    for stmt in &cls.body {
        let Stmt::FunctionDef(func) = stmt else {
            continue;
        };
        if func.name.as_str() != "__init__" {
            continue;
        }
        let is_overload = is_overload_decorated(&func.decorator_list);
        if is_overload && !has_overloads {
            has_overloads = true;
            first_param_types.clear();
        }
        if has_overloads && !is_overload {
            continue;
        }
        match first_positional_type(&func.parameters, true) {
            FirstParam::Type(t) => first_param_types.push(t),
            FirstParam::Unannotated => return ConverterInput::Unknown,
            FirstParam::Missing => {}
        }
    }

    if first_param_types.is_empty() {
        ConverterInput::Unknown
    } else {
        first_param_types.dedup();
        ConverterInput::Type(first_param_types.join(" | "))
    }
}

/// The first positional parameter of a signature.
enum FirstParam {
    Type(String),
    Unannotated,
    Missing,
}

/// Find the annotation of the first positional parameter (`skip_self` for methods).
/// A bare `*args: T` vararg counts as accepting positional arguments of type `T`.
fn first_positional_type(params: &ruff_python_ast::Parameters, skip_self: bool) -> FirstParam {
    let positional = params
        .posonlyargs
        .iter()
        .chain(params.args.iter())
        .nth(usize::from(skip_self));
    if let Some(param) = positional {
        return param
            .parameter
            .annotation
            .as_deref()
            .map_or(FirstParam::Unannotated, |ann| {
                FirstParam::Type(ann_str(ann))
            });
    }
    if let Some(vararg) = &params.vararg {
        return vararg
            .annotation
            .as_deref()
            .map_or(FirstParam::Unannotated, |ann| {
                FirstParam::Type(ann_str(ann))
            });
    }
    FirstParam::Missing
}

/// `true` when the decorator list contains `@overload`.
fn is_overload_decorated(decorators: &[ruff_python_ast::Decorator]) -> bool {
    decorators
        .iter()
        .any(|d| matches!(&d.expression, Expr::Name(n) if n.id.as_str() == "overload"))
}

/// `default_factory` return type for builtins and local functions.
fn factory_return_type(name: &str, module_stmts: &[Stmt]) -> Option<String> {
    match name {
        "str" | "int" | "float" | "bool" | "bytes" | "list" | "dict" | "set" | "tuple" => {
            Some(name.to_owned())
        }
        _ => module_stmts.iter().find_map(|stmt| match stmt {
            Stmt::FunctionDef(func) if func.name.as_str() == name => {
                func.returns.as_deref().map(ann_str)
            }
            _ => None,
        }),
    }
}

/// Check positional constructor-call arguments against each field's input type.
fn check_constructor_calls(
    stmts: &[Stmt],
    ctx: &ConverterCtx<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    basilisk_resolver::visit_calls(stmts, &mut |call| {
        let Expr::Name(callee) = call.func.as_ref() else {
            return;
        };
        let Some(fields) = ctx.class_fields.get(callee.id.as_str()) else {
            return;
        };
        for (idx, arg) in call.arguments.args.iter().enumerate() {
            let Some(field) = fields.get(idx) else {
                break;
            };
            let Some(expected) = field.input_type.as_deref() else {
                continue;
            };
            let Some(actual) = infer_expr_literal_type(arg) else {
                continue;
            };
            if !input_assignable(actual, expected) {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Argument for field `{}` of `{}` has type `{actual}`, but its \
                         converter accepts `{expected}`",
                        field.name, callee.id
                    ),
                    text_range_to_span(arg.range()),
                    &ctx.module.path,
                    None,
                    None,
                ));
            }
        }
    });
}

/// Check `instance.field = value` assignments against the field's input type.
fn check_attr_assignments(ctx: &ConverterCtx<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let source = &ctx.module.source;

    // Map module-level instance variables to their transform subclass.
    let mut instances: HashMap<&str, &str> = HashMap::new();
    for var in &ctx.module.module_vars {
        let Some(rhs_span) = var.rhs_span else {
            continue;
        };
        let Some(rhs_text) = slice_span(source, rhs_span) else {
            continue;
        };
        let callee = rhs_text.split('(').next().unwrap_or("").trim();
        if ctx.class_fields.contains_key(callee) {
            let _ = instances.insert(var.name.as_str(), callee);
        }
    }

    for assign in &ctx.module.module_attr_assignments {
        let Some(class_name) = instances.get(assign.object_name.as_str()) else {
            continue;
        };
        let Some(fields) = ctx.class_fields.get(*class_name) else {
            continue;
        };
        let Some(field) = fields.iter().find(|f| f.name == assign.attr_name) else {
            continue;
        };
        let Some(expected) = field.input_type.as_deref() else {
            continue;
        };
        let Some(rhs_span) = assign.rhs_span else {
            continue;
        };
        let Some(actual) = slice_span(source, rhs_span).and_then(literal_type_of_text) else {
            continue;
        };
        if !input_assignable(actual, expected) {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Cannot assign `{actual}` to field `{}` of `{class_name}`: its \
                     converter accepts `{expected}`",
                    assign.attr_name
                ),
                assign.target_span,
                &ctx.module.path,
                None,
                None,
            ));
        }
    }
}

/// Classify a source-text literal into a type name.
fn literal_type_of_text(text: &str) -> Option<&'static str> {
    let text = text.trim();
    if text.starts_with('"') || text.starts_with('\'') {
        return Some("str");
    }
    if text.starts_with("b\"") || text.starts_with("b'") {
        return Some("bytes");
    }
    if text == "True" || text == "False" {
        return Some("bool");
    }
    if text == "None" {
        return Some("None");
    }
    if text.parse::<i64>().is_ok() {
        return Some("int");
    }
    if text.ends_with('j') && text[..text.len() - 1].parse::<f64>().is_ok() {
        return Some("complex");
    }
    if text.parse::<f64>().is_ok() {
        return Some("float");
    }
    None
}

/// `actual` is acceptable for `expected` — like [`is_type_compatible`], but a
/// bare container literal type also matches a parameterized union member
/// (`list` matches `str | list[str]`).
fn input_assignable(actual: &str, expected: &str) -> bool {
    if is_type_compatible(actual, expected) {
        return true;
    }
    expected
        .split('|')
        .map(str::trim)
        .any(|part| part.split('[').next().unwrap_or(part).trim() == actual)
}
