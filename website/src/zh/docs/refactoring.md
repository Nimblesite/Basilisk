---
layout: layouts/docs.njk
title: 重构
description: Basilisk 中所有 16 个重构代码操作——提取、内联、移动、重命名、转换和更改签名。
keywords: basilisk, 重构, 提取变量, 提取函数, 内联, 移动符号, 重命名, 代码操作, pylance
lang: zh
eleventyNavigation:
  key: Refactoring
  parent: Introduction
  order: 7
---

# 重构

Basilisk 通过 LSP 协议提供 **16 个重构代码操作**。它们自动出现在 VS Code、Zed 和 Neovim 的灯泡菜单中——无需额外的扩展或配置。

每个重构都会产生一个编辑器原子性应用的 `WorkspaceEdit`。多文件重构（移动符号、模块重命名）使用带有 `CreateFile` 操作的 `DocumentChanges`。

## 提取

### 提取变量

选择一个表达式。Basilisk 在当前行之前插入 `extracted_value = <expression>` 并将选择替换为 `extracted_value`。

如果表达式在文件中出现多次，则提供第二个操作**"提取变量——替换所有"**，替换所有相同的出现。

```python
# 之前
result = some_func(42) + other_func(7)

# "提取变量"作用于 `some_func(42)` 之后
extracted_value = some_func(42)
result = extracted_value + other_func(7)
```

### 提取常量

与提取变量相同，但将赋值放置在最后一个导入行之后的模块级别，名称为 `SCREAMING_SNAKE_CASE`。

```python
# 之前
def f():
    return 42

# "提取常量"作用于 `42` 之后
EXTRACTED_VALUE = 42

def f():
    return EXTRACTED_VALUE
```

### 提取函数

选择一行或多行完整的行。Basilisk 将它们提取到一个新函数中，包含：

- **数据流分析** — 在内部读取但在外部定义的变量成为参数；在内部写入并在之后读取的变量成为返回值
- **异步检测** — 如果封闭函数是 `async def`，提取的函数也是 `async`，并用 `await` 调用
- **方法检测** — 如果在方法内部，提取的函数将 `self`/`cls` 作为第一个参数，并通过 `self.extracted_function()` 调用
- **安全防护** — 包含 `yield`、`break` 或 `continue` 的选择被拒绝（这些不能在不改变语义的情况下提取）
- **PEP 8 格式** — 顶级提取在之前/之后有两个空行；嵌套提取有一个

```python
# 之前
def process(data: list[int]) -> int:
    total = sum(data)
    average = total / len(data)
    return average

# "提取函数"作用于第 2-3 行之后
def extracted_function(data: list[int]) -> int:
    total = sum(data)
    average = total / len(data)
    return average

def process(data: list[int]) -> int:
    average = extracted_function(data)
    return average
```

## 内联

### 内联变量

将光标放在简单的 `name = expr` 赋值上。Basilisk 将 `name` 的每个后续使用替换为 `expr` 并删除赋值。

只有当赋值是简单的 `name = expr`（无元组解包，无增强赋值）且名称在赋值之后在同一缩进范围内至少出现一次时才提供。

```python
# 之前
temp = calculate()
result = temp + 1

# "内联变量"之后
result = calculate() + 1
```

### 内联函数

将光标放在函数调用上。如果被调用的函数有单个 `return expr` 主体，Basilisk 将调用替换为表达式，用提供的参数替换参数名。

```python
# 之前
def double(x: int) -> int:
    return x * 2

result = double(5)

# "内联函数"作用于 `double(5)` 之后
result = 5 * 2
```

## 移动

### 移动到新文件

将光标放在顶级 `def` 或 `class` 行上。Basilisk：

1. 创建以符号命名的新 `.py` 文件（`CamelCase` 变为 `snake_case.py`）
2. 将完整定义及其导入移入新文件
3. 将原始文件中的定义替换为 `from .snake_case import SymbolName`

### 移动到现有文件

与上述相同，但将符号移动到您选择的文件。这使用 `basilisk.moveSymbol` 命令，编辑器扩展可以将其连接到文件选择器。

## 重命名

Basilisk 的重命名是**范围感知的**——在一个函数内重命名 `x` 不会触及另一个函数或模块级别的 `x`。

额外的重命名功能：

- **关键字参数** — 重命名参数也会在调用点重命名 `param=value`
- **`self.attr` / `cls.attr`** — 重命名类属性会更新所有方法中的所有 `self.attr` 引用
- **`__all__` 条目** — 如果符号出现在 `__all__` 中，其条目会被更新
- **文档字符串引用** — `:param name:`（Sphinx）、`name (type):`（Google）和 `name :`（NumPy）模式会被更新
- **验证** — 拒绝重命名为 Python 内置函数、关键字或会遮蔽现有变量的名称

## 更改签名

### 删除参数

将光标放在 `def` 行中的参数名上。Basilisk 删除该参数和文件中每个调用点的相应参数。不能删除 `self` 和 `cls`。

### 添加参数

将光标放在 `def` 行上。Basilisk 将 `new_param=None` 追加到参数列表。

### 排序参数

将光标放在有 3+ 个非 self 参数的 `def` 行上。Basilisk 按字母顺序排序它们，保持 `self`/`cls` 在第一位。

## 实现抽象方法

将光标放在继承自抽象基类的类上。Basilisk 为所有未实现的抽象方法生成方法存根，具有正确的 `self` 参数和 `pass` 主体。

## 构造转换

这些是在光标位于相关语法上时提供的双向转换：

| 转换 | 示例 |
|-----------|---------|
| `Union[X, Y]` ↔ `X \| Y` | `Union[int, str]` 变为 `int \| str` |
| `Optional[X]` ↔ `X \| None` | `Optional[int]` 变为 `int \| None` |
| f-string ↔ `.format()` | `f"hello {name}"` 变为 `"hello {}".format(name)` |
| `dict()` ↔ `{}` | `dict(a=1, b=2)` 变为 `{"a": 1, "b": 2}` |
| `list()` ↔ `[]` | `list()` 变为 `[]` |
| 三元 ↔ if/else | `x = a if cond else b` 变为 4 行 if/else 块 |
| NamedTuple 类 ↔ 函数式 | 类语法变为 `namedtuple()` 调用，反之亦然 |

## 竞争对比

| 功能 | Basilisk | Pylance | Pyright | Jedi-LSP | pylsp + Rope |
|---------|----------|---------|---------|----------|-------------|
| 重命名（范围感知） | 是 | 是 | 是 | 是 | 是 |
| 重命名模块/文件 | 是 | 是 | 否 | 否 | 是 |
| 提取函数 | 是 | 是 | 否 | 是 | 是 |
| 提取变量 | 是 | 是 | 否 | 是 | 是 |
| 移动符号 | 是 | 是 | 否 | 否 | 是 |
| 实现抽象方法 | 是 | 是 | 否 | 否 | 否 |
| 构造转换 | 是 | 部分 | 否 | 否 | 否 |
| 内联变量 | 是 | 否 | 否 | 否 | 是 |
| 更改签名 | 是 | 否 | 否 | 否 | 部分 |
| 内联函数 | 是 | 否 | 否 | 否 | 是 |
