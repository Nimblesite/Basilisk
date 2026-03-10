//! Bundled type stubs for Basilisk.
//!
//! Will house typeshed bundles and the auto-stub generation engine in Phase 5.

/// Look up the type annotation string for a built-in symbol.
///
/// Returns type information for Python built-in types.
/// Unknown names return `None`.
#[must_use]
pub fn lookup_builtin(name: &str) -> Option<&'static str> {
    match name {
        "int" => Some("int"),
        "float" => Some("float"),
        "str" => Some("str"),
        "bytes" => Some("bytes"),
        "bool" => Some("bool"),
        "list" => Some("list"),
        "dict" => Some("dict"),
        "set" => Some("set"),
        "tuple" => Some("tuple"),
        "frozenset" => Some("frozenset"),
        "type" => Some("type"),
        "object" => Some("object"),
        "None" => Some("None"),
        "complex" => Some("complex"),
        "range" => Some("range"),
        "bytearray" => Some("bytearray"),
        "memoryview" => Some("memoryview"),
        _ => None,
    }
}
