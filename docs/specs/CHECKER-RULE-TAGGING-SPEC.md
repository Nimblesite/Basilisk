# Rule Tagging {#CHKTAG}

Basilisk classifies every diagnostic rule with a flat set of string **tags**.
This is a *tagging* system, not a *categorisation* system: there is no single
hierarchical "category" field a rule must slot into. A rule simply carries the
set of labels that are true of it.

The one place a hierarchy survives is the **PEP category** axis, and it exists
only because the `python/typing` conformance suite defines it. Basilisk does not
invent its own category taxonomy; it tags PEP rules with the conformance
category they belong to, and tags everything else with plain descriptive labels.

- **Authoritative source (code):** [`crates/basilisk-checker/src/rule_tags.rs`](../../crates/basilisk-checker/src/rule_tags.rs)
- **Conformance test (tests):** [`crates/basilisk-checker/tests/rule_tags_tests.rs`](../../crates/basilisk-checker/tests/rule_tags_tests.rs)
- **Related:** [CHKARCH-DIAG](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG) (diagnostic rules),
  [CHKARCH-CONFORMANCE](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFORMANCE) (conformance scoring)

## Tag Model {#CHKTAG-MODEL}

A **tag** is a non-empty, lowercase string label. A rule carries an *ordered,
de-duplicated* set of tags. The set always contains **exactly one** provenance
tag, optionally one PEP-category tag (PEP rules only), and zero or more free-form
tags.

The set for any diagnostic code is produced by
[`rule_tags::tags_for_code`](../../crates/basilisk-checker/src/rule_tags.rs). It
never panics and never returns an empty set; an unrecognised code resolves to a
bare `pep` rule. The first element is always the provenance tag.

```text
aliases_newtype        -> ["pep", "aliases"]
narrowing_typeguard    -> ["pep", "narrowing"]
returns_compatibility  -> ["pep"]                       # cross-cutting core check
imports_unresolved     -> ["pep"]                       # core check, on by default
BSK-W0050              -> ["basilisk", "redundancy", "style"]   # opt-in house rule
```

## Provenance Tags {#CHKTAG-PROVENANCE}

Exactly one of the following is present on every rule. They are mutually
exclusive.

| Tag | Meaning |
|---|---|
| `pep` | A core rule selected by the **default** configuration — the `python/typing` conformance rules plus the core checks that run by default. The default config is exactly this "core PEP" set. |
| `basilisk` | A Basilisk-original rule, **off by default**, that turns on only via opt-in configuration ([CHKARCH-CONFIGURATION-ONLY]). |

Provenance is **self-declared by each rule**: a rule returns
`Some(OptInSpec { .. })` from `Rule::opt_in_spec()` to mark itself `basilisk`, or
returns `None` (the trait default) to be a `pep` rule. `rule_tags` reads these
declarations from the live rule registry — there is **no central rule list**, and
provenance is never derived from a code prefix (see [CHKTAG-BSK-PREFIX]).

**Rule selection is configuration-only** ([CHKARCH-CONFIGURATION-ONLY]): running
rules through `check_with_config` is the *only* valid way to select them, and the
**default config selects exactly the core PEP set and nothing else**. Selection
and classification read the **same** self-declared source, so they cannot drift:

- `check_with_config` gates a rule off when it declares an `opt_in_spec()` whose
  tags the configuration has not enabled (the `opt_in_tag_enabled` bridge maps the
  config's opt-in switches to tags). A rule with no `opt_in_spec()` is `pep` and
  always runs.
- A default-on check is `pep` **even if it is Basilisk-authored** (e.g.
  `imports_unresolved` and `version_target_syntax` declare no `opt_in_spec()`, so
  they run by default and are `pep`, not `basilisk`). There is no "strict mode"
  and no behaviour the tag switches on.

## PEP Category Tags {#CHKTAG-PEP-CATEGORIES}

The **only** category axis Basilisk keeps. The vocabulary is taken verbatim from
the `python/typing` conformance suite — the file-name prefixes under
[`conformance/tests/`](../../conformance/tests) and the `category` column of
[`conformance/conformance_status.csv`](../../conformance/conformance_status.csv).
It is mirrored as `rule_tags::PEP_CATEGORIES`:

```
aliases      annotations  callables    classes      constructors
dataclasses  directives   enums        exceptions   generics
historical   literals     namedtuples  narrowing    overloads
protocols    qualifiers   specialtypes tuples       typeddicts
typeforms
```

Rules:

- A PEP-category tag may appear **only** on a rule that also has the `pep`
  provenance tag.
- A `pep` rule **should** carry exactly one PEP-category tag — the category it
  belongs to. It is derived from the rule's conformance name (the portion of the
  code before the first `_`, which the rule-rename work aligns with the
  conformance category).
- Cross-cutting core checks with no single home category (e.g.
  `returns_compatibility`, `calls_argument_type`, `names_undefined`) are `pep`
  with **no** category tag. This is allowed and expected.
- These 21 names are **reserved**: no free-form tag may reuse one
  ([CHKTAG-FREEFORM]).

## Free-form Tags {#CHKTAG-FREEFORM}

Any rule may carry additional descriptive tags. The current vocabulary
(`rule_tags::FREE_FORM_TAGS`):

| Tag | Meaning |
|---|---|
| `strictness` | Enforces a stricter-than-spec requirement (e.g. requiring annotations). Describes a rule's intent — **not** a "strict mode"; like all `basilisk` rules these are off by default and opt-in via config. |
| `style` | A stylistic nudge (e.g. prefer a concrete type over `Any`). |
| `redundancy` | Detects redundant code (e.g. a redundant annotation). |
| `dependencies` | Dependency / lock-file hygiene. |
| `imports` | Import-related house rule (e.g. undeclared-dependency import). |
| `stubs` | Type-stub hygiene. |

**Conflict rule (the load-bearing constraint):** a free-form tag must be
non-empty and must collide with neither a provenance tag (`pep`/`basilisk`) nor a
reserved PEP-category name. `rule_tags::is_valid_free_form` is the guard, and
[CHKTAG-TESTS] asserts it for every declared and emitted free-form tag. So, for
example, a Basilisk missing-annotation rule is tagged `strictness` — **never**
`annotations`, because `annotations` is the reserved PEP category.

New free-form tags are added by extending `FREE_FORM_TAGS` and the relevant
rules' `opt_in_spec()` declarations; the test fails if any new tag collides.

## The `BSK-` Naming Convention {#CHKTAG-BSK-PREFIX}

Basilisk-original rules are conventionally named/prefixed `BSK` (e.g.
`BSK-W0050`). This is **purely cosmetic** — a branding/recognition convention for
humans. The type checker identifies a Basilisk rule **solely by its `basilisk`
tag** (the rule's own `opt_in_spec()` declaration), and never by inspecting the
prefix. The prefix could be dropped entirely and provenance would be unchanged —
[CHKTAG-TESTS] asserts the convention and the self-declared set agree both ways.

Conversely, PEP rules are named after their conformance test (e.g.
`aliases_newtype`) and carry no `BSK` prefix.

## Invariants {#CHKTAG-INVARIANTS}

Enforced by [CHKTAG-TESTS] over the full, live rule set:

1. Every rule resolves to **exactly one** provenance tag.
2. A PEP-category tag appears only on a `pep` rule.
3. Every emitted tag that is neither provenance nor a PEP category is a **valid
   free-form tag** (no collision with the reserved vocabulary).
4. A `basilisk` rule never carries a PEP-category tag.
5. `PEP_CATEGORIES` are unique, lowercase, and distinct from the provenance tags.
6. Every declared `FREE_FORM_TAGS` entry is a valid free-form tag.
7. Every code a rule declares via `opt_in_spec()` is a **live** rule code (no
   stale declaration after a rename — a stale code would silently demote a rule
   to on-by-default `pep`).
8. Every `PEP_CATEGORIES` entry is a real `python/typing` conformance test-file
   prefix (the vocabulary cannot drift from its source).
9. The default-on Basilisk-authored checks (`imports_unresolved`,
   `imports_module_attribute`, `version_target_syntax`) resolve to `pep`, never
   `basilisk` — provenance follows config selection, not authorship.
10. The set of `opt_in_spec()`-declared codes equals **exactly** the set of
    `BSK-`-prefixed live rule codes — the cosmetic naming convention and the
    self-declared provenance agree both ways, so a new `BSK-` rule that forgot to
    tag itself, or a non-`BSK-` rule wrongly tagged opt-in, fails CI.

## Implementation {#CHKTAG-IMPL}

Provenance lives **on the rules**: each Basilisk-original rule declares
`Rule::opt_in_spec() -> Option<OptInSpec>` in its own source file, returning
`Some(OptInSpec { code, tags })`; core PEP rules use the trait default (`None`).
This is the **single source of truth** — there is no `BASILISK_RULES` table and no
hand-maintained gating list. Adding a Basilisk rule means tagging the rule, and
nothing else.

[`rule_tags.rs`](../../crates/basilisk-checker/src/rule_tags.rs) gathers those
declarations once from the live rule registry
([`rules::opt_in_specs`](../../crates/basilisk-checker/src/rules/mod.rs)) and
exposes the public, queryable API:

- `PEP`, `BASILISK` — provenance tag constants.
- `PEP_CATEGORIES` — the reserved category vocabulary.
- `FREE_FORM_TAGS` — the descriptive vocabulary.
- `OptInSpec { code, tags }` — the opt-in declaration a rule attaches to itself.
- `opt_in_spec_for_code(code) -> Option<OptInSpec>` — the gating answer: `Some` for
  a Basilisk (opt-in) rule, `None` for a core PEP rule.
- `tags_for_code(code) -> Vec<&'static str>` — the full tag set for a diagnostic
  code (the bridge for any consumer holding a `Diagnostic`: look up by
  `diagnostic.code.code`).
- `basilisk_rule_codes() -> Vec<&'static str>` — the opt-in Basilisk set, exposed
  for the drift guards ([CHKTAG-TESTS], invariants 7 and 10).
- `is_pep_category`, `is_provenance`, `is_valid_free_form` — vocabulary
  predicates.

This is now a **runtime** source of truth, not only a vocabulary: `check_with_config`
calls `opt_in_spec_for_code` to gate opt-in rules, with `opt_in_tag_enabled`
([`lib.rs`](../../crates/basilisk-checker/src/lib.rs)) the single bridge from the
config's opt-in switches to tags. Because selection and classification read the
same declarations, they cannot drift. The remaining consumers in
[CHKTAG-CONSUMERS] (LSP/CLI/website) are follow-ups.

As the rule-rename work
([CHKARCH-DIAG-CODES](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CODES)) settles
codes, a rule's `opt_in_spec().code` moves with its emitted code in the same file;
the drift guard (invariant 7) fails CI if a declared code goes stale, and the
parity guard (invariant 10) fails if the self-declared set and the `BSK-`
convention ever disagree.

[CHKARCH-CONFIGURATION-ONLY]: CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIGURATION-ONLY

## Consumers {#CHKTAG-CONSUMERS}

Tags are designed to drive, without re-deriving classification:

- LSP/CLI filtering and bulk rule operations ("disable all `style` rules",
  "show only `pep` diagnostics").
- The website rules reference and per-error pages
  ([WEBSITE-ERROR-PAGES](WEBSITE-ERROR-PAGES-SPEC.md)) — surfacing tags is a
  follow-up that reads the same source rather than maintaining a parallel list.

## Testing {#CHKTAG-TESTS}

[`rule_tags_tests.rs`](../../crates/basilisk-checker/tests/rule_tags_tests.rs) is
a coarse e2e test that scans every `code: "…"` literal under `src/rules` and
asserts the [CHKTAG-INVARIANTS] for the live rule set, plus the worked examples
in [CHKTAG-MODEL]. It needs no fixture updates when rules are added: a new rule's
code is picked up automatically and validated. Beyond the shape invariants it
also enforces the drift/source guards:

- `no_basilisk_rule_key_is_stale` — every `opt_in_spec()` code is a live code
  (invariant 7).
- `pep_categories_match_conformance_test_prefixes` — reads (read-only) the
  `conformance/tests/` file names and asserts each `PEP_CATEGORIES` entry is a
  real prefix (invariant 8).
- `default_on_core_checks_are_pep_not_basilisk` — the default-on Basilisk-authored
  checks resolve to `pep` (invariant 9).
- `basilisk_provenance_matches_the_bsk_naming_convention` — the `opt_in_spec()`
  set equals exactly the `BSK-`-prefixed live codes (invariant 10).
