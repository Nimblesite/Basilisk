#!/usr/bin/env python3
"""Submit or bump Basilisk's listing in `zed-industries/extensions`.

Implements [ZED-MIRROR]; see docs/specs/ZED-SPEC.md#ZED-MIRROR.

Zed has no upload API. Pushing the rendered tree to `Nimblesite/basilisk-zed`
publishes nothing on its own — the extension only becomes installable once
`zed-industries/extensions` lists it, as a git submodule pinned to a commit plus
an `extensions.toml` entry naming the version. That listing step used to be a
manual to-do that was never done, which is why Basilisk has never appeared in
Zed's extensions view. This script performs it, and is safe to re-run: the first
release opens the listing PR, every later release moves the submodule pointer
and the version on the same branch.

Usage:
    scripts/publish_zed_registry.py <version> <tag>
    scripts/publish_zed_registry.py 0.41.0 v0.41.0

Requires `gh` authenticated as a token that can fork into UPSTREAM_FORK's owner
and push to that fork.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

UPSTREAM = "zed-industries/extensions"
FORK = "Nimblesite/extensions"
MIRROR_URL = "https://github.com/Nimblesite/basilisk-zed.git"
EXTENSION_ID = "basilisk"
SUBMODULE_PATH = f"extensions/{EXTENSION_ID}"
REGISTRY_TOML = "extensions.toml"
GITMODULES = ".gitmodules"


def run(cmd: list[str], cwd: Path | None = None, check: bool = True) -> str:
    """Run `cmd`, echoing it, and return stdout."""
    print(f"    $ {' '.join(cmd)}", flush=True)
    done = subprocess.run(cmd, cwd=cwd, check=check, capture_output=True, text=True)
    if done.stdout:
        print(done.stdout.rstrip(), flush=True)
    if done.returncode != 0 and done.stderr:
        print(done.stderr.rstrip(), file=sys.stderr, flush=True)
    return done.stdout


def blocks(text: str, opener: str) -> list[tuple[str, list[str]]]:
    """Split a flat TOML-ish file into `(header, lines)` blocks.

    `extensions.toml` and `.gitmodules` are both a preamble followed by a flat
    run of sections, each introduced by a line starting with `opener`. Splitting
    on those headers lets us edit one section without reformatting the other
    ~2000, which a whole-document rewrite would do. The result is parse-verified
    by `tomllib` in `write_registry` before anything is committed.
    """
    out: list[tuple[str, list[str]]] = []
    header, current = "", []
    for line in text.splitlines(keepends=True):
        if line.startswith(opener):
            if header or current:
                out.append((header, current))
            header, current = line.strip(), [line]
        else:
            current.append(line)
    if header or current:
        out.append((header, current))
    return out


def sort_key(header: str) -> str:
    """The section's sort key, matching upstream's `pnpm sort-extensions`."""
    return header.strip("[]").strip('"').casefold()


def splice(text: str, opener: str, header: str, body: list[str]) -> str:
    """Replace the `header` section in `text`, or insert it in sorted order."""
    sections = blocks(text, opener)
    kept = [s for s in sections if s[0] != header]
    entry = (header, body)
    at = len(kept)
    for index, (existing, _) in enumerate(kept):
        if existing.startswith(opener) and sort_key(existing) > sort_key(header):
            at = index
            break
    kept.insert(at, entry)
    return "".join("".join(lines) for _, lines in kept)


def write_registry(repo: Path, version: str) -> None:
    """Point `extensions.toml`'s `[basilisk]` entry at `version`."""
    path = repo / REGISTRY_TOML
    body = [
        f"[{EXTENSION_ID}]\n",
        f'submodule = "{SUBMODULE_PATH}"\n',
        f'version = "{version}"\n',
        "\n",
    ]
    updated = splice(path.read_text(encoding="utf-8"), "[", f"[{EXTENSION_ID}]", body)
    path.write_text(updated, encoding="utf-8")

    listed = tomllib.loads(updated).get(EXTENSION_ID)
    if listed != {"submodule": SUBMODULE_PATH, "version": version}:
        raise SystemExit(f"✗ {REGISTRY_TOML} did not round-trip: {listed!r}")
    print(f"  {REGISTRY_TOML}: [{EXTENSION_ID}] version = {version}")


def write_gitmodules(repo: Path) -> None:
    """Re-sort `.gitmodules` so `git submodule add`'s append stays ordered."""
    path = repo / GITMODULES
    text = path.read_text(encoding="utf-8")
    sections = blocks(text, "[submodule ")
    preamble = [s for s in sections if not s[0].startswith("[submodule ")]
    entries = sorted(
        (s for s in sections if s[0].startswith("[submodule ")),
        key=lambda s: sort_key(s[0].removeprefix("[submodule ")),
    )
    ordered = "".join("".join(lines) for _, lines in [*preamble, *entries])
    if ordered != text:
        path.write_text(ordered, encoding="utf-8")
        print(f"  {GITMODULES}: re-sorted")


def clone_fork(work: Path) -> Path:
    """Fork upstream if needed, then clone the fork reset to upstream's head."""
    run(["gh", "repo", "fork", UPSTREAM, "--clone=false", "--remote=false"])
    repo = work / "extensions"
    run(["gh", "repo", "clone", FORK, str(repo), "--", "--depth=50"])
    run(
        ["git", "remote", "add", "upstream", f"https://github.com/{UPSTREAM}.git"], repo
    )
    run(["git", "fetch", "--depth=50", "upstream", "HEAD"], repo)
    run(["git", "checkout", "-B", f"listing-{EXTENSION_ID}", "FETCH_HEAD"], repo)
    return repo


def pin_submodule(repo: Path, tag: str) -> None:
    """Add the mirror submodule if absent, then pin it to `tag`."""
    module = repo / SUBMODULE_PATH
    if not (module / ".git").exists():
        run(["git", "submodule", "add", "--force", MIRROR_URL, SUBMODULE_PATH], repo)
    run(["git", "fetch", "--tags", "origin"], module)
    run(["git", "checkout", f"tags/{tag}"], module)


def commit_and_push(repo: Path, version: str) -> bool:
    """Commit the listing change and push the branch. False if nothing changed."""
    run(["git", "config", "user.name", "github-actions[bot]"], repo)
    email = "41898282+github-actions[bot]@users.noreply.github.com"
    run(["git", "config", "user.email", email], repo)
    run(["git", "add", "-A"], repo)
    if not run(["git", "status", "--porcelain"], repo).strip():
        print(f"  {UPSTREAM} already lists {EXTENSION_ID} {version}")
        return False
    run(["git", "commit", "-m", f"{EXTENSION_ID}: {version}"], repo)
    run(
        ["git", "push", "--force-with-lease", "origin", f"listing-{EXTENSION_ID}"], repo
    )
    return True


def open_pr(repo: Path, version: str) -> None:
    """Open the listing PR, unless one is already open for this branch."""
    head = f"{FORK.split('/')[0]}:listing-{EXTENSION_ID}"
    existing = run(
        ["gh", "pr", "list", "--repo", UPSTREAM, "--head", head, "--json", "url"],
        check=False,
    )
    if '"url"' in existing:
        print(f"  PR already open for {head} — pointer updated in place")
        return
    body = (
        f"Adds the Basilisk language-server extension at {version}.\n\n"
        f"Submodule: {MIRROR_URL} (pinned to the release tag).\n"
        "The extension attaches to Zed's built-in Python language and ships no "
        "`languages/` tree or grammar, so it does not shadow the built-in "
        "definition.\n\nRefs https://github.com/Nimblesite/Basilisk\n"
    )
    create = ["gh", "pr", "create", "--repo", UPSTREAM, "--head", head]
    create += ["--title", f"Add {EXTENSION_ID} {version}", "--body", body]
    run(create, repo, check=False)


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    version, tag = argv
    if not os.environ.get("GH_TOKEN") and not os.environ.get("GITHUB_TOKEN"):
        print(
            "✗ GH_TOKEN not set — cannot fork or open the listing PR", file=sys.stderr
        )
        return 1

    with tempfile.TemporaryDirectory() as tmp:
        repo = clone_fork(Path(tmp))
        pin_submodule(repo, tag)
        write_registry(repo, version)
        write_gitmodules(repo)
        if commit_and_push(repo, version):
            open_pr(repo, version)
    print(f"  {EXTENSION_ID} {version} submitted to {UPSTREAM}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
