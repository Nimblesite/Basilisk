---
layout: layouts/docs.njk
title: "Install the Basilisk CLI — PyPI, Homebrew, Scoop & Binaries"
description: "Install the Basilisk Python type checker as a standalone CLI via PyPI (uv tool install or pipx), Homebrew, Scoop, pre-built binaries, or from source. A single Rust binary with no runtime dependencies — ideal for CI."
keywords: basilisk, cli, pypi, pip, uv, pipx, homebrew, scoop, binary, install, rust, python type checker, ci, github actions
date: 2026-02-28
dateModified: 2026-07-19
author: The Basilisk Project
eleventyNavigation:
  key: CLI & Package Managers
  parent: Installation
  order: 4
---

# CLI & package managers

Use these methods when you want the `basilisk` binary on its own — for the command line, for CI, or to back an editor that talks to a system install. Basilisk is a single Rust binary with no runtime dependencies: no Node.js, no Python interpreter, no package manager required after installation.

> Using **VS Code, Cursor, or Windsurf**? The binary is bundled in the extension — see [VS Code & Cursor](/docs/install-vscode/). Using **Zed**? The binary downloads with the extension — see [Zed](/docs/install-zed/). Neither needs a separate CLI install.

## PyPI (uv, pipx)

The wheel [`basilisk-python`](https://pypi.org/project/basilisk-python/) bundles the same native `basilisk` CLI that ships via Homebrew, Scoop, and GitHub Releases — built from the same source at the same version, in its own release job. Install it as a standalone tool, so the `basilisk` command lands on your PATH without touching any project environment:

```bash
uv tool install basilisk-python
# or
pipx install basilisk-python
```

The installed command is `basilisk` (the distribution is named `basilisk-python` only because the [`basilisk`](https://pypi.org/project/basilisk/) name on PyPI is held by an unrelated project). Wheels are published for Linux (x86_64, aarch64), macOS (Apple Silicon), and Windows (x64, arm64). The wheel contains no Python code — no shim, no console-script entry point, just the standalone Rust binary — so it works on any CPython or PyPy meeting the distribution's `requires-python = ">=3.8"`. Intel macOS is not a published target on any channel — no wheel, no release archive, no Homebrew bottle — so build [from source](#build-from-source) there.

## Homebrew (macOS, Linux)

```bash
brew tap Nimblesite/tap
brew install basilisk
```

Installs the latest released `basilisk` binary on macOS (Apple Silicon) and Linux (x86_64, aarch64). Upgrade with `brew upgrade basilisk`.

## Scoop (Windows)

```powershell
scoop bucket add nimblesite https://github.com/Nimblesite/scoop-bucket
scoop install basilisk
```

Installs the latest released `basilisk.exe` on Windows (x86_64 and arm64). Upgrade with `scoop update basilisk`.

## Pre-built binaries

Download the latest release for your platform from [GitHub Releases](https://github.com/Nimblesite/Basilisk/releases):

```bash
# macOS (Apple Silicon)
curl -sSfL -o basilisk.zip https://github.com/Nimblesite/Basilisk/releases/latest/download/basilisk-aarch64-apple-darwin.zip
unzip basilisk.zip && sudo mv basilisk-darwin/basilisk /usr/local/bin/

# Linux (x86_64)
curl -sSfL https://github.com/Nimblesite/Basilisk/releases/latest/download/basilisk-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv basilisk /usr/local/bin/

# Linux (aarch64)
curl -sSfL https://github.com/Nimblesite/Basilisk/releases/latest/download/basilisk-aarch64-unknown-linux-gnu.tar.gz | tar xz
sudo mv basilisk /usr/local/bin/
```

Verify the installation:

```bash
basilisk --version
```

## Build from source

```bash
git clone https://github.com/Nimblesite/Basilisk
cd Basilisk
cargo build --release
```

The binary is built at `target/release/basilisk`. Add it to your PATH:

```bash
cp target/release/basilisk /usr/local/bin/
```

Rust 1.87+ required.

## CI integration

Basilisk integrates naturally into any CI pipeline. Download the binary in your workflow:

```yaml
# GitHub Actions example
- name: Install Basilisk
  run: |
    curl -sSfL https://github.com/Nimblesite/Basilisk/releases/latest/download/basilisk-x86_64-unknown-linux-gnu.tar.gz | tar xz
    sudo mv basilisk /usr/local/bin/

- name: Type check
  run: basilisk check src/
```

In a pipeline that already has `uv` available, `uv tool install basilisk-python` works just as well as downloading the release binary.

Exit codes:

- `0` — No errors found
- `1` — Type errors found
- `2` — Configuration error
- `3` — Internal error
