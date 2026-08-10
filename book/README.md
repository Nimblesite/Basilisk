# The Basilisk Book

> **NOT BEING PUBLISHED.** Basilisk is unlisted: its type checker was producing incorrect results, and every distribution channel is being unlisted ([the statement](https://www.basilisk-python.dev/)). A book teaching people to install and rely on that checker is not going out. This folder stays as the record of what was drafted; nothing in it is a current claim about a product, and none of it is being finished.

This folder is the publication workspace for *The Basilisk Book*: a free,
cover-to-cover guide to using Basilisk and understanding the Python typing
ideas that make its feedback useful.

The current edition is a **structural prototype**. It establishes the learning
journey, page and visual budgets, source rules, chapter contracts, publication
tooling, and first design assets before long-form drafting begins.

## Reader promise

By the end of the book, a Python developer should be able to:

- install Basilisk and complete a first check;
- read a diagnostic and decide what to change;
- reason about the Python types that appear in everyday code;
- use inference, narrowing, generics, protocols, structured data, and stubs;
- configure Basilisk for a real repository;
- adopt an existing codebase without hiding its remaining work;
- use the editor, refactoring, debugging, testing, and profiling workflows; and
- run Basilisk consistently in local development and CI.

The book is a practical guide, not a replacement for the Python typing
specification or Basilisk's live documentation. Each chapter sends readers to
the relevant official sources and to the
[Basilisk website](https://www.basilisk-python.dev/).

## Project map

```text
book/
├── book.json                 # canonical reading order and production targets
├── metadata.yaml             # EPUB metadata
├── OUTLINE.md                # detailed chapter architecture
├── EDITORIAL-BRIEF.md        # audience, voice, teaching pattern, scope
├── SOURCE-POLICY.md          # authority and citation rules
├── VISUAL-DESIGN-SYSTEM.md   # cover, diagrams, screenshots, accessibility
├── sources.json              # approved source ledger
├── evidence.json             # per-chapter spec/code/test agreement gate
├── figures.json              # planned and completed visual ledger
├── manuscript/               # book text in reading order
├── examples/                 # the running Signal Box project
├── assets/
│   ├── brand/
│   ├── cover/
│   ├── diagrams/
│   ├── illustrations/
│   └── screenshots/
├── scripts/                  # structure, link, and EPUB checks
├── styles/                   # EPUB CSS
└── dist/                     # generated output; never hand-edited
```

## Production commands

The initial toolchain follows the reference book's Pandoc/EPUBCheck approach,
but ordering and quality gates come from machine-readable manifests.

```sh
make check             # structure, sources, local links, and assets
make check-links       # include every external URL
make epub              # build and validate the structural EPUB prototype
make release           # strict checks, external links, EPUB, EPUBCheck
```

## Drafting rules

1. Treat `book.json` as the one source of chapter order and production targets.
2. Cite claims beside the sentence they support; do not save citations for a
   generic bibliography.
3. Use only entries in `sources.json` for published external claims.
4. Validate Basilisk commands against the release binary being documented.
5. Omit any topic whose governing specification, release implementation, and
   executable tests do not agree. A caveat is not permission to publish it.
6. Any visual intended to show Basilisk, an editor, a terminal, diagnostics,
   controls, or product output must be a direct capture of the edition's pinned
   release. Never mock, redraw, reconstruct, generate, or hand-compose it —
   even if it is labelled a diagram, wireframe, or conceptual map. Cropping and
   uniform publication resizing are allowed, as are external callouts;
   product pixels and text may not be repainted, replaced, or composited. If a
   real capture is unavailable, omit the visual.
7. Give every visual a useful caption, descriptive alt text, provenance, and a
   source master.
8. Run `make release` before publishing the EPUB or website edition.

See [OUTLINE.md](OUTLINE.md) for the learning journey and
[SOURCE-POLICY.md](SOURCE-POLICY.md) for the authority model.
