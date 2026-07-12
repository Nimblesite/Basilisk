<p align="center"><a href="README.md">English</a> · <strong>简体中文</strong></p>

> 📝 本文档由机器翻译生成，欢迎母语者校对改进。

# basilisk.nvim

为 Basilisk 打造的一流 Neovim 插件 —— 零配置的 Python 类型检查、调试、性能分析与测试探索。

唯一在官方 [`python/typing` 符合性套件](https://github.com/python/typing/blob/main/conformance/results/results.html)中取得 100% 满分的 Python 类型检查器 —— 也是我们测过的最快的。使用 Rust 构建的完整开源 Python 开发环境：类型检查器、语言服务器、调试器与性能分析器，并提供 VS Code、Cursor、Zed 与 Neovim 扩展。默认严格。

<p align="center">
  <img src="https://raw.githubusercontent.com/Nimblesite/Basilisk/main/website/src/assets/images/screenshot.png" alt="Basilisk in action — type checking, diagnostics, and refactoring in the editor" width="900">
</p>

## 在 Basilisk 中的角色

这是 **Neovim 编辑器集成**。它将 Neovim 内置的 LSP 客户端连接到 Basilisk 语言服务器，提供与 VS Code 扩展相同的功能集：实时诊断、悬停信息、跳转到定义、代码操作、内嵌提示（inlay hints）、集成调试以及性能分析。

## 功能特性

- **零配置安装** —— 自动检测 `basilisk` 二进制文件并建立连接
- **实时诊断** —— 错误在你输入时即时内联显示
- **跳转到定义、悬停信息、查找引用** —— 完整的 LSP 导航
- **代码操作与重构** —— 提取、重命名、移动、内联
- **内嵌提示（inlay hints）** —— 参数名称与推断类型
- **集成调试** —— 兼容 nvim-dap，按 F5 即可调试
- **测试浏览器** —— 在编辑器中发现并运行 pytest 测试
- **Python 性能分析** —— 直接在编辑器中查看 py-spy 热力图
- **内存泄漏追踪** —— 在开发过程中检测泄漏
- **uv 集成** —— `uv sync` 与 `uv add` 命令
- **状态栏** —— 在状态栏中显示 LSP 状态
- **健康检查** —— `:checkhealth basilisk` 进行诊断

## 要求

- Neovim 0.11+（插件使用内置的 `vim.lsp.config` / `vim.lsp.enable` API）
- `curl`（仅用于一次性下载 `basilisk` 二进制文件，见下文）

## 安装

需要安装两部分：**插件**（本仓库，通过你的插件管理器安装）和 **`basilisk` 二进制文件**（自动下载 —— 通常无需手动安装）。

### 1. 安装插件

<details open>
<summary><strong>lazy.nvim</strong></summary>

```lua
{
  "Nimblesite/basilisk.nvim",
  ft = "python",
  dependencies = { "mfussenegger/nvim-dap" }, -- 可选，用于调试
  opts = {},
}
```
</details>

<details>
<summary><strong>packer.nvim</strong></summary>

```lua
use {
  "Nimblesite/basilisk.nvim",
  ft = "python",
  config = function()
    require("basilisk").setup({})
  end,
}
```
</details>

<details>
<summary><strong>vim-plug</strong></summary>

```vim
Plug 'Nimblesite/basilisk.nvim'
```

然后在 `plug#end()` 之后：

```lua
lua require("basilisk").setup({})
```
</details>

<details>
<summary><strong>vim.pack（内置，Neovim 0.12+）</strong></summary>

```lua
vim.pack.add({
  { src = "https://github.com/Nimblesite/basilisk.nvim",
    version = vim.version.range("*") }, -- 最新稳定标签；或固定 "v0.33.0"
})
require("basilisk").setup({})
```
</details>

### 2. 二进制文件自动安装

打开任意 Python 文件。若未找到 `basilisk` 二进制文件，插件会自动从 [GitHub Release](https://github.com/Nimblesite/Basilisk/releases) 下载适合你平台的最新版本到 Neovim 数据目录并启动 LSP —— 无需配置 PATH，无需手动操作。也可以用 `:BasiliskInstall` 显式触发。

偏好包管理器？插件会自动识别已有安装：

```sh
# macOS（Apple Silicon）/ Linux
brew tap Nimblesite/tap && brew install basilisk

# Windows
scoop bucket add nimblesite https://github.com/Nimblesite/scoop-bucket
scoop install basilisk

# 任何有 Rust 工具链的环境
cargo install basilisk-cli
```

就这样 —— 诊断、悬停、补全、格式化、调试、测试与性能分析全部通过这一个插件运行。用 `:checkhealth basilisk` 验证。

## 更新

- **插件**：像其他插件一样更新 —— `:Lazy update`（lazy.nvim）、`:PackerSync`（packer）、`:PlugUpdate`（vim-plug）。
- **二进制文件**：有新版本时插件会在启动时通知你。运行 **`:BasiliskUpdate`** —— 确认后下载新版本并就地重启 LSP。由包管理器管理的安装不会被覆盖；通知会提示你改用 `brew upgrade basilisk` / `scoop update basilisk` / `cargo install basilisk-cli`。

## 配置

零配置即可开箱即用：

```lua
require("basilisk").setup()
```

所有选项（分析模式、内嵌提示、格式化器、调试器、测试浏览器、uv、快捷键等）见 [doc/basilisk.txt](doc/basilisk.txt) —— `:h basilisk-configuration`。

## 许可证

MIT。
