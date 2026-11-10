# Checker Advanced Features — Implementation Plan {#CHKADVPLAN}

> **Spec**: [CHECKER-ARCHITECTURE-SPEC.md](../specs/CHECKER-ARCHITECTURE-SPEC.md) — read the
> relevant sections before touching code.

---

## Status {#CHKADVPLAN-STATUS}

Concrete-but-unbuilt checker capabilities that the architecture spec describes and that had
no owning plan. These are real planned features (not bloat), captured here as ordered TODOs
so each spec section is referenced by a plan rather than left orphaned. None of this blocks
the prime directive (accuracy on unseen Python) — it is opt-in surface area beyond the
typing-spec rules, and it waits behind the text-matching audit
([LINESCANPLAN-DISPOSAL](CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md#LINESCANPLAN-DISPOSAL)).
New rules added here must decide on the resolved model from day one; anything that would
ship as a text scan does not ship.

---

## TODO {#CHKADVPLAN-TODO}

### Dependency hygiene {#CHKADVPLAN-TODO-DEPHYGIENE}

- [ ] First-party / third-party package-name collision
  ([#47](https://github.com/Nimblesite/Basilisk/issues/47)). When a declared PyPI
  dependency shares a name with a package the project itself ships, an unrelated
  distribution is silently pulled into the graph and precedence depends on
  install order — dependency confusion that never fails loudly. Warning by
  default, gradable per project. Must handle hyphen/underscore variants (PyPI
  `pydantic-ai` → import `pydantic_ai`), stay silent when the package is purely
  first-party or the dependency has no local twin, and offer a quick fix that
  removes the dependency line or renames the local package. The real case that
  prompted it: a project shipping `src/nap/` that also declared `nap>=2.0.0`,
  pulling an unrelated HTTP library.

  The check is a three-way join, and Basilisk holds **one** of the three inputs
  today — local package roots, from import resolution. The other two are
  prerequisites, not existing capability:

  - [ ] Declared-dependency extraction. `basilisk-config` parses the
    `[tool.basilisk]` table only; nothing reads `[project].dependencies`,
    `[dependency-groups]`, or `uv.lock`.
  - [ ] Installed-distribution inventory. `missing_type_stubs` (BSK-0152) tests
    whether a *resolved import* landed under site-packages without a `py.typed`
    marker — it never enumerates installed distributions.

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
