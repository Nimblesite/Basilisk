//! Implements [`literals_literalstring`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Diagnostic constructors for `literals_literalstring`.
//!
//! Empty: every constructor here served a verdict that was reached by matching
//! the name written at a use site against a typing special form. That
//! recognition mechanism is banned permanently (see the symbol-naming ban in
//! `CLAUDE.md`), the rule is inert, and its diagnostic constructors and error
//! code went with it.
