# Checker Advanced Features — Implementation Plan {#CHKADVPLAN}

> **Spec**: [CHECKER-ARCHITECTURE-SPEC.md](../specs/CHECKER-ARCHITECTURE-SPEC.md) — read the
> relevant sections before touching code.

---

## Status {#CHKADVPLAN-STATUS}

Concrete-but-unbuilt checker capabilities that the architecture spec describes and that had
no owning plan. These are real planned features (not bloat), captured here as ordered TODOs
so each spec section is referenced by a plan rather than left orphaned. None of this blocks
the prime directive (PEP conformance) — it is opt-in surface area beyond the spec rules.

---

## TODO {#CHKADVPLAN-TODO}

### Mojo-style safety checks (`basilisk-mojo`) {#CHKADVPLAN-TODO-MOJO}

- [ ] Ownership tracking via `Borrowed`/`InOut`/`Owned` `Annotated` conventions: mutation-of-borrowed and use-after-move detection ([CHKARCH-MOJO-OWNERSHIP]).
- [ ] Parameter-immutability-by-default analysis with `InOut` opt-out + frozen-dataclass nudges ([CHKARCH-MOJO-IMMUTABLE]).
- [ ] Structural-discipline checks: dynamic-attribute-on-typed-class, missing `__init__`, `__slots__` suggestions ([CHKARCH-MOJO-STRUCTURAL]).
- [ ] No-implicit-coercion checks (int→float, bool→int, bytes→str) ([CHKARCH-MOJO-COERCION]).
- [ ] Mojo-concept → Basilisk-check mapping table backing the above ([CHKARCH-MOJO-COMPAT]).

### Auto-stub generation {#CHKADVPLAN-TODO-STUBS}

- [ ] Stub-generation engine with runtime/AST/hybrid modes ([CHKARCH-STUBS], [CHKARCH-STUBS-AUTOGEN]).
- [ ] Three quality tiers for generated stubs ([CHKARCH-STUBS-TIERS]).
- [ ] Typeshed-bundled baseline integration ([CHKARCH-STUBS-TYPESHED]).

### Plugin host {#CHKADVPLAN-TODO-PLUGINS}

- [ ] WASM-sandboxed plugin host architecture ([CHKARCH-PLUGINS-ARCH]).
- [ ] Extension points for third-party rules ([CHKARCH-PLUGINS-EXTENSIONS]).
- [ ] `pyproject.toml` plugin declaration + distribution ([CHKARCH-PLUGINS-DIST]).

### Migration from other type checkers {#CHKADVPLAN-TODO-MIGRATION}

- [ ] `basilisk migrate --from {mypy,pyright}` config import ([CHKARCH-CONFIG-MIGRATION]).
- [ ] mypy config/semantic mapping ([CHKARCH-MIGRATION-MYPY]).
- [ ] pyright config/semantic mapping ([CHKARCH-MIGRATION-PYRIGHT]).

### CI integration helpers {#CHKADVPLAN-TODO-CI}

- [ ] `setup-basilisk` GitHub Action + pre-commit hook integration ([CHKARCH-CLI-CI]).
