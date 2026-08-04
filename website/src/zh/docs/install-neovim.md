---
layout: layouts/docs.njk
title: Neovim 版 Basilisk——安装与更新 basilisk.nvim
description: 使用 basilisk.nvim 在 Neovim 中安装 Basilisk Python 语言服务器。二进制文件自动下载；用 :BasiliskUpdate 在编辑器内更新。通过内置 LSP 客户端提供诊断、自动补全、调试、测试和性能分析。
keywords: basilisk, neovim, nvim, basilisk.nvim, python, 语言服务器, lsp, 安装, 更新, lazy.nvim, packer, vim-plug, nvim-dap
lang: zh
date: 2026-07-11
dateModified: 2026-07-11
---

# Neovim 版 Basilisk

[`basilisk.nvim`](https://github.com/Nimblesite/basilisk.nvim) 将 Neovim 内置的 LSP 客户端（0.11+，通过 `vim.lsp.config` / `vim.lsp.enable`）连接到 Basilisk 语言服务器。一个插件覆盖整个工作流：诊断、悬停、自动补全、跳转到定义、重命名、代码操作、格式化、内联提示、调试（通过 nvim-dap）、测试浏览器和性能分析。

需要安装两个部分——**插件**（通过您的插件管理器）和 **`basilisk` 二进制文件**（自动下载；通常您永远不需要自己安装它）。

## 1. 安装插件

**lazy.nvim**

```lua
{
  "Nimblesite/basilisk.nvim",
  ft = "python",
  dependencies = { "mfussenegger/nvim-dap" }, -- 可选，用于调试
  opts = {},
}
```

**packer.nvim**

```lua
use {
  "Nimblesite/basilisk.nvim",
  ft = "python",
  config = function()
    require("basilisk").setup({})
  end,
}
```

**vim-plug**

```vim
Plug 'Nimblesite/basilisk.nvim'
```

然后，在 `plug#end()` 之后：

```lua
lua require("basilisk").setup({})
```

**vim.pack（内置，Neovim 0.12+）**

```lua
vim.pack.add({
  { src = "https://github.com/Nimblesite/basilisk.nvim",
    version = vim.version.range("*") }, -- 最新稳定标签
})
require("basilisk").setup({})
```

## 2. 二进制文件随插件一同提供

**您不需要单独安装 Basilisk 二进制文件。** 打开一个 Python 文件：如果找不到 `basilisk` 二进制文件，插件会从 [GitHub 发布页](https://github.com/Nimblesite/Basilisk/releases)下载适合您平台的最新版本，缓存到 Neovim 的数据目录中，然后启动语言服务器。您也可以用 `:BasiliskInstall` 显式触发这一过程。

已经安装了 Basilisk——通过 [Homebrew、Scoop 或 cargo](/zh/docs/install-cli/)，或者它已在您的 `PATH` 上？插件会找到并使用那个安装。

用 `:checkhealth basilisk` 验证一切是否正常。

## 更新

插件和二进制文件分别更新：

- **插件**——和其他插件一样：`:Lazy update`、`:PackerSync` 或 `:PlugUpdate`。
- **二进制文件**——当存在更新的发布版本时，启动时会有提示。运行 **`:BasiliskUpdate`**：它会请求确认、下载新版本并就地重启 LSP。如果您的二进制文件由某个包管理器管理，Basilisk 绝不会覆盖它——提示会给出正确的命令（`brew upgrade basilisk`、`scoop update basilisk` 或 `cargo install --git https://github.com/Nimblesite/Basilisk basilisk-cli`）。

## 配置 Basilisk 设置

开箱即可零配置工作。要调整行为，请向 `setup()` 传入选项：

```lua
require("basilisk").setup({
  analysis_mode = "wholeModule", -- "openFilesOnly" | "wholeModule" | "crossModule"
  inlay_hints = {
    parameter_names = true,
    variable_types = true,
  },
  formatter = "ruff", -- 内嵌于 basilisk 二进制文件；或 "none"
})
```

完整的选项列表（调试器、测试浏览器、uv 集成、按键映射、状态栏）在插件的帮助文件中：`:h basilisk-configuration`。

## 调试、测试与性能分析

- **调试**——安装 [nvim-dap](https://github.com/mfussenegger/nvim-dap) 后，`:BasiliskDebugFile`（或您的 DAP 按键映射）会通过 `debugpy` 调试当前文件。参见[调试](/zh/docs/debugging/)。
- **测试浏览器**——`:BasiliskTestToggle` 打开测试面板；`:BasiliskTestRun` 运行测试。
- **性能分析**——`:BasiliskProfile` / `:BasiliskProfileStop` 驱动内置分析器。参见[性能分析](/zh/docs/profiler/)指南。

所有命令都列在 `:h basilisk-commands` 中。

## 高级：覆盖二进制文件

对于开发版构建或特定的系统安装，请让插件指向一个显式路径：

```lua
require("basilisk").setup({
  binary_path = "/absolute/path/to/basilisk",
})
```

……或者设置 `BASILISK_PATH` 环境变量。该设置优先；两者都未设置时，插件使用上文的解析顺序。

## 后续步骤

- [快速开始](/zh/docs/quick-start/)——您的第一次类型检查
- [配置](/zh/docs/configuration/)——`pyproject.toml` 参考
- [重构](/zh/docs/refactoring/)——提取、内联、移动等
