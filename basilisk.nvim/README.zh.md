<p align="center"><a href="README.md">English</a> · <strong>简体中文</strong></p>

> 📝 本文档由机器翻译生成，欢迎母语者校对改进。

# basilisk.nvim

为 Basilisk 打造的一流 Neovim 插件 —— 零配置的 Python 类型检查、调试、性能分析与测试探索。

Basilisk 是用 Rust 打造的开源 Python 类型检查器与语言服务器：诊断、自动补全、重构、调试与性能分析，严格程度按规则配置。

<p align="center">
  <img src="https://raw.githubusercontent.com/Nimblesite/Basilisk/main/website/src/assets/images/screenshot.png" alt="Basilisk in action — type checking, diagnostics, and refactoring in the editor" width="900">
</p>

> ## ⚠️ 请勿在流水线中使用 Basilisk 的类型检查器
>
> **类型检查器中仍然存在没有做真正类型检查的代码，它目前还不值得信任。** 有些规则依据的是代码的**写法**而不是含义，因此两个方向上都可能出错 —— 既可能对正确的代码报出虚假错误，也可能对真实的缺陷保持沉默。请不要用它作为 CI 的门禁，也不要把一次干净的运行结果当作代码库是干净的。此前的一致性宣称与基准测试数字均已撤回，并主动请求[从官方结果中移除](https://github.com/python/typing/blob/main/conformance/results/results.html)。
>
> **这是一个错误、一次验证上的失职，而不是有意操纵测试套件。** 我们没有向 `python/typing` 隐瞒任何东西：提交时运行的是套件自己未经修改的评分工具；我们仅凭一次全绿的运行就发布了结果，却从未检查过我们的规则能否经受住保持语义的改写。Basilisk 作者已发表[个人说明与致歉](https://www.christianfindlay.com/blog/basilisk-conformance-apology)。
>
> **我们正在逐条审计规则，并删除那些站不住脚的规则** —— 不是重写，也不是打补丁，而是删除，并留下一个失败的测试，让缺口保持可见。如果一条规则无法以直截了当的方式做到可靠，我们会转而依赖另一个成熟的类型检查器，而不是端出我们自己那份不可靠的实现。
>
> **Basilisk 远不只是一个类型检查器。** 语言服务器、重构、格式化、调试与性能分析都不建立在正在接受审计的规则之上 —— 审计期间，这些正是我们着力打磨的部分，并移除任何可能给出误导性结果的东西。我们这样做，是为了重建信任，把 Basilisk 变回一个你可以信赖的工具。[阅读更正](https://www.basilisk-python.dev/zh/docs/conformance/)。

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

# 任何有 Python 工具链的环境
uv tool install basilisk-python

# 任何有 Rust 工具链的环境（从源码构建）
cargo install --git https://github.com/Nimblesite/Basilisk basilisk-cli
```

就这样 —— 诊断、悬停、补全、格式化、调试、测试与性能分析全部通过这一个插件运行。用 `:checkhealth basilisk` 验证。

## 更新

- **插件**：像其他插件一样更新 —— `:Lazy update`（lazy.nvim）、`:PackerSync`（packer）、`:PlugUpdate`（vim-plug）。
- **二进制文件**：有新版本时插件会在启动时通知你。运行 **`:BasiliskUpdate`** —— 确认后下载新版本并就地重启 LSP。由包管理器管理的安装不会被覆盖；通知会提示你改用 `brew upgrade basilisk` / `scoop update basilisk` / `cargo install --git https://github.com/Nimblesite/Basilisk basilisk-cli`。

## 配置

零配置即可开箱即用：

```lua
require("basilisk").setup()
```

所有选项（分析模式、内嵌提示、格式化器、调试器、测试浏览器、uv、快捷键等）见 [doc/basilisk.txt](doc/basilisk.txt) —— `:h basilisk-configuration`。

## 许可证

MIT。
