---
layout: layouts/docs.njk
title: "Python 性能分析——CPU 热图与内存泄漏检测"
description: Basilisk 集成的 Python 性能分析——CPU 热图、火焰图、内存泄漏检测和引用图可视化，全部在您的编辑器内。
keywords: basilisk, 性能分析, python, py-spy, 火焰图, 热图, 内存泄漏, tracemalloc, cpu分析器, vs code, cursor, windsurf, open vsx, zed, neovim
lang: zh
---

# Python 性能分析

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

键盘快捷键：`Cmd+Shift+P Cmd+Shift+S`（macOS）/ `Ctrl+Shift+P Ctrl+Shift+S`（Windows/Linux）。

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

- **VS Code：** 命令面板 → **Basilisk: Take Profile Snapshot**
- **Zed：** `/profsnapshot`
- **Neovim：** `:BasiliskProfileSnapshot`

快照作为 Speedscope 兼容的 JSON 保存到工作区中的 `basilisk-profiles/` 目录。您可以在 [Speedscope](https://www.speedscope.app) 中打开它们进行更深入的分析。

### 停止分析

- **VS Code：** 命令面板 → **Basilisk: Stop Profiling**（或单击状态栏项）。快捷键：`Cmd+Shift+P Cmd+Shift+X` / `Ctrl+Shift+P Ctrl+Shift+X`
- **Zed：** `/profstop`
- **Neovim：** `:BasiliskProfileStop`

### 火焰图查看器

当您停止会话（或拍摄快照）时，Basilisk 在 VS Code 中直接打开火焰图 webview。

火焰图查看器包括：

- **摘要卡片** — 总采样数、持续时间、顶部函数、峰值线程数
- **交互式火焰图** — 单击帧缩放进入；单击面包屑缩放退出
- **顶部函数表** — 按 CPU 时间排序，带有导航到源代码的文件/行链接
- **导出** — 下载原始 Speedscope JSON 进行外部分析

### 附加到调试会话

您可以分析已经在 Basilisk 调试器下运行的程序：

- **VS Code：** 命令面板 → **Basilisk: Profile Debug Session**

这会将 py-spy 附加到由 debugpy 管理的进程，因此您可以同时设置断点并查看分析数据。

### 配置文件比较（差异）

要比较两个分析会话：

1. 拍摄会话 A 的快照（保存为基线）
2. 进行您的更改
3. 拍摄会话 B 的快照
4. 运行 **Basilisk: Compare Profile Results** 并选择两个快照

差异视图突出显示在两个会话之间变快（绿色）或变慢（红色）的函数。

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

### 内存时间轴

内存仪表板将分配增长显示为按对象类型堆叠的面积图。

### 停止内存跟踪

- **VS Code：** 命令面板 → **Basilisk: Stop Memory Tracking**
- **Zed：** `/memstop`
- **Neovim：** `:BasiliskMemoryStop`

---

## 内存仪表板

内存仪表板将所有内存分析视图汇总到一个面板中：

| 面板 | 描述 |
|-------|-------------|
| **时间轴** | 会话期间堆增长，按对象类型堆叠 |
| **顶部分配器** | 负责最多活动分配的函数 |
| **泄漏候选** | 带有置信度评分和分配调用栈的排序列表 |
| **引用图** | 选定泄漏候选的力向对象图 |

---

## 分析预设

Basilisk 提供四个分析预设，在保真度和开销之间进行权衡：

| 预设 | 采样率 | 时长 | 开销 |
|--------|------------|----------|----------|
| `default` | 您的 `sampleRate` 设置（100 Hz） | 无限制 | ~1% |
| `quick` | 100 Hz | 10 秒 | ~1% |
| `detailed` | 200 Hz | 60 秒 | ~2% |
| `longRunning` | 50 Hz | 无限制 | <0.5% |

从 Basilisk 设置面板中选择预设或在 `pyproject.toml` 中设置：

```toml
[tool.basilisk.profiler]
preset = "detailed"
```

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

| 设置 | 类型 | 默认值 | 描述 |
|---------|------|---------|-------------|
| `enabled` | bool | `true` | 启用分析器功能 |
| `sample-rate` | int | `100` | 采样频率（Hz） |
| `include-native` | bool | `false` | 包含 C 扩展和 Rust 帧 |
| `line-threshold` | float | `1.0` | 行注解的最小 CPU % |
| `function-threshold` | float | `2.0` | 表条目的最小 CPU % |
| `output-directory` | string | `"basilisk-profiles"` | 快照保存位置 |

VS Code 设置在 `basilisk.profiler.*` 下镜像这些：

```json
{
  "basilisk.profiler.sampleRate": 100,
  "basilisk.profiler.includeNative": false,
  "basilisk.profiler.lineThreshold": 1.0
}
```

---

## 平台要求

### macOS

py-spy 需要读取其他进程内存的能力。在 macOS 上，这需要对您不拥有的进程提升权限。

Basilisk 附带一个特权辅助二进制文件（`basilisk-profiler-helper`），可透明地处理此问题。第一次分析进程时，macOS 将通过标准系统对话框提示您输入管理员密码。该辅助程序经过代码签名，仅运行所需的特定操作。

要分析您自己的进程（例如从 Basilisk 启动的脚本），无需提升权限。

### Linux

在 Linux 上，`ptrace` 访问由 `/proc/sys/kernel/yama/ptrace_scope` 管理：

| 范围 | 含义 | Basilisk 行为 |
|-------|---------|-------------------|
| `0` | 所有进程 | 无需提升 |
| `1`（许多发行版的默认值） | 仅父/子 | Basilisk 启动子 shim |
| `2` | 仅管理员 | 需要 `sudo` |
| `3` | 禁用 | 分析不可用 |

如果您在 Linux 上看到权限错误，请检查：

```bash
cat /proc/sys/kernel/yama/ptrace_scope
```

临时允许分析：

```bash
sudo sysctl kernel.yama.ptrace_scope=0
```

### Windows

Windows 上无需提升权限。分析器使用任何进程所有者可用的标准 Win32 API 进行连接。

---

## 诊断代码

| 代码 | 严重性 | 描述 |
|------|----------|-------------|
| `BSK-PROF-LINE` | 警告 | 行花费 ≥ `line-threshold`% 的 CPU 时间 |
| `BSK-PROF-FUNC` | 警告 | 函数花费 ≥ `function-threshold`% 的 CPU 时间 |
| `BSK-PROF-GIL` | 警告 | 线程在 GIL 上阻塞 ≥ 20% 的时间 |
| `BSK-PROF-MEM` | 警告 | 在快照之间未释放的内存分配 |

这些诊断与类型错误一起出现在问题面板中。在没有活动分析会话时，它们会被抑制。

---

## 在 Zed 中分析

Zed 扩展通过 AI 助手面板中的斜杠命令呈现分析。Zed 中没有 webview；相反，火焰图数据导出到 `basilisk-profiles/` 目录，诊断在编辑器中内联显示。

| 命令 | 操作 |
|---------|--------|
| `/profile` | 开始 CPU 分析 |
| `/profstop` | 停止 CPU 分析 |
| `/profsnapshot` | 拍摄 CPU 快照 |
| `/memleak` | 开始内存跟踪 |
| `/memstop` | 停止内存跟踪 |
| `/memrefs` | 显示顶部泄漏的引用图 |

---

## 在 Neovim 中分析

Neovim 插件通过用户命令公开分析：

| 命令 | 操作 |
|---------|--------|
| `:BasiliskProfileStart` | 开始 CPU 分析 |
| `:BasiliskProfileStop` | 停止 CPU 分析 |
| `:BasiliskProfileSnapshot` | 拍摄 CPU 快照 |
| `:BasiliskMemoryStart` | 开始内存跟踪 |
| `:BasiliskMemoryStop` | 停止内存跟踪 |

内联热图注解作为虚拟文本出现在符号列中。

---

## 架构

```
┌──────────────┐    LSP (JSON-RPC)    ┌──────────────────────────────┐
│  VS Code /   │◄────────────────────►│  Basilisk LSP (Rust)         │
│  Zed / Nvim  │                      │  ┌──────────────────────────┐ │
└──────────────┘                      │  │  ProfileSessionManager   │ │
                                      │  │  采样器线程 (py-spy)      │ │
                                      │  │  SampleAggregator        │ │
                                      │  │  SpeedscopeExporter      │ │
                                      │  │  MemorySessionManager    │ │
                                      │  │  SnapshotDiffer          │ │
                                      │  │  ReferenceGraphWalker    │ │
                                      │  └──────────────────────────┘ │
                                      └──────────────┬───────────────┘
                                                     │ 附加到
                                                     ▼
                                         ┌──────────────────┐
                                         │  Python 进程      │
                                         │  （您的代码）       │
                                         └──────────────────┘
```

LSP 服务器拥有所有分析状态。编辑器扩展是纯 UI——它们发送命令并渲染 LSP 通过通知发送回的内容。这意味着分析在 VS Code、Zed 和 Neovim 中以相同方式工作。

---

## 性能目标

分析器默认设计为低开销：

| 操作 | 目标 |
|-----------|--------|
| 采样开销 | < 3% CPU |
| 10 分钟会话的 LSP 内存 | < 50 MB |
| 诊断生成 | < 100 ms |
| Speedscope 导出 | < 200 ms |
| 火焰图 SVG 渲染 | < 500 ms |

---

## 故障排除

### "Process not found"

目标进程在分析器可以连接之前退出。请确保在开始分析会话之前您的脚本仍在运行。

### "Not a Python process"

py-spy 仅适用于 CPython。不支持 PyPy 和其他解释器。

### "Permission denied"（macOS）

单击出现的系统对话框上的**允许**，或使用管理员权限运行 VS Code/Zed。请参阅[平台要求](#平台要求)。

### "Permission denied"（Linux）

检查 `ptrace_scope` — 请参阅上面的 [Linux 部分](#linux)。

### "Already profiling"

一次只能运行一个 CPU 分析会话。请先停止当前会话。

### 快照未出现

确保 `output-directory` 可写。默认情况下，这是相对于您工作区根目录的 `basilisk-profiles/`。

---

## 与其他分析器的比较

| 功能 | Basilisk | cProfile | Scalene | Memray | Austin |
|---------|----------|---------|---------|--------|--------|
| 零代码更改 | 是 | 否 | 否 | 否 | 是 |
| 内联编辑器注解 | 是 | 否 | 否 | 否 | 否 |
| 内存泄漏检测 | 是 | 否 | 是 | 是 | 否 |
| 引用图 | 是 | 否 | 否 | 否 | 否 |
| IDE 集成（原生） | 是 | 否 | 否 | 否 | 否 |
| 火焰图查看器 | 是 | 否 | 是 | 是 | 否 |
| 附加时分析 | 是 | 否 | 否 | 否 | 是 |
| GIL 争用 | 是 | 否 | 是 | 否 | 是 |

---

## 下一步

- [配置参考](/zh/docs/configuration/) — 完整的 `pyproject.toml` 模式，包括分析器设置
- [调试](/zh/docs/debugging/) — 与分析器一起使用集成调试器
- [安装](/zh/docs/installation/) — 平台设置，包括权限要求
