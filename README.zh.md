<p align="center">
  <img src="images/basilisk-logo.png" alt="Basilisk" width="160">
</p>

<h1 align="center">Basilisk</h1>

<p align="center"><a href="README.md">English</a> · <strong>简体中文</strong></p>

> 📝 本文档由机器翻译生成，欢迎母语者校对改进。

<p align="center">
  <strong>开源的 Python 语言服务器。</strong><br>
  唯一在官方 <a href="https://github.com/python/typing/blob/main/conformance/results/results.html"><code>python/typing</code> 符合性结果</a>中取得满分 100% 的 Python 类型检查器。<br>
  完整的语言服务器、类型检查器、调试器与性能分析器 —— 默认严格。<br>
  VS Code、Cursor 与 Windsurf（Open VSX）&bull; Zed &bull; Neovim。使用 <strong>Rust</strong> 构建 —— 单一二进制文件，无需运行时。
</p>

<p align="center">
  <a href="https://www.basilisk-python.dev">官网</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/installation/">安装</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/quick-start/">快速开始</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/rules/">规则</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/refactoring/">重构</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/comparison/">对比</a>
</p>

<p align="center">
  <a href="https://www.basilisk-python.dev/zh/docs/conformance/"><strong>PEP 符合性 <!--g:score-->100.0%<!--/g:score--></strong></a> &mdash; 官方
  <a href="https://github.com/python/typing/tree/f4f2952f3ac94d7af819c5c71b60a50a100370e0/conformance"><code>python/typing</code></a>
  符合性套件 <!--g:total-->141<!--/g:total--> 个测试中通过 <!--g:pass-->141<!--/g:pass--> 个（提交 <code><!--g:short-->f4f2952<!--/g:short--></code>），由真实的上游评分工具在默认配置下对通过 wheel 安装的 CLI 评分。
  我们对标 <code>python/typing@main</code>，得分只升不降。
</p>

## 唯一满分 —— 且最快

Basilisk 是**唯一**在官方
[`python/typing` 符合性套件](https://github.com/python/typing/blob/main/conformance/results/results.html)
上取得满分的 Python 类型检查器：**<!--g:score-->100.0%<!--/g:score-->**（<!--g:pass-->141<!--/g:pass-->/<!--g:total-->141<!--/g:total--> 个文件，捕获 <!--g:caught-->970<!--/g:caught--> 个必需错误，<!--g:fp-->0<!--/g:fp--> 个误报），由真实的上游评分工具在默认配置下对通过 wheel 安装的 CLI 评分。

<p align="center">
  <img src="images/screenshot.png" alt="Basilisk 实战 —— 在编辑器中进行类型检查、诊断与重构" width="900">
</p>

它也是**我们测试过最快的检查器** —— 从零冷启动的全文件检查中位数：

| 类型检查器 | 冷启动检查中位数 |
| --- | --- |
| ⚡ **Basilisk** | **<!--g:benchBasilisk-->12<!--/g:benchBasilisk--> ms** |
| zuban | <!--g:benchZuban-->26<!--/g:benchZuban--> ms |
| ty | <!--g:benchTy-->37<!--/g:benchTy--> ms |
| Pyrefly | <!--g:benchPyrefly-->142<!--/g:benchPyrefly--> ms |
| Pyright | <!--g:benchPyright-->570<!--/g:benchPyright--> ms |
| mypy | <!--g:benchMypy-->573<!--/g:benchMypy--> ms |

在 <!--g:benchMachine-->Apple M4 Max<!--/g:benchMachine--> 上，跨 <!--g:benchCount-->26<!--/g:benchCount--> 个单一类型构造压力测试样本的冷启动全文件检查中位数 —— 数值越低越好。Basilisk 的热重检查可降至约 <!--g:benchWarm-->5<!--/g:benchWarm--> ms。每个数字均由 [`hyperfine`](https://github.com/sharkdp/hyperfine) 生成并按机器提交，没有一个是手写的。**克隆仓库，在你自己的硬件上运行 `make bench`，并把 CSV 发给我们 —— 欢迎社区独立复核。** [完整基准测试与方法论（英文）&rarr;](https://www.basilisk-python.dev/docs/benchmarks/)

## 试用

`examples/` 文件夹中提供了可直接运行的 Python 文件：

```sh
basilisk check examples/bad.py    # everything flagged
basilisk check examples/good.py   # clean
basilisk check examples/mixed.py  # some errors, some clean
basilisk check examples/          # all three at once
```

## 编辑器

一个扩展，覆盖完整工作流：默认严格的诊断、自动补全、悬停信息、跳转到定义、重构代码操作、调试与性能分析。无需 Node.js 或 Python 运行时 —— 由单一 Rust 二进制文件驱动一切。

- **VS Code、Cursor 与 Windsurf** —— 从 [Open VSX](https://open-vsx.org/) 安装
- **Zed** &bull; **Neovim 0.10+**

每条诊断都有教育意义：rustc 风格的输出，附带 `help`、`note` 以及指向每条规则详解页的链接。参见[完整诊断参考](https://www.basilisk-python.dev/zh/docs/rules/)与[安装指南](https://www.basilisk-python.dev/zh/docs/installation/)。

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
- **[typeshed](https://github.com/python/typeshed)** —— 标准库类型存根（Apache-2.0）。
- **[Salsa](https://github.com/salsa-rs/salsa)** —— 增量查询引擎。
- **[Rayon](https://github.com/rayon-rs/rayon)** —— 数据并行。
- **[tower-lsp](https://github.com/ebkalderon/tower-lsp)** —— LSP 脚手架。
- **[debugpy](https://github.com/microsoft/debugpy)** —— 调试适配器（捆绑于 VS Code 扩展）。
- [`python/typing`](https://github.com/python/typing) 一致性测试套件。

完整的组件与许可证列表见 [NOTICES](NOTICES)。所有依赖均采用宽松许可证。

---

## 许可证

MIT。

由 [NIMBLESITE PTY LTD](https://www.nimblesite.co) 构建。
