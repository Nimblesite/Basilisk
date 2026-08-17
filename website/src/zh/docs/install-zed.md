---
layout: layouts/docs.njk
title: Zed 版 Basilisk——安装与使用该 Python 扩展
description: 在 Zed 编辑器中安装并使用 Basilisk Python 语言服务器。匹配的二进制文件会随扩展自动下载——零配置，无需单独安装。诊断、自动补全、调试和性能分析。
keywords: basilisk, zed, zed编辑器, python, 语言服务器, lsp, 安装, 扩展, 调试, 性能分析, 斜杠命令
lang: zh
date: 2026-02-28
dateModified: 2026-08-04
---

# Zed 版 Basilisk

Basilisk 提供了一个原生 [Zed](https://zed.dev) 扩展，为 Python 注册 Basilisk 语言服务器。安装后，Basilisk 会为每个 `.py` 文件自动激活——诊断、自动补全、悬停、跳转到定义、重命名、代码操作、格式化、内联提示、调试和性能分析。

## 安装扩展

Basilisk **尚未收录进 Zed 的扩展注册表**——向 [`zed-industries/extensions`](https://github.com/zed-industries/extensions) 的提交仍在进行中，因此在扩展视图中搜索“Basilisk”是找不到的。请改为直接安装；只需一次克隆和一个命令面板操作。

1. 克隆扩展仓库：

   ```sh
   git clone https://github.com/Nimblesite/basilisk-zed.git
   ```

2. 打开命令面板（`Cmd+Shift+P` / `Ctrl+Shift+P`）→ **zed: install dev extension**
3. 选择克隆下来的 `basilisk-zed` 目录

Zed 会自行把扩展编译为 WASM——您永远不需要预构建或复制 `.wasm` 文件。打开一个 Python 文件，Basilisk 就是您的语言服务器。

> **在 monorepo 中工作？** 直接选择 [Basilisk](https://github.com/Nimblesite/Basilisk) 检出目录下的 `basilisk-zed/`，无需另行克隆。`make _package_zed` 会一步构建扩展和本地 `basilisk` 二进制文件。

要更新，请在克隆目录中执行 `git pull`，然后重新运行 **zed: install dev extension**。等注册表收录完成后，扩展视图就会为您处理安装和更新。

## 二进制文件随扩展一同提供

**您不需要单独安装 Basilisk 二进制文件。** 首次激活时，扩展会直接从 [GitHub 发布页](https://github.com/Nimblesite/Basilisk/releases)下载适合您平台的匹配二进制文件，将其缓存在 Zed 的扩展目录中，并一直复用到有更新的发布版本出现为止。无需 `cargo install`，无需 Homebrew，无需设置 PATH——安装扩展就是全部流程。

当有更新的发布版本可用时，扩展会记录一条更新提示；重启 Zed 即可采用。

## 配置 Basilisk 设置

Basilisk 零配置即可工作。要调整其行为，请在您的 Zed `settings.json` 中的 `lsp.basilisk.settings` 下添加设置：

```json
{
  "lsp": {
    "basilisk": {
      "settings": {
        "analysisMode": "wholeModule"
      }
    }
  },
  "languages": {
    "Python": {
      "language_servers": ["basilisk", "..."]
    }
  }
}
```

> 语言服务器目前仅识别 `analysisMode`（`wholeModule` 或 `openFilesOnly`）和
> `testExplorer` 设置。其他键会被接受，但服务器尚未读取——请参阅
> [配置参考](/zh/docs/configuration/)了解当前实际生效的设置。

## 调试

在 Python 文件上按 **F5** 即可调试。Basilisk 通过调试适配器协议代理一个 `debugpy` 会话——断点、单步执行、变量、调用栈和监视表达式在 Zed 中都能原生工作。会话的代理方式请参见[调试](/zh/docs/debugging/)。

## 斜杠命令

Basilisk 在 Zed 的 AI 助手面板中注册了用于性能分析、内存分析、测试和工作区洞察的斜杠命令。性能分析和内存命令是**指南**：每条命令都会说明对应的 `basilisk.profiler.*` / `basilisk.memory.*` 语言服务器命令以及如何驱动它——性能分析本身通过 LSP 运行：

| 命令 | 作用 |
|---------|--------------|
| `/profile` | 如何启动 CPU 性能分析（可选 PID） |
| `/profstop` | 如何停止分析以及结果保存在哪里 |
| `/profsnapshot` | 如何在不停止的情况下快照热点 |
| `/memleak` | 通过 `tracemalloc` 的内存跟踪工作流 |
| `/memstop` | 如何停止内存跟踪 |
| `/memrefs <Type>` | 如何遍历某个 Python 类型的引用图 |
| `/tests` | 发现 pytest/unittest 测试 |
| `/runtests` | 按节点 ID 或文件运行测试 |
| `/testfile` | 运行当前文件中的所有测试 |
| `/modules` | 显示工作区模块树 |
| `/symbols <module>` | 显示某个模块中的符号 |
| `/health` | 类型覆盖率健康统计 |
| `/basilisk` | 服务器信息与命令参考 |

完整的性能分析工作流请参见[性能分析](/zh/docs/profiler/)指南。

## 高级：覆盖二进制文件

只有在开发时（运行本地构建的二进制文件）或想让 Zed 指向系统安装时才需要这样做。您可以在 `settings.json` 中显式设置路径：

```json
{
  "lsp": {
    "basilisk": {
      "binary": { "path": "/absolute/path/to/basilisk" }
    }
  }
}
```

……或者设置 `BASILISK_PATH` 环境变量。该设置优先于环境变量；两者都未设置时，扩展会下载发布版二进制文件（即上文的默认行为）。

## 后续步骤

- [快速开始](/zh/docs/quick-start/)——您的第一次类型检查
- [重构](/zh/docs/refactoring/)——提取、内联、移动等
- [配置](/zh/docs/configuration/)——`pyproject.toml` 参考
