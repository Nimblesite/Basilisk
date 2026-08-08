<!-- GENERATED FILE — DO NOT EDIT.
     Source: docs/readme/README.zh.src.md · Regenerate: python3 scripts/gen_readmes.py
     Spec: docs/specs/DOCS-README-SPEC.md [README] -->
<p align="center">
  <img src="https://raw.githubusercontent.com/Nimblesite/Basilisk/main/images/basilisk-logo.png" alt="Basilisk" width="160">
</p>

<h1 align="center">Basilisk</h1>

<p align="center"><a href="https://github.com/Nimblesite/Basilisk/blob/main/vscode-extension/README.md">English</a> · <strong>简体中文</strong></p>

<p align="center">
  <strong>用 Rust 打造的开源 Python 类型检查器与语言服务器。</strong><br>
  一个扩展覆盖整套工作流 —— 诊断、自动补全、重构、格式化、调试与性能分析 —— 全部由单一捆绑的二进制文件驱动。
</p>

> **你正在阅读 Basilisk 的扩展页面**，适用于 VS Code、Cursor、Windsurf 以及所有 VS Code 分支 —— 同一个扩展同时发布到 [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=Nimblesite.basilisk) 与 [Open VSX](https://open-vsx.org/extension/Nimblesite/basilisk)。

<p align="center">
  <a href="https://www.basilisk-python.dev/zh/">网站</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/installation/">安装</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/quick-start/">快速上手</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/rules/">规则</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/refactoring/">重构</a> &nbsp;&bull;&nbsp;
  <a href="https://github.com/Nimblesite/Basilisk">GitHub</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/Nimblesite/Basilisk/main/images/screenshot.png" alt="Basilisk 实战 —— 编辑器中的类型检查、诊断与重构" width="900">
</p>

> ## ⚠️ 请勿在流水线中使用 Basilisk 的类型检查器
>
> **类型检查器中仍然存在没有做真正类型检查的代码，它目前还不值得信任。** 有些规则
> 依据的是代码的**写法**而不是含义，因此两个方向上都可能出错 —— 既可能对正确的代码
> 报出虚假错误，也可能对真实的缺陷保持沉默。在下文所述的审计完成之前，请不要用
> `basilisk check` 作为 CI 的门禁，不要用它拦截合并，也不要把一次干净的运行结果当作
> 代码库是干净的。
>
> Basilisk 的其余部分 —— 语言服务器、重构、格式化、调试、性能分析 —— 并不依赖这些
> 规则，因此不受影响。

## 重建信任：审计、删除，并倚重真正可靠的检查器

我们撤回了此前的一致性宣称与基准测试数字，并主动请求
[从官方 `python/typing` 结果中移除](https://github.com/python/typing/blob/main/conformance/results/results.html)。
原因是检查器中存在针对一致性测试文件内容而写的逻辑，而不是对类型规范的通用实现：
那些规则匹配的是代码的**写法**，而不是代码的含义。改一个导入别名或重新格式化文件，
结论就会变。这样得出的分数并不能作为证据。

**这是一个错误、一次验证上的失职。** 我们的流程把分数当成了目标，而匹配文本比真正做
分析更快地提高分数；我们在发布之前，始终没有问过这样一个问题 —— 同一个程序换一种
写法时，这条规则是否依然成立。Basilisk 作者已发表
[个人说明与致歉](https://www.christianfindlay.com/blog/basilisk-conformance-apology)。

**因此，我们正在逐条审计规则，并删除那些没有做真正类型检查的规则。** 不是重写，不是
打补丁，也不是标一个 TODO —— 是删除，并留下一个失败的测试，让这个缺口可见而不是被
掩盖。一条规则只有在依据已解析的语法树做判断、并且在代码换一种写法时给出相同结论的
情况下，才会保留。

**如果一条规则无法以直截了当的方式做到可靠，我们会转而依赖另一个成熟的类型检查器，
而不是端出我们自己那份不可靠的实现。** 一个已经赢得信任的引擎给出的答案，对你而言
比一个挂着 Basilisk 名号却没有赢得信任的答案更有价值。在通过套件之外的用例与变异
测试之前，我们不会发布任何替代数字。

这意味着 Basilisk 会**先变小，再变好**。规则会更少，诊断会更少，一致性数字也会更低。
每一次下降我们都会如实报告，而不是设法回避。留下来的，将是对自己所做之事诚实的代码
—— 仅此而已。

### Basilisk 远不只是一个类型检查器

类型检查只是其中一部分。其余部分是装在单个 Rust 二进制文件里的完整 Python 工作流
—— 语言服务器、重构、格式化、集成调试、性能分析，以及各个编辑器扩展 —— 它们都不
建立在正在接受审计的规则之上。这正是我们在审计期间着力打磨的地方：把真正有用的部分
做扎实，并移除任何可能给出误导性结果的东西。变小的意义，是最终得到一个你可以信赖的
工具。

[阅读完整更正 &rarr;](https://www.basilisk-python.dev/zh/docs/conformance/) &nbsp;&bull;&nbsp;
[完整性审计 &rarr;](https://github.com/Nimblesite/Basilisk/blob/main/docs/CONFORMANCE-INTEGRITY-AUDIT.md)

## 你能得到什么

一个扩展即可覆盖整套 Python 工作流。一切由单一捆绑的 Rust 二进制文件驱动 ——
无需 Node.js、无需 npm、无需 `pip install`：

- **随输入实时诊断** —— 由 [Salsa](https://github.com/salsa-rs/salsa) 提供增量分析
- **自动补全、悬停信息、跳转到定义、查找引用、重命名**
- **重构代码操作** —— 提取、内联、移动符号、整理导入
- **集成调试** —— 按 F5 即可通过捆绑的 [debugpy](https://github.com/microsoft/debugpy) 调试；无需额外扩展
- **集成性能分析** —— CPU 热力图、火焰图，以及带泄漏检测的内存面板
- **活动面板** —— 模块树与逐模块的类型健康度覆盖率，并可切换功能开关
- 内置 **Inlay hints** 与 **Ruff** 格式化／导入整理
- **来自 [typeshed](https://github.com/python/typeshed) 的标准库类型** —— 完整的 `stdlib/` 快照已编译进二进制文件，因此悬停与诊断在离线且零配置的情况下依然可用

严格程度按**规则**配置，而不是靠模式切换：未配置的默认值即启用类型规范规则集，
每条规则都可以降级为 `warning`/`info`，让代码库能够渐进地采用类型安全。每条诊断
都附带 `help`、`note` 以及指向每条规则详解页的链接，因此一条红色波浪线总能告诉你
*为什么*。

## 安装

**编辑器扩展** —— 从 [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=Nimblesite.basilisk) 或 [Open VSX](https://open-vsx.org/extension/Nimblesite/basilisk) 安装 *Basilisk*（Cursor、Windsurf 等分支读取 Open VSX）。Basilisk 二进制文件已为 macOS（Apple Silicon）、Linux（x86_64、aarch64）与 Windows（x86_64、aarch64）捆绑 —— 无需再安装其他东西。Zed 与 Neovim 0.10+ 的扩展同样可用。

**命令行工具** —— 在 [PyPI 上名为 `basilisk-python`](https://pypi.org/project/basilisk-python/)；安装后的命令是 `basilisk`：

```sh
uv tool install basilisk-python     # 或：pipx install basilisk-python、pip install basilisk-python
```

也可通过 Homebrew（`brew install Nimblesite/tap/basilisk`）、Scoop（`scoop bucket add nimblesite https://github.com/Nimblesite/scoop-bucket && scoop install basilisk`）与 [GitHub Releases](https://github.com/Nimblesite/Basilisk/releases) 获取。每个渠道都发布同一个 Rust 命令行工具，由本仓库在同一版本构建，且没有运行时依赖。把 `basilisk.executablePath` 指向你自己的构建，扩展就会使用它。完整选项：[安装指南](https://www.basilisk-python.dev/zh/docs/installation/)。

## 试一试

[`examples/`](https://github.com/Nimblesite/Basilisk/blob/main/examples/) 目录中有可直接运行的 Python 文件：

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

这两条命令读取的是按来源划分的同一套规则宇宙（[`CHKARCH-COMMANDS`](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/CHECKER-ARCHITECTURE-SPEC.md)）：`check`
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
[CONTRIBUTING.md](https://github.com/Nimblesite/Basilisk/blob/main/CONTRIBUTING.md) —— **For Humans**（测试、代码质量审查、
一致性/安全审计、IDE 功能对等、打磨 AI 指令）以及
**For AI**（在 [CLAUDE.md](https://github.com/Nimblesite/Basilisk/blob/main/CLAUDE.md) 既定规则下的技术执行）。

## 致谢

Basilisk 建立在开源社区之上 —— 特别感谢：

- **[Astral](https://astral.sh/)** —— [Ruff](https://github.com/astral-sh/ruff)，Basilisk 嵌入了其解析器、AST 与格式化器 crate（MIT）。我们最倚重的基础。
- **[typeshed](https://github.com/python/typeshed)** —— 标准库类型存根（Apache-2.0，部分内容采用 MIT 许可证）。
- **[Salsa](https://github.com/salsa-rs/salsa)** —— 增量查询引擎。
- **[Rayon](https://github.com/rayon-rs/rayon)** —— 数据并行。
- **[tower-lsp](https://github.com/ebkalderon/tower-lsp)** —— LSP 脚手架。
- **[debugpy](https://github.com/microsoft/debugpy)** —— 调试适配器（捆绑于 VS Code 扩展）。
- [`python/typing`](https://github.com/python/typing) 一致性测试套件。

完整的组件、所选许可证与必要声明见 [NOTICES](https://github.com/Nimblesite/Basilisk/blob/main/NOTICES) 和
[RUST-DEPENDENCY-LICENSES](https://github.com/Nimblesite/Basilisk/blob/main/RUST-DEPENDENCY-LICENSES)。每个发布的产物也各自
携带副本：VSIX 在 `RUST-DEPENDENCY-LICENSES` 中提供 Rust 声明，在
`VSCODE-DEPENDENCY-LICENSES` 中提供 npm 声明，并在 `bundled/debugpy` 内保留
debugpy 自身的许可证与 `ThirdPartyNotices.txt`；wheel 则在 `.dist-info/licenses/`
目录中携带完整的锁定声明。

---

## 许可证

Basilisk 源代码采用 MIT 许可证。二进制发行物还包含第三方组件；其许可证
随每个发行物一并提供。

由 [NIMBLESITE PTY LTD](https://www.nimblesite.co) 构建。
