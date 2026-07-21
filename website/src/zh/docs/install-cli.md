---
layout: layouts/docs.njk
title: 安装 Basilisk CLI——PyPI、Homebrew、Scoop 与二进制文件
description: 将 Basilisk Python 类型检查器作为独立 CLI 安装——通过 PyPI（uv tool install 或 pipx）、Homebrew、Scoop、预构建二进制文件或从源代码构建。单个 Rust 二进制文件，无运行时依赖——非常适合 CI。
keywords: basilisk, cli, pypi, pip, uv, pipx, homebrew, scoop, 二进制文件, 安装, rust, python类型检查器, ci, github actions
lang: zh
date: 2026-02-28
dateModified: 2026-07-19
---

# CLI 与包管理器

当您只想要 `basilisk` 二进制文件本身时——用于命令行、用于 CI，或者用来支撑一个连接系统安装的编辑器——请使用这些安装方式。Basilisk 是一个单一的 Rust 二进制文件，没有运行时依赖：无需 Node.js，无需 Python 解释器，安装后也不需要包管理器。

> 使用 **VS Code、Cursor 或 Windsurf**？二进制文件已捆绑在扩展中——参见 [VS Code 与 Cursor](/zh/docs/install-vscode/)。使用 **Zed**？二进制文件随扩展下载——参见 [Zed](/zh/docs/install-zed/)。两者都不需要单独安装 CLI。

## PyPI（uv、pipx）

wheel 包 [`basilisk-python`](https://pypi.org/project/basilisk-python/) 捆绑的是与 Homebrew、Scoop 和 GitHub Releases 相同的原生 `basilisk` CLI——由同一份源码、同一版本构建，只是走独立的发布任务。请将其作为独立工具安装，这样 `basilisk` 命令会进入 PATH，而不会影响任何项目环境：

```bash
uv tool install basilisk-python
# 或
pipx install basilisk-python
```

安装后的命令仍然是 `basilisk`（发行版之所以命名为 `basilisk-python`，只是因为 PyPI 上的 [`basilisk`](https://pypi.org/project/basilisk/) 这个名字已被一个无关项目占用）。wheel 发布平台：Linux（x86_64、aarch64）、macOS（Apple Silicon）和 Windows（x64、arm64）。wheel 中不含任何 Python 代码——没有包装脚本，也没有 console-script 入口点，只有那个独立的 Rust 二进制文件——因此适用于任何满足该发行版 `requires-python = ">=3.8"` 的 CPython 或 PyPy。Intel 版 macOS 不在任何渠道的发布目标之列（没有 wheel、没有发布归档、也没有 Homebrew bottle），请在该平台上[从源代码构建](#从源代码构建)。

## Homebrew（macOS、Linux）

```bash
brew tap Nimblesite/tap
brew install basilisk
```

在 macOS (Apple Silicon) 和 Linux (x86_64、aarch64) 上安装最新发布的 `basilisk` 二进制文件。使用 `brew upgrade basilisk` 升级。

## Scoop（Windows）

```powershell
scoop bucket add nimblesite https://github.com/Nimblesite/scoop-bucket
scoop install basilisk
```

在 Windows (x86_64 和 arm64) 上安装最新发布的 `basilisk.exe`。使用 `scoop update basilisk` 升级。

## 预构建二进制文件

从 [GitHub Releases](https://github.com/Nimblesite/Basilisk/releases) 下载适合您平台的最新版本：

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

验证安装：

```bash
basilisk --version
```

## 从源代码构建

```bash
git clone https://github.com/Nimblesite/Basilisk
cd Basilisk
cargo build --release
```

二进制文件构建在 `target/release/basilisk`。将其添加到您的 PATH：

```bash
cp target/release/basilisk /usr/local/bin/
```

需要 Rust 1.87+。

## CI 集成

Basilisk 自然地集成到任何 CI 管道中。在您的工作流程中下载二进制文件：

```yaml
# GitHub Actions 示例
- name: 安装 Basilisk
  run: |
    curl -sSfL https://github.com/Nimblesite/Basilisk/releases/latest/download/basilisk-x86_64-unknown-linux-gnu.tar.gz | tar xz
    sudo mv basilisk /usr/local/bin/

- name: 类型检查
  run: basilisk check src/
```

在已经有 `uv` 的管道中，`uv tool install basilisk-python` 与下载发布二进制文件同样好用。

退出代码：

- `0` — 未发现错误
- `1` — 发现类型错误
- `2` — 配置错误
- `3` — 内部错误
