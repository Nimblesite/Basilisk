---
layout: layouts/docs.njk
title: 安装
description: 如何安装 Basilisk——通过 PyPI（uv 或 pipx）、Homebrew、Scoop、预构建二进制文件、VS Code 扩展、Zed 扩展或从源代码构建。
keywords: basilisk, 安装, pypi, pip, uv, pipx, homebrew, scoop, rust, python类型检查器, vs code, zed
lang: zh
---

# 安装

Basilisk 是一个单一的 Rust 二进制文件，没有运行时依赖。无需 Node.js。无需 Python 解释器。安装后不需要包管理器。

## VS Code 扩展（推荐）

最快的入门方式。从 VS Code 市场安装 **Basilisk** 扩展：

1. 打开 VS Code
2. 进入扩展（`Ctrl+Shift+X` / `Cmd+Shift+X`）
3. 搜索 **Basilisk**
4. 点击**安装**

**扩展会捆绑适合您平台的 Basilisk 二进制文件。** 默认 VSIX 安装无需手动设置。

```bash
git clone https://github.com/Nimblesite/Basilisk
cd basilisk
cargo build --release
```

| 操作系统 | 架构 |
|----|-------------|
| macOS | Apple Silicon (aarch64) |
| macOS | Intel (x86_64) |
| Linux | x86_64 |
| Linux | aarch64 |
| Windows | x86_64 |

仅当您明确想覆盖 VSIX 中捆绑的二进制文件时，才需要设置 `basilisk.executablePath`、`basilisk.binaries.basilisk` 或 `basilisk.binaries.path`。

## PyPI（uv、pipx）

wheel 包 [`basilisk-python`](https://pypi.org/project/basilisk-python/) 捆绑的是与 Homebrew、Scoop 和 GitHub Releases 完全相同的原生二进制文件。请将其作为独立工具安装，这样 `basilisk` 命令会进入 PATH，而不会影响任何项目环境：

```bash
uv tool install basilisk-python
# 或
pipx install basilisk-python
```

安装后的命令仍然是 `basilisk`（发行版之所以命名为 `basilisk-python`，只是因为 `basilisk` 这个名字在 PyPI 上已被占用）。wheel 发布平台：Linux（x86_64、aarch64）、macOS（Apple Silicon）和 Windows（x64、arm64）。wheel 中不含任何 Python 代码——它就是同一个独立的 Rust 二进制文件，因此适用于任何 CPython 或 PyPy 3.x。

## Homebrew (macOS、Linux)

```bash
brew tap Nimblesite/tap
brew install basilisk
```

在 macOS (Apple Silicon) 和 Linux (x86_64、aarch64) 上安装最新发布的 `basilisk` 二进制文件。使用 `brew upgrade basilisk` 升级。

## Scoop (Windows)

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
unzip basilisk.zip && sudo mv basilisk /usr/local/bin/

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

在 [github.com/Nimblesite/Basilisk](https://github.com/Nimblesite/Basilisk) 跟踪进度。

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

退出代码：
- `0` — 未发现错误
- `1` — 发现类型错误
- `2` — 配置错误
- `3` — 内部错误

## Zed 扩展

Basilisk 提供了一个原生 Zed 扩展，为 Python 文件注册 LSP。安装后，Basilisk 自动激活为所有 `.py` 文件的语言服务器。

### 安装扩展

1. 构建并安装 Basilisk CLI 二进制文件：

```bash
cargo install --path crates/basilisk-cli
```

这将二进制文件安装到 `~/.cargo/bin/basilisk`。

2. 在 Zed 中安装开发扩展：

- 打开命令面板：`Cmd+Shift+P`
- 运行 **zed: install dev extension**
- 从存储库中选择 `basilisk-zed/` 目录

Zed 自动将扩展的 Rust 源代码编译为 WASM。您不需要预构建或复制任何 `.wasm` 文件。

3. 打开一个 Python 文件——Basilisk 现在是您的 Python 语言服务器。

### Zed 如何找到二进制文件

Zed 扩展按以下顺序解析 Basilisk 二进制文件：

1. **Zed LSP 设置** — 如果您在 Zed `settings.json` 中配置了显式路径：

```json
{
  "lsp": {
    "basilisk": {
      "binary": {
        "path": "/path/to/basilisk"
      }
    }
  }
}
```

2. **`BASILISK_PATH` 环境变量** — 设置此变量以覆盖默认位置
3. **`~/.cargo/bin/basilisk`** — `cargo install` 放置二进制文件的默认位置

Zed **不**从 PATH 解析裸命令名。扩展始终返回二进制文件的绝对路径。

### 在 Zed 中配置 Basilisk 设置

将 Basilisk 特定设置添加到您的 Zed `settings.json`：

```json
{
  "lsp": {
    "basilisk": {
      "settings": {
        "analysisMode": "wholeModule"
      }
    }
  }
}
```

> 语言服务器目前仅识别 `analysisMode`（`wholeModule` 或 `openFilesOnly`）和
> `testExplorer` 设置。其他设置尚未被服务器读取——请参阅
> [配置参考](/zh/docs/configuration/)了解当前实际生效的设置。

### 更改后重新构建

如果您修改了 Basilisk 源代码：

1. 重新构建 CLI 二进制文件：`cargo install --path crates/basilisk-cli --force`
2. 在 Zed 中重新安装开发扩展：`Cmd+Shift+P` → **zed: install dev extension** → 选择 `basilisk-zed/`

Zed 自动重新编译 WASM 并重新加载扩展。

## 编辑器支持 (LSP)

Basilisk 实现了语言服务器协议。任何支持 LSP 的编辑器都可以使用它：

- **VS Code** — 通过官方 Basilisk 扩展（捆绑匹配的二进制文件）
- **Zed** — 通过 Basilisk Zed 扩展（见上文）
- **Neovim** — 通过 nvim-lspconfig
- **Helix** — 原生 LSP 支持
- **Emacs** — 通过 eglot 或 lsp-mode

## VS Code 扩展如何找到二进制文件

扩展按以下顺序解析 Basilisk 二进制文件：

1. **显式组件路径** — `basilisk.binaries.basilisk` 或 `basilisk.executablePath`
2. **显式二进制目录** — `basilisk.binaries.path`
3. **捆绑的 VSIX 二进制文件** — `bin/<platform>/basilisk`
4. **外部安装** — Cargo、Homebrew、Scoop 或 PATH，前提是版本匹配

Homebrew 和 Scoop 是外部覆盖或修复来源。默认 VSIX 安装会运行 VSIX 内捆绑的二进制文件。
