# Authoritative source and citation policy

Trust is part of the product. Every factual claim in the published manuscript
must be traceable to the authority responsible for that fact.

## Source hierarchy

Use the first applicable level:

1. **Python typing semantics:** the maintained
   [Python typing specification](https://typing.python.org/en/latest/spec/),
   then the official [`python/typing`](https://github.com/python/typing)
   repository and conformance suite.
2. **Python language and runtime behavior:** the official
   [Python documentation](https://docs.python.org/3/) and the
   [`python/cpython`](https://github.com/python/cpython) source repository.
   Use the versioned documentation matching the behaviour only when the claim
   actually depends on a Python release.
3. **Design history and accepted changes:** published
   [Python Enhancement Proposals](https://peps.python.org/). A PEP explains the
   accepted proposal and rationale; the maintained typing specification wins
   when current normative wording differs.
4. **Stubs:** the official [`python/typeshed`](https://github.com/python/typeshed)
   repository and the typing specification's distribution rules.
5. **Packaging:** specifications and guides maintained by the Python Packaging
   Authority at [packaging.python.org](https://packaging.python.org/en/latest/).
6. **Basilisk behavior:** the documented release binary, CLI help, generated
   rule data, tests, and source at
   [`Nimblesite/Basilisk`](https://github.com/Nimblesite/Basilisk). Use a tag or
   commit permalink for exact implementation claims.
7. **Basilisk reader guidance:** the
   [Basilisk website](https://www.basilisk-python.dev/) for installation, live
   rules, release notes, configuration, migration, refactoring, debugging, and
   profiling. Re-test commands against the edition's release even when a live
   page describes them.
8. **Protocols and third-party tools:** the organization that owns the protocol
   or tool—for example, the official LSP/DAP specifications or Astral's own uv
   documentation.

Blogs, search summaries, forum posts, Wikipedia, generated prose, and competitor
documentation are not authorities for Python or Basilisk behavior. They may be
used to discover an issue, never as the published support for a claim.

## Claim rules

- Put the citation in the sentence or paragraph containing the claim.
- Link to the narrowest stable official page that supports the statement.
- Prefer versioned Python documentation for version-sensitive runtime behavior.
- Prefer a maintained spec page over a historical PEP for current semantics.
- Use a repository permalink when line-level implementation detail matters.
- State when a conclusion is an inference from source or observed behavior.
- Do not freeze moving figures such as rule counts, conformance scores,
  benchmarks, or supported platforms without a date, version, method, and live
  source.
- A screenshot proves only the captured version and scenario. Its manifest
  entry records both.
- A Basilisk claim requires agreement among its governing spec, the release
  implementation, and executable tests or captured behavior.
- If those sources disagree, omit the entire affected section and its visuals.
  Neither the implementation nor the intended specification is publishable in
  isolation.

## Source ledger

[`sources.json`](sources.json) is the machine-readable allowlist for published
external citations. Each entry records:

- a stable key used by the outline;
- the exact URL;
- the responsible authority;
- version scope; and
- the topics for which the source is suitable.

The manuscript link check fails if a published external citation is not in that
ledger. Adding a URL requires checking that it is authoritative, directly
supports the intended claim, and is reachable.

[`evidence.json`](evidence.json) is a separate publication gate for
Basilisk-specific behavior. A chapter cannot enter a release build until it
names the governing specification, pinned implementation evidence, and
executable or captured evidence, and the review decision is `publish`. A
conflict leaves the decision withheld and the affected material out of the
manuscript.

## Website links in every chapter

Every chapter contains one useful path back into the Basilisk website. These
are navigation links, not substitutes for Python authorities:

| Reader need | Website destination |
|---|---|
| Start and install | `https://www.basilisk-python.dev/docs/installation/` |
| First check | `https://www.basilisk-python.dev/docs/quick-start/` |
| Understand a rule | `https://www.basilisk-python.dev/docs/rules/` |
| Configure a project | `https://www.basilisk-python.dev/docs/configuration/` |
| Adopt existing code | `https://www.basilisk-python.dev/docs/migration/` |
| Refactor | `https://www.basilisk-python.dev/docs/refactoring/` |
| Debug | `https://www.basilisk-python.dev/docs/debugging/` |
| Profile | `https://www.basilisk-python.dev/docs/profiler/` |
| Confirm conformance method | `https://www.basilisk-python.dev/docs/conformance/` |
| Check changes | `https://www.basilisk-python.dev/docs/releases/` |

The release build performs a fresh HTTP check of every URL, checks local files
and fragments, and rejects images without local targets. External redirects are
reported so permanent moves can be updated before publication.

## Review checklist for a factual paragraph

- What exact fact is being asserted?
- Who owns that fact?
- Does the linked page say it directly?
- Is the Python/Basilisk version clear?
- Could the statement become false after a release?
- Is an observation being presented honestly as an observation?
- Do the Basilisk spec, release code, and executable evidence agree?
- Would the paragraph remain useful if a marketing adjective were removed?
