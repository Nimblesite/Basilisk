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
            || BUILTINS.contains(&callee)
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
fn check_self_inheriting_classes(module: &ResolvedModule, out: &mut Vec<Diagnostic>) {
    for class in &module.classes {
        let name = class.name.as_str();
        let sole_binding = module.module_bindings.get(name) == Some(&1);
        if !sole_binding || !class.bases.iter().any(|base| base == name) || BUILTINS.contains(&name)
        {
            continue;
        }
        out.push(error_diagnostic_owned(
            CODE.clone(),
            format!("Class `{name}` lists itself as a base, but `{name}` is not bound until the class statement completes"),
            class.name_span,
            &module.path,
            Some(format!(
                "Inherit from a different class, or bind another `{name}` (import or definition) before this one"
            )),
            Some(
                "Python evaluates base classes before binding the class name, so this raises \
                 NameError when the module is imported"
                    .to_owned(),
            ),
        ));
    }
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

/// Python builtin names that are always in scope.
///
/// When code references one of these names (e.g. `return int`, a module-level
/// `divmod(...)`), it is not an undefined-variable error — it is a reference
/// to a builtin type/function. The list is the complete `builtins` module
/// surface (union across supported Python versions, so version-gated names
/// like `anext` or `PythonFinalizationError` never false-positive), plus the
/// module-level dunder globals every module receives and the `site`-installed
/// interactive helpers (`help`, `exit`, ...).
const BUILTINS: &[&str] = &[
    "aiter",
    "anext",
    "ascii",
    "breakpoint",
    "compile",
    "copyright",
    "credits",
    "divmod",
    "eval",
    "exec",
    "exit",
    "globals",
    "help",
    "license",
    "locals",
    "quit",
    "__import__",
    "__build_class__",
    "__debug__",
    "__name__",
    "__file__",
    "__doc__",
    "__package__",
    "__spec__",
    "__loader__",
    "__builtins__",
    "__annotations__",
    "__dict__",
    "BaseExceptionGroup",
    "ExceptionGroup",
    "BlockingIOError",
    "BrokenPipeError",
    "ChildProcessError",
    "ConnectionAbortedError",
    "ConnectionError",
    "ConnectionRefusedError",
    "ConnectionResetError",
    "EncodingWarning",
    "EnvironmentError",
    "FileExistsError",
    "FloatingPointError",
    "InterruptedError",
    "IsADirectoryError",
    "MemoryError",
    "NotADirectoryError",
    "PermissionError",
    "ProcessLookupError",
    "PythonFinalizationError",
    "ReferenceError",
    "TimeoutError",
    "WindowsError",
    "int",
    "str",
    "float",
    "bool",
    "bytes",
    "complex",
    "list",
    "dict",
    "set",
    "tuple",
    "frozenset",
    "bytearray",
    "memoryview",
    "type",
    "object",
    "range",
    "slice",
    "property",
    "staticmethod",
    "classmethod",
    "super",
    "None",
    "True",
    "False",
    "Ellipsis",
    "NotImplemented",
    "len",
    "print",
    "repr",
    "hash",
    "id",
    "isinstance",
    "issubclass",
    "callable",
    "iter",
    "next",
    "enumerate",
    "zip",
    "map",
    "filter",
    "reversed",
    "sorted",
    "min",
    "max",
    "sum",
    "abs",
    "round",
    "pow",
    "any",
    "all",
    "dir",
    "vars",
    "getattr",
    "setattr",
    "delattr",
    "hasattr",
    "open",
    "input",
    "chr",
    "ord",
    "hex",
    "oct",
    "bin",
    "format",
    "Exception",
    "BaseException",
    "TypeError",
    "ValueError",
    "KeyError",
    "IndexError",
    "AttributeError",
    "RuntimeError",
    "StopIteration",
    "OSError",
    "IOError",
    "FileNotFoundError",
    "NotImplementedError",
    "OverflowError",
    "ZeroDivisionError",
    "ImportError",
    "ModuleNotFoundError",
    "NameError",
    "UnboundLocalError",
    "LookupError",
    "ArithmeticError",
    "AssertionError",
    "BufferError",
    "EOFError",
    "GeneratorExit",
    "KeyboardInterrupt",
    "SystemExit",
    "SystemError",
    "RecursionError",
    "StopAsyncIteration",
    "SyntaxError",
    "IndentationError",
    "TabError",
    "UnicodeError",
    "UnicodeDecodeError",
    "UnicodeEncodeError",
    "UnicodeTranslateError",
    "Warning",
    "DeprecationWarning",
    "PendingDeprecationWarning",
    "RuntimeWarning",
    "SyntaxWarning",
    "ResourceWarning",
    "FutureWarning",
    "ImportWarning",
    "UnicodeWarning",
    "BytesWarning",
    "UserWarning",
];

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
            || BUILTINS.contains(&name_str)
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
