---
layout: layouts/docs.njk
title: 快速开始
description: 5 分钟内开始使用 Basilisk。安装扩展，运行第一次类型检查，体验默认严格模式的 Python 诊断。
keywords: basilisk, 快速开始, python语言服务器, 类型检查, 教程, vs code, cursor, windsurf, zed, neovim
lang: zh
---

# 快速开始

本指南将引导您完成与 Basilisk 的第一次类型检查。预计时间：5 分钟。

## 第 1 步——运行您的第一次检查

创建一个 Python 文件，或使用存储库中的示例：

```python
# bad.py
def process(data):
    return data.upper()

class User:
    def __init__(self, name, age):
        self.name = name
        self.age  = age

    def greet(self):
        return f"Hello, {self.name}"
```

运行 Basilisk：

```bash
basilisk check bad.py
```

输出：

```
error[BSK-E0001]: Missing parameter type annotation for `data`
  --> bad.py:1:13
    |
1 | def process(data):
    |             ^^^^
    |
   = help: Add a type annotation: `data: <type>`
   = note: In Basilisk, all function parameters require explicit types
   = see: https://www.basilisk-python.dev/errors/BSK-E0001

error[BSK-E0002]: Missing return type annotation for function `process`
  --> bad.py:1:5
    |
1 | def process(data):
    |     ^^^^^^^^^^^^^
    |
   = help: Add a return type: `def process(...) -> <type>:`
   = note: In Basilisk, all functions require an explicit return type
   = see: https://www.basilisk-python.dev/errors/BSK-E0002

... 4 more errors (untyped `name`, `age`, `__init__`, and `greet`) ...

Found 6 diagnostics (6 errors).
```

## 第 2 步——修复错误

为每个参数和返回类型添加类型注解：

```python
# good.py
def process(data: str) -> str:
    return data.upper()

class User:
    name: str
    age: int

    def __init__(self, name: str, age: int) -> None:
        self.name = name
        self.age  = age

    def greet(self) -> str:
        return f"Hello, {self.name}"
```

```bash
basilisk check good.py
```

```
All checked. No issues found.
Checked 1 file — 0 errors, 0 warnings.
```

## 第 3 步——检查目录

Basilisk 递归检查目录中的每个 `.py` 文件：

```bash
basilisk check src/
```

检查当前目录：

```bash
basilisk check
```

## 第 4 步——添加到 pyproject.toml

在您的 `pyproject.toml` 中创建 `[tool.basilisk]` 部分：

```toml
[tool.basilisk]
python-version = "3.12"
include = ["src/", "tests/"]
exclude = ["**/migrations/**"]
```

有了配置文件，运行 `basilisk check` 会自动使用这些设置。

## 第 5 步——理解诊断

Basilisk 使用与 Rust 编译器（`rustc`）相同的输出格式。每个诊断包括：

```
error[BSK-E0001]: Missing parameter type annotation for `data`
^^^^^            ^                                  ← 严重性 + 消息
  --> bad.py:1:13                                   ← 文件:行:列
    |
1 | def process(data):                              ← 源代码上下文
    |             ^^^^                               ← 指向问题的插入符号
    |
   = help: Add a type annotation: `data: <type>`    ← 可操作的修复
   = note: In Basilisk, all function parameters require explicit types  ← 解释
   = see: https://www.basilisk-python.dev/errors/BSK-E0001  ← 文档链接
```

- **`error[BSK-EXXXX]`** — 带唯一代码的错误（橙色）
- **`-->`** — 文件中的位置（蓝色）
- **`^^^^`** — 导致错误的确切标记（红色下划线）
- **`= help:`** — 修复它的具体更改（绿色）
- **`= note:`** — 规则存在的原因
- **`= see:`** — 完整文档的链接

## 第 6 步——有意抑制

当您真的需要使用 `Any` 或抑制诊断时，您可以——但必须提供原因：

```python
# 此抑制需要原因注释
result: Any = legacy_sdk_call()  # basilisk: ignore[returns_compatibility] -- tracked in #847
```

没有原因的抑制本身会被标记。这是故意的：如果您需要抑制诊断，您应该能够解释原因。

## 第 7 步——检查统计

获取项目的类型覆盖率报告：

```bash
basilisk stats src/
```

输出包括：总函数数、已类型化的函数数、类型覆盖率百分比、无注解的文件。

## 第 8 步——分析运行中的脚本

Basilisk 包含一个集成的 CPU 和内存性能分析器。要在 VS Code 中试用：

1. 运行任何 Python 脚本（进程必须处于活动状态）
2. 打开命令面板（`Cmd+Shift+P` / `Ctrl+Shift+P`）——或使用快捷键 `Cmd+Shift+P Cmd+Shift+S` / `Ctrl+Shift+P Ctrl+Shift+S`
3. 运行 **Basilisk: Start Profiling** 并选择目标进程
4. 随着采样的积累，观察内联 CPU 热注解出现在热行上
5. 运行 **Basilisk: Stop Profiling** 以打开火焰图查看器

对于内存泄漏检测，使用 **Basilisk: Start Memory Tracking**，通过 **Basilisk: Take Memory Snapshot** 拍摄两个快照，然后 **Basilisk: Diff Memory Snapshots** 在问题面板中将泄漏显示为诊断。

请参阅[性能分析器指南](/zh/docs/profiler/)了解完整的工作流程——火焰图、引用图、配置文件差异、Zed 和 Neovim 命令以及平台要求。

## 下一步

- [配置参考](/zh/docs/configuration/) — 完整的 `pyproject.toml` 模式
- [性能分析器](/zh/docs/profiler/) — CPU 热图、火焰图和内存泄漏检测
- [调试](/zh/docs/debugging/) — F5 调试，断点，单步执行，监视表达式
- [所有规则](/zh/docs/rules/) — 每个 BSK-E 和 BSK-W 代码的解释
- [迁移指南](/zh/docs/migration/) — 从 Pyright 或 mypy 迁移
