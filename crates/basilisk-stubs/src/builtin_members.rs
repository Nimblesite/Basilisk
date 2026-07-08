//! Implements [STUBRES-TYPESHED]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED
//!
//! Hover signatures for methods on builtin types.
//!
//! Backs the LSP's hover on builtin member access (GitHub #288): `" ".join(x)`
//! has no local definition and no import to resolve through, so the signature
//! comes from this curated table mirroring typeshed's `builtins.pyi` for
//! Python 3.12. Optional-argument defaults render as `...` in typeshed style.

/// Hover signature for a method on a builtin type.
///
/// Returns the full rendered signature (e.g. `def str.join(iterable:
/// Iterable[str]) -> str`) or `None` when the type or method is unknown.
#[must_use]
pub fn builtin_method_signature(type_name: &str, method: &str) -> Option<&'static str> {
    match type_name {
        "str" => str_method_signature(method),
        _ => None,
    }
}

/// Signatures for the public methods of `str` (Python 3.12).
fn str_method_signature(method: &str) -> Option<&'static str> {
    match method {
        "capitalize" => Some("def str.capitalize() -> str"),
        "casefold" => Some("def str.casefold() -> str"),
        "center" => Some("def str.center(width: int, fillchar: str = \" \") -> str"),
        "count" => Some("def str.count(sub: str, start: int = ..., end: int = ...) -> int"),
        "encode" => {
            Some("def str.encode(encoding: str = \"utf-8\", errors: str = \"strict\") -> bytes")
        }
        "endswith" => Some(
            "def str.endswith(suffix: str | tuple[str, ...], start: int = ..., end: int = ...) -> bool",
        ),
        "expandtabs" => Some("def str.expandtabs(tabsize: int = 8) -> str"),
        "find" => Some("def str.find(sub: str, start: int = ..., end: int = ...) -> int"),
        "format" => Some("def str.format(*args: object, **kwargs: object) -> str"),
        "format_map" => Some("def str.format_map(mapping: Mapping[str, object]) -> str"),
        "index" => Some("def str.index(sub: str, start: int = ..., end: int = ...) -> int"),
        "isalnum" => Some("def str.isalnum() -> bool"),
        "isalpha" => Some("def str.isalpha() -> bool"),
        "isascii" => Some("def str.isascii() -> bool"),
        "isdecimal" => Some("def str.isdecimal() -> bool"),
        "isdigit" => Some("def str.isdigit() -> bool"),
        "isidentifier" => Some("def str.isidentifier() -> bool"),
        "islower" => Some("def str.islower() -> bool"),
        "isnumeric" => Some("def str.isnumeric() -> bool"),
        "isprintable" => Some("def str.isprintable() -> bool"),
        "isspace" => Some("def str.isspace() -> bool"),
        "istitle" => Some("def str.istitle() -> bool"),
        "isupper" => Some("def str.isupper() -> bool"),
        "join" => Some("def str.join(iterable: Iterable[str]) -> str"),
        "ljust" => Some("def str.ljust(width: int, fillchar: str = \" \") -> str"),
        "lower" => Some("def str.lower() -> str"),
        "lstrip" => Some("def str.lstrip(chars: str | None = None) -> str"),
        "maketrans" => Some(
            "def str.maketrans(x: dict[int | str, object] | str, y: str = ..., z: str = ...) -> dict[int, object]",
        ),
        "partition" => Some("def str.partition(sep: str) -> tuple[str, str, str]"),
        "removeprefix" => Some("def str.removeprefix(prefix: str) -> str"),
        "removesuffix" => Some("def str.removesuffix(suffix: str) -> str"),
        "replace" => Some("def str.replace(old: str, new: str, count: int = -1) -> str"),
        "rfind" => Some("def str.rfind(sub: str, start: int = ..., end: int = ...) -> int"),
        "rindex" => Some("def str.rindex(sub: str, start: int = ..., end: int = ...) -> int"),
        "rjust" => Some("def str.rjust(width: int, fillchar: str = \" \") -> str"),
        "rpartition" => Some("def str.rpartition(sep: str) -> tuple[str, str, str]"),
        "rsplit" => Some("def str.rsplit(sep: str | None = None, maxsplit: int = -1) -> list[str]"),
        "rstrip" => Some("def str.rstrip(chars: str | None = None) -> str"),
        "split" => Some("def str.split(sep: str | None = None, maxsplit: int = -1) -> list[str]"),
        "splitlines" => Some("def str.splitlines(keepends: bool = False) -> list[str]"),
        "startswith" => Some(
            "def str.startswith(prefix: str | tuple[str, ...], start: int = ..., end: int = ...) -> bool",
        ),
        "strip" => Some("def str.strip(chars: str | None = None) -> str"),
        "swapcase" => Some("def str.swapcase() -> str"),
        "title" => Some("def str.title() -> str"),
        "translate" => Some("def str.translate(table: Mapping[int, int | str | None]) -> str"),
        "upper" => Some("def str.upper() -> str"),
        "zfill" => Some("def str.zfill(width: int) -> str"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_str_method_has_signature() {
        assert_eq!(
            builtin_method_signature("str", "join"),
            Some("def str.join(iterable: Iterable[str]) -> str")
        );
    }

    #[test]
    fn unknown_method_and_type_return_none() {
        assert_eq!(builtin_method_signature("str", "nonexistent"), None);
        assert_eq!(builtin_method_signature("MyClass", "join"), None);
    }
}
