//! Implements helpers for [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Shared guard predicates used across multiple rules.
//!
//! These predicates identify Python typing patterns where strict annotation
//! enforcement is suspended because the construct has well-defined PEP
//! semantics that legitimately omit annotations. Every verdict reads the
//! resolver's binding-resolved flags — never a decorator's or base's spelling.

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, FunctionInfo, ResolvedModule, Span, TypingForm};
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
/// its `frozen_default`, plus every module-level class definition.
///
/// KEYED ON DEFINITION SITE — the span of the `def`/`class` statement's own
/// name token, which is unique per definition in a module and is what
/// [`basilisk_resolver::BindingTable::local_class_definition`] answers with.
/// The deleted version keyed all three maps on the RENDERED name, so a
/// transform base reached under an alias was not a transform base and an
/// unrelated class merely spelled alike was.
struct TransformProviders<'ast> {
    /// Module-level functions carrying `@dataclass_transform(...)`.
    functions: HashMap<Span, bool>,
    /// Module-level classes carrying `@dataclass_transform(...)`.
    classes: HashMap<Span, bool>,
    /// Every module-level class definition, so a base resolved to a site can
    /// be walked further.
    class_defs: HashMap<Span, &'ast StmtClassDef>,
}

/// The definition site of a class statement: its own name token.
///
/// Identical to [`basilisk_resolver::ClassInfo::name_span`], so a map built
/// here joins with the resolver's class table and with
/// [`basilisk_resolver::ClassGraph`].
fn class_site(class: &StmtClassDef) -> Span {
    Span::from(class.name.range)
}

/// The definition site of the class a base or `metaclass=` expression denotes,
/// resolved through the module's binding table.
///
/// `Base`, `Base[T]`, and `Alias` (bound by `Alias = Base`) all reach `Base`'s
/// definition; a base imported from another module reaches `None`, which is
/// abstention and never a negative answer ([CHKARCH-CONFORMANCE-MODE]).
fn resolved_class_site(module: &ResolvedModule, expr: &Expr) -> Option<Span> {
    module.bindings.local_class_definition(expr).map(Span::from)
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
    decorators
        .iter()
        .find_map(|decorator| match &decorator.expression {
            Expr::Call(call)
                if module
                    .bindings
                    .is_form(&call.func, TypingForm::DataclassTransform) =>
            {
                Some(keyword_bool(&call.arguments, "frozen_default").unwrap_or(false))
            }
            expr if module
                .bindings
                .is_form(expr, TypingForm::DataclassTransform) =>
            {
                Some(false)
            }
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
                    let _ = providers
                        .functions
                        .insert(Span::from(function.name.range), default);
                }
            }
            Stmt::ClassDef(class) => {
                let _ = providers.class_defs.insert(class_site(class), class);
                if let Some(default) = transform_frozen_default(module, &class.decorator_list) {
                    let _ = providers.classes.insert(class_site(class), default);
                }
            }
            _ => {}
        }
    }
    providers
}

/// The `metaclass=` expression of a class statement, if it has one.
///
/// `metaclass` is a keyword of the `class` statement's own grammar, not a name
/// the user may rebind, so matching it is syntax rather than spelling.
fn metaclass_expr(class: &StmtClassDef) -> Option<&Expr> {
    class
        .keywords()
        .iter()
        .find(|keyword| {
            keyword
                .arg
                .as_ref()
                .is_some_and(|arg| arg.as_str() == "metaclass")
        })
        .map(|keyword| &keyword.value)
}

/// The `frozen_default` reaching this class through PEP 681's base-class or
/// metaclass application forms: a transform class in the transitive
/// same-module base chain, or a transform metaclass on the class or any
/// transitive base.
///
/// REBUILT ON RESOLVED IDENTITY. The deleted version walked
/// `providers.classes.get(name)` / `providers.class_defs.get(name)` with a
/// `visited` set of name STRINGS, so:
///
/// * `Provider = TransformBase; class Model(Provider)` found no provider,
///   because `"Provider"` is not `"TransformBase"`;
/// * a local class merely spelled like a provider WAS treated as one;
/// * two same-named classes in one module shared a single `visited` entry, so
///   the walk stopped early on one of them.
///
/// Every hop now goes through [`resolved_class_site`], and `visited` holds
/// definition sites. A base this module does not define resolves to `None` and
/// contributes nothing — abstention, never a negative
/// ([CHKARCH-CONFORMANCE-MODE]). Iterative with a visited set, the same
/// termination guard as `shared::class_walks`.
fn transform_via_bases_or_metaclass(
    module: &ResolvedModule,
    class: &StmtClassDef,
    providers: &TransformProviders<'_>,
) -> Option<bool> {
    let mut pending: Vec<&StmtClassDef> = vec![class];
    let mut visited: std::collections::HashSet<Span> = std::collections::HashSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(class_site(current)) {
            continue;
        }
        // The metaclass application form, on this class or any base already
        // reached. A transform metaclass answers immediately; a metaclass this
        // module defines but which carries no transform is itself walked, so a
        // metaclass INHERITING the transform still applies.
        let metaclass = metaclass_expr(current).and_then(|expr| resolved_class_site(module, expr));
        if let Some(site) = metaclass {
            if let Some(default) = providers.classes.get(&site) {
                return Some(*default);
            }
            if let Some(definition) = providers.class_defs.get(&site) {
                pending.push(definition);
            }
        }
        // The base-class application form.
        for base in current
            .arguments
            .as_deref()
            .map(|arguments| arguments.args.as_ref())
            .unwrap_or_default()
        {
            let Some(site) = resolved_class_site(module, base) else {
                continue;
            };
            if let Some(default) = providers.classes.get(&site) {
                return Some(*default);
            }
            if let Some(definition) = providers.class_defs.get(&site) {
                pending.push(definition);
            }
        }
    }
    None
}

/// The effective `frozen` when the class is decorated by a module-level
/// transform decorator function: the application-site `frozen=` keyword wins
/// over the provider's `frozen_default`.
///
/// The decorator is resolved to a function DEFINITION SITE, so
/// `shorthand = model_transform; @shorthand` applies the transform and a
/// decorator merely spelled like a provider does not.
fn applied_transform_frozen(
    module: &ResolvedModule,
    class: &StmtClassDef,
    providers: &TransformProviders<'_>,
) -> Option<bool> {
    class.decorator_list.iter().find_map(|decorator| {
        let (callee, applied) = match &decorator.expression {
            Expr::Call(call) => (call.func.as_ref(), Some(&call.arguments)),
            expression => (expression, None),
        };
        let site = module
            .bindings
            .local_function_definition(callee)
            .map(Span::from)?;
        let default = providers.functions.get(&site)?;
        Some(
            applied
                .and_then(|arguments| keyword_bool(arguments, "frozen"))
                .unwrap_or(*default),
        )
    })
}

/// The class's effective `frozen` under whichever PEP 681 application form
/// governs it, `None` when none does. For the base/metaclass forms the
/// class-definition `frozen=` keyword overrides the provider default.
fn governed_frozen(
    module: &ResolvedModule,
    class: &StmtClassDef,
    providers: &TransformProviders<'_>,
) -> Option<bool> {
    if let Some(frozen) = applied_transform_frozen(module, class, providers) {
        return Some(frozen);
    }
    let default = transform_via_bases_or_metaclass(module, class, providers)?;
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
///
/// KEYED ON DEFINITION SITE. This map used to be keyed on the class's rendered
/// NAME, and its consumers looked up by `ClassInfo::name`, so two classes
/// spelled alike in one module collapsed into a single entry and whichever was
/// collected last decided `frozen` for both. `ClassInfo::name_span` is the
/// lawful key on the consumer side.
pub(crate) fn collect_transform_classes(
    module: &ResolvedModule,
) -> HashMap<Span, TransformClassInfo> {
    let Some(parsed) = super::shared::parse_module(module) else {
        return HashMap::new();
    };
    let providers = transform_providers(module, &parsed.ast.body);
    providers
        .class_defs
        .values()
        .filter_map(|class| {
            governed_frozen(module, class, &providers)
                .map(|frozen| (class_site(class), TransformClassInfo { frozen }))
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
pub(crate) fn inherits_dataclass_transform(
    module: &ResolvedModule,
    class_info: &ClassInfo,
) -> bool {
    let Some(parsed) = super::shared::parse_module(module) else {
        return false;
    };
    let providers = transform_providers(module, &parsed.ast.body);
    providers
        .class_defs
        .get(&class_info.name_span)
        .is_some_and(|class| transform_via_bases_or_metaclass(module, class, &providers).is_some())
}
