---
layout: layouts/docs.njk
title: 快速开始
description: 5 分钟内替换 Pylance。安装 Basilisk 的 VS Code 扩展并获得完整的 Python 语言支持——开源。
keywords: basilisk, 快速开始, pylance替代, python语言服务器, 类型检查, 教程, vs code
lang: zh
eleventyNavigation:
  key: Quick Start
  order: 3
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
error[BSK-E0001]: Missing parameter type annotation
  --> bad.py:1:12
   |
 1 | def process(data):
   |             ^^^^ parameter `data` has no type annotation
   |
   = help: add type annotation: `data: str`
   = note: all parameters must be explicitly typed
   = see: https://www.basilisk-python.dev/docs/rules/#BSK-E0001

error[BSK-E0002]: Missing return type annotation
  --> bad.py:1:1
   |
 1 | def process(data):
   |     ^^^^^^^ function has no return type annotation
   |
   = help: add return type: `def process(data: str) -> str:`

Found 5 errors in 1 file.
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
error[BSK-E0001]: Missing parameter type annotation
^^^^^            ^                                  ← 严重性 + 消息
  --> bad.py:1:12                                   ← 文件:行:列
   |
 1 | def process(data):                             ← 源代码上下文
   |             ^^^^  parameter `data` ...         ← 指向问题的插入符号
   |
   = help: add type annotation: `data: str`         ← 可操作的修复
   = note: all parameters must be explicitly typed  ← 解释
   = see: https://www.basilisk-python.dev/docs/rules/#BSK-E0001  ← 文档链接
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
result: Any = legacy_sdk_call()  # basilisk: ignore[BSK-E0011] -- tracked in #847
```

没有原因的抑制本身会被标记。这是故意的：如果您需要抑制诊断，您应该能够解释原因。

## 第 7 步——检查统计

获取项目的类型覆盖率报告：

```bash
basilisk stats src/
```

输出包括：总函数数、已类型化的函数数、类型覆盖率百分比、无注解的文件。

## 下一步

- [配置参考](/zh/docs/configuration/) — 完整的 `pyproject.toml` 模式
- [所有规则](/zh/docs/rules/) — 每个 BSK-E 和 BSK-W 代码的解释
- [迁移指南](/zh/docs/migration/) — 从 Pyright 或 mypy 迁移
