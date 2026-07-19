---
layout: layouts/docs.njk
title: 快速开始
description: 5 分钟内开始使用 Basilisk。安装扩展，运行第一次类型检查，体验默认符合 PEP 规范的 Python 诊断。
keywords: basilisk, 快速开始, python语言服务器, 类型检查, 教程, vs code, cursor, windsurf, zed, neovim
lang: zh
---

# 快速开始

本指南将引导您完成与 Basilisk 的第一次类型检查。预计时间：5 分钟。

## 第 1 步——运行您的第一次检查

创建一个 Python 文件，或使用存储库中的示例。文件中的每个问题都是对
Python 类型规范的真实违反——无需任何配置：

```python
# bad.py
def greet(name: str) -> str:
    return "Hello, " + name


greet(42)


count: int = "zero"


def describe(flag: bool) -> str:
    if flag:
        label = "on"
    return label
```

运行 Basilisk：

```bash
basilisk check bad.py
```

输出：

```
error[calls_argument_type]: Argument `name` of `greet` expects `str` but received an `int` literal
  --> bad.py:5:7
  |
5 | greet(42)
  |       ^^
  |
   = help: Pass a value of type `str` for parameter `name`
   = note: Basilisk checks that literal arguments are compatible with declared parameter types
   = see: https://www.basilisk-python.dev/errors/calls_argument_type

error[assignment_compatibility]: Type mismatch: `count` is annotated `int` (int) but assigned str
  --> bad.py:8:1
  |
8 | count: int = "zero"
  | ^^^^^
  |
   = help: Either change the annotation to match the value, or change the value to `int`
   = note: Basilisk requires the inferred type to be assignable to the declared type
   = see: https://www.basilisk-python.dev/errors/assignment_compatibility

error[names_unbound]: Function `describe` returns `label` but `label` may be unbound on some paths
  --> bad.py:14:12
   |
14 |     return label
   |            ^^^^^
   |
   = help: Assign `label` unconditionally before the `return`, or add a default value
   = note: Basilisk detects variables that are assigned only inside conditional branches (if/while/try) and may not be defined on every execution path
   = see: https://www.basilisk-python.dev/errors/names_unbound

Found 3 diagnostics (3 errors).
```

开箱即用，Basilisk 启用完整的 PEP 类型规范规则集，每个违反都是**错误**。
这里没有任何主观风格约束——这是
[Python 类型系统规范](https://typing.python.org/en/latest/spec/index.html)的严格执行。

## 第 2 步——修复错误

```python
# good.py
def greet(name: str) -> str:
    return "Hello, " + name


greet("world")


count: int = 0


def describe(flag: bool) -> str:
    return "on" if flag else "off"
```

```bash
basilisk check good.py
```

```
All checked. No issues found.
```

## 第 3 步——把严格度拉到最高

检查通过意味着您的代码符合类型规范——但还没有任何规则*要求*写注解。
Basilisk 自己的严格性规则（每个参数、返回类型、属性等都必须有注解）
**默认关闭，需要主动启用**。迁移期间可以先以**警告**级别启用——代码依然
通过类型检查，而警告在提醒您：严格度还没有拉满：

```toml
# pyproject.toml
[tool.basilisk.rules]
"BSK-0001" = "warning"  # 无法推断类型的参数需要类型注解
"BSK-0002" = "warning"  # 无法推断返回类型的函数需要返回类型注解
```

在 `good.py` 中添加一个没有注解的函数：

```python
def process(data):
    return data.upper()
```

```
warning[BSK-0001]: Missing parameter type annotation for `data`
  --> good.py:15:13
   |
15 | def process(data):
   |             ^^^^
   |
   = help: Add a type annotation: `data: <type>`
   = note: Basilisk requires an explicit parameter type wherever it cannot be inferred; a literal default (e.g. `timeout=30`) infers the type and needs no annotation
   = see: https://www.basilisk-python.dev/errors/BSK-0001

warning[BSK-0002]: Missing return type annotation for function `process`
  --> good.py:15:5
   |
15 | def process(data):
   |     ^^^^^^^^^^^^^
   |
   = help: Add a return type: `def process(...) -> <type>:`
   = note: Basilisk requires an explicit return type wherever it cannot be inferred; literal-only returns (e.g. `return 42`) infer the type and need no annotation
   = see: https://www.basilisk-python.dev/errors/BSK-0002

Found 2 diagnostics (0 errors).
```

错误是规范违反；警告是您尚未完成采纳的严格性。当警告清零后，把这些规则
提升为 `"error"`，您的项目就达到了完全严格。

在 VS Code 中无需手动编辑：从命令面板运行 **Basilisk: Open Configuration
Editor**。它展示每条规则的实时严重级别，应用前可预览更改，其标签操作用
一条 `rule-tags` 配置行（例如 `"basilisk" = "error"`）启用整组规则。
完整模式请参阅[配置参考](/zh/docs/configuration/)。

![Basilisk 的标签优先 VS Code 配置编辑器，显示实时规则分面和每条规则的严重级别控件](/assets/images/vscode-configuration-editor.png)

## 第 4 步——检查目录

Basilisk 递归检查目录中的每个 `.py` 文件：

```bash
basilisk check src/
```

检查当前目录：

```bash
basilisk check
```

## 第 5 步——添加到 pyproject.toml

在您的 `pyproject.toml` 中创建 `[tool.basilisk]` 部分：

```toml
[tool.basilisk]
include = ["src/", "tests/"]
exclude = ["**/migrations/**"]
```

有了配置文件，运行 `basilisk check` 会自动使用这些设置。Basilisk 没有固定的
Python 版本默认值；只有要覆盖项目或解释器证据时才显式设置。版本判断遵循固定
提交的 typing 指令规范
（[`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst)）。

## 第 6 步——理解诊断

Basilisk 使用与 Rust 编译器（`rustc`）相同的输出格式。每个诊断包括：

```
error[calls_argument_type]: Argument `name` of `greet` expects `str`…
^^^^^                      ^                     ← 严重性 + 消息
  --> bad.py:5:7                                 ← 文件:行:列
  |
5 | greet(42)                                    ← 源代码上下文
  |       ^^                                     ← 指向问题的插入符号
  |
   = help: Pass a value of type `str` for parameter `name`   ← 可操作的修复
   = note: Basilisk checks that literal arguments are…       ← 解释
   = see: https://www.basilisk-python.dev/errors/calls_argument_type  ← 文档链接
```

- **`error[rule_code]`** — 严重性加上触发的规则。PEP 类型规范规则使用描述性
  代码（`calls_argument_type`）；Basilisk 的可选严格性规则使用 `BSK-` 代码
  （`BSK-0001`）
- **`-->`** — 文件中的位置
- **`^^^^`** — 导致诊断的确切标记
- **`= help:`** — 修复它的具体更改
- **`= note:`** — 规则存在的原因
- **`= see:`** — 完整文档的链接

## 第 7 步——有意抑制

当您真的需要使用 `Any` 或抑制诊断时，您可以——但必须提供原因：

```python
# 此抑制需要原因注释
result: Any = legacy_sdk_call()  # type: ignore[returns_compatibility]
```

没有原因的抑制本身会被标记。这是故意的：如果您需要抑制诊断，您应该能够解释原因。

## 第 8 步——检查统计

获取项目的类型覆盖率报告：

```bash
basilisk stats src/
```

输出包括：总函数数、已类型化的函数数、类型覆盖率百分比、无注解的文件。

## 第 9 步——分析运行中的脚本

Basilisk 包含一个集成的 CPU 和内存性能分析器。要在 VS Code 中试用：

1. 打开一个 Python 文件，在 **Python 进程面板**（活动栏中的 Basilisk 图标）中点击 **Run & Profile CPU (Current File)**——它会启动脚本并从第一行开始分析。若要分析已在运行的进程，改为点击面板中该进程行上的 **Profile CPU**。
2. 随着采样的积累，观察内联 CPU 热注解出现在热行上
3. 运行 **Basilisk: Stop Profiling**（`Cmd+Shift+P Cmd+Shift+X` / `Ctrl+Shift+P Ctrl+Shift+X`，或点击状态栏计数）——Basilisk Profiler 面板打开，显示火焰图和可跳转到源代码的热点函数表格

对于内存泄漏检测，在同一面板中点击 **Run & Track Memory (Current File)**（内存跟踪依托调试器，此操作会为您启动调试会话）。每次暂停都会自动捕获快照——也可通过 **Basilisk: Take Memory Snapshot** 手动拍摄——然后 **Basilisk: Compare Memory Snapshots** 在问题面板中将泄漏显示为诊断。

请参阅[性能分析器指南](/zh/docs/profiler/)了解完整的工作流程——火焰图、引用图、内存快照对比、VS Code 命令、Zed 和 Neovim 命令以及平台要求。

## 下一步

- [配置参考](/zh/docs/configuration/) — 完整的 `pyproject.toml` 模式
- [性能分析器](/zh/docs/profiler/) — CPU 热图、火焰图和内存泄漏检测
- [调试](/zh/docs/debugging/) — F5 调试，断点，单步执行，监视表达式
- [所有规则](/zh/docs/rules/) — 每条规则的解释，PEP 与可选规则一并涵盖
- [迁移指南](/zh/docs/migration/) — 从 Pyright 或 mypy 迁移
