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

## Principles {#CHKTAG-PRINCIPLES}

1. **Tags, not categories.** Rules are described by a set of tags, not by
   membership in one taxonomy. A rule can be `pep` + `narrowing`, or `basilisk`
   + `style` + `strictness`.
2. **Categories pertain to PEP only.** The only category vocabulary Basilisk
   maintains is the set of `python/typing` conformance categories
   ([CHKTAG-PEP-CATEGORIES]). Basilisk-original rules have **no** category — they
   are described purely by tags.
3. **Provenance is explicit.** Every rule declares whether it implements the
   typing specification (`pep`) or is a Basilisk-original house rule
   (`basilisk`) ([CHKTAG-PROVENANCE]). This is data, not something inferred from
   a code prefix.
4. **The `BSK-` prefix is cosmetic.** Basilisk-original rules are conventionally
   named/prefixed `BSK` for human recognition. That prefix is *semantically
   meaningless* to the checker ([CHKTAG-BSK-PREFIX]).
5. **Free-form tags must not conflict.** Arbitrary descriptive tags (`style`,
   `redundancy`, …) are allowed, but must be named so they never collide with a
   reserved PEP-category name ([CHKTAG-FREEFORM]).

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
BSK-W0050              -> ["basilisk", "redundancy", "style"]
imports_unresolved     -> ["basilisk", "strictness", "imports"]
```

## Provenance Tags {#CHKTAG-PROVENANCE}

Exactly one of the following is present on every rule. They are mutually
exclusive.

| Tag | Meaning |
|---|---|
| `pep` | The rule enforces the Python typing specification and is exercised by the `python/typing` conformance suite. |
| `basilisk` | A Basilisk-original, strict-by-default house rule that goes beyond the specification. |

Provenance is the curated source of truth in `rule_tags.rs`: the
`BASILISK_RULES` table lists every Basilisk-original rule; **everything else is a
PEP rule.** Provenance is never derived from a code prefix — see
[CHKTAG-BSK-PREFIX].

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
| `strictness` | Strict-by-default requirement beyond the spec (e.g. require annotations). |
| `style` | A stylistic nudge (e.g. prefer a concrete type over `Any`). |
| `redundancy` | Detects redundant code (e.g. a redundant annotation). |
| `dependencies` | Dependency / lock-file hygiene. |
| `imports` | Import-related strictness. |
| `stubs` | Type-stub hygiene. |
| `version` | Target-version-aware syntax checks. |

**Conflict rule (the load-bearing constraint):** a free-form tag must be
non-empty and must collide with neither a provenance tag (`pep`/`basilisk`) nor a
reserved PEP-category name. `rule_tags::is_valid_free_form` is the guard, and
[CHKTAG-TESTS] asserts it for every declared and emitted free-form tag. So, for
example, a Basilisk missing-annotation rule is tagged `strictness` — **never**
`annotations`, because `annotations` is the reserved PEP category.

New free-form tags are added by extending `FREE_FORM_TAGS` and the relevant
`BASILISK_RULES` rows; the test fails if any new tag collides.

## The `BSK-` Naming Convention {#CHKTAG-BSK-PREFIX}

Basilisk-original rules are conventionally named/prefixed `BSK` (e.g.
`BSK-W0050`). This is **purely cosmetic** — a branding/recognition convention for
humans. The type checker identifies a Basilisk rule **solely by its `basilisk`
tag** (membership in `BASILISK_RULES`), and never by inspecting the prefix. The
prefix could be dropped entirely and provenance would be unchanged.

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

## Implementation {#CHKTAG-IMPL}

[`rule_tags.rs`](../../crates/basilisk-checker/src/rule_tags.rs) is the single
source of truth and the public, queryable API:

- `PEP`, `BASILISK` — provenance tag constants.
- `PEP_CATEGORIES` — the reserved category vocabulary.
- `FREE_FORM_TAGS` — the descriptive vocabulary.
- `tags_for_code(code) -> Vec<&'static str>` — the full tag set for a diagnostic
  code (the bridge for any consumer holding a `Diagnostic`: look up by
  `diagnostic.code.code`).
- `is_pep_category`, `is_provenance`, `is_valid_free_form` — vocabulary
  predicates.

The table keys on the **current diagnostic code**. As the rule-rename work
([CHKARCH-DIAG-CODES](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CODES)) settles
codes, `BASILISK_RULES` is updated in lockstep; the test scans the live rule
source, so a renamed or newly restored rule is validated automatically (defaulting
to a `pep` rule unless listed as Basilisk).

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
code is picked up automatically and validated.
