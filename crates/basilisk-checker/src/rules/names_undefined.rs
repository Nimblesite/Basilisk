//! Implements [`names_undefined`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `names_undefined`: Reference to a name with no visible definition.
//!
//! Flags any name referenced in a `return` expression — bare (`return x`), the
//! base of an attribute/subscript chain (`return x.y`), a call argument, or the
//! **callee of a call** (`return x()`) — that is not defined in scope. A name is
//! considered defined if it is a parameter, a local assignment (`=`, `for`,
//! `with`), a module-level function, class, variable, import, or PEP 695
//! `type` alias, an enclosing scope's binding, a cross-module imported symbol,
//! or a builtin.
//!
//! Also flags a module-level statement that calls a name bound nowhere in the
//! module (issue #397), and a class that lists **its own name among its bases**
//! (issue #398) — Python evaluates the bases tuple before binding the class
//! name, so both raise `NameError` the moment the module is imported.
//! Shadowing stays legal: `class D(D)` is only flagged when the class statement
//! is the SOLE binding of that name (no earlier class, import, assignment, or
//! builtin to inherit from). A `from m import *` disables both module-level
//! passes: the star can bind any name.
//!
//! ```python
//! def compute() -> int:
//!     return undefined_name     # never defined → E0018
//!     return undefined_fn()     # undefined callee → E0018
//!
//!
//! a: int = print2("abc")        # no `print2` anywhere → E0018
//!
//! class D(D):                   # `D` unbound in its own bases → E0018
//!     pass
//! ```

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "names_undefined",
    docs_url: "https://www.basilisk-python.dev/errors/names_undefined",
};

/// Emits `names_undefined` for return statements that reference undefined names.
pub(crate) struct UndefinedVariable;

impl Rule for UndefinedVariable {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Collect all import-bound names (both `import X` and `from X import Y`).
        let import_names: Vec<&str> = module
            .imports
            .iter()
            .flat_map(|imp| {
                if imp.names.is_empty() {
                    // Plain `import X` — the bound name is the top-level module.
                    imp.module.split('.').next().into_iter().collect::<Vec<_>>()
                } else {
                    imp.names.iter().map(String::as_str).collect::<Vec<_>>()
                }
            })
            .collect();

        // Collect module-level variable names so functions can reference them.
        let module_var_names: Vec<&str> = basilisk_resolver::collect_names(&module.module_vars);

        // Module-level class names are in scope for any function body, just like
        // module-level functions, variables, and imports.
        let class_names: Vec<&str> = basilisk_resolver::collect_names(&module.classes);

        // A PEP 695 `type` statement binds its alias name to a lazily evaluated
        // `TypeAliasType` object — a first-class runtime value (issue #372).
        // Only MODULE-scope aliases are visible to every function body:
        // class-scope names don't nest, and function-scope aliases are local
        // (they reach `all_local_assigns`, so same-function use stays clean).
        let type_alias_names: Vec<&str> =
            basilisk_resolver::collect_names_where(&module.pep695_scoping.aliases, |alias| {
                !alias.in_function && !alias.in_class
            });

        let scope = ModuleScope {
            builtin_names: module.builtin_names.as_deref(),
            import_names: &import_names,
            module_var_names: &module_var_names,
            class_names: &class_names,
            type_alias_names: &type_alias_names,
            imported_symbols: &module.imported_symbols,
        };

        module.functions.iter().for_each(|func| {
            check_function(func, &module.functions, &scope, &module.path, diagnostics);
        });

        // A star import can bind any name, so it disables both module-level
        // passes entirely.
        let has_star_import = module
            .imports
            .iter()
            .any(|imp| matches!(imp.kind, basilisk_resolver::scope::ImportKind::Star));
        if has_star_import {
            return;
        }

        check_module_level_callees(module, &scope, diagnostics);
        check_self_inheriting_classes(module, diagnostics);
    }
}

/// Flag module-level calls to names bound nowhere in the module (issue #397).
///
/// `module.calls` also contains calls from function and class bodies (its
/// collector walks every body), where locals and parameters are legal callees —
/// those are excluded by span containment against `def_span`s.
fn check_module_level_callees(
    module: &ResolvedModule,
    scope: &ModuleScope<'_>,
    out: &mut Vec<Diagnostic>,
) {
    let inside_any_body = |span: &Span| {
        module
            .functions
            .iter()
            .map(|func| &func.def_span)
            .chain(module.classes.iter().map(|class| &class.def_span))
            .any(|body| body.start <= span.start && span.end <= body.end)
    };

    for call in &module.calls {
        let callee = call.callee.as_str();
        if call.receiver.is_some() || inside_any_body(&call.span) {
            continue;
        }
        if module.module_bindings.contains_key(callee)
            || scope.imported_symbols.contains_key(callee)
            || is_builtin_name(scope.builtin_names, callee)
        {
            continue;
        }
        out.push(module_level_diagnostic(callee, call.span, &module.path));
    }
}

/// Flag a class that names its own yet-unbound self among its bases (#398).
///
/// Python evaluates the bases tuple BEFORE binding the class name, so
/// `class D(D)` raises `NameError` at import time — unless the name was
/// already bound (an earlier class, import, assignment, or a builtin), in
/// which case the base legally resolves to that earlier binding. The binding
/// census counts sites, so a count of exactly 1 means the class statement is
/// the sole binder and the base reference cannot resolve.
// ##########################################################################
// # DELETED BODY — `check_self_inheriting_classes`. DO NOT RESTORE IT.
// #
// #   class.bases.iter().any(|base| base == name)   && is_builtin_name(name)
// #
// # Two spelling tests. The first compared a base's RENDERED TEXT to the
// # class's own rendered name, which is not the question: Python evaluates
// # the bases tuple against the bindings in force BEFORE the class statement,
// # so
// #
// #   class Foo: ...
// #   class Foo(Foo): ...      # LEGAL — the base is the FIRST Foo
// #
// # is fine, and the old check leaned on a binding-count census to guess
// # around that rather than resolving the base. The second suppressed the
// # diagnostic whenever the class's name appeared in a hard-coded BUILTINS
// # spelling list, so `class ascii(ascii)` was silently exempt while the
// # identical `class Foo(Foo)` was reported.
// #
// # The replacement resolves the base expression through the binding table at
// # its own offset — exactly what `BindingTable::binding_at` already models.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
fn check_self_inheriting_classes(module: &ResolvedModule, out: &mut Vec<Diagnostic>) {
    for class in &module.classes {
        for base in &class.resolved_bases {
            // ##############################################################
            // # THIS CONDITION CANNOT FIRE. THE RULE IS DEAD.
            // #
            // # An earlier comment here explained how the check "comes out
            // # right". It does not come out anything: it never runs.
            // #
            // # `resolved_bases` resolves each base POSITIONALLY, at the base
            // # expression's own offset. A class does not bind its own name
            // # until its statement completes — the resolver says so in as
            // # many words at `visitor/class_info_ext.rs::resolved_bases`:
            // # "A class never resolves to itself." So for `class Foo(Foo)`
            // # with no earlier `Foo`, the base resolves to
            // # `ResolvedBase::Unknown`, `local_site()` is `None`, and the
            // # comparison below is `None == Some(..)` — false, every time,
            // # on every input.
            // #
            // # The text-matching version this replaced DID report
            // # `class Foo(Foo)`. Replacing it with an identity comparison
            // # that cannot hold did not rebuild the rule; it deleted the
            // # diagnostic while leaving something that reads like a rule.
            // # That is worse than the panic the deletion protocol asks for,
            // # because silence looks like a clean file.
            // #
            // # What a real implementation needs: the resolver must model the
            // # PENDING class binder — the fact that a base expression named
            // # `Foo` inside `class Foo`'s own base list refers to a name
            // # that this statement is in the middle of binding and that
            // # nothing else has bound. `ResolvedBase::Unknown` throws that
            // # away. Until it is modelled, `class Foo(Foo)` goes unreported.
            // #
            // # Do not "fix" this by comparing `class.name` to the base's
            // # spelling. That is the defect that was deleted, and it is what
            // # made `class ascii(ascii)` need a builtin-spelling exemption.
            // ##############################################################
            if base.resolved.local_site() == Some(class.name_span) {
                out.push(self_inheritance_diagnostic(
                    &class.name,
                    base.span,
                    &module.path,
                ));
            }
        }
    }
}

/// `class Foo(Foo)` with no earlier binding for `Foo`.
fn self_inheritance_diagnostic(name: &str, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!("`{name}` is used as its own base class but is not defined yet"),
        span,
        path,
        Some(format!(
            "A class cannot inherit from itself; the base list is evaluated before `{name}` is bound"
        )),
        Some(
            "The bases tuple is evaluated before the class statement binds its name, so a \
             self-reference raises NameError"
                .to_owned(),
        ),
    )
}

fn module_level_diagnostic(name: &str, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!("`{name}` is called here but defined nowhere in this module"),
        span,
        path,
        Some(format!(
            "Define `{name}`, import it, or check for a typo in the name"
        )),
        Some(
            "A module-level call to an undefined name raises NameError as soon as the module \
             is imported"
                .to_owned(),
        ),
    )
}

/// Module-scope names visible to every function body.
struct ModuleScope<'a> {
    /// The BUILTIN SCOPE: every name `builtins` binds, from the active
    /// typeshed generation, or `None` when it could not be established.
    /// `None` means "unknown", which suppresses rather than reports; an EMPTY
    /// `Some` is a real answer and reports (see [`is_builtin_name`]).
    builtin_names: Option<&'a std::collections::HashSet<String>>,
    import_names: &'a [&'a str],
    module_var_names: &'a [&'a str],
    class_names: &'a [&'a str],
    type_alias_names: &'a [&'a str],
    imported_symbols:
        &'a std::collections::HashMap<String, basilisk_resolver::scope::ExternalSymbol>,
}

/// Check whether `name` is defined in any enclosing function's scope.
///
/// A function is "enclosing" if its `def_span` fully contains the current
/// function's `def_span` (i.e. the current function is nested inside it).
fn is_in_enclosing_scope(name: &str, func: &FunctionInfo, all_functions: &[FunctionInfo]) -> bool {
    let my_start = func.def_span.start;
    let my_end = func.def_span.end;

    all_functions.iter().any(|outer| {
        // Must strictly contain this function (not be the same function).
        outer.def_span.start < my_start
            && outer.def_span.end >= my_end
            && (outer.parameters.iter().any(|p| p.name == name)
                || outer.vararg.as_ref().is_some_and(|v| v.name == name)
                || outer.kwarg.as_ref().is_some_and(|k| k.name == name)
                || outer.all_local_assigns.iter().any(|a| a == name))
    })
}

// ##########################################################################
// # DELETED — the `BUILTINS` NAME WHITELIST — the whole table. DO NOT        #
// # RESTORE IT AND DO NOT REPLACE IT WITH A SMALLER LIST.                  #
// #                                                                        #
// # It was a hard-coded table of builtin SPELLINGS, consulted as           #
// # `BUILTINS.contains(&name)` at three sites, to decide whether a name    #
// # was defined. CLAUDE.md names this construct explicitly: "a whitelist   #
// # of `int`/`str`/`isinstance` names. Builtins are not an exception —     #
// # Python lets any name be shadowed, rebound, or aliased, so builtin uses #
// # resolve through the binding table like everything else."               #
// #                                                                        #
// # Both directions were wrong. A module that rebinds a builtin name kept  #
// # the whitelist's blessing, and any user symbol that happened to be      #
// # SPELLED like a builtin inherited it too:                               #
// #                                                                        #
// #   class ascii(ascii): ...   # self-referencing base — suppressed only  #
// #                             # because "ascii" is in the list, while    #
// #                             # `class Foo(Foo)` is correctly reported    #
// #                                                                        #
// # `BindingTable::binds_name` / `form_of_with_builtins` already answer    #
// # this lawfully: a bare name the module never rebinds resolves to its    #
// # `builtins` definition, and a rebinding at or before the use site stops  #
// # it. That is the replacement — resolution, not membership.              #
// #                                                                        #
// # Pinned by:                                                             #
// #   crates/basilisk-checker/tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################

/// Whether `name` is bound in the BUILTIN SCOPE this module is checked
/// against, and not shadowed by the module itself.
///
/// REBUILT from a hard-coded whitelist of builtin SPELLINGS. That list decided
/// whether a name was defined by looking its characters up in a table, so a
/// module that rebound a builtin name kept the whitelist's blessing, and any
/// user symbol merely SPELLED like a builtin inherited one:
///
/// ```python
/// class ascii(ascii): ...   # self-referencing base, suppressed only because
///                           # "ascii" was in the list, while the identical
///                           # `class Foo(Foo)` was correctly reported
/// ```
///
/// `ResolvedModule::builtin_names` is the `builtins` module's own namespace,
/// read from the active typeshed generation, so the answer tracks the
/// configured target version instead of a table in this file. A name the
/// module binds itself is NOT resolved here — Python looks in the module scope
/// first, and the callers have already checked it.
///
/// `None` — the builtin scope could not be established — means "unknown", not
/// "no builtins": every caller uses this to SUPPRESS a diagnostic, so an
/// unknown scope suppresses rather than inventing a report about a name it
/// cannot see ([CHKARCH-CONFORMANCE-MODE]).
///
/// An EMPTY `Some` is a different fact: the scope WAS read and binds nothing.
/// That is not the same as not knowing, and it does not suppress. The two used
/// to share one value, and a loader bug that produced the empty one on every
/// run silently disabled this rule everywhere.
fn is_builtin_name(builtin_names: Option<&std::collections::HashSet<String>>, name: &str) -> bool {
    match builtin_names {
        Some(names) => names.contains(name),
        None => true,
    }
}

fn check_function(
    func: &FunctionInfo,
    all_functions: &[FunctionInfo],
    scope: &ModuleScope<'_>,
    path: &str,
    out: &mut Vec<Diagnostic>,
) {
    let param_names: Vec<&str> = basilisk_resolver::collect_names(&func.parameters);

    for (name, span) in &func.return_name_refs {
        let name_str = name.as_str();
        if param_names.contains(&name_str)
            || func.vararg.as_ref().is_some_and(|v| v.name == name_str)
            || func.kwarg.as_ref().is_some_and(|k| k.name == name_str)
            || func.all_local_assigns.iter().any(|a| a == name)
            || scope.import_names.contains(&name_str)
            || scope.module_var_names.contains(&name_str)
            || scope.class_names.contains(&name_str)
            || scope.type_alias_names.contains(&name_str)
            || scope.imported_symbols.contains_key(name_str)
            // Any function defined in the module (sibling, nested, or the function
            // itself for recursion) is a name in scope — `return helper()` and
            // `return helper` must not be flagged for a real `def helper`.
            || all_functions.iter().any(|f| f.name == name_str)
            || is_in_enclosing_scope(name_str, func, all_functions)
            || is_builtin_name(scope.builtin_names, name_str)
        {
            continue;
        }
        out.push(make_diagnostic(func, name, *span, path));
    }
}

fn make_diagnostic(func: &FunctionInfo, name: &str, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Function `{}` returns `{name}` but `{name}` is not defined in this scope",
            func.name
        ),
        span,
        path,
        Some(format!(
            "Define `{name}` before returning it, or check for a typo"
        )),
        Some(
            "Basilisk detects names in return expressions that have no visible definition"
                .to_owned(),
        ),
    )
}
