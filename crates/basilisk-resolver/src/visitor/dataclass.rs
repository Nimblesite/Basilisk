//! Dataclass visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtClassDef};

use crate::scope::{AttributeInfo, ClassInfo, FunctionInfo};

use super::annotations::annotation_is_kw_only;
use super::class_info_ext::{
    decorator_name, expr_simple_name, parse_dataclass_transform_decorator, DcTransformFactory,
    FieldSpecOverload,
};

pub(super) fn apply_dataclass_transform(
    stmts: &[Stmt],
    classes: &mut [ClassInfo],
    functions: &[FunctionInfo],
) {
    let factories = collect_dc_transform_factories(stmts);
    let class_factories = collect_dc_transform_class_factories(stmts);
    let metaclass_factories = collect_dc_transform_metaclass_factories(stmts);

    if factories.is_empty() && class_factories.is_empty() && metaclass_factories.is_empty() {
        return;
    }
    let mut specifier_overloads: std::collections::HashMap<&str, Vec<FieldSpecOverload>> =
        std::collections::HashMap::new();
    for factory in factories
        .iter()
        .chain(class_factories.iter())
        .chain(metaclass_factories.iter())
    {
        for spec_name in &factory.field_specifier_names {
            if specifier_overloads.contains_key(spec_name.as_str()) {
                continue;
            }
            let overloads = build_field_specifier_overloads(stmts, spec_name, functions);
            let _ = specifier_overloads.insert(spec_name.as_str(), overloads);
        }
    }

    // Apply function-based factories (classes decorated by factory functions).
    for cls in classes.iter_mut() {
        if let Some(factory) = find_matching_factory(stmts, &cls.name, &factories) {
            cls.is_dataclass = true;
            cls.is_dataclass_kw_only = factory.kw_only_default;
            cls.is_dataclass_frozen = factory.frozen_default;
            cls.is_dataclass_order = factory.order_default;

            if let Some(class_def) = find_class_def(stmts, &cls.name) {
                apply_class_decorator_overrides(class_def, &factory.name, cls);
                resolve_transform_field_attrs(
                    class_def,
                    &mut cls.attributes,
                    &factory.field_specifier_names,
                    &specifier_overloads,
                    factory.kw_only_default,
                );
            }
        }
    }

    // Apply class-based `@dataclass_transform` (subclasses of the decorated
    // base class become dataclasses).
    apply_class_or_metaclass_factories(stmts, classes, &class_factories, &specifier_overloads);

    // Apply metaclass-based `@dataclass_transform` (classes whose metaclass
    // is the decorated metaclass become dataclasses).
    apply_metaclass_transform_factories(stmts, classes, &metaclass_factories, &specifier_overloads);
}

/// Collect `@dataclass_transform(...)` decorated functions at module level.
pub(super) fn collect_dc_transform_factories(stmts: &[Stmt]) -> Vec<DcTransformFactory> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::FunctionDef(func) = stmt else {
            continue;
        };
        for dec in &func.decorator_list {
            if let Some(parsed) = parse_dataclass_transform_decorator(&dec.expression) {
                out.push(DcTransformFactory {
                    name: func.name.to_string(),
                    kw_only_default: parsed.kw_only_default,
                    frozen_default: parsed.frozen_default,
                    order_default: parsed.order_default,
                    field_specifier_names: parsed.field_specifier_names,
                });
            }
        }
    }
    out
}

/// Collect `@dataclass_transform(...)` decorated classes (non-metaclass) at module level.
///
/// When `@dataclass_transform` is applied to a base class, all subclasses of
/// that class are treated as dataclasses.
fn collect_dc_transform_class_factories(stmts: &[Stmt]) -> Vec<DcTransformFactory> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else {
            continue;
        };
        // Skip metaclasses (classes inheriting from `type`).
        let is_metaclass = cls.arguments.as_ref().is_some_and(|args| {
            args.args
                .iter()
                .any(|expr| matches!(expr, Expr::Name(n) if n.id.as_str() == "type"))
        });
        if is_metaclass {
            continue;
        }
        for dec in &cls.decorator_list {
            if let Some(parsed) = parse_dataclass_transform_decorator(&dec.expression) {
                out.push(DcTransformFactory {
                    name: cls.name.to_string(),
                    kw_only_default: parsed.kw_only_default,
                    frozen_default: parsed.frozen_default,
                    order_default: parsed.order_default,
                    field_specifier_names: parsed.field_specifier_names,
                });
            }
        }
    }
    out
}

/// Collect `@dataclass_transform(...)` decorated metaclasses at module level.
///
/// When `@dataclass_transform` is applied to a metaclass, all classes using
/// that metaclass (directly or transitively) are treated as dataclasses.
fn collect_dc_transform_metaclass_factories(stmts: &[Stmt]) -> Vec<DcTransformFactory> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else {
            continue;
        };
        // Only match metaclasses (classes inheriting from `type`).
        let is_metaclass = cls.arguments.as_ref().is_some_and(|args| {
            args.args
                .iter()
                .any(|expr| matches!(expr, Expr::Name(n) if n.id.as_str() == "type"))
        });
        if !is_metaclass {
            continue;
        }
        for dec in &cls.decorator_list {
            if let Some(parsed) = parse_dataclass_transform_decorator(&dec.expression) {
                out.push(DcTransformFactory {
                    name: cls.name.to_string(),
                    kw_only_default: parsed.kw_only_default,
                    frozen_default: parsed.frozen_default,
                    order_default: parsed.order_default,
                    field_specifier_names: parsed.field_specifier_names,
                });
            }
        }
    }
    out
}

/// Mark subclasses of `@dataclass_transform`-decorated base classes as dataclasses.
fn apply_class_or_metaclass_factories(
    stmts: &[Stmt],
    classes: &mut [ClassInfo],
    class_factories: &[DcTransformFactory],
    specifier_overloads: &std::collections::HashMap<&str, Vec<FieldSpecOverload>>,
) {
    if class_factories.is_empty() {
        return;
    }
    let factory_names: Vec<&str> = class_factories.iter().map(|f| f.name.as_str()).collect();

    for cls in classes.iter_mut() {
        // Skip the base class itself (it is the factory, not a dataclass).
        if factory_names.contains(&cls.name.as_str()) {
            continue;
        }
        // Check if any base class (direct or transitive) is a factory.
        let matching = cls.bases.iter().find_map(|base| {
            let base_name = base.split('[').next().unwrap_or(base);
            class_factories
                .iter()
                .find(|f| f.name == base_name)
                .or_else(|| find_transitive_class_factory(stmts, base_name, class_factories))
        });
        if let Some(factory) = matching {
            cls.is_dataclass = true;
            cls.is_dataclass_kw_only = factory.kw_only_default;
            cls.is_dataclass_frozen = factory.frozen_default;
            cls.is_dataclass_order = factory.order_default;

            if let Some(class_def) = find_class_def(stmts, &cls.name) {
                apply_class_keyword_overrides(class_def, cls);
                resolve_transform_field_attrs(
                    class_def,
                    &mut cls.attributes,
                    &factory.field_specifier_names,
                    specifier_overloads,
                    factory.kw_only_default,
                );
            }
        }
    }
}

/// Walk the class hierarchy to find a transitive `@dataclass_transform` base class.
fn find_transitive_class_factory<'a>(
    stmts: &[Stmt],
    class_name: &str,
    class_factories: &'a [DcTransformFactory],
) -> Option<&'a DcTransformFactory> {
    let class_def = find_class_def(stmts, class_name)?;
    let bases = class_def.arguments.as_ref()?;
    for base_expr in &bases.args {
        let base_name = match base_expr {
            Expr::Name(n) => n.id.as_str(),
            Expr::Subscript(sub) => {
                if let Expr::Name(n) = sub.value.as_ref() {
                    n.id.as_str()
                } else {
                    continue;
                }
            }
            _ => continue,
        };
        if let Some(factory) = class_factories.iter().find(|f| f.name == base_name) {
            return Some(factory);
        }
        if let Some(factory) = find_transitive_class_factory(stmts, base_name, class_factories) {
            return Some(factory);
        }
    }
    None
}

/// Mark classes using `@dataclass_transform`-decorated metaclasses as dataclasses.
fn apply_metaclass_transform_factories(
    stmts: &[Stmt],
    classes: &mut [ClassInfo],
    metaclass_factories: &[DcTransformFactory],
    specifier_overloads: &std::collections::HashMap<&str, Vec<FieldSpecOverload>>,
) {
    if metaclass_factories.is_empty() {
        return;
    }

    // Collect names of classes that directly use a factory metaclass.
    let mut metaclass_base_names: Vec<String> = Vec::new();
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else {
            continue;
        };
        let uses_factory_metaclass = cls.arguments.as_ref().is_some_and(|args| {
            args.keywords.iter().any(|kw| {
                kw.arg.as_ref().is_some_and(|a| a.as_str() == "metaclass")
                    && metaclass_factories
                        .iter()
                        .any(|f| matches!(&kw.value, Expr::Name(n) if n.id.as_str() == f.name))
            })
        });
        if uses_factory_metaclass {
            metaclass_base_names.push(cls.name.to_string());
        }
    }

    for cls in classes.iter_mut() {
        // Skip the metaclass bases themselves (they are not dataclasses).
        if metaclass_base_names.contains(&cls.name) {
            continue;
        }

        // Check if any base is a class that uses the transform metaclass.
        let matching = cls.bases.iter().find_map(|base| {
            let base_name = base.split('[').next().unwrap_or(base);
            if metaclass_base_names.iter().any(|n| n == base_name) {
                // Find the factory for this metaclass base.
                find_metaclass_factory_for_base(stmts, base_name, metaclass_factories)
            } else {
                None
            }
        });

        // Also check if the class itself directly uses the metaclass.
        let direct_match = matching.or_else(|| {
            cls.metaclass_name
                .as_ref()
                .and_then(|mc_name| metaclass_factories.iter().find(|f| f.name == *mc_name))
        });

        if let Some(factory) = direct_match {
            cls.is_dataclass = true;
            cls.is_dataclass_kw_only = factory.kw_only_default;
            cls.is_dataclass_frozen = factory.frozen_default;
            cls.is_dataclass_order = factory.order_default;

            if let Some(class_def) = find_class_def(stmts, &cls.name) {
                apply_class_keyword_overrides(class_def, cls);
                resolve_transform_field_attrs(
                    class_def,
                    &mut cls.attributes,
                    &factory.field_specifier_names,
                    specifier_overloads,
                    factory.kw_only_default,
                );
            }
        }
    }
}

/// Find the `DcTransformFactory` for a class that uses a metaclass-based transform.
fn find_metaclass_factory_for_base<'a>(
    stmts: &[Stmt],
    base_name: &str,
    metaclass_factories: &'a [DcTransformFactory],
) -> Option<&'a DcTransformFactory> {
    let class_def = find_class_def(stmts, base_name)?;
    let args = class_def.arguments.as_ref()?;
    for kw in &args.keywords {
        if kw.arg.as_ref().is_some_and(|a| a.as_str() == "metaclass") {
            if let Expr::Name(n) = &kw.value {
                return metaclass_factories.iter().find(|f| f.name == n.id.as_str());
            }
        }
    }
    None
}

/// Parse a `@dataclass_transform(...)` expression.
///
/// Returns `(is_dc_transform, kw_only_default, field_specifier_names)`.
pub(super) fn build_field_specifier_overloads(
    stmts: &[Stmt],
    spec_name: &str,
    functions: &[FunctionInfo],
) -> Vec<FieldSpecOverload> {
    let mut overloads = Vec::new();

    let has_overloads = functions.iter().any(|f| {
        f.name == spec_name
            && f.class_name.is_none()
            && f.decorators.iter().any(|d| d == "overload")
    });

    for stmt in stmts {
        let Stmt::FunctionDef(func) = stmt else {
            continue;
        };
        if func.name.as_str() != spec_name {
            continue;
        }

        let is_overload = func
            .decorator_list
            .iter()
            .any(|d| matches!(decorator_name(d), Some(n) if n == "overload"));

        if has_overloads && !is_overload {
            continue;
        }

        let params = &func.parameters;
        let mut required_kwargs = Vec::new();
        let mut init_default = None;
        let mut kw_only_default = None;

        for pwd in &params.kwonlyargs {
            let param_name = pwd.parameter.name.as_str();
            let has_default = pwd.default.is_some();

            if param_name == "init" {
                if let Some(default_expr) = &pwd.default {
                    init_default =
                        Some(matches!(default_expr.as_ref(), Expr::BooleanLiteral(b) if b.value));
                }
            } else if param_name == "kw_only" {
                if let Some(default_expr) = &pwd.default {
                    kw_only_default =
                        Some(matches!(default_expr.as_ref(), Expr::BooleanLiteral(b) if b.value));
                }
            }

            if !has_default && param_name != "init" && param_name != "kw_only" {
                required_kwargs.push(param_name.to_string());
            }
        }

        for pwd in params.posonlyargs.iter().chain(params.args.iter()) {
            let param_name = pwd.parameter.name.as_str();
            let has_default = pwd.default.is_some();

            if param_name == "init" {
                if let Some(default_expr) = &pwd.default {
                    init_default =
                        Some(matches!(default_expr.as_ref(), Expr::BooleanLiteral(b) if b.value));
                }
            } else if param_name == "kw_only" {
                if let Some(default_expr) = &pwd.default {
                    kw_only_default =
                        Some(matches!(default_expr.as_ref(), Expr::BooleanLiteral(b) if b.value));
                }
            }

            if !has_default && param_name != "init" && param_name != "kw_only" {
                required_kwargs.push(param_name.to_string());
            }
        }

        overloads.push(FieldSpecOverload {
            required_kwargs,
            init_default,
            kw_only_default,
        });
    }

    overloads
}

/// Find which factory decorates a class, if any.
pub(super) fn find_matching_factory<'a>(
    stmts: &[Stmt],
    class_name: &str,
    factories: &'a [DcTransformFactory],
) -> Option<&'a DcTransformFactory> {
    let class_def = find_class_def(stmts, class_name)?;
    for dec in &class_def.decorator_list {
        let callee = match &dec.expression {
            Expr::Call(call) => match call.func.as_ref() {
                Expr::Name(n) => n.id.as_str(),
                Expr::Attribute(a) => a.attr.as_str(),
                _ => continue,
            },
            Expr::Name(n) => n.id.as_str(),
            _ => continue,
        };
        for factory in factories {
            if factory.name == callee {
                return Some(factory);
            }
        }
    }
    None
}

/// Apply per-class keyword overrides from the decorator call.
///
/// For `@create_model(frozen=True)`, overrides the factory's `frozen_default`.
fn apply_class_decorator_overrides(
    class_def: &StmtClassDef,
    factory_name: &str,
    cls: &mut ClassInfo,
) {
    for dec in &class_def.decorator_list {
        let Expr::Call(call) = &dec.expression else {
            continue;
        };
        let callee = match call.func.as_ref() {
            Expr::Name(n) => n.id.as_str(),
            Expr::Attribute(a) => a.attr.as_str(),
            _ => continue,
        };
        if callee != factory_name {
            continue;
        }
        for kw in &call.arguments.keywords {
            let Some(arg_name) = kw.arg.as_ref() else {
                continue;
            };
            match arg_name.as_str() {
                "frozen" => {
                    cls.is_dataclass_frozen =
                        matches!(&kw.value, Expr::BooleanLiteral(b) if b.value);
                }
                "kw_only" => {
                    cls.is_dataclass_kw_only =
                        matches!(&kw.value, Expr::BooleanLiteral(b) if b.value);
                }
                "order" => {
                    cls.is_dataclass_order =
                        matches!(&kw.value, Expr::BooleanLiteral(b) if b.value);
                }
                _ => {}
            }
        }
        break;
    }
}

/// Apply class-level keyword overrides from the class definition.
///
/// For `class Customer2(ModelBase, order=True)`, overrides the factory defaults
/// using the keywords passed to the class definition itself (via `__init_subclass__`).
fn apply_class_keyword_overrides(class_def: &StmtClassDef, cls: &mut ClassInfo) {
    let Some(args) = class_def.arguments.as_ref() else {
        return;
    };
    for kw in &args.keywords {
        let Some(arg_name) = kw.arg.as_ref() else {
            continue;
        };
        match arg_name.as_str() {
            "frozen" => {
                cls.is_dataclass_frozen = matches!(&kw.value, Expr::BooleanLiteral(b) if b.value);
            }
            "kw_only" => {
                cls.is_dataclass_kw_only = matches!(&kw.value, Expr::BooleanLiteral(b) if b.value);
            }
            "order" => {
                cls.is_dataclass_order = matches!(&kw.value, Expr::BooleanLiteral(b) if b.value);
            }
            _ => {}
        }
    }
}

/// Find a class definition by name in the top-level statements.
pub(super) fn find_class_def<'a>(stmts: &'a [Stmt], name: &str) -> Option<&'a StmtClassDef> {
    for stmt in stmts {
        if let Stmt::ClassDef(cls) = stmt {
            if cls.name.as_str() == name {
                return Some(cls);
            }
        }
    }
    None
}

/// Resolve `is_init_false` and `is_kw_only` for attributes of a `dataclass_transform` class.
pub(super) fn resolve_transform_field_attrs(
    class_def: &StmtClassDef,
    attributes: &mut [AttributeInfo],
    field_specifier_names: &[String],
    specifier_overloads: &std::collections::HashMap<&str, Vec<FieldSpecOverload>>,
    kw_only_default: bool,
) {
    let mut attr_idx = 0;
    for stmt in &class_def.body {
        let Stmt::AnnAssign(ann) = stmt else {
            continue;
        };
        let Some(attr_name) = expr_simple_name(&ann.target) else {
            continue;
        };
        if attr_name == "_" && annotation_is_kw_only(&ann.annotation) {
            continue;
        }

        let Some(attr) = attributes.get_mut(attr_idx) else {
            break;
        };
        if attr.name != attr_name {
            attr_idx += 1;
            continue;
        }
        attr_idx += 1;

        let Some(value_expr) = ann.value.as_deref() else {
            if kw_only_default {
                attr.is_kw_only = true;
            }
            continue;
        };

        let Expr::Call(call) = value_expr else {
            continue;
        };

        let callee_name = match call.func.as_ref() {
            Expr::Name(n) => n.id.as_str(),
            Expr::Attribute(a) => a.attr.as_str(),
            _ => continue,
        };

        if !field_specifier_names.iter().any(|n| n == callee_name) {
            continue;
        }

        let Some(overloads) = specifier_overloads.get(callee_name) else {
            continue;
        };

        let call_kwargs: Vec<&str> = call
            .arguments
            .keywords
            .iter()
            .filter_map(|kw| kw.arg.as_ref().map(ruff_python_ast::Identifier::as_str))
            .collect();

        let explicit_init: Option<bool> = call.arguments.keywords.iter().find_map(|kw| {
            if kw.arg.as_ref().is_some_and(|a| a.as_str() == "init") {
                Some(matches!(&kw.value, Expr::BooleanLiteral(b) if b.value))
            } else {
                None
            }
        });

        let explicit_kw_only: Option<bool> = call.arguments.keywords.iter().find_map(|kw| {
            if kw.arg.as_ref().is_some_and(|a| a.as_str() == "kw_only") {
                Some(matches!(&kw.value, Expr::BooleanLiteral(b) if b.value))
            } else {
                None
            }
        });

        let mut matched_init: Option<bool> = None;
        let mut matched_kw_only: Option<bool> = None;

        for overload in overloads {
            let all_required_present = overload
                .required_kwargs
                .iter()
                .all(|req| call_kwargs.contains(&req.as_str()));
            if all_required_present {
                matched_init = overload.init_default;
                matched_kw_only = overload.kw_only_default;
                break;
            }
        }

        let effective_init = explicit_init.or(matched_init);
        let effective_kw_only = explicit_kw_only.or(matched_kw_only);

        if effective_init == Some(false) {
            attr.is_init_false = true;
        }
        attr.is_kw_only = effective_kw_only.unwrap_or(kw_only_default);

        // Extract alias from field specifier call (e.g., `model_field(alias="other_name")`).
        attr.alias = call.arguments.keywords.iter().find_map(|kw| {
            if kw.arg.as_ref().is_some_and(|a| a.as_str() == "alias") {
                if let Expr::StringLiteral(s) = &kw.value {
                    return Some(s.value.to_str().to_owned());
                }
            }
            None
        });
    }
}

// ---------------------------------------------------------------------------
// Function info
// ---------------------------------------------------------------------------

pub(super) fn dataclass_flag(class: &StmtClassDef, key: &str) -> bool {
    for dec in &class.decorator_list {
        let Expr::Call(call) = &dec.expression else {
            continue;
        };
        let is_dc = match call.func.as_ref() {
            Expr::Name(n) => n.id.as_str() == "dataclass",
            Expr::Attribute(a) => a.attr.as_str() == "dataclass",
            _ => false,
        };
        if !is_dc {
            continue;
        }
        for kw in &call.arguments.keywords {
            if kw.arg.as_ref().map(ruff_python_ast::Identifier::as_str) == Some(key) {
                return matches!(&kw.value, Expr::BooleanLiteral(b) if b.value);
            }
        }
    }
    false
}

/// Returns `true` when the annotation expression is `KW_ONLY`
/// (the sentinel that makes all following fields keyword-only).
pub(super) fn field_kw_only_override(value: &Expr) -> Option<bool> {
    let Expr::Call(call) = value else { return None };
    let is_field_call = match call.func.as_ref() {
        Expr::Name(n) => n.id.as_str() == "field",
        Expr::Attribute(a) => a.attr.as_str() == "field",
        _ => false,
    };
    if !is_field_call {
        return None;
    }
    for kw in &call.arguments.keywords {
        if kw.arg.as_ref().is_some_and(|arg| arg.as_str() == "kw_only") {
            return Some(matches!(&kw.value, Expr::BooleanLiteral(b) if b.value));
        }
    }
    None
}

/// Returns `true` when the value expression is a `field(init=False, ...)` call.
///
/// Only checks calls to the standard `dataclasses.field` function.  Field specifier
/// calls from `@dataclass_transform` are resolved in `apply_dataclass_transform`.
pub(super) fn field_init_is_false(value: &Expr) -> bool {
    let Expr::Call(call) = value else {
        return false;
    };
    let is_field_call = match call.func.as_ref() {
        Expr::Name(n) => n.id.as_str() == "field",
        Expr::Attribute(a) => a.attr.as_str() == "field",
        _ => false,
    };
    if !is_field_call {
        return false;
    }
    call.arguments.keywords.iter().any(|kw| {
        kw.arg.as_ref().is_some_and(|a| a.as_str() == "init")
            && matches!(&kw.value, Expr::BooleanLiteral(b) if !b.value)
    })
}

pub(super) fn dataclass_bool_flag_is_false(class: &StmtClassDef, key: &str) -> bool {
    for dec in &class.decorator_list {
        let Expr::Call(call) = &dec.expression else {
            continue;
        };
        let is_dc = match call.func.as_ref() {
            Expr::Name(n) => n.id.as_str() == "dataclass",
            Expr::Attribute(a) => a.attr.as_str() == "dataclass",
            _ => false,
        };
        if !is_dc {
            continue;
        }
        for kw in &call.arguments.keywords {
            if kw.arg.as_ref().map(ruff_python_ast::Identifier::as_str) == Some(key) {
                return matches!(&kw.value, Expr::BooleanLiteral(b) if !b.value);
            }
        }
    }
    false
}

pub(super) fn extract_string_list(
    elts: &[Expr],
    final_constants: &std::collections::HashMap<&str, &str>,
) -> Vec<String> {
    elts.iter()
        .filter_map(|elt| match elt {
            Expr::StringLiteral(s) => Some(s.value.to_str().to_owned()),
            Expr::Name(n) => final_constants.get(n.id.as_str()).map(|s| (*s).to_owned()),
            _ => None,
        })
        .collect()
}
