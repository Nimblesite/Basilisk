<p align="center"><a href="README.md">English</a> · <strong>简体中文</strong></p>

> 📝 本文档由机器翻译生成，欢迎母语者校对改进。

# basilisk-zed

Basilisk 的 Zed 编辑器扩展 —— 基于 WASM 的 Python 类型检查与语言服务器集成。

Basilisk 是用 Rust 打造的开源 Python 类型检查器与语言服务器：诊断、自动补全、重构、调试与性能分析，严格程度按规则配置。

<p align="center">
  <img src="https://raw.githubusercontent.com/Nimblesite/Basilisk/main/website/src/assets/images/zed-screenshot.png" alt="Zed 编辑器中的 Basilisk —— 行内 Python 类型检查与诊断" width="900">
</p>

**当前的类型检查器存在不准确之处，请勿将其用于你的开发流水线。我们正在尽快移除任何具有误导性的分析器。详情请见下文**

> **我们撤回了类型一致性结果。** 100% 的宣称与基准测试数字均已撤回，并主动请求[从官方结果中移除](https://github.com/python/typing/blob/main/conformance/results/results.html)。无论是按规范重建检查器，还是让扩展由一个成熟的开源检查器驱动，我们都在把 Basilisk 打造成准确的 Python 开发体验。[阅读更正](https://www.basilisk-python.dev/zh/docs/conformance/)。

## 安装

命令面板（`Cmd+Shift+P` / `Ctrl+Shift+P`）→ **zed: install dev extension** → 选择本目录（如果没有 monorepo，请先克隆 [`Nimblesite/basilisk-zed`](https://github.com/Nimblesite/basilisk-zed)）。Zed 会自行把扩展编译为 WASM —— 你无需预先构建或复制 `.wasm` 文件。

**你无需单独安装 Basilisk 二进制文件。** 首次激活时，扩展会从 [GitHub Release](https://github.com/Nimblesite/Basilisk/releases) 下载与你的平台匹配的二进制文件，缓存在 Zed 的扩展目录中，并一直复用到出现更新的发行版为止。仅在开发或指向系统安装时才需要覆盖它：在 `settings.json` 中设置 `lsp.basilisk.binary.path`，或设置 `BASILISK_PATH` 环境变量。

> 该扩展尚未收录进 [Zed 扩展注册表](https://github.com/zed-industries/extensions)；在收录完成之前，上述开发扩展方式就是安装路径。

完整的安装说明、设置项、调试与斜杠命令参考：[basilisk-python.dev/docs/install-zed](https://www.basilisk-python.dev/docs/install-zed/)。

## 在 Basilisk 中的角色

这是 **Zed 编辑器集成**。它是一个编译为 WASM 的原生 Zed 扩展，将 Basilisk 语言服务器连接到 Zed，提供实时诊断、悬停提示、跳转到定义、代码操作，以及通过 DAP 实现的调试。

## 核心概念

- **WASM 扩展** —— 编译为面向 `wasm32-wasip2` 的 `cdylib` crate，由 Zed 原生加载。
- **`zed_extension_api`** —— 使用 Zed 官方扩展 API 管理语言服务器生命周期。
- **`basilisk-common`** —— 与 Basilisk 工作区的其余部分共享诊断代码和常量（同样兼容 WASM）。
- **不改动内置 Python** —— 按名称绑定到 Zed 自带的 Python 语言。扩展不附带 `languages/` 目录，也不附带语法，因此 Zed 不会从源码编译任何东西，你的语法高亮、括号匹配、缩进规则和可运行项都保持 Zed 出厂时的样子。
- **DAP 调试** —— 支持 Debug Adapter Protocol，实现集成的 Python 调试。

## 构建

在 monorepo 检出中，构建扩展并配置本地开发循环：

```sh
make package-zed
```

独立仓库（仅本仓库）中，构建命令与发布流水线用于放行发布的那一条完全相同：

```sh
cargo build --release --target wasm32-wasip2
```

## 依赖

| Crate | 用途 |
|-------|---------|
| `zed_extension_api` | Zed 扩展 API |
| `basilisk-common` | 共享的常量和类型 |

## 许可证

MIT。
