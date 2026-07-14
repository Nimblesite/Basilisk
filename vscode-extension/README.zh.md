<p align="center">
  <img src="https://basilisk-python.dev/assets/images/favicon.png" alt="Basilisk" width="140">
</p>

<h1 align="center">Basilisk for VS Code</h1>

<p align="center"><a href="https://github.com/Nimblesite/Basilisk/blob/main/vscode-extension/README.md">English</a> · <strong>简体中文</strong></p>

> 📝 本文档由机器翻译生成，欢迎母语者校对改进。

<p align="center">
  <strong>唯一在官方 <a href="https://github.com/python/typing/blob/main/conformance/results/results.html"><code>python/typing</code> 符合性套件</a>中取得 100% 满分的 Python 类型检查器 —— 也是我们测过的最快的。</strong><br>
  使用 <strong>Rust</strong> 构建的完整开源 Python 开发环境：类型检查器、语言服务器、调试器与性能分析器，并提供 VS Code、Cursor、Zed 与 Neovim 扩展。默认严格。
</p>

<p align="center">
  <a href="https://www.basilisk-python.dev">官网</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/quick-start/">快速开始</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/rules/">规则</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/conformance/">一致性</a> &nbsp;&bull;&nbsp;
  <a href="https://github.com/Nimblesite/Basilisk">GitHub</a>
</p>

<p align="center">
  <a href="https://github.com/python/typing/blob/main/conformance/results/results.html"><strong>PEP 符合性 <!--g:score-->100.0%<!--/g:score--></strong></a> —— 官方
  <a href="https://github.com/python/typing/tree/main/conformance"><code>python/typing</code></a>
  一致性测试套件 <!--g:total-->141<!--/g:total--> 个测试中通过 <!--g:pass-->141<!--/g:pass--> 个，由真实的上游评分工具在默认配置下对通过 wheel 安装的 CLI 评分。该榜单上唯一取得满分的检查器。
</p>

## 唯一满分 —— 且最快

Basilisk 是**唯一**在官方
[`python/typing` 符合性套件](https://github.com/python/typing/blob/main/conformance/results/results.html)
上取得满分的 Python 类型检查器：**<!--g:score-->100.0%<!--/g:score-->**（<!--g:pass-->141<!--/g:pass-->/<!--g:total-->141<!--/g:total--> 个文件，捕获 <!--g:caught-->970<!--/g:caught--> 个必需错误，<!--g:fp-->0<!--/g:fp--> 个误报），由真实的上游评分工具在默认配置下对通过 wheel 安装的 CLI 评分。

<p align="center">
  <img src="https://raw.githubusercontent.com/Nimblesite/Basilisk/main/website/src/assets/images/screenshot.png" alt="Basilisk 实战 —— 在 VS Code 中进行类型检查、诊断与重构" width="900">
</p>

它也是**我们测试过最快的检查器** —— 从零冷启动的全文件检查中位数：

| 类型检查器 | 冷启动检查中位数 |
| --- | --- |
| ⚡ **Basilisk** | **<!--g:benchBasilisk-->11<!--/g:benchBasilisk--> ms** |
| zuban | <!--g:benchZuban-->27<!--/g:benchZuban--> ms |
| ty | <!--g:benchTy-->39<!--/g:benchTy--> ms |
| Pyrefly | <!--g:benchPyrefly-->148<!--/g:benchPyrefly--> ms |
| Pyright | <!--g:benchPyright-->582<!--/g:benchPyright--> ms |
| mypy | <!--g:benchMypy-->610<!--/g:benchMypy--> ms |

在 <!--g:benchMachine-->Apple M4 Max<!--/g:benchMachine--> 上，跨 <!--g:benchCount-->26<!--/g:benchCount--> 个单一类型构造压力测试样本的冷启动全文件检查中位数 —— 数值越低越好；在编辑器内热重检查更快。每个数字均由 [`hyperfine`](https://github.com/sharkdp/hyperfine) 生成并按机器提交，没有一个是手写的。**克隆仓库，在你自己的硬件上运行 `make bench`，并把 CSV 发给我们 —— 欢迎社区独立复核。** [完整基准测试与方法论（英文）&rarr;](https://www.basilisk-python.dev/docs/benchmarks/)

## 一个扩展，全部搞定

一个扩展替代 Pylance，覆盖完整工作流 —— 无需 Node.js、Python 运行时、pip 或 npm。由单一内置 Rust 二进制文件驱动一切：

- **默认严格的诊断** —— 边输入边内联显示，基于 Salsa 的增量分析（与 rust-analyzer 同源）
- **自动补全、悬停信息、跳转到定义、查找引用、重命名**
- **重构代码操作** —— 提取、内联、移动符号、整理导入
- **集成调试** —— 按 F5 通过内置 debugpy 调试；无需额外扩展
- **集成性能分析** —— CPU 热力图、火焰图，以及带泄漏检测的内存仪表盘
- **活动面板** —— 带每模块类型健康覆盖率的模块树，以及功能开关
- **内嵌提示**与内置的 **Ruff** 格式化 / 导入整理

每条诊断都有教育意义：rustc 风格的输出，附带 `help`、`note` 以及指向每条规则详解页的链接，因此一条红色波浪线总能告诉你*原因*。其他检查器默认宽松；Basilisk **默认严格**并始终保持严格。

## 零安装

Basilisk 二进制文件已随本扩展捆绑，覆盖 macOS（Apple Silicon）、Linux（x86_64、aarch64）与 Windows（x86_64、aarch64）。安装扩展即可使用。

也想把 `basilisk` CLI 放到 PATH 上（用于 CI 或终端）？`brew install Nimblesite/tap/basilisk`、`scoop install basilisk`（先执行 `scoop bucket add nimblesite https://github.com/Nimblesite/scoop-bucket`），或从 [GitHub Releases](https://github.com/Nimblesite/Basilisk/releases) 下载二进制文件。将 `basilisk.executablePath` 指向它即可使用你自己的构建。

## 致谢

基于 [Astral](https://astral.sh/) 的 [Ruff](https://github.com/astral-sh/ruff)（MIT）与 [typeshed](https://github.com/python/typeshed)（Apache-2.0）构建；捆绑了 [debugpy](https://github.com/microsoft/debugpy)（Microsoft，MIT）。完整声明见 [NOTICES](https://github.com/Nimblesite/Basilisk/blob/main/NOTICES)。

## 许可证

MIT。

由 [NIMBLESITE PTY LTD](https://www.nimblesite.co) 构建。
