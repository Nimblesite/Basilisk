# Chapter 9 — Configure the project, not a mood

*Part III — Make it your workflow*

> **Reader promise:** Turn a typing preference into visible, reviewable project
> policy; use the Basilisk configuration editor to choose rules, preview their
> effect, and apply a bounded change.

Two people can agree about what a function means and still disagree about
whether every function must spell out its parameter and return types. That
second question is policy. If the answer lives only in somebody's memory—or in
an editor setting on one laptop—the project does not really have an answer.

This chapter puts the answer in the repository. We will enable two annotation
rules for Signal Box, inspect them through the configuration editor, and scope
a milder severity to its test tree. The important habit is not “turn on as
much as possible.” It is: make one deliberate choice, see exactly what it will
change, and leave a configuration another reader can understand.

The Basilisk behavior in this chapter is limited to the official 0.39.0
release. Its [versioned configuration-editor specification](https://github.com/Nimblesite/Basilisk/blob/b8ae454cfabc54d26d7e4efc029f2f01bd083bc8/docs/specs/LSP-CONFIGURATION-EDITOR-SPEC.md)
and released command-line behavior provide the product boundary used below.

## Python semantics and project policy are different layers

Python's authorities draw a useful boundary around this discussion. The
official Python documentation says:

> “The Python runtime does not enforce function and variable type
> annotations.” — [Python `typing` documentation](https://docs.python.org/3/library/typing.html)

Changing a Basilisk severity therefore changes static feedback. It does not
insert a runtime check, convert a value, or change what Python executes. Your
tests still need to run; your program still needs to handle real input.

The maintained typing specification also says:

> “It is recommended but not required that checked functions have annotations
> for all arguments and the return type.” — [Python typing specification: annotations](https://typing.python.org/en/latest/spec/annotations.html)

That sentence is why this chapter treats required annotations as an explicit
Basilisk choice. `BSK-0001` reports a missing parameter annotation and
`BSK-0002` reports a missing return annotation, but both are opt-in Basilisk
rules, and 0.39.0 stays silent where its inference already determines the
parameter from a literal default or the return from literal-only paths. They
are not requirements that the Python typing specification silently forgot to
mention.

There are consequently two useful rule sources and two command lanes:

- Python typing-spec rules, labelled `pep` in the Source facet, are selected by
  Basilisk's unconfigured default and run under `basilisk check`.
- Basilisk rules, labelled `basilisk`, add project policy beyond that baseline
  and run under `basilisk analyze` only after project configuration selects
  them.

The 0.39.0 [checker architecture specification](https://github.com/Nimblesite/Basilisk/blob/b8ae454cfabc54d26d7e4efc029f2f01bd083bc8/docs/specs/CHECKER-ARCHITECTURE-SPEC.md)
defines that partition. Enabling `BSK-0001` does not add it to `check`; it makes
it eligible for `analyze`. A project that wants both typing semantics and house
policy runs both commands.

This distinction is more precise than a “strictness level.” A project may want
required annotations but not another house rule, or may want an opt-in rule at
warning rather than error. A single label such as *strict* cannot express that
decision; a rule entry can.

## Keep one active policy source

The Python Packaging Authority provides the standard home for tool-owned
project data:

> “The `[tool]` table is where any tool related to your Python project … can
> have users specify configuration data.” — [`pyproject.toml` specification](https://packaging.python.org/en/latest/specifications/pyproject-toml/)

For a new Basilisk project, put policy under `[tool.basilisk]` in the
root-level `pyproject.toml`. The rule table is a child of that namespace:

```toml
[tool.basilisk]
typeshed-commit = "83c2518a9e6abbda0c44592c3483de459198f887"

[tool.basilisk.rules]
"BSK-0001" = "error"
"BSK-0002" = "error"
```

The quotes around these two keys are valid TOML and make the diagnostic codes
easy to recognize. This configuration makes both opt-in rules active at error
severity across the project.

`pyproject.toml` is the only file Basilisk reads for policy, but a repository
can hold more than one. Basilisk resolves configuration per checked file by
walking up from that file's directory: every ancestor `pyproject.toml` with a
`[tool.basilisk]` table takes part, and for each rule the nearest table that
decides that rule wins outright. A child folder's table therefore settles the
rules it names and leaves every rule it does not name to the ancestors above
it. In a monorepo the useful question is therefore not "which file is
active?" but "which `pyproject.toml` is nearest to the file I am looking at?"
The configuration editor answers it — its source badge names the file an
approved edit will write. If a legacy root-level `basilisk.json` is still
lying around, Basilisk ignores it silently. Migrate its keys into
`[tool.basilisk]`; the editor neither reads it nor reports it as another source.

The editor itself is not a second policy store. It asks the Basilisk language
server for the live catalog and active configuration, then asks the server to
prepare and apply changes to that file. Once the source exists, **Open raw**
takes you back to the durable source of truth.

## Read the Rules view

In VS Code, open the Command Palette and run **Basilisk: Open Configuration
Editor**. In 0.39.0 the command is capability-gated: it appears when the
running server advertises the configuration-editor operations. It opens a full
editor tab for the server-computed project view.

![A direct capture of the Basilisk 0.39.0 Configuration Editor in VS Code shows its five views, tag facets, searchable rule rows, severity controls, and active pyproject source.](../assets/screenshots/09-configuration-editor.png)

*Figure 9.1 — The real 0.39.0 Configuration Editor renders state supplied by
the language server. Catalog and diagnostic totals belong to this captured
Signal Box workspace, not to a permanent product contract.*

Read the screen from left to right:

1. **Sections** separate the overview, rules, adoption work, the inventory of
   nested folder configurations, and the project source. We are staying in
   Rules for now.
2. **Tags** filter the server's live catalog. Sources distinguish core and
   opt-in rules; PEP categories organize typing topics; policy tags group
   concerns such as `strictness`. A tag is a way to find rules, not a hidden
   switch.
3. **Search** accepts ordinary rule text and focused terms. Try
   `tag:strictness`, `severity:error`, `status:entry`, `status:disabled`, or
   `has:diagnostics`. Combine terms to narrow the list.
4. **Rule rows** show the stable code, title, short explanation, tags, current
   diagnostic count, and a severity control. Select the rule title to open its
   detail and occurrences.
5. **The source badge** tells you which root file will receive an approved
   edit. In the capture it is Signal Box's `pyproject.toml`.

Do not turn a moving total from this screen into team policy. “We enable
`BSK-0002` at error” is reviewable. “We enable every current rule” will become
stale as the catalog changes and does not explain why any one rule belongs.

## Four editor choices, four stored severities

Each rule control is a plain severity list. Every choice writes a value; none
of them is a mode, a placeholder, or a level.

| Editor choice | What it means | Stored result |
|---|---|---|
| **Error** | Select the rule and report its diagnostics as errors | `error` |
| **Warning** | Select the rule and report its diagnostics as warnings | `warning` |
| **Info** | Select the rule and report its diagnostics as information | `info` |
| **Disabled** | Keep an explicit record that the rule is off | `disabled` |

A typing-spec rule offers only the first three. It can be graded down, but no
table may disable it; a narrow inline directive on the offending line,
discussed at the end of this chapter, is one way to record an honest exception.

There is no *inherited* or *native* choice, because Basilisk stores no default,
inherited, or native severity values. A rule with no entry is not sitting in a
fifth state waiting to be resolved: for a typing-spec rule it means `error`,
and for an opt-in Basilisk rule it means the rule does not run. The control
therefore shows what the rule resolves to today, and picking a value writes
that one line. Withdrawing a decision means deleting the entry in the file
itself — **Open raw** is one click away — after which the nearest ancestor
table that decides the rule takes over again.

This separation also keeps two authorities in their proper places. The Python
typing specification owns the typing relationship. Basilisk's verified
implementation owns its stable code, message, configured severity, and editor
control.

## Preview before the file changes

Changing a rule control does not immediately edit the project. It asks the
language server to calculate a preview from the active configuration, the live
rule catalog, and the current workspace diagnostics. The preview expands a tag
or rule selection into concrete rule codes and shows effective severity changes
and their hypothetical diagnostic impact.

![A four-stage diagram shows one Basilisk rule moving through server preview, human review, one approved configuration edit, and a project recheck.](../assets/diagrams/09-configuration-resolution.png)

*Figure 9.2 — Configuration is a short transaction: choose, preview, review,
then apply once. Until the last step, `pyproject.toml` is unchanged.*

Suppose Signal Box considers grading its missing-return policy from Error to
Warning. Set the root control for `BSK-0002` to Warning and read what comes back
before anything is written. The preview names one effective severity change
and shows the current diagnostic impact. The selected control and source badge
identify the entry to be written.

![A direct Basilisk 0.39.0 VS Code capture shows a BSK-0002 error-to-warning preview, the current error and warning impact, and separate Cancel and Apply change actions.](../assets/screenshots/09-configuration-preview.png)

*Figure 9.3 — This real 0.39.0 preview is a proposal tied to Signal Box's
current source revision. It has no durable effect until the reader approves
the resulting edit.*

Read the lower line first: it names `BSK-0002` and the severity it moves from
and to. The header's source badge names the file that will receive the entry.
Then read the impact cards.
Those numbers are a forecast for the current workspace, not a promise about
future files. **Cancel** closes the preview without changing anything. **Apply
change** approves this preview and asks the client to apply one versioned
workspace edit. VS Code saves a configuration document dirtied by that edit,
but does not implicitly save a file that already contained the reader's
unsaved changes. Basilisk then reloads and rechecks the root.

If the configuration changes after the preview was calculated, Basilisk
rejects the stale revision instead of overwriting the newer text. Refresh,
read the new source, and preview again. That interruption is useful: the facts
on which you based the decision have changed.

## Scope is a folder, not a pattern

That preview considered grading `BSK-0002` to Warning for the whole project.
Cancel it. The checkpoint instead keeps the root rule at Error and makes a
narrower decision for tests: Basilisk scopes rules by folder, so it uses a
second, smaller file, `tests/pyproject.toml`:

```toml
[tool.basilisk.rules]
"BSK-0002" = "warning"
```

The root file keeps its two error entries untouched. For a file under
`tests/`, the nearer table decides `BSK-0002` and wins outright. It says
nothing about `BSK-0001`, so the root entry still reports that rule as an error
there, and source files elsewhere still receive the project-level error for
both.

Folder configurations are best when they express a real boundary—tests,
generated code, or a deliberately isolated compatibility area. There is no
pattern language to reason about here, and that is the point: no globs, no
per-module tables, no precedence scores. A file's own folder chain answers
every question, and the nearest table that names a rule decides it.

The editor's **Path Overrides** view is where a repository's scoped policy
becomes visible. It lists the nested `[tool.basilisk]` tables the server
discovered beneath the root, each with its folder, its entries, and a link that
opens that folder's file for editing. It is a window on real files rather than
a second place to store patterns.

## Basilisk has no presets and no modes

There is no *strict* mode to switch on, no *standard* mode to fall back to, and
no preset that expands into a policy you did not write. Basilisk stores no
`mode = "strict"` key, and no label hovers in the background changing meaning
as the catalog grows.

What Basilisk offers instead is one written line that grades a whole family:

```toml
[tool.basilisk.rule-tags]
"basilisk" = "error"
```

That tag entry selects every rule carrying the `basilisk` tag at error
severity. It is configuration, not a hidden switch: a per-rule entry in the
same table outranks it, so a project can adopt the family and still grade one
member down. This is a large policy decision, so read the preview first, and
prefer the rules whose purpose you can explain.

## Signal Box checkpoint

Open `book/examples/signal-box` as the VS Code workspace. The checkpoint already
contains the root policy and a narrower `tests/pyproject.toml`, so its result is
reproducible without changing a file:

```console
basilisk --version
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=src \
  python3 -m unittest discover -s tests -v
basilisk check --color never
basilisk analyze --color never
```

The recorded release prints `basilisk 0.39.0`, and the runtime suite passes one
test. `check` reports no diagnostics because this fixture contains no detected
typing-spec violation; it also notes that configured non-PEP rules belong to
`analyze`. `analyze` reports three policy diagnostics: `BSK-0001` and
`BSK-0002` are errors in `src/signal_box/readings.py`, while `BSK-0002` is a
warning for `sample_reading` under `tests/`. Its summary is `Found 3 diagnostics
(2 errors).`

Now inspect the files and predict that result from configuration alone. The
root selects both rules as errors. The test folder's nearer table names only
`BSK-0002`, so it grades that rule to warning and leaves `BSK-0001` to the root.
Open **Basilisk: Open Configuration Editor** and confirm that the Project view
names the root source and Path Overrides lists `tests/`.

For a guided variation, keep the same folder and choose Info rather than
Warning in `tests/pyproject.toml`. Re-run `analyze`: only the test helper should
change category. Restore Warning when you finish.

For an independent variation, choose one real directory in your own project
and one rule whose purpose you understand. Write down the expected affected
files before you edit that folder's configuration. If the result surprises
you, revert it; the surprise is evidence that the proposed policy was not yet
clear enough.

Project and folder settings answer broad, durable questions. An inline
`# type: ignore` answers a different question about one source line; the
[official specification](https://typing.python.org/en/latest/spec/directives.html)
defines that comment as a way to silence type-checker errors. Use the narrowest
scope that honestly represents the reason, and do not scatter inline exceptions
when the repository is actually making a project choice.

## What changed

- Python typing semantics and Basilisk project policy now occupy separate
  layers in your mental model.
- `basilisk check` evaluates typing-spec rules; `basilisk analyze` evaluates
  configured opt-in policy, so a project using both runs both commands.
- Opt-in rules become active through an explicit non-disabled severity.
- Removing an entry withdraws a decision rather than choosing a default; the
  next table up the folder chain then decides the rule.
- The configuration editor renders server-owned rules and edits the active
  project file rather than keeping a private settings copy.
- Preview makes the exact scope, persisted entry, and current diagnostic impact
  visible before you apply the changes.
- A folder configuration changes one named rule in one bounded area without
  weakening unrelated checks.
- There are no presets and no modes; a tag entry is the only batch decision,
  and it is an ordinary written line.

The live [Basilisk configuration guide](https://www.basilisk-python.dev/docs/configuration/)
is the companion reference for syntax, and the generated
[rule reference](https://www.basilisk-python.dev/docs/rules/) explains the
current catalog. Check both against the Basilisk release used by your project.

## Authoritative sources

- [The Python type system](https://typing.python.org/en/latest/spec/type-system.html)
- [Python typing specification: annotations](https://typing.python.org/en/latest/spec/annotations.html)
- [Python typing specification: type-checker directives](https://typing.python.org/en/latest/spec/directives.html)
- [Python `typing` documentation](https://docs.python.org/3/library/typing.html)
- [`pyproject.toml` specification](https://packaging.python.org/en/latest/specifications/pyproject-toml/)
- [Basilisk configuration guide](https://www.basilisk-python.dev/docs/configuration/)
- [Basilisk rule reference](https://www.basilisk-python.dev/docs/rules/)
- [Basilisk 0.39.0 checker architecture](https://github.com/Nimblesite/Basilisk/blob/b8ae454cfabc54d26d7e4efc029f2f01bd083bc8/docs/specs/CHECKER-ARCHITECTURE-SPEC.md)
- [Basilisk 0.39.0 configuration-editor specification](https://github.com/Nimblesite/Basilisk/blob/b8ae454cfabc54d26d7e4efc029f2f01bd083bc8/docs/specs/LSP-CONFIGURATION-EDITOR-SPEC.md)
