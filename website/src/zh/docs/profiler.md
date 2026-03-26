---
layout: layouts/docs.njk
title: 性能分析器
description: Basilisk 集成的 Python 性能分析——CPU 热图、火焰图、内存泄漏检测和引用图可视化，全部在您的编辑器内。
keywords: basilisk, 性能分析, python, py-spy, 火焰图, 热图, 内存泄漏, tracemalloc, cpu分析器, vs code, zed, neovim
lang: zh
eleventyNavigation:
  key: Profiler
  order: 5
---

# 性能分析器

Basilisk 包含一个完全集成的 Python 性能分析器。CPU 热点热图直接内联显示在代码的每一行上，内存泄漏被标记为诊断，火焰图直接在您的编辑器中打开——无需离开工作区。

## 概述

Basilisk 性能分析器结合了两个互补的引擎：

- **CPU 分析** — 由 [py-spy](https://github.com/benfred/py-spy) 提供支持，这是一个采样分析器，无需任何代码更改即可连接到正在运行的 Python 进程。以可配置的速率（默认：100 Hz）采样调用栈，并在每个热行上显示内联热注解。
- **内存分析** — 由 Python 的 `tracemalloc` 模块提供支持，在运行时注入。随时间跟踪分配，对快照进行差异以找到泄漏，并遍历引用图以识别保留链。

两个引擎都通过 Basilisk LSP 服务器（Rust）协调。IDE 扩展监听 LSP 通知并渲染结果——不需要独立的 CLI。

---

## CPU 分析

### 开始分析会话

**VS Code：**

1. 运行您想要分析的 Python 脚本或测试（它必须正在运行）
2. 打开命令面板（`Cmd+Shift+P` / `Ctrl+Shift+P`）
3. 运行 **Basilisk: Start Profiling**
4. 从列表中选择目标进程

**Zed：**

在 AI 面板中使用斜杠命令：

```
/profile
```

**Neovim：**

```vim
:BasiliskProfileStart
```

### 内联热图注解

随着采样的积累，Basilisk 直接在编辑器装订线中注解每个热行。热调色板使用四个级别：

| 级别 | 阈值 | 颜色 | 含义 |
|-------|-----------|--------|---------------|
| 严重 | ≥ 10% CPU 时间 | 🔴 `#e8500a` | 严重瓶颈——首先优化 |
| 热 | ≥ 5% | 🟠 `#f97316` | 显著开销 |
| 温 | ≥ 2% | 🟡 `#fbbf24` | 适度成本，值得审查 |
| 凉 | ≥ 1% | ⚫ `#4a5468` | 轻微开销 |

低于 1% 的行不接收注解。

### 拍摄快照

在会话运行时捕获时间点配置文件：

- **VS Code：** 命令面板 → **Basilisk: Snapshot Profile**
- **Zed：** `/profsnapshot`
- **Neovim：** `:BasiliskProfileSnapshot`

快照作为 Speedscope 兼容的 JSON 保存到工作区中的 `basilisk-profiles/` 目录。您可以在 [Speedscope](https://www.speedscope.app) 中打开它们进行更深入的分析。

### 停止分析

- **VS Code：** 命令面板 → **Basilisk: Stop Profiling**（或单击状态栏项）
- **Zed：** `/profstop`
- **Neovim：** `:BasiliskProfileStop`

### 火焰图查看器

当您停止会话（或拍摄快照）时，Basilisk 在 VS Code 中直接打开火焰图 webview。

火焰图查看器包括：

- **摘要卡片** — 总采样数、持续时间、顶部函数、峰值线程数
- **交互式火焰图** — 单击帧缩放进入；单击面包屑缩放退出
- **顶部函数表** — 按 CPU 时间排序，带有导航到源代码的文件/行链接
- **导出** — 下载原始 Speedscope JSON 进行外部分析

---

## 内存分析

### 开始内存跟踪

内存跟踪将 Python 的 `tracemalloc` 模块注入到正在运行的进程中，以记录每个分配及其调用栈。

- **VS Code：** 命令面板 → **Basilisk: Start Memory Tracking**
- **Zed：** `/memleak`
- **Neovim：** `:BasiliskMemoryStart`

### 拍摄内存快照

捕获当前分配状态的快照：

- **VS Code：** 命令面板 → **Basilisk: Take Memory Snapshot**
- **Zed：** `/memsnap`

快照按顺序编号（快照 1、快照 2、…）。至少拍摄两个快照以启用差异分析。

### 查找内存泄漏

对两个快照进行差异以识别在它们之间增长的分配：

- **VS Code：** 命令面板 → **Basilisk: Diff Memory Snapshots**
- **Zed：** `/memdiff`

Basilisk 比较快照并在未释放内存的行上发出 LSP 诊断。诊断使用 `BSK-PROF-MEM` 代码，在问题面板中显示为警告。

泄漏置信度评分：

| 徽章 | 得分 | 含义 |
|-------|-------|---------|
| **确定** | 95–100% | 对象只能从泄漏的根访问 |
| **高** | 70–94% | 强保留证据 |
| **中** | 40–69% | 可能泄漏，需要调查 |
| **低** | < 40% | 可疑增长但不确定 |

### 引用图

引用图从疑似泄漏根遍历 Python 对象图，以显示确切是什么在内存中保留它。

- **VS Code：** 命令面板 → **Basilisk: Show Reference Graph**
- **Zed：** `/memrefs`

图以交互式力向布局渲染：

- **节点大小** — 与对象的内存占用成比例
- **节点颜色** — 对象类型（dict、list、类实例、模块等）
- **边标签** — 保存引用的属性名、索引或键
- **循环** — 以红色突出显示；循环阻止垃圾回收

单击任何节点以检查其类型、`repr()`、大小和传出引用。

---

## 分析预设

Basilisk 提供四个分析预设，在覆盖率和开销之间进行权衡：

| 预设 | 采样率 | 本机帧 | 内存 | 开销 |
|--------|------------|---------------|--------|----------|
| `default` | 100 Hz | 否 | 否 | ~1% |
| `lightweight` | 25 Hz | 否 | 否 | <0.5% |
| `detailed` | 200 Hz | 是 | 否 | ~3% |
| `memory` | 50 Hz | 否 | 是 | ~2% |

---

## 配置

所有分析器设置都在 `pyproject.toml` 的 `[tool.basilisk.profiler]` 下：

```toml
[tool.basilisk.profiler]
enabled = true
sample-rate = 100          # Hz——每秒采样数
include-native = false     # 包含 C 扩展帧
line-threshold = 1.0       # 显示行注解的最小 %
function-threshold = 2.0   # 在表中显示函数的最小 %
output-directory = "basilisk-profiles"
```

---

## 诊断代码

| 代码 | 严重性 | 描述 |
|------|----------|-------------|
| `BSK-PROF-LINE` | 警告 | 行花费 ≥ `line-threshold`% 的 CPU 时间 |
| `BSK-PROF-FUNC` | 警告 | 函数花费 ≥ `function-threshold`% 的 CPU 时间 |
| `BSK-PROF-GIL` | 警告 | 线程在 GIL 上阻塞 ≥ 20% 的时间 |
| `BSK-PROF-MEM` | 警告 | 在快照之间未释放的内存分配 |

---

## 下一步

- [配置参考](/zh/docs/configuration/) — 完整的 `pyproject.toml` 模式，包括分析器设置
- [调试](/zh/docs/debugging/) — 与分析器一起使用集成调试器
- [安装](/zh/docs/installation/) — 平台设置，包括权限要求
