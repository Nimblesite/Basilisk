# Withdrawal messaging and final-release review

**Verdict: mostly fixed, but do not tag yet.** The seven real release paths—GitHub Releases, VS Code Marketplace, Open VSX, PyPI, Homebrew, Scoop, and Neovim—are wired and currently serve `0.41.1`.

- **Remove Zed from distribution scope; do not create a listing now.** The [official registry](https://github.com/zed-industries/extensions/blob/main/extensions.toml) has no `basilisk` entry, and the repository records that listing was never completed. Remove the Zed channel claims and generated copies; retire `delist/00-publish-zed-final.sh`, `delist/06-unlist-zed.sh`, the registry publisher/tests, and the Zed checks in the final-release verifier. Keep the source or mirror only as a historical artefact.

- **Make the release mechanically one-shot.** Pin `.github/workflows/release.yml` to the chosen stable final tag—currently implied to be `v0.42.0`—instead of every `v*`; allow reruns only for that tag.

- **Preflight every real publisher before tagging.** Reconfirm `VSCODE_MARKETPLACE_PAT`, `OPEN_VSX_PAT`, `BREW_SCOOP_PAT` access to the tap, bucket, and Neovim mirror, plus the PyPI trusted publisher. The `v0.41.1` workflow succeeded on all seven paths on 2026-08-08, but credentials can change.

- **Make retries safe and confirm the target set.** Add `skip-existing: true` to the PyPI action. The workflow covers Linux x64/ARM64, macOS ARM64, Windows x64/ARM64, and one universal VSIX; add Intel macOS now if its omission is not intentional.

- **Generate every final-release message from the spec.** Delete the hand-written VS Code `ANNOUNCEMENT` and show the generated notice. Define one final-release block and render it verbatim from `gen_release_notes.py`; remove the custom “This release” copy and its conflicting unlisting tenses.

- **Make the unlisting promise achievable.** PyPI yanking and Open VSX removal are external, so promise: publish → verify live → begin unlisting → verify absent, not “unlisted immediately.” Confirm irreversible Marketplace `unpublish` is intended.

- **Resolve the remaining copy-contract conflicts, then regenerate.** Choose one README assignment (`Short` conflicts with `Full + Action`), allow notice-only commands and any required legal footer explicitly, and apply the recorded wording fixes: “used how code was *spelled*…”, “prints the withdrawal notice”, “Treat every result Basilisk produced as unverified”, and explicit subjects instead of dangling “it”.
