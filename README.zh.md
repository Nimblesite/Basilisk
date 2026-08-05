<!-- GENERATED FILE — DO NOT EDIT.
     Source: docs/readme/README.zh.src.md · Regenerate: python3 scripts/gen_readmes.py
     Spec: docs/specs/DOCS-README-SPEC.md [README] -->
<p align="center">
  <img src="images/basilisk-logo.png" alt="Basilisk" width="160">
</p>

<h1 align="center">Basilisk</h1>

<p align="center"><a href="README.md">English</a> · <strong>简体中文</strong></p>

<p align="center">
  <strong>使用 Rust 构建的开源 Python 类型检查与开发工具。</strong><br>
  用 <strong>Rust</strong> 打造的完整开源 Python 开发环境：类型检查器、语言服务器、调试器、性能分析器，以及 VS Code、Cursor、Zed 与 Neovim 扩展。默认严格。
</p>

> **你正在阅读 Basilisk 的源码仓库** —— 检查器、语言服务器、编辑器扩展与网站都在这里。

<p align="center">
  <a href="https://www.basilisk-python.dev/zh/">网站</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/installation/">安装</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/quick-start/">快速上手</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/rules/">规则</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/refactoring/">重构</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/comparison/">对比</a> &nbsp;&bull;&nbsp;
  <a href="https://github.com/Nimblesite/Basilisk">GitHub</a>
</p>

<p align="center">
  <strong>当前一致性水平：暂时未知。</strong> 此前的一致性结果及所有已公布的基准测试数据均已撤回，待从头重做相关实现并完成审计。
</p>

## 已撤回一致性与基准测试结果

> **诚信说明：** 我们已撤回 Basilisk 此前“100% 一致性”的说法。该结果并不可信：检查器的部分规则针对上游测试文件的具体文本进行了拟合；面对不改变程序语义的变异（例如一致地重命名类型变量），分数无法保持稳定。应我们的要求，Basilisk 已从官方 [`python/typing` 结果表](https://github.com/python/typing/blob/main/conformance/results/results.html)中移除。目前真实的一致性水平暂时未知。
>
> 在完成测量流程审计之前，我们也撤回所有已公布的基准测试数据与性能排名。我们正在删除这些针对测试拟合的代码，并依据 Python 类型规范从头重写受影响的逻辑。在公布新分数或申请重新收录之前，包括语义保持重命名在内的变异测试必须证明结果足够稳健。一旦得出可信结论，我们就会公布新的一致性与基准测试结果，即使结果低于或慢于此次撤回的数据也会如实发布。[查看一致性审计与修复计划 &rarr;](https://www.basilisk-python.dev/zh/docs/conformance/)

<p align="center">
  <img src="images/screenshot.png" alt="Basilisk 实战 —— 编辑器中的类型检查、诊断与重构" width="900">
</p>

## 一个扩展，覆盖全部

一个扩展即可取代 Pylance 并提供完整工作流 —— 无需 Node.js、无需 Python 运行时、无需 pip、无需 npm。一切由单一捆绑的 Rust 二进制文件驱动：

- **默认严格的诊断** —— 随输入实时呈现，由 Salsa（rust-analyzer 的引擎）提供增量分析
- **自动补全、悬停信息、跳转到定义、查找引用、重命名**
- **重构代码操作** —— 提取、内联、移动符号、整理导入
- **集成调试** —— 按 F5 即可通过捆绑的 debugpy 调试；无需额外扩展
- **集成性能分析** —— CPU 热力图、火焰图，以及带泄漏检测的内存面板
- **活动面板** —— 模块树与逐模块的类型健康度覆盖率，并可切换功能开关
- 内置 **Inlay hints** 与 **Ruff** 格式化／导入整理
- **来自 [typeshed](https://github.com/python/typeshed) 的标准库类型** —— 完整的 `stdlib/` 快照已编译进二进制文件，因此悬停与诊断在离线且零配置的情况下依然可用

每条诊断都有教育意义：rustc 风格的输出，附带 `help`、`note` 以及指向每条规则详解页的链接，因此一条红色波浪线总能告诉你*为什么*。Basilisk **一开始就严格**并始终严格 —— 未配置的默认值会启用当前已注册的所有 PEP 标签规则，严格程度按规则微调，而不是靠模式切换。这只是配置行为，并不证明这些规则的实现完整或正确。

## 安装

**编辑器扩展** —— 从 [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=Nimblesite.basilisk) 或 [Open VSX](https://open-vsx.org/extension/Nimblesite/basilisk) 安装 *Basilisk*（Cursor、Windsurf 等分支读取 Open VSX）。Basilisk 二进制文件已为 macOS（Apple Silicon）、Linux（x86_64、aarch64）与 Windows（x86_64、aarch64）捆绑 —— 无需再安装其他东西。Zed 与 Neovim 0.10+ 的扩展同样可用。

**命令行工具** —— 在 [PyPI 上名为 `basilisk-python`](https://pypi.org/project/basilisk-python/)；安装后的命令是 `basilisk`：

```sh
uv tool install basilisk-python     # 或：pipx install basilisk-python、pip install basilisk-python
```

也可通过 Homebrew（`brew install Nimblesite/tap/basilisk`）、Scoop（`scoop bucket add nimblesite https://github.com/Nimblesite/scoop-bucket && scoop install basilisk`）与 [GitHub Releases](https://github.com/Nimblesite/Basilisk/releases) 获取。每个渠道都发布同一个 Rust 命令行工具，由本仓库在同一版本构建，且没有运行时依赖。把 `basilisk.executablePath` 指向你自己的构建，扩展就会使用它。完整选项：[安装指南](https://www.basilisk-python.dev/zh/docs/installation/)。

## 试一试

[`examples/`](examples/) 目录中有可直接运行的 Python 文件：

```sh
basilisk check   examples/bad.py    # 8 处类型规范错误 —— 始终启用，无需配置
basilisk analyze examples/bad.py    # 同一文件上可选的严格性警告
basilisk analyze examples/good.py   # 即使在完全严格下也是干净的
basilisk check   examples/mixed.py  # 一处真实的类型错误
basilisk check   examples/          # 一次检查整个目录
```

供 CI 与工具使用的机器可读输出：

```sh
basilisk check path/to/your_code.py --output json --color never
```

这两条命令读取的是按来源划分的同一套规则宇宙（[`CHKARCH-COMMANDS`](docs/specs/CHECKER-ARCHITECTURE-SPEC.md)）：`check`
只报告带 `pep` 标签的类型规范规则 —— 该集合始终启用，配置表虽可将其中某条
降级为 `warning`/`info`，但都不能将其关闭。`analyze` 报告非 `pep` 的自有规则，
它们在被配置表选用之前始终保持沉默。只有 `analyze` 会输出 `BSK-` 诊断。

## 标准库类型：始终离线

Basilisk 从 [typeshed](https://github.com/python/typeshed) 解析标准库类型，
而且检查**从不下载任何东西**。开箱即用时它使用编译进二进制文件的完整 typeshed
`stdlib/` 快照，并将来源报告为未固定（unpinned）—— 因此在飞机上、防火墙后或
隔离网络的 CI 中，标准库类型都无需配置即可使用。

在 `[tool.basilisk]` 中使用 `typeshed-commit = "<40 位 sha>"` 固定到某个确切提交。
固定只做一件事：离线校验本地存储库中的 typeshed 树是否哈希为该提交。若该提交
不在本机上，运行会以 `NO SOURCE` 硬失败，而不会替换为其他来源 —— 请先用
`basilisk typeshed download` 取回（不带 `--commit` 时会下载最新提交并替你写入
固定项），或使用编辑器中的 **Download latest** 按钮。或者，把 `typeshed-path`
指向你自己的 typeshed 目录树。完整选项参见[配置指南](https://www.basilisk-python.dev/zh/docs/configuration/)。

## 开发

```sh
cargo build          # build all crates
cargo test           # run all tests
cargo clippy         # lint (zero warnings policy)
cargo fmt            # format
```

需要 Rust 1.87+。

## 贡献

Basilisk 由人类与 AI 的协作打造，并有意地划分了各自的工作。请参阅
[CONTRIBUTING.md](CONTRIBUTING.md) —— **For Humans**（测试、代码质量审查、
一致性/安全审计、IDE 功能对等、打磨 AI 指令）以及
**For AI**（在 [CLAUDE.md](CLAUDE.md) 既定规则下的技术执行）。

## 致谢

Basilisk 建立在开源社区之上 —— 特别感谢：

- **[Astral](https://astral.sh/)** —— [Ruff](https://github.com/astral-sh/ruff)，Basilisk 嵌入了其解析器、AST 与格式化器 crate（MIT）。我们最倚重的基础。
- **[typeshed](https://github.com/python/typeshed)** —— 标准库类型存根（Apache-2.0，部分内容采用 MIT 许可证）。
- **[Salsa](https://github.com/salsa-rs/salsa)** —— 增量查询引擎。
- **[Rayon](https://github.com/rayon-rs/rayon)** —— 数据并行。
- **[tower-lsp](https://github.com/ebkalderon/tower-lsp)** —— LSP 脚手架。
- **[debugpy](https://github.com/microsoft/debugpy)** —— 调试适配器（捆绑于 VS Code 扩展）。
- [`python/typing`](https://github.com/python/typing) 一致性测试套件。

完整的组件、所选许可证与必要声明见 [NOTICES](NOTICES) 和
[RUST-DEPENDENCY-LICENSES](RUST-DEPENDENCY-LICENSES)。每个发布的产物也各自
携带副本：VSIX 在 `RUST-DEPENDENCY-LICENSES` 中提供 Rust 声明，在
`VSCODE-DEPENDENCY-LICENSES` 中提供 npm 声明，并在 `bundled/debugpy` 内保留
debugpy 自身的许可证与 `ThirdPartyNotices.txt`；wheel 则在 `.dist-info/licenses/`
目录中携带完整的锁定声明。

---

## 许可证

Basilisk 源代码采用 MIT 许可证。二进制发行物还包含第三方组件；其许可证
随每个发行物一并提供。

由 [NIMBLESITE PTY LTD](https://www.nimblesite.co) 构建。
