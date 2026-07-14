# Editorial brief

## Positioning

*The Basilisk Book* is the friendly field guide between a five-minute quick
start and reference documentation. It teaches readers how to use Basilisk by
teaching the Python type relationships that make its output meaningful.

It is not marketing copy. A diagnostic is interesting because it reveals a
relationship in the reader's program, not because Basilisk produced it.

## Reader

The primary reader can write and run ordinary Python but may have learned
typing piecemeal. They understand functions, classes, imports, collections,
exceptions, and basic tests. They do not need prior knowledge of type theory,
language-server protocols, stubs, or static-analysis terminology.

Experienced typed-Python users should still find the workflow, configuration,
adoption, editor, and investigation chapters useful.

## Voice

- Direct, calm, and technically exact
- Second person when guiding an action; first person plural only for a shared
  investigation
- Define a term at first use, then use the real term consistently
- Prefer a concrete program state over an analogy
- Never shame dynamic Python or describe valid Python as inherently bad
- Never treat a type checker as proof that a program is correct
- Avoid comparative superlatives unless the claim is necessary, current,
  reproducible, and cited beside the sentence

Target readable prose rather than compressed reference language. Paragraphs
should usually make one move. Code blocks should usually fit without horizontal
scrolling on a small e-reader.

## Teaching pattern

Start from a question a developer actually has:

- Why is this argument rejected?
- Why did the type become broader here?
- Where did this imported signature come from?
- What will a quick fix change?
- Why does the editor update another file?

Then show evidence, explain the smallest general principle, change the code,
and check the result. Use a worked example, a partially guided variation, and
an independent variation. Ask readers to predict a result before revealing it
whenever prediction will expose their mental model.

## Scope and versions

- Canonical language target: Python 3.12
- Typing semantics: current Python typing specification, with historical PEPs
  used for rationale rather than as a substitute for the maintained spec
- Basilisk behavior: one named release per book edition
- Screenshots: captured from that same release and recorded in `figures.json`
- Website: practical companion links may move forward; release-specific claims
  remain tied to the edition's release and source provenance

Before prose drafting moves beyond outline status, set the exact Basilisk
release in `metadata.yaml` and replace every unversioned implementation claim
with a tested, edition-specific statement.

## Agreement gate

A Basilisk topic is publishable only when all three sources agree:

1. the governing repository specification;
2. the pinned release implementation; and
3. executable tests or captured behavior from that release.

If they do not agree, omit the section, example, screenshot, command, and claim.
Do not publish the implementation as a workaround, do not publish the intended
specification as a promise, and do not turn the discrepancy into a caveat. The
repository can resolve it first; a later edition can then add the topic.

## Chapter limits

- 1,800–2,700 words
- Four to six core sections
- Six to twelve short code blocks
- Two to five visuals
- One guided Signal Box checkpoint
- One partially guided and one independent practice task
- Five to seven closing takeaways

## Code standards

- Every complete example must be runnable in the committed book environment.
- Every diagnostic excerpt must be generated from the documented Basilisk
  release, not composed from memory.
- Show file paths when an example spans files.
- Prefer Python 3.12 syntax unless a compatibility comparison is the lesson.
- Do not add typing syntax purely to make an example look sophisticated.
- Explain whether an annotation changes runtime behavior in the demonstrated
  context.

## Corrections and maintenance

Every edition records its build date, Basilisk version, Python target, example
test result, link-audit result, and screenshot environment. Readers should be
sent to the Basilisk website and repository issue tracker for live corrections;
the final URLs are confirmed before publication.
