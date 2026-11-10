# One README, many storefronts {#README}

## Purpose {#README-PURPOSE}

Basilisk's front page is published to three storefronts — the GitHub repository,
the VS Code Marketplace / Open VSX (both read the **same** file packaged into the
VSIX), and PyPI. They were three hand-maintained files, so they drifted: one
claimed a retired typeshed behaviour months after the others were corrected.

There is now exactly **one** README per language, and the published files are
**identical except for a single line** that says which artifact you are looking
at. Everything else — the conformance claim, the benchmark table, the install
options, the feature list, the typeshed section, the acknowledgments — is one
body of text, generated to every storefront by `scripts/gen_readmes.py`.

## Source {#README-SOURCE}

| Source | Language |
|---|---|
| `docs/readme/README.src.md` | English |
| `docs/readme/README.zh.src.md` | 简体中文 |

Nothing else is authored. Editing a generated README directly is a drift bug —
CI fails it ([README-DRIFT](#README-DRIFT)).

## Targets {#README-TARGETS}

| Target key | Output | Storefront |
|---|---|---|
| `github` | `README.md`, `README.zh.md` | The GitHub repository |
| `vscode` | `vscode-extension/README.md`, `vscode-extension/README.zh.md` | VS Code Marketplace **and** Open VSX — one VSIX, one file |
| `pypi` | `README-pypi.md` | The `basilisk-python` wheel |

Open VSX is not a fourth README: `publish-vsix-ovsx` pushes the very VSIX the
Marketplace job pushes, so both registries render `vscode-extension/README.md`.

## The one-line difference {#README-IDENTITY}

Exactly one paragraph varies between targets, and it exists solely to tell the
reader which artifact this listing is. It is a single `<!--v:…-->` line per
target in the source. **No other content may be made target-specific** — a
second variant block is a review failure, not a feature: if a fact is worth
saying on one storefront it is worth saying on all of them.

Two values are substituted rather than duplicated, because they are the same
statement expressed differently per target: `{{altLangHref}}` (the
language-switch link, which must be absolute anywhere but GitHub) and, in the
Chinese source, its mirror. They are not content.

## Rendering {#README-RENDER}

The generator applies three transforms, in order:

1. **Variant blocks.** `<!--v:github-->` … `<!--/v:github-->` keeps the enclosed
   lines only for the listed targets; the list is comma-separated
   (`<!--v:vscode,pypi-->`). Markers are HTML comments, so the source renders
   correctly on its own.
2. **Tokens.** `{{altLangHref}}` is substituted per target.
3. **Link absolutisation.** Every repo-relative link and image target is
   rewritten for the non-`github` targets: images to
   `raw.githubusercontent.com/.../main/<path>`, everything else to
   `github.com/.../blob/main/<path>`. A relative path that resolves to no file
   in the repository is a generation error, not a broken published link.

## Stamped values {#README-STAMPED}

The conformance and benchmark figures are not typed by hand anywhere. They are
`<!--g:NAME-->value<!--/g:NAME-->` markers stamped into the **source** by
`scripts/gen_conformance_reference.py` from `conformance_report.json` and the
committed benchmark CSVs ([CHKARCH-CONFORMANCE](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFORMANCE)).
Generation runs after stamping, so every storefront quotes the same
self-measured number.

## Drift guard {#README-DRIFT}

`python3 scripts/gen_readmes.py --check` re-renders every target and fails if
the committed file differs. It runs in the CI website job beside the conformance
stamp check, and `make lint` runs it locally. A README edited directly, or a
source edit without regeneration, fails the build.

The same check asserts the structural rule in
[README-IDENTITY](#README-IDENTITY): every rendered target must differ from
`github` by the identity paragraph alone.
