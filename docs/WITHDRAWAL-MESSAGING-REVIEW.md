# Withdrawal messaging review — remaining actions

- **Make the VS Code extension notice-only.** Reduce `vscode-extension/src/extension.ts` to the generated notice plus the website link; remove the checker/LSP/debugger/profiler UI, activation events, runtime dependencies, and bundled binaries from `package.json`, `shipwright.json`, and the release workflow. Replace `package.json:4` with the exact one-line copy and add activation/package-content tests.

- **Redirect every retired website route to `/`.** Replace the 297 generated HTTP-200 notice pages in `website/src/notice.njk`; update `WEBSITE-E2E-SPEC.md` and `withdrawal.spec.ts` to assert redirects. Remove the hand-written 404 CTA, and render one approved block intact in `llms.txt` instead of the current hybrid.

- **Make the final release genuinely one-shot.** Restrict `release.yml` to one explicit final version, use the canonical statement as the release body, repair `gen_release_notes.py` for the inert CLI, add the missing `delist/` runbook (including Zed), verify publish → live → unlist per channel, then disable future publishing. Resolve the spec's current contradiction between “all unlisted,” “being unlisted,” and a final release that has not shipped yet.

- **Remove the remaining public product copy.** Replace or retire the feature, result, and install messaging still exposed by `basilisk.nvim/doc/basilisk.txt`, `basilisk.nvim/lspconfig/basilisk.lua`, the Zed extension's commands/configuration, `book/`, and the other crate/package READMEs. Add a repository-wide prohibited-copy scan so the five generated storefront READMEs are not the only surfaces checked.

- **Restore and supersede internal records.** Restore `WEBSITE-ERROR-PAGES-SPEC.md` and `WEBSITE-SCREENSHOTS-SPEC.md`; mark retired specs and plans as superseded and reframe `docs/INDEX.md` so obsolete checker/editor behavior is not presented as a current shipped contract.

- **Make the README/surface contract exact.** Resolve the short-copy assignment at messaging-spec line 25 against the full + action requirement at line 89. Remove the generated READMEs' extra identity, acknowledgment, license, and company prose—or explicitly permit the required legal footer—and test exact normalized output plus every package/store description.

- **Tighten the canonical wording, then regenerate.** Use “The checker used how code was *spelled* instead of what it meant, so it could report false errors and miss real bugs”; change “It prints this statement” to “It prints the withdrawal notice”; change the results paragraph to “Treat every result Basilisk produced as unverified. A clean run did not verify your code, and a reported error may have been incorrect”; and replace each dangling “Nothing … until it has been rebuilt” with an explicit Basilisk/new-product subject.

- **Align internal guidance with the canonical vocabulary.** In `CLAUDE.md`, remove “delisting,” the extra “Python dev experience tool” category, and the remaining paraphrased/emotive claims; copy the approved claims and prohibitions verbatim. Update `docs/INDEX.md:16` to say “prohibited claims” and include the CLI's output among governed surfaces.
