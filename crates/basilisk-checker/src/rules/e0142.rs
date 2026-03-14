//! BSK-E0142: `dataclass_transform` violations when the transform is applied via a base class.
//!
//! When a class is decorated with `@dataclass_transform(...)`, subclasses that inherit
//! from it behave like dataclasses with the transform's default settings overridable by
//! keyword arguments on the class definition.
//!
//! This rule detects:
//! 1. A non-frozen subclass inheriting from a frozen transform-class (line 51).
//! 2. Attribute assignment on a frozen transform-class instance (lines 63, 122).
//! 3. Positional arguments to a `kw_only` transform-class constructor (lines 66, 82).
//! 4. Comparison operators on transform-class instances that lack `order=True` (line 72).
//!
//! ```python
//! from typing import dataclass_transform
//!
//! @dataclass_transform(kw_only_default=True)
//! class ModelBase: ...
//!
//! class Customer(ModelBase, frozen=True):
//!     id: int
//!
//! c = Customer(3)           # E — kw_only requires keyword args
//! c.id = 4                  # E — frozen instance is immutable
//! ```

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0142",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0142",
};

/// Effective settings for a class that inherits from a `@dataclass_transform` base.
#[derive(Debug, Clone)]
struct TransformClassSettings {
    /// Whether this class is effectively frozen.
    frozen: bool,
    /// Whether this class has keyword-only constructor parameters.
    kw_only: bool,
    /// Whether this class has ordering comparisons synthesised.
    order: bool,
}

/// Defaults extracted from a `@dataclass_transform(...)` decorator on a class.
#[derive(Debug, Clone)]
struct TransformBaseDefaults {
    frozen_default: bool,
    kw_only_default: bool,
    order_default: bool,
}

/// Extract a boolean keyword argument value from a parenthesised argument text.
fn extract_bool_kwarg(args_text: &str, key: &str) -> Option<bool> {
    let pattern = format!("{key}=");
    let idx = args_text.find(&pattern)?;
    let after = args_text[idx + pattern.len()..].trim_start();
    if after.starts_with("True") {
        Some(true)
    } else if after.starts_with("False") {
        Some(false)
    } else {
        None
    }
}

/// Extract the text inside `(...)` following `end_pos` in `source`.
fn extract_paren_args(source: &str, name_end: usize) -> Option<&str> {
    let rest = source.get(name_end..)?;
    let open = rest.find('(')?;
    let after_open = name_end + open + 1;
    let inner = source.get(after_open..)?;
    // Find the matching `)` respecting one level of bracket nesting.
    let mut depth = 1i32;
    let mut close = None;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(idx);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    source.get(after_open..after_open + close)
}

/// Find all class names (in this module) that are decorated with `@dataclass_transform`
/// and parse their default settings.
fn collect_transform_base_classes(
    module: &ResolvedModule,
) -> HashMap<String, TransformBaseDefaults> {
    let source = &module.source;
    let mut result = HashMap::new();

    for cls in &module.classes {
        // Check decorator_spans for "dataclass_transform".
        let has_dt = cls
            .decorator_spans
            .iter()
            .any(|(name, _)| name == "dataclass_transform");
        if !has_dt {
            continue;
        }

        // Find the `@dataclass_transform` in the source before the class keyword.
        let cls_start = cls.def_span.start_usize();
        let search_end = cls_start
            + source
                .get(cls_start..)
                .and_then(|s| s.find("class "))
                .unwrap_or(0);

        let search_region = source.get(..search_end).unwrap_or("");
        let marker = "@dataclass_transform";
        let Some(marker_pos) = search_region.rfind(marker) else {
            continue;
        };

        let name_end = marker_pos + marker.len();
        let mut defaults = TransformBaseDefaults {
            frozen_default: false,
            kw_only_default: false,
            order_default: false,
        };

        if let Some(args_text) = extract_paren_args(source, name_end) {
            if let Some(val) = extract_bool_kwarg(args_text, "frozen_default") {
                defaults.frozen_default = val;
            }
            if let Some(val) = extract_bool_kwarg(args_text, "kw_only_default") {
                defaults.kw_only_default = val;
            }
            if let Some(val) = extract_bool_kwarg(args_text, "order_default") {
                defaults.order_default = val;
            }
        }

        let _ = result.insert(cls.name.clone(), defaults);
    }

    result
}

/// For each class in the module, determine if it inherits (directly) from a
/// `@dataclass_transform` base class and compute its effective settings.
fn collect_transform_subclasses<'a>(
    module: &'a ResolvedModule,
    transform_bases: &HashMap<String, TransformBaseDefaults>,
) -> HashMap<&'a str, TransformClassSettings> {
    let source = &module.source;
    let mut result = HashMap::new();

    for cls in &module.classes {
        // Find a transform base that this class inherits from.
        let Some((_, base_defaults)) = cls
            .bases
            .iter()
            .find_map(|b| transform_bases.get_key_value(b.as_str()))
        else {
            continue;
        };

        // Start with the base defaults.
        let mut settings = TransformClassSettings {
            frozen: base_defaults.frozen_default,
            kw_only: base_defaults.kw_only_default,
            order: base_defaults.order_default,
        };

        // Override with keyword arguments on the class definition itself.
        // These appear in `class Foo(Base, frozen=True, kw_only=False, order=True): ...`
        // We extract them from the source text around the class name.
        let name_end = cls.name_span.end_usize();
        let Some(rest) = source.get(name_end..) else {
            let _ = result.insert(cls.name.as_str(), settings);
            continue;
        };

        // Find the bases parenthesis.
        let Some(open_paren) = rest.find('(') else {
            let _ = result.insert(cls.name.as_str(), settings);
            continue;
        };

        let paren_start = name_end + open_paren + 1;
        let Some(inner) = source.get(paren_start..) else {
            let _ = result.insert(cls.name.as_str(), settings);
            continue;
        };

        // Find closing `)`.
        let mut depth = 1i32;
        let mut close_offset = inner.len();
        for (idx, ch) in inner.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        close_offset = idx;
                        break;
                    }
                }
                _ => {}
            }
        }

        let Some(bases_text) = source.get(paren_start..paren_start + close_offset) else {
            let _ = result.insert(cls.name.as_str(), settings);
            continue;
        };

        if let Some(val) = extract_bool_kwarg(bases_text, "frozen") {
            settings.frozen = val;
        }
        if let Some(val) = extract_bool_kwarg(bases_text, "kw_only") {
            settings.kw_only = val;
        }
        if let Some(val) = extract_bool_kwarg(bases_text, "order") {
            settings.order = val;
        }

        let _ = result.insert(cls.name.as_str(), settings);
    }

    result
}

/// Compute the effective `frozen/kw_only/order` settings for a class that
/// **inherits from another transform subclass** (not directly from the base).
///
/// This handles the case where `Customer1Subclass(Customer1)` where `Customer1`
/// is itself a transform subclass.
fn resolve_inherited_settings<'a>(
    cls_name: &str,
    module: &'a ResolvedModule,
    direct_settings: &HashMap<&'a str, TransformClassSettings>,
) -> Option<TransformClassSettings> {
    // If this class itself is a direct transform subclass, return that.
    if let Some(s) = direct_settings.get(cls_name) {
        return Some(s.clone());
    }

    // Otherwise check if any base is a direct (or indirect) transform subclass.
    let cls = module.classes.iter().find(|c| c.name == cls_name)?;
    for base in &cls.bases {
        if let Some(s) = resolve_inherited_settings(base, module, direct_settings) {
            return Some(s);
        }
    }

    None
}

/// Return the byte offset of the start of a 1-based line.
#[expect(clippy::cast_possible_truncation, reason = "byte offsets fit u32 for source files")]
fn line_start_offset(source: &str, line: usize) -> u32 {
    let mut current = 1usize;
    for (idx, ch) in source.char_indices() {
        if current == line {
            return idx as u32;
        }
        if ch == '\n' {
            current += 1;
        }
    }
    source.len() as u32
}

/// Return a span covering the trimmed content of a source line (1-based).
#[expect(clippy::as_conversions, clippy::cast_possible_truncation, reason = "u32<->usize safe on 32-bit+")]
fn span_for_source_line(source: &str, line: usize) -> Span {
    let start = line_start_offset(source, line) as usize;
    let line_text = source
        .get(start..)
        .and_then(|s| s.lines().next())
        .unwrap_or("");
    let trim_leading = line_text.len() - line_text.trim_start().len();
    let trimmed = line_text.trim();
    Span {
        start: (start + trim_leading) as u32,
        end: (start + trim_leading + trimmed.len()) as u32,
    }
}

/// Emits BSK-E0142 for `dataclass_transform` violations via class-based transform.
pub(crate) struct DataclassTransformClassViolation;

impl Rule for DataclassTransformClassViolation {
    #[expect(clippy::too_many_lines, reason = "dataclass_transform checking requires many steps")]
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let transform_bases = collect_transform_base_classes(module);
        if transform_bases.is_empty() {
            return;
        }

        // Direct transform subclasses (inherit from a @dataclass_transform base).
        let direct_settings = collect_transform_subclasses(module, &transform_bases);
        if direct_settings.is_empty() {
            return;
        }

        let source = &module.source;
        let path = &module.path;

        // Build a map of all transform-class instances at module level:
        // variable_name -> (class_name, settings).
        let mut instance_map: HashMap<&str, (&str, TransformClassSettings)> = HashMap::new();
        for var in &module.module_vars {
            let Some(rhs_span) = var.rhs_span else {
                continue;
            };
            let Some(rhs_text) = slice_span(source, rhs_span) else {
                continue;
            };
            // Extract the callee name: `Customer1(...)` -> `"Customer1"`.
            let callee = rhs_text.split(['(', '[']).next().unwrap_or("").trim();
            if callee.is_empty() {
                continue;
            }
            // Resolve through any dot-qualifiers.
            let callee = callee.rsplit('.').next().unwrap_or(callee);

            if let Some(settings) = resolve_inherited_settings(callee, module, &direct_settings) {
                let _ = instance_map.insert(var.name.as_str(), (callee, settings));
            }
        }

        // --- Check 1: Non-frozen subclass inheriting from a frozen transform class ---
        check_frozen_inheritance(module, &direct_settings, path, diagnostics);

        // --- Check 2: Attribute assignment on frozen transform-class instances ---
        check_frozen_instance_assignment(module, &instance_map, source, path, diagnostics);

        // --- Check 3: Positional args to kw_only transform-class constructor ---
        check_kw_only_positional_args(module, &direct_settings, source, path, diagnostics);

        // --- Check 4: Comparison operator on instance without order=True ---
        check_no_order_comparison(module, &instance_map, source, path, diagnostics);
    }
}

/// Check 1: A non-frozen class directly inheriting from a frozen transform subclass.
fn check_frozen_inheritance(
    module: &ResolvedModule,
    direct_settings: &HashMap<&str, TransformClassSettings>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for cls in &module.classes {
        // This class must NOT itself be a direct transform subclass.
        if direct_settings.contains_key(cls.name.as_str()) {
            continue;
        }

        for base in &cls.bases {
            let Some(base_settings) = direct_settings.get(base.as_str()) else {
                continue;
            };
            if base_settings.frozen {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Non-frozen class `{}` cannot inherit from frozen \
                         dataclass-transform class `{}`",
                        cls.name, base
                    ),
                    span: cls.name_span,
                    path: path.to_owned(),
                    help: Some(
                        "A non-frozen class cannot subclass a frozen \
                         dataclass-transform class"
                            .to_owned(),
                    ),
                    note: Some(
                        "dataclass_transform: frozen and non-frozen classes \
                         cannot be mixed in the same hierarchy"
                            .to_owned(),
                    ),
                });
            }
        }
    }
}

/// Check 2: Attribute assignment on a frozen transform-class instance.
fn check_frozen_instance_assignment(
    module: &ResolvedModule,
    instance_map: &HashMap<&str, (&str, TransformClassSettings)>,
    _source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for assign in &module.module_attr_assignments {
        let Some((class_name, settings)) = instance_map.get(assign.object_name.as_str()) else {
            continue;
        };
        if !settings.frozen {
            continue;
        }
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Cannot assign to attribute `{}` of frozen \
                 dataclass-transform class `{}` instance `{}`",
                assign.attr_name, class_name, assign.object_name
            ),
            span: assign.target_span,
            path: path.to_owned(),
            help: Some(
                "Instances of frozen dataclass-transform classes are immutable \
                 after construction"
                    .to_owned(),
            ),
            note: Some(
                "dataclass_transform(frozen=True) prohibits attribute assignment \
                 after construction"
                    .to_owned(),
            ),
        });
    }
}

/// Parse the parenthesised argument list of a call site, returning whether any
/// positional (non-keyword) arguments are present beyond the first.
///
/// Returns `(call_site_line, has_positional_args)`.
fn parse_call_positional(_source: &str, rhs_text: &str, _rhs_start: usize) -> bool {
    // Find the opening `(` in rhs_text.
    let Some(paren_pos) = rhs_text.find('(') else {
        return false;
    };
    let args_start_in_rhs = paren_pos + 1;
    let args_text_raw = &rhs_text[args_start_in_rhs..];
    // Find the matching `)`.
    let mut depth = 1i32;
    let mut args_end = args_text_raw.len();
    for (idx, ch) in args_text_raw.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    args_end = idx;
                    break;
                }
            }
            _ => {}
        }
    }
    let args_text = args_text_raw[..args_end].trim();
    if args_text.is_empty() {
        return false;
    }

    // Check if the first argument is positional (does not contain `=` before the first `,`).
    // We need to split at top-level commas.
    let first_arg = split_first_top_level_arg(args_text);
    if first_arg.is_empty() {
        return false;
    }
    // If the first arg has `=` that is not inside brackets, it's a keyword arg.
    !is_keyword_arg(first_arg)
}

/// Return the text of the first top-level comma-separated argument.
fn split_first_top_level_arg(args: &str) -> &str {
    let mut depth = 0i32;
    for (idx, ch) in args.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => return args[..idx].trim(),
            _ => {}
        }
    }
    args.trim()
}

/// Returns `true` if the argument text looks like a keyword arg (`name=value`).
fn is_keyword_arg(arg: &str) -> bool {
    let arg = arg.trim();
    // A keyword arg starts with an identifier followed by `=`.
    let eq_pos = arg.find('=');
    let Some(eq_pos) = eq_pos else {
        return false;
    };
    let before = arg[..eq_pos].trim();
    !before.is_empty()
        && before
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        && before
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
}

/// Check 3: Positional arguments to a `kw_only` transform-class constructor.
///
/// This scans module-level assignment RHS call expressions for the pattern
/// `ClassName(positional_arg, ...)` where `ClassName` is a `kw_only` transform class.
fn check_kw_only_positional_args(
    module: &ResolvedModule,
    direct_settings: &HashMap<&str, TransformClassSettings>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in &module.module_vars {
        let Some(rhs_span) = var.rhs_span else {
            continue;
        };
        let Some(rhs_text) = slice_span(source, rhs_span) else {
            continue;
        };

        // Extract the callee name.
        let callee = rhs_text.split(['(', '[']).next().unwrap_or("").trim();
        if callee.is_empty() {
            continue;
        }
        let callee = callee.rsplit('.').next().unwrap_or(callee);

        // Resolve settings (including inherited ones).
        let Some(settings) = resolve_inherited_settings(callee, module, direct_settings) else {
            continue;
        };

        if !settings.kw_only {
            continue;
        }

        if parse_call_positional(source, rhs_text, rhs_span.start_usize()) {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Constructor of `{callee}` only accepts keyword arguments \
                     (kw_only=True)"
                ),
                span: rhs_span,
                path: path.to_owned(),
                help: Some(format!(
                    "Pass arguments as keyword arguments: `{callee}(field=value, ...)`"
                )),
                note: Some(
                    "dataclass_transform with kw_only_default=True makes all \
                     constructor parameters keyword-only"
                        .to_owned(),
                ),
            });
        }
    }
}

/// Check 4: Comparison operator on a transform-class instance without `order=True`.
///
/// Scans source lines for binary comparison operators (`<`, `>`, `<=`, `>=`)
/// where either operand is a known transform-class instance that lacks `order=True`.
fn check_no_order_comparison(
    _module: &ResolvedModule,
    instance_map: &HashMap<&str, (&str, TransformClassSettings)>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if instance_map.is_empty() {
        return;
    }

    // Collect variable names that are non-order transform instances.
    let no_order_vars: Vec<&str> = instance_map
        .iter()
        .filter(|(_, (_, s))| !s.order)
        .map(|(name, _)| *name)
        .collect();

    if no_order_vars.is_empty() {
        return;
    }

    for (line_idx, line) in source.lines().enumerate() {
        let line_number = line_idx + 1;
        // Strip trailing comment.
        let code_part = line.split('#').next().unwrap_or(line);

        // Check for comparison operators.
        let has_comparison = code_part.contains(" < ")
            || code_part.contains(" > ")
            || code_part.contains(" <= ")
            || code_part.contains(" >= ");

        if !has_comparison {
            continue;
        }

        // Check if any non-order variable appears in this line.
        let offending_var = no_order_vars
            .iter()
            .find(|&&var_name| contains_identifier(code_part, var_name));

        let Some(&var_name) = offending_var else {
            continue;
        };

        let Some(&(class_name, _)) = instance_map.get(var_name) else {
            continue;
        };

        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Comparison operator not supported: \
                 `{class_name}` does not synthesise ordering methods \
                 (order=False by default)"
            ),
            span: span_for_source_line(source, line_number),
            path: path.to_owned(),
            help: Some(format!(
                "Use `order=True` in `class {class_name}(...)` to enable ordering, \
                 or avoid `<`, `>`, `<=`, `>=` comparisons"
            )),
            note: Some(
                "dataclass_transform without order=True does not synthesise \
                 __lt__, __le__, __gt__, __ge__ methods"
                    .to_owned(),
            ),
        });
    }
}

/// Returns `true` if `name` appears as a whole identifier in `text`.
fn contains_identifier(text: &str, name: &str) -> bool {
    let name_bytes = name.as_bytes();
    let text_bytes = text.as_bytes();
    let mut start = 0;
    while start + name.len() <= text.len() {
        let Some(pos) = text[start..].find(name) else {
            break;
        };
        let abs = start + pos;
        let before_ok = abs == 0 || !is_ident_char(text_bytes[abs - 1]);
        let after_end = abs + name_bytes.len();
        let after_ok = after_end >= text_bytes.len() || !is_ident_char(text_bytes[after_end]);
        if before_ok && after_ok {
            return true;
        }
        start = abs + 1;
    }
    false
}

/// Returns `true` for ASCII alphanumeric or underscore characters.
const fn is_ident_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
