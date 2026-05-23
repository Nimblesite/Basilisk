//! Tests for [STUBRES-ENGINE]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-ENGINE
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for basilisk-stubs.

#[test]
fn lookup_builtin_str_type() {
    // Phase 5: the stubs library must know about Python built-in types.
    // Currently returns None for all names (placeholder).
    assert!(
        basilisk_stubs::lookup_builtin("str").is_some(),
        "str must be a known builtin type — Phase 5 stubs not yet implemented"
    );
}

#[test]
fn lookup_builtin_int_type() {
    // Phase 5: int must be a known built-in type.
    assert!(
        basilisk_stubs::lookup_builtin("int").is_some(),
        "int must be a known builtin type — Phase 5 stubs not yet implemented"
    );
}

#[test]
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

#[test]
fn lookup_builtin_float_type() {
    assert_eq!(basilisk_stubs::lookup_builtin("float"), Some("float"));
}

#[test]
fn lookup_builtin_bytes_type() {
    assert_eq!(basilisk_stubs::lookup_builtin("bytes"), Some("bytes"));
}

#[test]
fn lookup_builtin_bool_type() {
    assert_eq!(basilisk_stubs::lookup_builtin("bool"), Some("bool"));
}

#[test]
fn lookup_builtin_dict_type() {
    assert_eq!(basilisk_stubs::lookup_builtin("dict"), Some("dict"));
}

#[test]
fn lookup_builtin_set_type() {
    assert_eq!(basilisk_stubs::lookup_builtin("set"), Some("set"));
}

#[test]
fn lookup_builtin_tuple_type() {
    assert_eq!(basilisk_stubs::lookup_builtin("tuple"), Some("tuple"));
}

#[test]
fn lookup_builtin_frozenset_type() {
    assert_eq!(
        basilisk_stubs::lookup_builtin("frozenset"),
        Some("frozenset")
    );
}

#[test]
fn lookup_builtin_type_type() {
    assert_eq!(basilisk_stubs::lookup_builtin("type"), Some("type"));
}

#[test]
fn lookup_builtin_object_type() {
    assert_eq!(basilisk_stubs::lookup_builtin("object"), Some("object"));
}

#[test]
fn lookup_builtin_none_type() {
    assert_eq!(basilisk_stubs::lookup_builtin("None"), Some("None"));
}

#[test]
fn lookup_builtin_complex_type() {
    assert_eq!(basilisk_stubs::lookup_builtin("complex"), Some("complex"));
}

#[test]
fn lookup_builtin_range_type() {
    assert_eq!(basilisk_stubs::lookup_builtin("range"), Some("range"));
}

#[test]
fn lookup_builtin_bytearray_type() {
    assert_eq!(
        basilisk_stubs::lookup_builtin("bytearray"),
        Some("bytearray")
    );
}

#[test]
fn lookup_builtin_memoryview_type() {
    assert_eq!(
        basilisk_stubs::lookup_builtin("memoryview"),
        Some("memoryview")
    );
}
