//! Implements helpers for [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Shared guard predicates used across multiple rules.
//!
//! These predicates identify Python typing patterns where strict annotation
//! enforcement is suspended because the construct has well-defined PEP
//! semantics that legitimately omit annotations. Every verdict reads the
//! resolver's binding-resolved flags — never a decorator's or base's spelling.

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ClassInfo, FunctionInfo, ResolvedModule, TypingForm};
use ruff_python_ast::{Arguments, Decorator, Expr, Stmt, StmtClassDef};

/// Returns `true` when a function is in a "stub context" — a context where
/// annotation enforcement (BSK-0001, BSK-0002, BSK-0004) is skipped.
///
/// Implements the exemption side of [TYPEINF-FUNC-PARAMS] /
/// [TYPEINF-FUNC-OVERLOADS]:
///
/// - A pure stub body (only `...`, `pass`, or a docstring) legitimately omits
///   annotations: Protocol method stubs and abstract placeholders.
/// - `@abstractmethod` bodies are exempt even when non-stub; the flag resolves
///   through the module's bindings (`abc.abstractmethod`, aliased or not).
/// - Methods of a `Protocol` class are interface contracts, not
///   implementations; `is_protocol` comes from resolved base forms.
/// - `@overload` variants are explicitly NOT exempt — the typing spec's
///   overloads chapter makes their signatures drive resolution, so they must
///   be annotated. The flag resolves through the bindings.
pub(crate) fn is_stub_context(func: &FunctionInfo, classes: &[ClassInfo]) -> bool {
    if func.is_overload {
        return false;
    }
    if func.is_stub_body || func.is_abstractmethod {
        return true;
    }
    func.class_name.as_ref().is_some_and(|class_name| {
        classes
            .iter()
            .find(|class| &class.name == class_name)
            .is_some_and(|class| class.is_protocol)
    })
}

/// Returns `true` when a function is decorated with `@no_type_check`.
///
/// The typing spec's directives chapter: a checker supporting the decorator
/// "should suppress all type errors for the `def` statement and its body" and
/// "ignore all parameter and return type annotations and treat the function
/// as if it were unannotated". The flag resolves through the module's
/// bindings, so `typing.no_type_check`, an alias, and a shadowing local
/// definition all answer correctly.
pub(crate) fn is_no_type_check(func: &FunctionInfo) -> bool {
    func.is_no_type_check
}

// ---------------------------------------------------------------------------
// PEP 681 `dataclass_transform`
// ---------------------------------------------------------------------------

/// Effective dataclass-transform settings for a governed class (PEP 681).
pub(crate) struct TransformClassInfo {
    /// Whether the class is effectively frozen: the application-site or
    /// class-definition `frozen=` keyword, else the provider's
    /// `frozen_default` (spec default `False`).
    pub(crate) frozen: bool,
}

/// PEP 681 transform providers declared at module level: decorator functions
/// and base/metaclass classes carrying `@dataclass_transform(...)`, each with
/// its `frozen_default`, plus a name map of every module-level class.
struct TransformProviders<'ast> {
    functions: HashMap<&'ast str, bool>,
    classes: HashMap<&'ast str, bool>,
    class_defs: HashMap<&'ast str, &'ast StmtClassDef>,
}

/// The bool value of a literal `name=True/False` keyword argument.
fn keyword_bool(arguments: &Arguments, name: &str) -> Option<bool> {
    arguments
        .keywords
        .iter()
        .find(|keyword| keyword.arg.as_ref().is_some_and(|arg| arg.as_str() == name))
        .and_then(|keyword| match &keyword.value {
            Expr::BooleanLiteral(literal) => Some(literal.value),
            _ => None,
        })
}

/// `Some(frozen_default)` when any decorator resolves to
/// `typing.dataclass_transform` through the module's bindings — aliased and
/// module-qualified spellings included, module-local shadows excluded.
fn transform_frozen_default(module: &ResolvedModule, decorators: &[Decorator]) -> Option<bool> {
    decorators.iter().find_map(|decorator| match &decorator.expression {
        Expr::Call(call) if module.bindings.is_form(&call.func, TypingForm::DataclassTransform) => {
            Some(keyword_bool(&call.arguments, "frozen_default").unwrap_or(false))
        }
        expr if module.bindings.is_form(expr, TypingForm::DataclassTransform) => Some(false),
        _ => None,
    })
}

/// Collect the module's transform providers and class definitions in one walk
/// of the module-level statements.
fn transform_providers<'ast>(
    module: &ResolvedModule,
    body: &'ast [Stmt],
) -> TransformProviders<'ast> {
    let mut providers = TransformProviders {
        functions: HashMap::new(),
        classes: HashMap::new(),
        class_defs: HashMap::new(),
    };
    for stmt in body {
        match stmt {
            Stmt::FunctionDef(function) => {
                if let Some(default) = transform_frozen_default(module, &function.decorator_list) {
                    let _ = providers.functions.insert(function.name.as_str(), default);
                }
            }
            Stmt::ClassDef(class) => {
                let _ = providers.class_defs.insert(class.name.as_str(), class);
                if let Some(default) = transform_frozen_default(module, &class.decorator_list) {
                    let _ = providers.classes.insert(class.name.as_str(), default);
                }
            }
            _ => {}
        }
    }
    providers
}

/// The simple name a base-class expression denotes: `Base` or `Base[...]`.
fn base_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(name) => Some(name.id.as_str()),
        Expr::Subscript(subscript) => base_name(&subscript.value),
        _ => None,
    }
}

/// The class's own `metaclass=` keyword when it (or a metaclass ancestor)
/// is a transform provider.
fn metaclass_transform_default(
    class: &StmtClassDef,
    providers: &TransformProviders<'_>,
) -> Option<bool> {
    let name = class
        .keywords()
        .iter()
        .find(|keyword| {
            keyword
                .arg
                .as_ref()
                .is_some_and(|arg| arg.as_str() == "metaclass")
        })
        .and_then(|keyword| base_name(&keyword.value))?;
    if let Some(default) = providers.classes.get(name) {
        return Some(*default);
    }
    let metaclass = providers.class_defs.get(name)?;
    transform_via_bases_or_metaclass(metaclass, providers)
}

/// The `frozen_default` reaching this class through PEP 681's base-class or
/// metaclass application forms: a transform class in the transitive
/// same-module base chain, or a transform metaclass on the class or any
/// transitive base. Iterative with a visited set (the same termination
/// guards as `shared::class_walks`).
fn transform_via_bases_or_metaclass(
    class: &StmtClassDef,
    providers: &TransformProviders<'_>,
) -> Option<bool> {
    let mut visited: HashSet<&str> = HashSet::new();
    let _ = visited.insert(class.name.as_str());
    let mut worklist: Vec<&StmtClassDef> = vec![class];
    while let Some(current) = worklist.pop() {
        if let Some(default) = metaclass_transform_default(current, providers) {
            return Some(default);
        }
        for base in current.bases() {
            let Some(name) = base_name(base) else {
                continue;
            };
            if let Some(default) = providers.classes.get(name) {
                return Some(*default);
            }
            if visited.insert(name) {
                worklist.extend(providers.class_defs.get(name));
            }
        }
    }
    None
}

/// The effective `frozen` when the class is decorated by a module-level
/// transform decorator function: the application-site `frozen=` keyword wins
/// over the provider's `frozen_default`.
fn applied_transform_frozen(
    class: &StmtClassDef,
    providers: &TransformProviders<'_>,
) -> Option<bool> {
    class
        .decorator_list
        .iter()
        .find_map(|decorator| match &decorator.expression {
            Expr::Name(name) => providers.functions.get(name.id.as_str()).copied(),
            Expr::Call(call) => match call.func.as_ref() {
                Expr::Name(name) => providers
                    .functions
                    .get(name.id.as_str())
                    .map(|default| keyword_bool(&call.arguments, "frozen").unwrap_or(*default)),
                _ => None,
            },
            _ => None,
        })
}

/// The class's effective `frozen` under whichever PEP 681 application form
/// governs it, `None` when none does. For the base/metaclass forms the
/// class-definition `frozen=` keyword overrides the provider default.
fn governed_frozen(class: &StmtClassDef, providers: &TransformProviders<'_>) -> Option<bool> {
    if let Some(frozen) = applied_transform_frozen(class, providers) {
        return Some(frozen);
    }
    let default = transform_via_bases_or_metaclass(class, providers)?;
    Some(
        class
            .arguments
            .as_deref()
            .and_then(|arguments| keyword_bool(arguments, "frozen"))
            .unwrap_or(default),
    )
}

/// PEP 681: every class in the module governed by a `dataclass_transform`
/// provider — via a transform decorator function, a transform base class, or
/// a transform metaclass — mapped to its effective settings.
pub(crate) fn collect_transform_classes(
    module: &ResolvedModule,
) -> HashMap<String, TransformClassInfo> {
    let Some(parsed) = super::shared::parse_module(module) else {
        return HashMap::new();
    };
    let providers = transform_providers(module, &parsed.ast.body);
    providers
        .class_defs
        .values()
        .filter_map(|class| {
            governed_frozen(class, &providers)
                .map(|frozen| (class.name.to_string(), TransformClassInfo { frozen }))
        })
        .collect()
}

/// PEP 681's base-class and metaclass application forms: `true` when the
/// class derives from a transform base or uses a transform metaclass
/// (directly or through a transitive base), so a synthesized `__init__`
/// accepts field arguments. The decorator-function form is deliberately
/// excluded — the caller handles decorator-built classes separately.
///
/// ORPHANED, NOT DELETED. Its only caller — `constructors_call_init`'s
/// `check_no_init_with_args` — was deleted for deriving base-class identity
/// from source text. This function does NOT have that defect: it reads the
/// parsed AST through `transform_providers`. It is kept because the rebuilt
/// rule needs exactly this question answered. Do not delete it to silence the
/// dead-code lint, and do not repair its caller by restoring the text walk.
#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; this AST-based helper is \
              retained for the rebuild — see tests/no_type_spelling_surgery_tests.rs"
)]
pub(crate) fn inherits_dataclass_transform(module: &ResolvedModule, class_info: &ClassInfo) -> bool {
    let Some(parsed) = super::shared::parse_module(module) else {
        return false;
    };
    let providers = transform_providers(module, &parsed.ast.body);
    providers
        .class_defs
        .get(class_info.name.as_str())
        .is_some_and(|class| transform_via_bases_or_metaclass(class, &providers).is_some())
}
