//! Implements [names_undefined] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! names_undefined: Undefined variable used in a return statement.
//!
//! Flags any name referenced in a `return` expression — bare (`return x`), the
//! base of an attribute/subscript chain (`return x.y`), a call argument, or the
//! **callee of a call** (`return x()`) — that is not defined in scope. A name is
//! considered defined if it is a parameter, a local assignment (`=`, `for`,
//! `with`), a module-level function, class, variable, or import, an enclosing
//! scope's binding, a cross-module imported symbol, or a builtin.
//!
//! ```python
//! def compute() -> int:
//!     return undefined_name     # never defined → E0018
//!     return undefined_fn()     # undefined callee → E0018
//! ```

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "names_undefined",
    docs_url: "https://www.basilisk-python.dev/errors/names_undefined",
};

/// Emits names_undefined for return statements that reference undefined names.
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
        let class_names: Vec<&str> = module.classes.iter().map(|c| c.name.as_str()).collect();

        let scope = ModuleScope {
            import_names: &import_names,
            module_var_names: &module_var_names,
            class_names: &class_names,
            imported_symbols: &module.imported_symbols,
        };

        module.functions.iter().for_each(|func| {
            check_function(func, &module.functions, &scope, &module.path, diagnostics);
        });
    }
}

/// Module-scope names visible to every function body.
struct ModuleScope<'a> {
    import_names: &'a [&'a str],
    module_var_names: &'a [&'a str],
    class_names: &'a [&'a str],
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
/// When a function returns one of these names (e.g. `return int`), it is not
/// an undefined-variable error — it is a reference to a builtin type/function.
const BUILTINS: &[&str] = &[
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
