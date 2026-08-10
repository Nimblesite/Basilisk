# One README, many storefronts {#README}

## Purpose {#README-PURPOSE}

Basilisk's front page is published to five storefronts — the GitHub repository,
the VS Code Marketplace / Open VSX (both read the **same** file packaged into the
VSIX), PyPI, the Zed registry, and the Neovim plugin mirror. They were separate
hand-maintained files, so they drifted: one claimed a retired typeshed behaviour
months after the others were corrected.

There is now exactly **one** authored README, and the published files are
**identical except for a single line** that says which artifact you are looking
at. That matters more now than it did: every one of those pages carries the
withdrawal statement, and a storefront whose copy lags is a storefront still
selling a checker that produced incorrect results.

## Source {#README-SOURCE}

| Source | Holds |
|---|---|
| `docs/readme/README.src.md` | The identity line per target, the acknowledgments, and the licence |
| `docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md` | **The statement itself** ([WITHDRAWAL-COPY](DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-COPY)) |

Nothing else is authored, and there is no Chinese source: the statement has no
approved translation, and a surface that translated it would be writing its own
version of an apology for being wrong. Editing a generated README directly is a
drift bug — CI fails it ([README-DRIFT](#README-DRIFT)).

## Targets {#README-TARGETS}

| Target key | Output | Storefront |
|---|---|---|
| `github` | `README.md` | The GitHub repository |
| `vscode` | `vscode-extension/README.md` | VS Code Marketplace **and** Open VSX — one VSIX, one file |
| `pypi` | `README-pypi.md` | The `basilisk-python` wheel |
| `zed` | `basilisk-zed/README.md` | The Zed extension listing |
| `nvim` | `basilisk.nvim/README.md` | The `basilisk.nvim` plugin mirror |

Open VSX is not a fourth README: `publish-vsix-ovsx` pushes the very VSIX the
Marketplace job pushes, so both registries render `vscode-extension/README.md`.

## The one-line difference {#README-IDENTITY}

Exactly one paragraph varies between targets, and it exists solely to tell the
reader which artifact this listing is. It is a single `<!--v:…-->` line per
target in the source. **No other content may be made target-specific** — a
second variant block is a review failure, not a feature: if a fact is worth
saying on one storefront it is worth saying on all of them.

The statement is substituted, not duplicated: `{{withdrawal:title}}`,
`{{withdrawal:full}}` and `{{withdrawal:action}}` are lifted from the messaging
spec at generation time ([WITHDRAWAL-COPY](DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-COPY)).
The message therefore has exactly one author, and changing it means editing that
spec — never a README, and never this file.

## Rendering {#README-RENDER}

The generator applies three transforms, in order:

1. **Variant blocks.** `<!--v:github-->` … `<!--/v:github-->` keeps the enclosed
   lines only for the listed targets; the list is comma-separated
   (`<!--v:vscode,pypi-->`). Markers are HTML comments, so the source renders
   correctly on its own.
2. **Tokens.** `{{withdrawal:line|title|short|action|full}}` are substituted
   from the messaging spec, as the markdown it authored.
3. **Link absolutisation.** Every repo-relative link and image target is
   rewritten for the non-`github` targets: images to
   `raw.githubusercontent.com/.../main/<path>`, everything else to
   `github.com/.../blob/main/<path>`. A relative path that resolves to no file
   in the repository is a generation error, not a broken published link.

## Stamped values {#README-STAMPED}

**No figure of any kind appears in a README, and nothing is stamped.** Every
`<!--g:NAME-->value<!--/g:NAME-->` marker is gone from the sources, and the
generators that produced them are deleted. Re-introducing one is forbidden
([WITHDRAWAL-PROHIBITED](DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-PROHIBITED)):
no conformance figure, no benchmark, no rule count, in any tense.

## Drift guard {#README-DRIFT}

`python3 scripts/gen_readmes.py --check` re-renders every target and fails if
the committed file differs. It runs in the CI website job beside
`gen_withdrawal_copy.py --check`, and `make lint` runs both locally. A README
edited directly, a source edit without regeneration, or a spec edit without
regeneration all fail the build.

The same check asserts the structural rule in
[README-IDENTITY](#README-IDENTITY): every rendered target must differ from
`github` by the identity paragraph alone.

`scripts/test_published_readmes.py` is the second gate, and it tests the words
rather than the rendering: every published README opens with the statement,
contains every paragraph of the action block, links the apology without quoting
it, shows no product image, and contains nothing
[WITHDRAWAL-PROHIBITED](DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-PROHIBITED)
bars — a percentage, an install command, a marketplace link, a competitor name,
a benchmark claim, a `BSK-` code, or a `basilisk` invocation.
