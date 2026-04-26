//! Integration tests for basilisk-stubs.

#[test]
#[ignore = "Phase 5 not yet implemented — builtin stubs are placeholders"]
fn lookup_builtin_str_type() {
    // Phase 5: the stubs library must know about Python built-in types.
    // Currently returns None for all names (placeholder).
    assert!(
        basilisk_stubs::lookup_builtin("str").is_some(),
        "str must be a known builtin type — Phase 5 stubs not yet implemented"
    );
}

#[test]
#[ignore = "Phase 5 not yet implemented — builtin stubs are placeholders"]
fn lookup_builtin_int_type() {
    // Phase 5: int must be a known built-in type.
    assert!(
        basilisk_stubs::lookup_builtin("int").is_some(),
        "int must be a known builtin type — Phase 5 stubs not yet implemented"
    );
}

#[test]
#[ignore = "Phase 5 not yet implemented — builtin stubs are placeholders"]
fn lookup_builtin_list_type() {
    // Phase 5: list must be a known built-in type.
    assert!(
        basilisk_stubs::lookup_builtin("list").is_some(),
        "list must be a known builtin type — Phase 5 stubs not yet implemented"
    );
}

#[test]
fn lookup_unknown_name_returns_none() {
    // Unknown symbols must always return None.
    assert!(
        basilisk_stubs::lookup_builtin("definitely_not_a_real_builtin").is_none(),
        "unknown names must return None"
    );
}
