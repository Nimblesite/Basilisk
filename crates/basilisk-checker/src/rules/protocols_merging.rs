//! Implements [BSK-E0098] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! BSK-E0098: Non-Protocol base class in a Protocol definition.
//!
//! Per PEP 544, a Protocol class may only inherit from other Protocol classes
//! (with the exception of `object`). Inheriting from a non-Protocol concrete
//! class is a violation.
//!
//! ```python
//! from typing import Protocol
//!
//! class Base:
//!     x: int = 0
//!
//! class BadProto(Base, Protocol):  # E — Base is not a Protocol
//!     def method(self) -> int: ...
//! ```

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0098",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0098",
};

/// Names that are always allowed as bases alongside `Protocol`.
const ALLOWED_BASES: &[&str] = &["Protocol", "object", "Generic", "ABC", "ABCMeta"];

/// Well-known stdlib Protocol classes that may be imported.
const KNOWN_PROTOCOLS: &[&str] = &[
    "Sized",
    "Hashable",
    "Iterable",
    "Iterator",
    "Reversible",
    "Container",
    "Collection",
    "Callable",
    "Awaitable",
    "AsyncIterable",
    "AsyncIterator",
    "AsyncGenerator",
    "Generator",
    "Sequence",
    "MutableSequence",
    "Set",
    "MutableSet",
    "Mapping",
    "MutableMapping",
    "ByteString",
    "SupportsInt",
    "SupportsFloat",
    "SupportsComplex",
    "SupportsBytes",
    "SupportsAbs",
    "SupportsRound",
    "SupportsIndex",
    "Buffer",
    "ContextManager",
    "AsyncContextManager",
    "runtime_checkable",
];

/// Emits BSK-E0098 when a Protocol class inherits from a non-Protocol base.
pub(crate) struct NonProtocolBaseInProtocol;

/// Check if a class name refers to a Protocol class (has `Protocol` in its bases).
fn is_protocol_class(name: &str, module: &ResolvedModule) -> bool {
    module
        .classes
        .iter()
        .any(|cls| cls.name == name && cls.bases.iter().any(|b| b == "Protocol"))
}

impl Rule for NonProtocolBaseInProtocol {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for class in &module.classes {
            // Only check classes that have Protocol as a base.
            let has_protocol = class.bases.iter().any(|b| b == "Protocol");
            if !has_protocol {
                continue;
            }

            // Check each base: it must be either an allowed name or a Protocol class.
            for base_name in &class.bases {
                if ALLOWED_BASES.contains(&base_name.as_str()) {
                    continue;
                }

                // Known stdlib Protocol classes (imported, not defined in module).
                if KNOWN_PROTOCOLS.contains(&base_name.as_str()) {
                    continue;
                }

                if is_protocol_class(base_name, module) {
                    continue;
                }

                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Non-protocol class `{base_name}` cannot be a base of protocol `{}`",
                        class.name
                    ),
                    class.def_span,
                    &module.path,
                    Some(format!(
                        "All bases of a Protocol class must also be protocols; \
                         `{base_name}` does not inherit from `Protocol`"
                    )),
                    Some(
                        "Per PEP 544, a Protocol class may only inherit from other \
                         Protocol classes (aside from `object`)"
                            .to_owned(),
                    ),
                ));
            }
        }
    }
}
