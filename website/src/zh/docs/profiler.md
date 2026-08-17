---
layout: layouts/docs.njk
title: "Python 性能分析——CPU 热图与内存泄漏检测"
description: Basilisk 集成的 Python 性能分析——内联 CPU 热图、火焰图结果面板，以及与调试器集成的内存泄漏检测，全部在您的编辑器内。
keywords: basilisk, 性能分析, python, py-spy, 火焰图, 热图, 内存泄漏, tracemalloc, cpu分析器, vs code, cursor, windsurf, open vsx, zed, neovim
lang: zh
---

# 性能分析器

Basilisk 包含一个完全集成的 Python 性能分析器。CPU 热点热图内联显示在代码上，内存泄漏被标记为诊断，分析结果在火焰图面板中打开——无需离开工作区。


## 概述

Basilisk 性能分析器结合了两个互补的引擎：

- **CPU 分析** — 基于采样，无需任何代码更改。外部进程由 [py-spy](https://github.com/benfred/py-spy)（以 Rust 库形式嵌入 Basilisk）采样；通过 Basilisk 调试器启动的程序在 macOS 上改用协作式进程内采样器，因为现代 macOS 即使对 root 也会阻止外部内存读取（见[平台要求](#platform-requirements)）。默认采样率：100 Hz。
- **内存分析** — 由 Python 的 `tracemalloc` 和 `gc` 模块驱动，**通过活动的 Basilisk 调试会话**运行：编辑器经由调试适配器注入检测代码，因此内存跟踪适用于正在调试（或通过"运行并跟踪内存"启动）的程序，而不是任意外部进程。

两个引擎都位于 Basilisk LSP 服务器（Rust）中。编辑器调用 LSP 命令（通过 `workspace/executeCommand` 调用 `basilisk.profiler.*`、`basilisk.memory.*`）并渲染服务器返回的结果——没有独立 CLI，无需 `pip install`。


---

## CPU 分析

### 启动会话（VS Code）

**Python 进程面板**（活动栏中的 Basilisk 图标）是性能分析的主入口：

- **Run & Profile CPU (Current File)** — 启动当前 Python 文件并从第一行开始分析。端到端分析脚本的最简单方式。
- **Profile CPU** — 面板中每行进程上的内联按钮；面板列出系统上所有正在运行的 Python 进程，无需手动输入 PID。
- **Basilisk: Profile Debug Session** — 将分析器附加到调试器当前运行的进程，断点和分析数据可以同时工作。

命令面板中的 **Basilisk: Start Profiling** 不会猜测目标——它会聚焦该面板，让您显式选择。

会话运行期间，状态栏显示实时采样计数；点击它（或运行 **Basilisk: Stop Profiling**）即可结束。

### 内联热图注解

随着样本累积以及会话结束时，热点行会直接在编辑器中获得彩色注解：

| 级别 | 阈值 | 颜色 |
|------|------|------|
| Critical | ≥ 20% CPU 时间 | 🔴 `#e8500a` |
| Hot | ≥ 10% | 🟠 `#f97316` |
| Warm | ≥ 5% | 🟡 `#fbbf24` |
| Cool | ≥ 1% | ⚫ `#4a5468` |

低于 1% 的行没有注解。热点行和函数还会以 **hint** 级别的诊断（`BSK-PROF-LINE`、`BSK-PROF-FUNC`）出现在问题面板中并附带测得的百分比，因此可以在与类型错误相同的位置浏览分析结果。

### 拍摄快照

**Basilisk: Take Profile Snapshot** 在会话继续运行的同时捕获至今收集的热点——装饰和诊断立即更新，提示框中的 **View Results** 按钮可打开结果面板。快照仅在内存中；文件在会话停止时写出。

### 结果面板

停止会话后，**Basilisk Profiler** 面板在源代码旁打开：

- **摘要卡片** — 总样本数、时长、热点函数与热点行数量
- **火焰图** — 附带 **Open Interactive Flame Graph** 按钮，打开可缩放搜索的 SVG
- **热点函数 / 热点行表格** — 点击任意行跳转到源代码
- **Open Trace in VS Code Viewer** — 在 VS Code 内置分析查看器中打开原始跟踪（与 [Node.js 分析](https://code.visualstudio.com/docs/nodejs/profiling)相同的火焰图和自底向上表格）
- **Open in Speedscope (external)** — 打开 [speedscope.app](https://www.speedscope.app) 并自动加载分析结果（通过私有 localhost URL 提供；如果浏览器阻止 localhost 请求，配套提示框会提供文件供手动拖入）

关闭了面板？**Basilisk: Show Profile Results** 会重新打开最近一次分析的结果。

停止时，Basilisk 会向系统临时目录写出三个产物：`basilisk-<session>.speedscope.json`、`basilisk-<session>.flamegraph.svg` 和 `basilisk-<session>.cpuprofile`。

<span id="short-programs"></span>
### 太短而无法采样的程序

采样分析器每 10 毫秒（100 Hz 时）拍一次快照。如果脚本比这更快完成工作，就没有样本落在您的代码中——Basilisk 会如实说明，而不是展示空火焰图。请分析更长的运行，或让热点路径重复执行的循环。

### 分析预设

在编辑器设置中配置 `basilisk.profiler.preset`：

| 预设 | 采样率 | 时长 |
|------|--------|------|
| `default` | 您的 `sampleRate` 设置（100 Hz） | 直到手动停止 |
| `quick` | 100 Hz | 10 秒 |
| `detailed` | 200 Hz | 60 秒 |
| `longRunning` | 50 Hz | 直到手动停止 |

---

## 内存分析

内存跟踪依托调试器：Basilisk 通过调试适配器向被调试程序注入 `tracemalloc`，因此**需要活动的 Basilisk 调试会话**（或使用启动流程，它会为您创建一个）。

### 启动

- **Run & Track Memory (Current File)** — 面板按钮；启动当前文件并从第一行开始跟踪。如果程序运行完毕，退出时会自动捕获最终快照——您总能得到结果。
- **Basilisk: Start Memory Tracking** — 在您已处于的调试会话中开始跟踪。每当 Basilisk 调试会话处于活动状态时，内存状态栏项也提供一键启动。

跟踪期间，**快照 / 比较 / 停止按钮就在调试工具栏上**，与继续和单步按钮并排。

### 快照与泄漏检测

- **Basilisk: Take Memory Snapshot** — 记录当前分配状态，绘制逐行分配装饰，并打开 Basilisk 内存仪表盘；原始 V8 `.heapprofile` 通过仪表盘的 **Open Heap Profile in VS Code Viewer** 和 **Open in Speedscope (external)** 按钮一键打开（speedscope 也支持导入堆分析文件）。
- **Basilisk: Compare Memory Snapshots** — 对比最近两个快照，并在分配行上发出泄漏诊断。
- **自动驾驶** — 启用 `basilisk.profiler.autoSnapshotOnPause`（默认开启）后，每次调试器暂停都会静默捕获快照，有值得展示的内容时才显示对比结果。`basilisk.profiler.autoSnapshot` 为长时间运行增加按间隔捕获。

泄漏置信度基于明确标准，而非猜测：

| 徽章 | 标准 |
|------|------|
| **Definite** | 带 `__del__` 的对象参与引用循环——不可回收 |
| **High** | 同一行在 3 次以上连续快照对比中持续增长 |
| **Medium** | 连续 2 次对比增长，或单次对比超过 10 MB |
| **Low** | 观察到一次增长——值得关注 |

<span id="memory-diagnostics"></span>
### 内存诊断

| 代码 | 严重性 | 含义 |
|------|--------|------|
| `BSK-MEM-ALLOC` | Hint | 该行是主要分配位置 |
| `BSK-MEM-GROWTH` | Warning | 该行的分配在快照之间增长 |
| `BSK-MEM-LEAK` | Warning（Definite 时为 Error） | 带置信度徽章的疑似泄漏 |
| `BSK-MEM-CYCLE` | Error | 阻止垃圾回收的引用循环 |

### 引用图与仪表盘

- **Basilisk: Show Reference Graph** — 通过 `gc.get_referrers()` 从泄漏候选对象遍历对象图，展示是什么在保留它；循环会被高亮。
- **内存仪表盘**汇总整个会话：堆时间线、主要分配者、泄漏候选——每一行都可跳转到源代码。
- **Basilisk: Force Garbage Collection** — 在被调试程序中运行 `gc.collect()`，以确认"泄漏"的内存是否真的可回收。

用 **Basilisk: Stop Memory Tracking** 停止跟踪（也在调试工具栏和状态栏菜单中）。

---

<span id="profiler-configuration"></span>
## 配置

性能分析器通过**编辑器设置**（VS Code 中的 `basilisk.profiler.*`）配置——它没有 `pyproject.toml` 配置段：

| 设置 | 默认值 | 说明 |
|------|--------|------|
| `basilisk.profiler.sampleRate` | `100` | 采样频率（Hz，1–1000） |
| `basilisk.profiler.includeNative` | `false` | 包含 C 扩展帧（仅 py-spy 附加路径） |
| `basilisk.profiler.lineThreshold` | `1` | 行注解的最小 CPU % |
| `basilisk.profiler.functionThreshold` | `2` | 函数诊断的最小 CPU % |
| `basilisk.profiler.maxDiagnosticsPerFile` | `20` | 每个文件的分析器诊断上限 |
| `basilisk.profiler.showInlineHeatMap` | `true` | 开关内联热图装饰 |
| `basilisk.profiler.profileOnLaunch` | `false` | 每次 Basilisk 调试启动自动分析 |
| `basilisk.profiler.processRefreshMs` | `2000` | Python 进程面板刷新间隔 |
| `basilisk.profiler.preset` | `"default"` | `default` / `quick` / `detailed` / `longRunning` |
| `basilisk.profiler.autoSnapshotOnPause` | `true` | 每次调试器暂停时拍内存快照 |
| `basilisk.profiler.autoSnapshot` | `false` | 按间隔拍内存快照 |
| `basilisk.profiler.autoSnapshotInterval` | `30` | 上述间隔（秒） |

---

<span id="platform-requirements"></span>
## 平台要求

### macOS

两条路径，自动选择：

- **通过 Basilisk 启动的程序**（"Run & Profile CPU"、调试会话）由**协作式进程内采样器**分析——无需提权，无弹窗。现代 macOS 即使对 root 也拒绝外部内存读取，因此对启动的程序，Basilisk 改为在被调试程序内部采样。代价：此路径看不到 C 扩展帧。
- **附加到外部进程**（您在终端中启动的进程——即使是同一用户）需要提权：Basilisk 附带特权辅助程序（`basilisk-profiler-helper`），首次使用时 macOS 会显示标准管理员密码提示。辅助程序以 root 身份附加，并通过本地 Unix 套接字把样本流回。取消提示会以明确的错误停止分析。

### Linux

`ptrace` 访问由 `/proc/sys/kernel/yama/ptrace_scope` 控制：

| Scope | 含义 | Basilisk 行为 |
|-------|------|---------------|
| `0` | 任意同用户进程 | 直接附加 |
| `1`（许多发行版的默认值） | 仅祖先进程 | 通过 Basilisk 启动的程序可正常附加（LSP 是祖先）；附加到无关进程失败并给出补救措施 |
| `2` | 仅管理员 | 需要提升的能力 |
| `3` | 禁用 | 外部附加不可用 |

附加被拒绝时，Basilisk 的错误会解释选项：`sudo sysctl kernel.yama.ptrace_scope=0`、为二进制授予 `cap_sys_ptrace`，或改为通过 Basilisk 调试会话进行分析。

### Windows

您自己用户拥有的进程无需提权即可附加（标准 Win32 API）。不支持分析其他用户的进程。

---

## CPU 诊断代码

| 代码 | 严重性 | 说明 |
|------|--------|------|
| `BSK-PROF-LINE` | Hint | 行占用 ≥ `lineThreshold`% 的 CPU 时间 |
| `BSK-PROF-FUNC` | Hint | 函数占用 ≥ `functionThreshold`% 的 CPU 时间 |

分析器诊断仅在会话产生数据后出现，并随会话清除。内存代码见[上文](#memory-diagnostics)。

---

## 在 Zed 中分析

Zed 的扩展 API 没有 webview 或自定义面板，因此 Zed 扩展通过 **LSP 诊断**（热点行以 hint 形式内联显示）和助手面板中的**说明性斜杠命令**呈现分析——每个命令解释对应的 LSP 命令（`basilisk.profiler.*` / `basilisk.memory.*`）及其用法，而不是直接执行：

| 命令 | 主题 |
|------|------|
| `/profile` | 启动 CPU 分析 |
| `/profstop` | 停止并获取结果 |
| `/profsnapshot` | 会话中途快照 |
| `/memleak` | 启动内存跟踪 |
| `/memstop` | 停止内存跟踪 |
| `/memrefs` | 引用图遍历 |

火焰图通过导出的 SVG 打开，或将 speedscope JSON 拖入 [speedscope.app](https://www.speedscope.app)。

---

## 在 Neovim 中分析

`basilisk.nvim` 插件提供基于 LSP 的用户命令：

| 命令 | 作用 |
|------|------|
| `:BasiliskProfile [pid]` | 启动 CPU 分析（不带参数时通过进程列表选择） |
| `:BasiliskProfileStop` | 停止 CPU 分析 |
| `:BasiliskProfileSnapshot` | 拍摄 CPU 快照 |
| `:BasiliskMemLeak` | 启动内存跟踪 |
| `:BasiliskMemStop` | 停止内存跟踪 |
| `:BasiliskMemRefs {type}` | 按类型查看引用图（带补全） |

默认键位：`<prefix>p` 开始分析，`<prefix>P` 停止。热点注解以虚拟文本渲染。

---

## 架构

```
┌──────────────┐  workspace/executeCommand   ┌──────────────────────────────┐
│  VS Code /   │────────────────────────────►│  Basilisk LSP (Rust)          │
│  Zed / Nvim  │◄────────────────────────────│  ┌──────────────────────────┐ │
└──────────────┘  诊断 + 进度通知             │  │  ProfileSessionManager   │ │
                                             │  │  Sampler (py-spy /       │ │
                                             │  │    cooperative)          │ │
                                             │  │  SampleAggregator        │ │
                                             │  │  Speedscope / SVG /      │ │
                                             │  │    .cpuprofile 导出器     │ │
                                             │  │  MemorySessionManager    │ │
                                             │  │  SnapshotDiffer          │ │
                                             │  │  LeakTracker             │ │
                                             │  └──────────────────────────┘ │
                                             └──────────────┬───────────────┘
                                                            │ 采样
                                                            ▼
                                                ┌──────────────────┐
                                                │  Python 进程      │
                                                │  （您的代码）      │
                                                └──────────────────┘
```

LSP 服务器拥有所有分析状态。编辑器调用 `basilisk.profiler.*` / `basilisk.memory.*` 命令，并渲染通过诊断以及 `basilisk/profiler/progress` 和 `basilisk/memory/timeline` 通知送达的结果。这正是同一引擎能驱动 VS Code、Zed 和 Neovim 的原因。

---

## 故障排除

### "Process not found"

目标进程在分析器附加前已退出。确保脚本仍在运行，或使用 **Run & Profile CPU (Current File)** 一步完成启动和分析。

### "Not a Python process"

分析器适用于 CPython。不支持 PyPy 及其他解释器。

### "Permission denied"（macOS）

附加到外部进程时，请在管理员提示中输入密码——或改为通过 Basilisk 启动程序，则完全无需提权。

### "Permission denied"（Linux）

检查 `cat /proc/sys/kernel/yama/ptrace_scope`——见 [Linux 要求](#linux)。通过 Basilisk 启动的程序不受影响。

### "Already profiling"

每个进程同时只支持一个 CPU 分析会话。请先停止当前会话。

### "Captured N samples, but none landed in your code"

程序完成工作的速度快于采样间隔——见[太短而无法采样的程序](#short-programs)。

---

## 引擎对比

Basilisk 嵌入 py-spy，因为它是唯一能作为 Rust 库 crate 使用的 Python 分析器——这正是零依赖、内置于 LSP 的分析器得以实现的原因：

| 属性 | py-spy | Scalene | Memray | Austin |
|---|---|---|---|---|
| 可作为 Rust crate 嵌入 | **是** | 否 | 否 | 否 |
| 附加到运行中的进程 | **是** | 否 | 否 | 是 |
| 修改目标程序 | **否** | 是 | 是 | 否 |

对比取自各项目自己的文档——[py-spy](https://github.com/benfred/py-spy)、[Scalene](https://github.com/plasma-umass/scalene)、[Memray](https://github.com/bloomberg/memray)、[Austin](https://github.com/P403n1x87/austin)。

---

## 下一步

- [调试](/zh/docs/debugging/) — 内存跟踪（以及 macOS 启动分析）所依托的集成调试器
- [安装](/zh/docs/installation/) — 编辑器设置
- [配置参考](/zh/docs/configuration/) — 检查器与 LSP 配置（分析器设置是编辑器设置，见[上文](#profiler-configuration)）
