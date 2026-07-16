# The Basilisk Book

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

Requirements: Python 3.12+, Pandoc 3+, and EPUBCheck 5+.

## Drafting rules

1. Treat `book.json` as the one source of chapter order and production targets.
2. Cite claims beside the sentence they support; do not save citations for a
   generic bibliography.
3. Use only entries in `sources.json` for published external claims.
4. Validate Basilisk commands against the release binary being documented.
5. Omit any topic whose governing specification, release implementation, and
   executable tests do not agree. A caveat is not permission to publish it.
6. Use real product captures for UI and terminal screenshots. Never generate a
   fictional Basilisk interface.
7. Give every visual a useful caption, descriptive alt text, provenance, and a
   source master.
8. Run `make release` before publishing the EPUB or website edition.

See [OUTLINE.md](OUTLINE.md) for the learning journey and
[SOURCE-POLICY.md](SOURCE-POLICY.md) for the authority model.
