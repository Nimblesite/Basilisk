---
layout: layouts/docs.njk
title: 调试
description: Basilisk 集成的 Python 调试——设置断点、单步执行代码、检查变量和评估表达式。无需单独的调试扩展。支持 VS Code、Cursor、Windsurf、Zed 和 Neovim。
keywords: basilisk, 调试, python, debugpy, 断点, 单步执行, 变量, 监视, 调试控制台, vs code, cursor, windsurf, zed, neovim, dap
lang: zh
---

# 调试

Basilisk 包含一个完全集成的 Python 调试器。设置断点、单步执行代码、检查变量和评估表达式——无需安装单独的调试扩展。Basilisk LSP 服务器端到端地管理调试会话。

## 工作原理

当您按 **F5** 时，会发生以下事情：

1. VS Code 调用 **Basilisk 调试适配器工厂**
2. 工厂请求 **Basilisk LSP**（Rust）在空闲 TCP 端口上启动 `debugpy`
3. LSP 启动 `debugpy.adapter --port <port>` 并等待其就绪
4. LSP 将主机和端口返回给 VS Code
5. VS Code 将其 **DAP（调试适配器协议）** 客户端直接连接到 debugpy
6. 您正常调试——断点、单步执行、变量、监视表达式

关键洞察：**Basilisk 代理连接**但不代理调试流量。VS Code 通过 TCP 直接与 debugpy 通信，因此 LSP 层没有性能开销。

## 先决条件

您需要在 Python 环境中安装 **debugpy**：

```bash
pip install debugpy
```

Basilisk 使用所选项目解释器，没有固定的 Python 目标。版本相关分析遵循固定
提交的 typing 指令规范
（[`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst)）。

## 快速开始

### 1. 打开 Python 文件

在激活 Basilisk 扩展的 VS Code 中打开任何 `.py` 文件。

### 2. 设置断点

单击装订线（行号左侧）设置红色断点圆点。您可以在任何可执行行上设置断点。

### 3. 按 F5

VS Code 将自动使用 Basilisk 调试配置。如果您没有 `launch.json`，Basilisk 提供默认配置。

### 4. 调试

调试器在您的断点处停止。从这里您可以：

- **F10** — 单步越过（执行当前行，移到下一行）
- **F11** — 单步进入（进入函数调用）
- **Shift+F11** — 单步退出（完成当前函数，返回调用者）
- **F5** — 继续（运行到下一个断点）
- **Shift+F5** — 停止调试

## launch.json 配置

Basilisk 注册 `basilisk-debug` 调试类型。典型配置：

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Python: Current File (Basilisk)",
      "type": "basilisk-debug",
      "request": "launch",
      "program": "${file}",
      "console": "internalConsole",
      "redirectOutput": true,
      "justMyCode": true
    }
  ]
}
```

### 配置选项

| 选项 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `program` | `string` | `${file}` | 要调试的 Python 文件路径 |
| `args` | `string[]` | `[]` | 传递给程序的命令行参数 |
| `cwd` | `string` | `${workspaceFolder}` | 工作目录 |
| `console` | `string` | `"internalConsole"` | 显示输出的位置：`internalConsole`、`integratedTerminal` 或 `externalTerminal` |
| `redirectOutput` | `boolean` | `true` | 将 stdout/stderr 重定向到调试控制台 |
| `justMyCode` | `boolean` | `true` | 只调试您的代码，跳过库内部 |
| `stopOnEntry` | `boolean` | `false` | 在程序的第一行中断 |
| `python` | `string` | 自动检测 | Python 解释器路径 |

### 控制台模式

- **`internalConsole`**（推荐）— 程序输出出现在**调试控制台**面板中。您也可以在那里评估表达式。
- **`integratedTerminal`** — 输出进入 VS Code **终端**选项卡。如果您的程序需要交互式 stdin 输入，请使用此模式。
- **`externalTerminal`** — 打开系统终端窗口。

## Python 解释器解析

Basilisk 按以下顺序解析 Python 解释器：

1. **`launch.json` 中的 `python` 字段** — 每个调试配置的显式路径
2. **`basilisk.python` VS Code 设置** — 工作区的全局设置
3. **自动检测** — LSP 服务器在工作区根目录中查找 `.venv`，然后检查 `BASILISK_PYTHON` 环境变量，然后回退到 PATH 上的 `python3`

为 Basilisk 全局设置 Python 路径：

```json
// .vscode/settings.json
{
  "basilisk.python": ".venv/bin/python"
}
```

## 检查变量

在断点处停止时，调试侧边栏中的**变量**窗格显示：

- **局部变量** — 当前范围中的所有局部变量
- **全局变量** — 模块级变量
- **返回值** — 单步退出函数后显示

复杂对象（dict、list、dataclass 实例）可以展开以检查其内容。

### 监视表达式

将表达式添加到**监视**面板以在各步骤中监控它们：

- `len(my_list)` — 跟踪集合大小
- `type(obj)` — 检查运行时类型
- `obj.__dict__` — 检查所有属性
- `[x for x in items if x > 10]` — 评估推导式

### 调试控制台（REPL）

暂停时，在**调试控制台**中键入任何 Python 表达式以在当前范围中评估它：

```
>>> sum(fibonacci_numbers)
88
>>> classified["even"]
[0, 2, 8, 34]
>>> exc.args[0]
'must be >= 0'
```

## 异常处理

Basilisk 的调试器自然地处理异常。在 `except` 块中设置断点以检查异常：

```python
try:
    data = load_config("missing.toml")
except FileNotFoundError as exc:
    print(f"Error: {exc}")  # 在此处设置断点
```

停止时，**变量**窗格显示：
- `exc` — 异常对象
- `exc.args` — 参数元组
- `exc.__cause__` — 链式异常（如果 `raise ... from ...`）
- 异常子类上的自定义属性

### 在引发异常时中断

使用 VS Code 的**断点**面板（调试侧边栏底部）配置异常断点：
- 勾选**已引发的异常（Raised Exceptions）**，在任何异常被引发时中断
- 勾选**未捕获的异常（Uncaught Exceptions）**，仅在未处理的异常上中断

## 附加模式

要调试正在运行的进程，请在您的脚本中启动 debugpy：

```python
import debugpy
debugpy.listen(5678)
debugpy.wait_for_client()  # 暂停直到 VS Code 连接
```

然后使用附加配置：

```json
{
  "name": "Attach (Basilisk)",
  "type": "basilisk-debug",
  "request": "attach",
  "connect": {
    "host": "localhost",
    "port": 5678
  }
}
```

这对于调试 Web 服务器（Django、Flask、FastAPI）和长时间运行的进程很有用。

## 验证调试链

要确认调试确实经过 Basilisk（而不是其他扩展），请打开 **Basilisk** 输出通道（**查看 > 输出 > Basilisk**）。启动调试会话时，您将看到：

```
[Basilisk Debug] createDebugAdapterDescriptor called — type=basilisk-debug, request=launch, program=/path/to/file.py
[Basilisk Debug] Requesting LSP to spawn debugpy (python: auto-detect)...
[Basilisk Debug] LSP spawned debugpy → connecting VS Code DAP client to localhost:57356 (session: dbg-11a1cb40)
```

这些消息证明了完整的链路：VS Code → Basilisk 扩展 → Basilisk LSP（Rust）→ debugpy → 您的代码。

## 故障排除

### debugpy 未安装

```
Basilisk Debug: debugpy is not installed. Run: pip install debugpy
```

在 Basilisk 使用的 Python 环境中安装 debugpy：

```bash
pip install debugpy
```

### 未找到 Python 解释器

```
Basilisk Debug: No Python interpreter found.
```

显式设置解释器：

```json
// .vscode/settings.json
{
  "basilisk.python": "/path/to/python"
}
```

### 调试控制台不显示输出

如果 `print()` 输出未出现在调试控制台中，请检查您的 `launch.json`：

- 设置 `"console": "internalConsole"`（不是 `"integratedTerminal"`）
- 设置 `"redirectOutput": true`

使用 `integratedTerminal` 时，输出进入终端选项卡。

### 调试启动时出现 ECONNREFUSED

如果您看到 `connect ECONNREFUSED 127.0.0.1:<port>`，则 debugpy 进程可能未能启动。请检查：

1. 是否安装了 debugpy？`python -m debugpy --version`
2. Python 路径是否正确？检查 `basilisk.python` 设置
3. 查看 Basilisk 输出通道以获取错误详情

## 在 Zed 中调试

Basilisk 的调试器通过同样的 DAP（调试适配器协议）机制在 Zed 中工作。Zed 扩展注册 `basilisk-debug` 调试适配器，它通过 TCP 连接到 debugpy。

### 先决条件

在您的 Python 环境中安装 debugpy：

```bash
pip install debugpy
```

### 启动调试会话

1. 在 Zed 中打开一个 Python 文件
2. 通过单击装订线设置断点
3. 从命令面板或调试面板启动调试

### 控制台输出

默认情况下，Basilisk 在 Zed 中使用 **`integratedTerminal`** 模式。这意味着程序输出（来自 `print()` 等）出现在**终端**选项卡中，而不是控制台选项卡中。

如果您希望输出出现在调试控制台中，请在调试配置中覆盖 `console` 设置：

```json
{
  "program": "${file}",
  "console": "internalConsole"
}
```

### Zed 中的控制台模式

| 模式 | 输出出现的位置 | 何时使用 |
|---|---|---|
| `integratedTerminal`（默认） | **终端**选项卡 | 需要 stdin 输入的程序，或当您希望输出与调试控件分开时 |
| `internalConsole` | **控制台**选项卡 | 当您希望输出与调试表达式求值放在一起时 |

### 调试配置

Zed 扩展在调试启动配置中支持这些选项：

| 选项 | 类型 | 默认值 | 描述 |
|---|---|---|---|
| `program` | `string` | — | 要调试的 Python 文件路径（必填） |
| `args` | `string[]` | `[]` | 传递给程序的命令行参数 |
| `cwd` | `string` | 工作区根目录 | 工作目录 |
| `python` | `string` | 自动检测 | Python 解释器路径 |
| `justMyCode` | `boolean` | `true` | 只调试您的代码，跳过库内部 |
| `stopOnEntry` | `boolean` | `false` | 在程序的第一行中断 |
| `console` | `string` | `integratedTerminal` | 显示输出的位置：`integratedTerminal` 或 `internalConsole` |

### 在 Zed 中验证调试链

查看 Zed 日志（`Cmd+Shift+P` → **zed: open log**）以确认调试适配器连接的消息：

```
[basilisk-debug] Spawning debugpy on port 57356
[basilisk-debug] DAP client connected
```

这证实了：Zed → Basilisk 扩展 → Basilisk LSP（Rust）→ debugpy → 您的代码。

## 架构

```
┌──────────────┐    LSP (JSON-RPC)    ┌──────────────┐    spawns    ┌──────────┐
│  VS Code /   │◄────────────────────►│ Basilisk LSP │────────────►│  debugpy │
│     Zed      │                      │   (Rust)     │             │ adapter  │
└──────┬───────┘                      └──────────────┘             └────┬─────┘
       │                                                                │
       │              DAP (TCP, 直接连接)                               │
       └────────────────────────────────────────────────────────────────┘
```

LSP 服务器的唯一作用是启动 debugpy 并返回 TCP 端口。所有 DAP 流量直接在编辑器和 debugpy 之间流动，Rust 进程没有任何开销。
