use std::collections::HashMap;
use std::path::PathBuf;

use basilisk_resolver::{ImportedModuleApi, ResolvedModule};

use crate::diagnostic::Diagnostic;
use crate::rules::Rule;

use super::ModuleAttributeUndefined;

/// Build a one-entry `imported_modules` map for `module` declaring `members`.
fn imported(
    module: &str,
    members: &[&str],
    has_getattr: bool,
) -> HashMap<String, ImportedModuleApi> {
    let mut map = HashMap::new();
    let _ = map.insert(
        module.to_owned(),
        ImportedModuleApi {
            member_names: members.iter().map(|s| (*s).to_owned()).collect(),
            has_getattr,
            stub_path: PathBuf::from(format!(".basilisk/stubs/{module}.pyi")),
        },
    );
    map
}

/// Run the rule over `source` with the given `imported_modules`.
fn run(source: &str, imported_modules: HashMap<String, ImportedModuleApi>) -> Vec<Diagnostic> {
    let module = ResolvedModule {
        source: source.to_owned(),
        path: "test.py".to_owned(),
        imported_modules,
        ..ResolvedModule::default()
    };
    let mut diagnostics = Vec::new();
    ModuleAttributeUndefined.check(
        &module,
        &crate::context::CheckContext::default(),
        &mut diagnostics,
    );
    diagnostics
}

#[test]
fn flags_undeclared_attribute_inside_function() {
    // The reported case: access lives in a function body, stub declares only `tux`.
    let src =
        "import cowsay\ndef moo(m: str) -> str:\n    return cowsay.get_output_string(\"cow\", m)\n";
    let diags = run(src, imported("cowsay", &["tux"], false));
    assert_eq!(diags.len(), 1, "{diags:#?}");
    assert_eq!(diags[0].code.code, "imports_module_attribute");
    assert!(diags[0].message.contains("has no attribute"));
    assert!(diags[0].message.contains("cowsay"));
    assert!(diags[0].message.contains("get_output_string"));
}

#[test]
fn help_points_at_stub_and_getattr_opt_out() {
    let diags = run(
        "import cowsay\nx = cowsay.bogus\n",
        imported("cowsay", &["tux"], false),
    );
    assert_eq!(diags.len(), 1);
    let help = diags[0].help.as_deref().unwrap_or_default();
    assert!(help.contains(".basilisk/stubs/cowsay.pyi"), "{help}");
    assert!(help.contains("__getattr__"), "{help}");
}

#[test]
fn allows_declared_attribute() {
    let diags = run(
        "import cowsay\nx = cowsay.get_output_string\n",
        imported("cowsay", &["get_output_string", "tux"], false),
    );
    assert!(diags.is_empty(), "{diags:#?}");
}

#[test]
fn getattr_opt_out_allows_any_attribute() {
    // Stub keeps `def __getattr__` → everything is permitted (the default skeleton).
    let diags = run(
        "import cowsay\nx = cowsay.anything_at_all\n",
        imported("cowsay", &[], true),
    );
    assert!(diags.is_empty(), "{diags:#?}");
}

#[test]
fn allows_module_dunders() {
    let diags = run(
        "import cowsay\nx = cowsay.__name__\ny = cowsay.__doc__\n",
        imported("cowsay", &[], false),
    );
    assert!(diags.is_empty(), "{diags:#?}");
}

#[test]
fn flags_top_level_access() {
    let diags = run(
        "import cowsay\ncowsay.bogus()\n",
        imported("cowsay", &["tux"], false),
    );
    assert_eq!(diags.len(), 1);
    assert!(diags[0].message.contains("bogus"));
}

#[test]
fn no_op_when_no_user_stub_imported() {
    // Empty `imported_modules` (no local stubs) → the rule never fires, even on
    // an obviously-undefined access. This is what keeps conformance/first-party
    // code free of false positives.
    let diags = run("import cowsay\nx = cowsay.whatever\n", HashMap::new());
    assert!(diags.is_empty(), "{diags:#?}");
}

#[test]
fn ignores_access_on_unrelated_base() {
    // `other` is not a stub-backed module binding — its attributes are untouched.
    let diags = run(
        "import cowsay\nother = object()\nz = other.foo\n",
        imported("cowsay", &["tux"], false),
    );
    assert!(diags.is_empty(), "{diags:#?}");
}

#[test]
fn flags_each_distinct_undeclared_attribute() {
    let src = "import cowsay\na = cowsay.one\nb = cowsay.two\n";
    let diags = run(src, imported("cowsay", &["tux"], false));
    assert_eq!(diags.len(), 2, "{diags:#?}");
}
