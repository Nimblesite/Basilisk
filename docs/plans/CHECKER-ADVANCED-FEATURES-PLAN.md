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

### Ownership and safety checks (`basilisk-checker::ownership`) {#CHKADVPLAN-TODO-SAFETY}

These are Basilisk rules. The concepts are borrowed from
[Mojo's ownership model](https://docs.modular.com/mojo/manual/values/ownership);
the analysis is Basilisk's own, expressed in Python's `Annotated` conventions.
`mojo` is not a legal identifier — prose may credit the inspiration, but no spec
ID, module path, file name, or rule name may carry it ([CHKARCH-SAFETY]).
Unbuilt and unadvertised: the module is unwired scaffolding, registers no rule,
and stays out of user-facing docs until it ships as a working, opt-in rule set.

- [ ] Ownership tracking via `Borrowed`/`InOut`/`Owned` `Annotated` conventions: mutation-of-borrowed and use-after-move detection ([CHKARCH-SAFETY-OWNERSHIP]).
- [ ] Parameter-immutability-by-default analysis with `InOut` opt-out + frozen-dataclass nudges ([CHKARCH-SAFETY-IMMUTABLE]).
- [ ] Structural-discipline checks: dynamic-attribute-on-typed-class, missing `__init__`, `__slots__` suggestions ([CHKARCH-SAFETY-STRUCTURAL]).
- [ ] No-implicit-coercion checks (int→float, bool→int, bytes→str) ([CHKARCH-SAFETY-COERCION]).
- [ ] Source-concept → Basilisk-check mapping table backing the above ([CHKARCH-SAFETY-COMPAT]).
- [ ] Only once the above land and are wired into the pipeline: surface the rule
      set in the website docs. Nothing here is advertised before then.

### Dependency hygiene {#CHKADVPLAN-TODO-DEPHYGIENE}

- [ ] First-party / third-party package-name collision
  ([#47](https://github.com/Nimblesite/Basilisk/issues/47)). When a declared PyPI
  dependency shares a name with a package the project itself ships, an unrelated
  distribution is silently pulled into the graph and precedence depends on
  install order — dependency confusion that never fails loudly. Basilisk already
  holds all three inputs: local package roots (import resolution), declared
  dependencies (`basilisk-config`), and installed site-packages contents
  (`missing_type_stubs`), so the check is a three-way join. Warning by default,
  gradable per project. Must handle hyphen/underscore variants (PyPI
  `pydantic-ai` → import `pydantic_ai`), stay silent when the package is purely
  first-party or the dependency has no local twin, and offer a quick fix that
  removes the dependency line or renames the local package. The real case that
  prompted it: a project shipping `src/nap/` that also declared `nap>=2.0.0`,
  pulling an unrelated HTTP library.

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
