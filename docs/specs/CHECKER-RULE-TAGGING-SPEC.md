# Rule Tagging {#CHKTAG}

Basilisk classifies every diagnostic rule with a flat, de-duplicated set of
string **tags** — no single hierarchical "category" field. The one hierarchy is
the **PEP category** axis, which exists only because the `python/typing`
conformance suite defines it: PEP rules are tagged with their conformance
category, everything else with plain descriptive labels.

- **Authoritative source (code):** [`crates/basilisk-checker/src/rule_tags.rs`](../../crates/basilisk-checker/src/rule_tags.rs)
- **Conformance test (tests):** [`crates/basilisk-checker/tests/rule_tags_tests.rs`](../../crates/basilisk-checker/tests/rule_tags_tests.rs)
- **Related:** [CHKARCH-DIAG](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG) (diagnostic rules),
  [CHKARCH-CONFORMANCE](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFORMANCE) (how the conformance number is measured, and why it is not a target)

## Tag Model {#CHKTAG-MODEL}

A **tag** is a non-empty, lowercase string label. A rule carries an *ordered,
de-duplicated* set: **exactly one** provenance tag (always first), optionally one
PEP-category tag (PEP rules only), and zero or more free-form tags.

[`rule_tags::tags_for_code`](../../crates/basilisk-checker/src/rule_tags.rs)
produces the set for any code. It never panics and never returns an empty set; an
unrecognised code resolves to a bare `pep` rule.

```text
aliases_newtype        -> ["pep", "aliases"]
narrowing_typeguard    -> ["pep", "narrowing"]
returns_compatibility  -> ["pep"]                       # cross-cutting core check
imports_unresolved     -> ["pep"]                       # core check, on by default
BSK-0050              -> ["basilisk", "redundancy", "style"]   # opt-in house rule
```

## Provenance Tags {#CHKTAG-PROVENANCE}

Exactly one is present on every rule; they are mutually exclusive.

| Tag | Meaning |
|---|---|
| `pep` | A core rule selected by the **default** config — the `python/typing` conformance rules plus the core checks that run by default. The default config is exactly this "core PEP" set. |
| `basilisk` | A Basilisk-original rule, **off by default**, on only via opt-in config ([CHKARCH-CONFIGURATION-ONLY]). |

Provenance is **self-declared by each rule**: returning `Some(OptInSpec { .. })`
from `Rule::opt_in_spec()` marks it `basilisk`; returning `None` (the trait
default) makes it `pep`. `rule_tags` reads these from the live rule registry —
**no central rule list**, never derived from a code prefix (see
[CHKTAG-BSK-PREFIX]).

**Rule selection is configuration-only** ([CHKARCH-CONFIGURATION-ONLY]):
`check_with_config` is the only way to select rules, and the default config
selects exactly the core PEP set. Selection and classification read the **same**
self-declared source, so they cannot drift:

- `check_with_config` gates a rule off when it has an `opt_in_spec()` and no
  explicit non-disabled severity. A rule with no `opt_in_spec()` is `pep` and
  runs unless explicitly disabled.
- A default-on check is `pep` **even if Basilisk-authored** (e.g.
  `imports_unresolved`, `version_target_syntax` declare no `opt_in_spec()`, so run
  by default and are `pep`). There is no "strict mode".

## PEP Category Tags {#CHKTAG-PEP-CATEGORIES}

The **only** category axis Basilisk keeps. The vocabulary is derived from the
`python/typing` conformance suite's file-name prefixes and the `category` column of
[`conformance/conformance_status.csv`](../../conformance/conformance_status.csv) —
mirrored as `rule_tags::PEP_CATEGORIES`:

```
aliases      annotations  callables    classes      constructors
dataclasses  directives   enums        exceptions   generics
historical   literals     namedtuples  narrowing    overloads
protocols    qualifiers   specialtypes tuples       typeddicts
typeforms
```

Rules:

- A PEP-category tag may appear **only** on a `pep` rule.
- A `pep` rule **should** carry exactly one PEP-category tag, derived from the
  rule's conformance name (the portion of the code before the first `_`).
- Cross-cutting core checks with no single home category (e.g.
  `returns_compatibility`, `calls_argument_type`, `names_undefined`) are `pep`
  with **no** category tag.
- These 21 names are **reserved**: no free-form tag may reuse one
  ([CHKTAG-FREEFORM]).

## Free-form Tags {#CHKTAG-FREEFORM}

Any rule may carry additional descriptive tags. The current vocabulary
(`rule_tags::FREE_FORM_TAGS`):

| Tag | Meaning |
|---|---|
| `strictness` | Enforces a stricter-than-spec requirement (e.g. requiring annotations). Intent, **not** a "strict mode"; off by default like all `basilisk` rules. |
| `style` | A stylistic nudge (e.g. prefer a concrete type over `Any`). |
| `redundancy` | Detects redundant code (e.g. a redundant annotation). |
| `dependencies` | Dependency / lock-file hygiene. |
| `imports` | Import-related house rule (e.g. undeclared-dependency import). |
| `stubs` | Type-stub hygiene. |

The configuration-editor plan adds `suppressions` when the opt-in suppression-
audit rule family lands. It means “visibility and hygiene for inline ignore /
severity directives”; those rules remain off in the unconfigured default. The
tag must be added to `FREE_FORM_TAGS` in the same change as the first live rule,
so the invariant test never permits a declared-but-unused parallel taxonomy.

**Conflict rule (load-bearing):** a free-form tag must be non-empty and collide
with neither a provenance tag (`pep`/`basilisk`) nor a reserved PEP-category
name. `rule_tags::is_valid_free_form` is the guard, asserted by [CHKTAG-TESTS]
for every declared and emitted free-form tag. So a Basilisk missing-annotation
rule is tagged `strictness`, **never** `annotations` (the reserved PEP category).

Add a free-form tag by extending `FREE_FORM_TAGS` and the relevant
`opt_in_spec()` declarations; the test fails on any collision.

## The `BSK-` Naming Convention {#CHKTAG-BSK-PREFIX}

Basilisk-original rules are conventionally prefixed `BSK` (e.g. `BSK-0050`) —
**purely cosmetic**. The checker identifies a Basilisk rule **solely by its
`basilisk` tag** (its `opt_in_spec()` declaration), never the prefix. Dropping
the prefix would leave provenance unchanged; [CHKTAG-TESTS] asserts convention
and self-declared set agree both ways. PEP rules are named after their
conformance test (e.g. `aliases_newtype`) with no `BSK` prefix.

> **This naming is a known hazard.** Naming a rule after the fixture it is scored
> on invites the rule to be *about* that fixture: it makes "does the file pass?"
> read as the rule's definition, and a rule file named `generics_base_class_2.rs`
> has no name left to describe the typing-spec concept it owes. Fixture-shaped
> naming is one of the detection signatures in
> [CHKARCH-TEXT-MATCHED-LOGIC](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TEXT-MATCHED-LOGIC),
> and the numeric `_2` / `_3` suffixes are the tell: they enumerate fixtures, not
> concepts. The convention stands for now because the codes are user-visible and
> renaming them is a breaking change the user has not scoped — **but a rule's
> name is never evidence that it implements anything.** Judge the rule by whether
> it decides on the resolved model, never by whether its fixture passes.

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
`Rule::opt_in_spec() -> Option<OptInSpec>` returning `Some(OptInSpec { code, tags })`
in its own source file; core PEP rules use the trait default (`None`). This is the
**single source of truth** — no `BASILISK_RULES` table, no hand-maintained gating
list. Adding a Basilisk rule means tagging the rule, nothing else.

[`rule_tags.rs`](../../crates/basilisk-checker/src/rule_tags.rs) gathers those
declarations once from the live rule registry
([`rules::opt_in_specs`](../../crates/basilisk-checker/src/rules/mod.rs)) and
exposes the queryable API:

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

This is a **runtime** source of truth: `check_with_config` calls
`opt_in_spec_for_code` to distinguish default-on rules from opt-in rules. An
explicit non-disabled severity selects an opt-in rule; tag-oriented editor
actions expand against this live registry and persist explicit rule entries.
Selection and classification therefore cannot drift. The configuration editor
consumes this registry through the LSP contract below; the VSIX never copies it.

A rule's `opt_in_spec().code` moves with its emitted code in the same file as the
rule-rename work
([CHKARCH-DIAG-CODES](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CODES)) settles
codes; the drift guard (invariant 7) fails CI on a stale declared code, and the
parity guard (invariant 10) on any disagreement between the self-declared set and
the `BSK-` convention.

[CHKARCH-CONFIGURATION-ONLY]: CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIGURATION-ONLY

## Configuration Editor Contract {#CHKTAG-CONFIGURATION-EDITOR}

Tags are the primary browse/filter/bulk-selection surface in
[CONFIGEDITOR-TAGS](LSP-CONFIGURATION-EDITOR-SPEC.md#CONFIGEDITOR-TAGS). The LSP
projects every live rule as one descriptor containing its ordered tags and the
kind of each tag (provenance, PEP category, or descriptive). Clients must not
infer categories from code names or maintain label-to-code maps.

Bulk operations accept tag selectors and expand them in the server against a
specific configuration revision. Preview returns the exact expanded rule-code
set; apply uses that preview and rejects a stale revision. This preserves the
flat/multi-tag model: one rule can appear in several tag facets, but it is
mutated once in a de-duplicated transaction.

## Consumers {#CHKTAG-CONSUMERS}

Tags drive, without re-deriving classification:

- LSP/CLI filtering and bulk rule operations ("disable all `style` rules", "show
  only `pep` diagnostics"), specified by
  [CONFIGEDITOR-OPERATIONS](LSP-CONFIGURATION-EDITOR-SPEC.md#CONFIGEDITOR-OPERATIONS).
- The website rules reference and per-error pages
  ([WEBSITE-ERROR-PAGES](WEBSITE-ERROR-PAGES-SPEC.md)) — a follow-up reading the
  same source rather than a parallel list.

## Testing {#CHKTAG-TESTS}

[`rule_tags_tests.rs`](../../crates/basilisk-checker/tests/rule_tags_tests.rs) is
a coarse e2e test that scans every `code: "…"` literal under `src/rules` and
asserts the [CHKTAG-INVARIANTS] for the live rule set plus the [CHKTAG-MODEL]
worked examples. It needs no fixture updates when rules are added — a new rule's
code is picked up automatically. Beyond the shape invariants it enforces the
drift/source guards:

- `no_basilisk_rule_key_is_stale` — every `opt_in_spec()` code is a live code
  (invariant 7).
- `pep_categories_match_conformance_test_prefixes` — reads (read-only) the
  `conformance/tests/` file names and asserts each `PEP_CATEGORIES` entry is a
  real prefix (invariant 8).
- `default_on_core_checks_are_pep_not_basilisk` — the default-on Basilisk-authored
  checks resolve to `pep` (invariant 9).
- `basilisk_provenance_matches_the_bsk_naming_convention` — the `opt_in_spec()`
  set equals exactly the `BSK-`-prefixed live codes (invariant 10).
