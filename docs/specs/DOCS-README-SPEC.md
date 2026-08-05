# One README, many storefronts {#README}

## Purpose {#README-PURPOSE}

Basilisk's front page is published to three storefronts — the GitHub repository,
the VS Code Marketplace / Open VSX (both read the **same** file packaged into the
VSIX), and PyPI. They were three hand-maintained files, so they drifted: one
claimed a retired typeshed behaviour months after the others were corrected.

There is now exactly **one** README per language, and the published files are
**identical except for a single line** that says which artifact you are looking
at. Everything else — the metrics retraction and status notice, the install
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

## Withdrawn metrics notice {#README-METRICS-WITHDRAWAL}

Both canonical sources MUST carry a prominent retraction and current-status
notice before the first product-feature section. The English and Chinese
notices MUST communicate the same facts:

- Basilisk's former 100% conformance claim is retracted because the result was
  not robust under semantics-preserving mutations and was therefore not
  trustworthy.
- Basilisk has been removed from the official `python/typing` results table at
  the project's request.
- The current conformance level is temporarily unknown while test-fitted code
  is deleted and the affected logic is reimplemented from the typing
  specification.
- All published benchmark figures and performance rankings are withdrawn while
  the measurement pipeline is audited.
- Replacement results will be published only after robustness and mutation
  validation, even if the trustworthy result is less favourable.

While that withdrawal is active, neither canonical source nor any generated
target may present a current conformance percentage, passed-test total, error
count, benchmark timing, comparative performance ranking, or numerical
benchmark table. The former 100% figure may appear only as the clearly labelled
historical claim being retracted. Removing the disclosure before trustworthy
replacement results are available is a documentation-integrity failure.

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

## Withdrawn values and future stamping {#README-STAMPED}

While [README-METRICS-WITHDRAWAL](#README-METRICS-WITHDRAWAL) is active, the
canonical README sources intentionally contain no stamped conformance or
benchmark values. `scripts/gen_readmes.py` propagates the shared retraction text
unchanged to every target.

If trustworthy figures are restored later, they MUST NOT be typed by hand.
`<!--g:NAME-->value<!--/g:NAME-->` markers in the **canonical sources**, stamped
by `scripts/gen_conformance_reference.py` from `conformance_report.json` and the
committed benchmark CSVs, remain the only permitted publication path
([CHKARCH-CONFORMANCE](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFORMANCE)).
Generation must run after stamping so every storefront receives the same
self-measured values. Restoring figures also requires replacing this temporary
withdrawal contract with acceptance criteria for the validated methodology.

## Drift guard {#README-DRIFT}

`python3 scripts/gen_readmes.py --check` re-renders every target and fails if
the committed file differs. It runs in the CI website job beside the conformance
stamp check, and `make lint` runs it locally. A README edited directly, or a
source edit without regeneration, fails the build.

The same check asserts the structural rule in
[README-IDENTITY](#README-IDENTITY): every rendered target must differ from
`github` by the identity paragraph alone.
