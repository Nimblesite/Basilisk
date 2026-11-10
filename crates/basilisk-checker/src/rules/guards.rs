//! Implements helpers for [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Shared guard predicates used across multiple rules.
//!
//! These predicates identify Python typing patterns where strict annotation
//! enforcement should be suspended because the construct has well-defined PEP
//! semantics that legitimately omit annotations.

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, FunctionInfo, ResolvedModule};

/// Returns `true` when a function is in a "stub context" — a context where
/// annotation enforcement (BSK-0001, BSK-0002, BSK-0004) should be skipped.
///
/// Implements the exemption side of [TYPEINF-FUNC-PARAMS] / [TYPEINF-FUNC-OVERLOADS]:
/// Protocol/abstract/stub bodies legitimately omit annotations, but `@overload`
/// variants are explicitly NOT exempt (their signatures drive resolution).
///
/// A stub context is any of:
/// - A non-`@overload` function whose body is a pure stub (only `...`, `pass`,
///   or a docstring): Protocol method stubs, abstract placeholders, `.pyi`-style
///   inline stubs.  **`@overload` variants are excluded** — they must carry
///   annotations because their signatures drive overload resolution.
/// - A function decorated with `@abstractmethod` (even with a non-stub body).
/// - A method inside a `Protocol` class (interface contract, not implementation).
pub(crate) fn is_stub_context(func: &FunctionInfo, classes: &[ClassInfo]) -> bool {
    // @overload variants MUST be annotated — their signatures drive type resolution.
    if super::shared::decorator_spelled(&func.decorators, "overload") {
        return false;
    }
    // Pure stub bodies (only `...` / `pass`) are exempt — covers Protocol stubs
    // and abstract placeholders that legitimately omit annotations.
    if func.is_stub_body {
        return true;
    }
    // Non-stub abstractmethod bodies are also exempt.
    if super::shared::decorator_spelled(&func.decorators, "abstractmethod") {
        return true;
    }
    // Protocol methods are interface contracts, not implementations.
    func.class_name.as_ref().is_some_and(|cls_name| {
        classes
            .iter()
            .find(|c| &c.name == cls_name)
            .is_some_and(is_protocol_class)
    })
}

/// Returns `true` when a function is decorated with `@no_type_check`.
///
/// PEP 484 / `typing.no_type_check` directs checkers to suppress *body* type
/// checks for the function, so return-value/assignment diagnostics (E0011) must
/// not fire. Argument-count (E0041) and similar signature checks still apply.
pub(crate) fn is_no_type_check(func: &FunctionInfo) -> bool {
    super::shared::decorator_spelled(&func.decorators, "no_type_check")
}

/// Returns `true` when a class is an Enum subclass, in either the bare
/// (`class C(Enum)`) or module-qualified (`class C(enum.Enum)`) spelling.
///
/// Enum members are unannotated by design — their type is `Literal[EnumClass.member]`,
/// synthesised by the Enum metaclass.  Firing BSK-0005 on them is a false positive.
pub(crate) fn is_enum_class(class: &ClassInfo) -> bool {
    class.bases.iter().any(|b| {
        matches!(
            b.strip_prefix("enum.").unwrap_or(b),
            "Enum" | "IntEnum" | "StrEnum" | "Flag" | "IntFlag" | "ReprEnum"
        )
    })
}

/// Returns `true` when a class is a `Protocol` subclass.
///
/// Protocol attributes are interface specifications, not concrete class variables.
/// Unannotated names in a Protocol body are structural members, not bugs.
///
/// Implements the gating predicate for [TYPEINF-SUBTYPING-PROTOCOL] — identifies
/// the structural-subtyping target class. The member-by-member conformance check
/// itself lives in the out-of-scope `protocols_subtyping` rule (see the map).
pub(crate) fn is_protocol_class(class: &ClassInfo) -> bool {
    class.bases.iter().any(|b| b == "Protocol")
}

/// Returns `true` when a class is a `NamedTuple` subclass.
///
/// `NamedTuple` fields are declared as bare annotations and synthesised into a
/// tuple by the metaclass, so strict attribute-annotation enforcement must be
/// suspended for them.
pub(crate) fn is_namedtuple_class(class: &ClassInfo) -> bool {
    class.bases.iter().any(|b| b == "NamedTuple")
}

/// Returns `true` when a class gets a synthesized `__init__` from a
/// class-applied or metaclass-applied `@dataclass_transform`.
///
/// Covers both PEP 681 application forms that don't go through a decorator
/// function: a transitive base class decorated with `@dataclass_transform`,
/// or a transitive base whose metaclass (or one of its bases) carries the
/// decorator.
pub(crate) fn inherits_dataclass_transform(
    class: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
) -> bool {
    let resolve = |base: &str| {
        class_map
            .get(base.split('[').next().unwrap_or(base))
            .copied()
    };
    let directly_decorated = |c: &ClassInfo| {
        c.decorator_spans
            .iter()
            .any(|(name, _)| name == "dataclass_transform")
    };
    let metaclass_is_transform = |c: &ClassInfo| {
        c.metaclass_name.as_deref().is_some_and(|meta| {
            class_map.get(meta).is_some_and(|m| {
                super::shared::class_or_base_matches(m, &resolve, &directly_decorated)
            })
        })
    };

    // A transform metaclass on this class or any transitive base…
    super::shared::class_or_base_matches(class, &resolve, &metaclass_is_transform)
        // …or a directly-decorated transitive base. The class's OWN decorator
        // is the decorator-function form and intentionally does not count.
        || class.bases.iter().any(|base| {
            resolve(base).is_some_and(|b| {
                super::shared::class_or_base_matches(b, &resolve, &directly_decorated)
            })
        })
}

// ---------------------------------------------------------------------------
// dataclass_transform detection
// ---------------------------------------------------------------------------

/// Default settings extracted from a `@dataclass_transform(...)` decorator.
#[derive(Debug, Clone)]
pub(crate) struct TransformDefaults {
    /// Default value of `frozen` for classes using this transform function.
    pub frozen: bool,
    /// Default value of `order` for classes using this transform function.
    pub order: bool,
}

/// Effective dataclass-transform settings for a class.
#[derive(Debug, Clone)]
pub(crate) struct TransformClassInfo {
    /// Whether the class is effectively frozen.
    pub frozen: bool,
    /// Whether the class has ordering comparisons enabled.
    pub order: bool,
}

/// Extracts a boolean kwarg value from a parenthesised argument string.
///
/// Given `"frozen_default=True, order_default=True"` and key `"frozen_default"`,
/// returns `Some(true)`.
fn extract_bool_kwarg(args_text: &str, key: &str) -> Option<bool> {
    let pattern = format!("{key}=");
    let idx = args_text.find(&pattern)?;
    let after = args_text.get(idx + pattern.len()..)?.trim_start();
    if after.starts_with("True") {
        Some(true)
    } else if after.starts_with("False") {
        Some(false)
    } else {
        None
    }
}

/// Extracts the parenthesised argument text from a decorator call.
///
/// Given source text and a position where the decorator name starts,
/// finds the matching `(...)` and returns the inner text.
fn extract_decorator_args(source: &str, decorator_name_end: usize) -> Option<&str> {
    let rest = source.get(decorator_name_end..)?;
    let open = rest.find('(')?;
    let after_open = decorator_name_end + open + 1;
    // Find matching close paren (simple: no nested parens in kwargs)
    let close = source.get(after_open..)?.find(')')?;
    source.get(after_open..after_open + close)
}

/// Scans functions for `@dataclass_transform(...)` decorators and returns
/// a map from function name to its default settings.
pub(crate) fn collect_transform_functions(
    module: &ResolvedModule,
) -> HashMap<String, TransformDefaults> {
    let mut result = HashMap::new();

    for func in &module.functions {
        if !super::shared::decorator_spelled(&func.decorators, "dataclass_transform") {
            continue;
        }

        // Extract the source region covering decorators + def line
        let func_source_start = func.def_span.start_usize();
        // Look at source from the function start to find @dataclass_transform(...)
        let Some(search_region) = module.source.get(func_source_start..) else {
            continue;
        };

        let marker = "@dataclass_transform";
        let Some(marker_pos) = search_region.find(marker) else {
            continue;
        };

        let name_end = func_source_start + marker_pos + marker.len();
        let mut defaults = TransformDefaults {
            frozen: false,
            order: false,
        };

        if let Some(args_text) = extract_decorator_args(&module.source, name_end) {
            if let Some(val) = extract_bool_kwarg(args_text, "frozen_default") {
                defaults.frozen = val;
            }
            if let Some(val) = extract_bool_kwarg(args_text, "order_default") {
                defaults.order = val;
            }
        }

        let _ = result.insert(func.name.clone(), defaults);
    }

    result
}

/// For each class, checks if it is decorated by a `dataclass_transform` function
/// and returns its effective settings (defaults overridden by class-level kwargs).
pub(crate) fn collect_transform_classes(
    module: &ResolvedModule,
) -> HashMap<String, TransformClassInfo> {
    let transform_funcs = collect_transform_functions(module);
    if transform_funcs.is_empty() {
        return HashMap::new();
    }

    let mut result = HashMap::new();

    for cls in &module.classes {
        // Already recognized as a standard dataclass — skip
        if cls.is_dataclass {
            continue;
        }

        // Look at source before the class definition to find decorators.
        // cls.def_span covers the entire class including decorators.
        let cls_start = cls.def_span.start_usize();
        // Find the `class` keyword to delimit the decorator region
        let Some(class_kw_offset) = module
            .source
            .get(cls_start..)
            .and_then(|s| s.find("class "))
        else {
            continue;
        };

        let Some(decorator_region) = module.source.get(cls_start..cls_start + class_kw_offset)
        else {
            continue;
        };

        // Check each transform function name against the decorator region
        for (func_name, defaults) in &transform_funcs {
            let at_name = format!("@{func_name}");
            let Some(at_pos) = decorator_region.find(&at_name) else {
                continue;
            };

            // Check character after decorator name to distinguish @create_model from @create_model_frozen
            let after_at_name = cls_start + at_pos + at_name.len();
            let next_char = module
                .source
                .as_bytes()
                .get(after_at_name)
                .copied()
                .map_or(b'\n', |c| c);
            if next_char.is_ascii_alphanumeric() || next_char == b'_' {
                continue;
            }

            let mut info = TransformClassInfo {
                frozen: defaults.frozen,
                order: defaults.order,
            };

            // Check for overriding kwargs in the class decorator call
            if let Some(args_text) = extract_decorator_args(&module.source, after_at_name) {
                if let Some(val) = extract_bool_kwarg(args_text, "frozen") {
                    info.frozen = val;
                }
                if let Some(val) = extract_bool_kwarg(args_text, "order") {
                    info.order = val;
                }
            }

            let _ = result.insert(cls.name.clone(), info);
            break;
        }
    }

    result
}
