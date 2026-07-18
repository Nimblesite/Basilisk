# Visual design system

The book should feel like a field guide made for working developers: confident,
precise, tactile, and unmistakably Basilisk. It should not look like a stack of
documentation screenshots or a generic corporate programming book.

## Two kinds of evidence

1. **Screenshots show what Basilisk actually did.** They are captured from the
   documented release and never generated or cosmetically rewritten.
2. **Diagrams explain why the evidence makes sense.** They are deterministic
   SVG masters with a clear teaching claim, not decorative boxes of nouns.

Generated raster illustration may be used for non-factual part openers or
texture after review. It must never invent UI, terminal output, Python syntax,
diagnostics, protocol sequences, or benchmark data.

## Palette

The palette adapts the existing Basilisk website instead of inventing a second
brand.

| Role | Color | Hex |
|---|---|---|
| Night ink | near-black navy | `#0A0C12` |
| Raised night | blue-black | `#141820` |
| Paper | warm ivory | `#F7F2E8` |
| Primary text on dark | cool white | `#F0F2F7` |
| Secondary text | slate | `#8892A4` |
| Basilisk ember | orange | `#E8500A` |
| Python current | sky blue | `#6EC5E9` |
| Success | mint | `#34D399` |
| Error | coral | `#F87171` |

Orange and blue must never be the only way to distinguish two meanings. Pair
color with labels, line style, shape, or iconography so diagrams survive
grayscale and common forms of color-vision deficiency.

## Typography

- Display and headings: Space Grotesk or a metrically compatible system sans
- Body: a highly readable EPUB-safe serif/sans stack chosen by the reader
- Code: JetBrains Mono, Fira Code, or EPUB-safe monospace fallback
- Minimum diagram label size at 1600 px: 28 px
- Minimum screenshot code size after final crop: equivalent to 16 CSS px at
  typical reading width

Typography carrying exact code or labels is overlaid deterministically. It is
not entrusted to an image generator.

## Canvas families

| Asset | Master | Publication derivative |
|---|---|---|
| Cover | 1600 × 2560 portrait SVG | optimized PNG for EPUB/web |
| Concept diagram | 1600 × 1000 SVG (16:10) | PNG fallback plus SVG master |
| Product screenshot | native high-DPI capture | 1600 × 1000 crop where practical |
| Annotated screenshot | screenshot + SVG annotation master | flattened PNG |
| Part opener | 1600 × 900 landscape | optimized WebP/PNG |

All publication images are opaque. Keep at least 72 px of safe margin around
diagram content and enough empty space for comfortable small-screen reading.

## Diagram language

Use a small, repeatable vocabulary:

| Idea | Form |
|---|---|
| Python source | paper/code card with monospace evidence |
| A type | labeled capsule with an example value |
| Compatibility | an opening gate: passes or visibly stops |
| Narrowing | one broad lane splitting into labelled smaller lanes |
| Function | contract card with input and output ports |
| Protocol | dashed outline showing required members |
| Stub | translucent interface sheet in front of a package |
| Import resolution | stacked search layers with one selected result |
| Diagnostic | source span connected to code, help, note, and link |
| LSP interaction | request/response loop between editor and Basilisk |
| Runtime evidence | separate dark lane for test/debug/profile output |

Every node must contain evidence: a code fragment, a value, a file, a command,
or an observed effect. A box that merely says “type safety” teaches nothing.

Choose among six layouts: flow, fork, stack, versus, annotated evidence, and
feedback loop. Use one focal point and no more than eight primary elements.

## Screenshot contract

- Capture real Basilisk behavior from the release recorded in `metadata.yaml`.
- Use a clean fixture from `book/examples/`; never expose personal paths,
  tokens, unrelated extensions, notifications, or private repository names.
- Record OS, architecture, editor, theme, zoom, Python interpreter, Basilisk version,
  fixture, capture command/manual steps, and source file in `figures.json`.
- Crop to the evidence while retaining enough editor/terminal context to orient
  the reader.
- Add numbered callouts outside the product UI where possible. Do not repaint
  text inside the screenshot.
- Prefer one lesson per capture. If six callouts are required, take two images.
- Terminal captures use deterministic dimensions and no animated cursor.
- Re-capture when the UI, rule wording, or documented release changes.

## Cover direction

The cover is portrait and distinct from the opening diagram. It uses the real
Basilisk mark, a dark field, an ember-orange vertical trace, and a pale code
grid that resolves into a clear path. The title remains readable at a 160 px
thumbnail. No invented snake mascot, fake IDE, stock laptop, or generated text.

Required text:

```text
THE BASILISK BOOK
A practical guide to typed Python and the Basilisk developer workflow
```

## Accessibility and production gates

Every ready visual must pass:

- descriptive alt text explaining the lesson, not merely naming the picture;
- a caption that tells the reader why the figure is present;
- readable 320 px thumbnail test;
- grayscale test;
- color-contrast check;
- no text smaller than the canvas minimum;
- source master present;
- exact target dimensions recorded;
- no personal or secret information;
- no fictional UI or diagnostic output;
- compressed derivative under the figure budget; and
- filename and status matching `figures.json`.

The release gate also finds missing, unreferenced, and duplicate assets. The
book should aim for one meaningful visual every two to three print-equivalent
pages, not filler at a fixed interval.
