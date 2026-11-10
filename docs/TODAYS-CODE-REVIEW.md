# Staged Code Review — 2026-08-09

Scope: the staged index only, fingerprint
`81c9a84feb89b9acd6889457576363eab35aebf199c6ccb611ddfe4d5afbdfa2`.
Unstaged edits are deliberately excluded, including edits in files that also
have staged changes. A working-tree fix does not make this staged snapshot safe
to commit.

## Verdict

| Question | Answer |
| --- | --- |
| Does the staged code address the architecture problems? | **Materially, but not end to end.** The new definition-site binding and class-graph layer is the right foundation, and several previously reported defects are fixed. Checker consumers still throw away that identity and recover it from renderings or bare names. |
| Is text pattern matching creeping back? | **Yes, in active verdict paths.** The graph core does not introduce a new regex/substr type parser, but nominal checking reparses `Named(String)`, constructor rules slice annotation source, and several joins still compare declaration spellings. |
| Is it ready to commit? | **No.** The staged snapshot violates formatting checks, breaks documented Make targets, reinstates a forbidden conformance ratchet, and retains runtime panic/text-derived type paths. |

The author is fixing real problems, not merely moving code around. The work is
clearly heading toward AST and resolved identity. The staged boundary is still
wrong, though: sound identities are built in the resolver and then converted
back to strings before some checker verdicts are issued.

## Commit blockers

### 1. Resolved nominal identity is still lost at the checker boundary

The resolver now has a definition-site-keyed `ClassGraph`, and top-level
`Named`/`Named` comparisons are correctly routed through it before the deleted
string comparator. That old finding is fixed.

The leaf representation is not fixed:

- `rules/shared/nominal.rs:48-65,112-127` accepts rendered `&str` values,
  reparses them with Ruff, and resolves the reconstructed expression in the
  final module namespace.
- `annotation/mod.rs:233-264` uses the same parse-back bridge while grounding a
  `Named(String)` leaf.
- `types.rs:80-81` still stores a rendering rather than a definition-site type
  identity.

Parsing a rendering is still deriving semantics from text. It cannot recover
the original AST node, use-site offset, scope, or definition ID. An earlier
annotation can be retargeted by a later same-spelled class definition, and two
aliases of one definition can remain different renderings.

This unfinished boundary is live, not just theoretical:

- `TypeJudge::fits` now safely intercepts top-level nominal leaves at
  `shared/judge.rs:102-115`, but containers, callables, tuples, unions, and
  other nested shapes recurse through `InferredType::is_assignable_to`.
- equal nested `Named(String)` values are accepted by the generic equality arm
  at `types.rs:331-332`;
- unequal nested named leaves reach the mandatory panic at
  `types.rs:493-500`;
- `narrowing_typeis_2.rs:74-78,136-145` still accepts whole-type rendered
  equality and calls `is_assignable_to` before consulting the nominal graph.

Carry a resolved type/class ID from the original annotation AST into every
nominal leaf. Do not parse `Named(String)` back into a fresh zero-offset AST.

### 2. Constructor rules have made text-derived verdicts live again

The class-site and graph plumbing is useful, but the active rules still decide
diagnostics from source text and name tables:

- `calls_argument_count/mod.rs:253-276` treats an unresolved or non-local
  explicit metaclass as pass-through semantic success.
- `:292-304` slices the return annotation source and compares it with a
  `TypeVarCallInfo.name`.
- `:313-315` treats any value-returning call as construction delegation instead
  of resolving `type.__call__` and its required argument forwarding.
- `constructors_call_type/mod.rs:135-177` recognizes only the literal bare
  spelling `type`, then slices the source text of `X` from `type[X]`.
- `:268-286` joins TypeVars, bounds, and classes through string-keyed maps.

Aliases, qualified forms, local rebindings, and imported metaclasses can
therefore change the diagnostic without changing the program's type meaning.
`constructors_call_type/helpers.rs:83-106` also converts an incomplete ancestry
walk into `NoArgs`, and uses a DFS reachability order as though it were Python's
C3 MRO when selecting a constructor.

These paths were previously loud deletion boundaries. Activating them before
their semantic inputs are rebuilt is a regression and should block the commit.

### 3. Module bindings are reused as though nested scopes did not exist

One module `BindingTable` is created at `basilisk-resolver/src/visitor/mod.rs:42-57`
and then reused while walking function and class bodies, including:

- calls/reveals in `visitor/calls_and_reveal.rs:27-116`;
- annotations in `visitor/annotations.rs:278-373`;
- runtime TypedDict calls in `visitor/typeddict_ext.rs:242-334`;
- the rebuilt generic-scoping rule in
  `checker/src/rules/generics_basic_2.rs:107-125,191-250`;
- overload call attribution in `rules/overloads_evaluation.rs:129-138`.

A parameter or local named `int`, `isinstance`, `C`, `TypeVar`, or an overload
name is invisible to that table. The checker can attribute a local call to a
global class, builtin, TypeVar factory, or overload definition. Deferred
function execution also means module position is not sufficient to model which
global binding the body observes.

The new directed module-level tests are valuable, but they do not establish
scope correctness. Build scope-aware environments before enabling these
verdicts in nested bodies.

### 4. The staged Make/CI interface violates the repository rules

The staged Makefile removes documented targets in favor of underscored names:
`bench`, `mutation-test`, `package-zed`, `reinstall-vsix`,
`reinstall-vsix-macos`, and `smoke-micropython` no longer exist. The staged
repository still tells users and CI-prep to run the old commands in
`CONTRIBUTING.md`, both Zed READMEs, checker/release/VSIX specs, benchmark
scripts, the website, and the MicroPython smoke script. Commit aliases or stage
all caller migrations as the same atomic change.

The staged conformance path is a harder policy violation:

- `.github/workflows/ci.yml:989-1031` still contains a job named **Mutated
  Conformance Ratchet** and calls the mutation conformance script as a gate.
- the new staged Makefile routes `conformance MUTATED=1` into that same script
  while claiming at `Makefile:347-364` that conformance is never a gate;
- staged `conformance/run_mutation_conformance.py:164-195` reads a pass-rate
  floor and returns failure when the measured rate falls below it.
- `CLAUDE.md:64-69` explicitly says conformance is never a gate, threshold, or
  ratchet anywhere.

There is an unstaged edit to the script, but it is not part of this review or
the staged commit. The Makefile header also calls itself the whole public
interface while omitting public `test-checker-all` and miscounting its targets.

### 5. The staged files do not pass basic repository hygiene

- `git diff --cached --check` fails at
  `crates/basilisk-checker/src/rules/overloads_evaluation.rs:403` because of a
  new blank line at EOF.
- `cargo fmt --all -- --check` reports staged diffs across checker, resolver,
  and resolver test files.

This is a mechanical blocker, but it matters: the exact staged tree is not
ready for review or commit while its required checks fail.

## Remaining correctness debt

### 6. Some consumers still collapse definition identity to spelling

- `visitor/typeddict.rs:20-105` keys schemas by `ClassInfo.name` and joins
  annotations through `Expr::Name.id`. Redefined classes collide and assignment
  aliases miss.
- `visitor/final_readonly.rs:14-70` repeats the same pattern for `ReadOnly`.
- `assignment_compatibility/default_spec.rs:94-101,153-170,248-256` lowers some
  expressions structurally, then joins TypeVars, classes, and declarations by
  their names.
- `classes_override_2.rs:127-155` compares child and base annotations as source
  text, making aliases and formatting semantic.
- `overloads_consistency.rs:50-58` groups methods by rendered class name rather
  than the available class definition site.

`overloads_evaluation` has correctly replaced its old bare-call-name join with
definition-site resolution for module-level aliases and rebindings. Its
remaining issues are nested-scope resolution and the lack of a Cartesian
product when multiple call arguments are unions at `:327-371`.

### 7. Unknown graph evidence is still converted into a negative answer

`ClassGraph::Ancestry { classes, complete }` is a strong addition, and the new
nominal consumer now honors `complete`. The old blanket nominal finding is
fixed and removed.

Other consumers still discard that evidence:

- `visitor/typeddict_schema.rs:36-51` merges only known ancestors, after which
  `visitor/typeddict.rs:38-70` issues missing/invalid-key verdicts as though the
  inherited schema were complete.
- constructor signature selection treats the known lower bound as the entire
  MRO.

Positive reachability can be definite on an incomplete graph. Negative
reachability, missing fields, and “no inherited constructor” cannot.

### 8. Canonical aliases and conditional branches remain incomplete

- `canonical/src/binding.rs:389-405` records assignment aliases, but
  `canonical_of` at `:487-492` and `canonical_of_final` at `:688-703` reject
  every `LocalDefinition`. `TD = TypedDict; class X(TD)` and `E = Enum; class
  X(E)` therefore become unknown even though local-class aliases now work.
- unresolved conditional branches are flattened into one event stream and the
  lexically latest binding wins at `binding.rs:143-148,212-223,455-463`.
  Reordering mutually exclusive branches can change canonical identity.
- alias traversal still has an arbitrary 32-hop ceiling; the positional walk
  should terminate from its decreasing source offsets rather than silently
  abandoning a valid long chain.

### 9. Several registered rules still present absence as computed success

`visitor/mod.rs:180-205,225-233` supplies ordinary empty vectors for deleted or
unrebuilt TypedDict, NewType, NamedTuple, protocol, generator, TypeAliasType,
tuple-index, and related collectors. `visitor/calls_and_reveal.rs:23-25` can
similarly return an empty reveal-call set. The comment calling these paths
“inert” does not make downstream silence distinguishable from a real clean
result.

Other staged rules advertise more than they implement:

- `typeddicts_inheritance.rs:115-117` invokes only the mixed-base check;
- `directives_disjoint_base.rs:55-60` ignores resolved `@disjoint_base` and
  slotted-dataclass facts;
- `generics_syntax_compatibility.rs:109-127` omits PEP 695 functions;
- `enums_behaviors.rs:47-53` treats every class attribute as an enum member;
- `names_undefined.rs:179-204` detects self-inheritance only if a base resolves
  to the class currently being defined, an identity the resolver correctly
  cannot produce before the class statement binds its name.

Keep incomplete obligations loud. An empty collection must not masquerade as a
successful analysis.

### 10. Smaller semantic defects

- `bidir/builtins.rs:55-58` maps `complex()` to `InferredType::Float`.
- `bidir/engine.rs:424-449` invokes canonical builtin-call handling only for
  `Expr::Name`, so qualified `builtins.int()` takes a different path.
- annotation construction at `annotation/mod.rs:310-366` still selects several
  builtin leaves/forms from rendered heads before complete binding resolution;
  aliases, qualified forms, and local rebindings can diverge.

## What is good and should be kept

- The staged index is now coherently wired. The old missing-argument, missing
  graph, and missing `builtin_names` construction failures are gone.
- The builtin-scope loader now keeps `(logical_uri, source_text)` in the correct
  order, represents unavailability explicitly, and no longer caches a parse
  failure as an empty clean namespace.
- `ResolvedBase::{LocalClass(Span), Form, Unknown}`, definition-site class
  edges, cycle-safe traversal, and explicit ancestry completeness are the right
  AST/resolution primitives.
- Enum propagation, runtime TypedDict recognition, bare tuple/Unpack handling,
  TypeVar references, and deferred local-class aliases now use binding or
  definition identity instead of the old raw-name paths.
- Nominal negative answers now honor incomplete ancestry, and enum-member
  acceptance is restricted to resolved enum owners.
- Top-level nominal comparison now routes around the deleted string comparator;
  the old `sub == sup` shortcut in `TypeJudge` is gone.
- The overload call join now follows module-level aliases/rebindings to function
  definition sites.
- The new identity, alias, rebinding, deep-chain, builtin-loader, and ancestry
  tests are generally directed and substantially better than code-only parity
  checks.

The direction is unquestionably better. No new substring or regex type parser
was found in the canonical binding/class-graph core. The remaining architectural
failure is concentrated at consumer boundaries that still turn resolved facts
into renderings, bare names, or silent defaults.

## Required order of work

1. Remove the CI conformance ratchet and restore/stage a coherent Make interface.
2. Run formatting and make `git diff --cached --check` clean.
3. Replace `Named(String)` with a resolved nominal leaf carried from the original
   AST; remove parse-back, equality, and nested-panic paths.
4. Add function/class scope to binding resolution before using canonical facts
   inside bodies.
5. Rebuild constructor inputs on resolved callee, metaclass, TypeVar, and MRO
   identity; abstain on incomplete evidence.
6. Convert TypedDict, `ReadOnly`, override, overload, and default joins from names
   to definition sites, and keep unreconstructed rules loudly unavailable.
7. Re-run the directed tests, then the unfiltered workspace suite.

## Verification snapshot

- Reviewed staged fingerprint:
  `81c9a84feb89b9acd6889457576363eab35aebf199c6ccb611ddfe4d5afbdfa2`.
- `git diff --cached --check`: **failed** at
  `overloads_evaluation.rs:403`.
- `cargo fmt --all -- --check`: **failed** with staged formatting diffs.
- `cargo check -p basilisk-canonical -p basilisk-resolver --tests`: **passed**
  in 6.2 seconds.
- No green broad-checker claim is made: an existing unfiltered run was still in
  progress and its live log already contained mandatory implementation panics.
- Every command started for this review was capped at ten seconds.

Bottom line: keep the definition-site resolver work and the newly directed
tests. Do not commit this exact staged snapshot until the repository-policy,
nominal-boundary, nested-scope, constructor, and formatting blockers above are
resolved.
