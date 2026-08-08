//! Pins the defect that condemns `InferredType::from_annotation`
//! ([TYPEINF-LEGACY]): a type's identity is decided by the SPELLING of the
//! annotation text, not by the binding the name resolves to.
//!
//! `from_annotation` lowercases its input and then string-matches builtin
//! spellings (`"list["`, `"dict["`, `"set["`, `"int"`, `"str"`, …). A
//! user-defined class whose name happens to lowercase onto one of those
//! spellings is therefore silently treated as the builtin, and every
//! diagnostic that depends on the distinction disappears.
//!
//! Each test below is the SAME program twice, differing only in the name of a
//! locally-defined class. Renaming a user class is semantics-preserving with
//! respect to these diagnostics, so both halves must agree. They do not.
//!
//! These tests are expected to FAIL until the text parser is gone and every
//! consumer resolves the annotation expression through
//! [`basilisk_checker::annotation`] (or abstains).

mod common;

use common::{assert_rule_count, run};

/// Diagnostic codes emitted for `source`.
fn codes(source: &str) -> Vec<String> {
    let diagnostics = run(source).expect("checker ran");
    let mut out: Vec<String> = diagnostics
        .iter()
        .map(|d| d.code.code.to_owned())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The control and the probe differ ONLY in the name of the user class, which
/// cannot change any typing verdict. Assert they produce the same diagnostics.
fn assert_rename_invariant(control_class: &str, probe_class: &str, value: &str) {
    let program = |class_name: &str| {
        format!(
            "class {class_name}:\n    pass\n\n\nAlias = {class_name}\nq: Alias = {value}\n"
        )
    };
    let control = codes(&program(control_class));
    let probe = codes(&program(probe_class));
    assert_eq!(
        control, probe,
        "renaming the user class `{control_class}` to `{probe_class}` changed the \
         diagnostics from {control:?} to {probe:?}; a type's identity comes from the \
         binding it resolves to, never from how the name is spelled"
    );
}

#[test]
fn user_class_named_list_is_not_the_builtin_list() {
    assert_rename_invariant("Widget", "List", "[1]");
}

#[test]
fn user_class_named_dict_is_not_the_builtin_dict() {
    assert_rename_invariant("Widget", "Dict", "{1: 2}");
}

#[test]
fn user_class_named_set_is_not_the_builtin_set() {
    assert_rename_invariant("Widget", "Set", "{1}");
}

#[test]
fn user_class_named_int_is_not_the_builtin_int() {
    assert_rename_invariant("Widget", "Int", "5");
}

#[test]
fn user_class_named_str_is_not_the_builtin_str() {
    assert_rename_invariant("Widget", "Str", "\"text\"");
}

/// A list literal is not an instance of a user-defined class, whatever that
/// class is called. Stated directly, so the failure is readable even if the
/// invariant helper above is ever weakened.
#[test]
fn list_literal_is_not_assignable_to_a_user_class_called_list() {
    let source = "class List:\n    pass\n\n\nAlias = List\nq: Alias = [1]\n";
    let diagnostics = run(source).expect("checker ran");
    assert_rule_count(
        &diagnostics,
        "assignment_compatibility",
        1,
        "a list literal is not an instance of the user-defined class `List`; it is \
         accepted only because the annotation text lowercases onto the builtin \
         spelling `list`",
    );
}
