---
layout: layouts/docs.njk
title: Mojo 风格安全注解
description: Basilisk 的所有权、不可变性和强制转换安全分析——标准 Python 中的 Mojo 概念。
keywords: basilisk所有权, borrowed, inout, owned, mojo, python所有权分析
lang: zh
eleventyNavigation:
  key: Mojo Safety
  order: 8
---

# Mojo 风格安全注解

Basilisk 用受 Mojo 启发的所有权语义扩展了 Python 的类型系统。这些是静态分析注解——无运行时开销，无需 Mojo 编译器——捕获了标准类型检查遗漏的整类错误。

## 为什么所有权在 Python 中很重要

Python 的标准类型系统描述了值*是什么*。它不描述值*可以如何使用*。您可以将函数参数注解为 `list[int]`，但调用者仍然可以意外地变异该列表，或者函数在看似转移后仍然持有引用。

Mojo 通过区分以下内容在其语言设计中解决了这个问题：
- **`borrowed`** — 只读引用
- **`inout`** — 可变引用
- **`owned`** — 完全所有权；被调用方可以消耗值

Basilisk 将这些区分作为 `Annotated` 类型元数据带到 Python，在分析时进行静态检查。

---

## 三个所有权注解

从 `basilisk` 导入注解：

```python
from typing import Annotated
from basilisk import Borrowed, InOut, Owned
```

### `Borrowed` — 只读引用

函数可以读取值，但不得修改它。这是 Basilisk 中所有函数参数的**默认值**。

```python
def summarise(items: Annotated[list[int], Borrowed]) -> int:
    return sum(items)  # OK——只读访问
```

尝试变异 `Borrowed` 参数是 **BSK-E0030**：

```python
def bad_summarise(items: Annotated[list[int], Borrowed]) -> int:
    items.sort()  # error[BSK-E0030]: mutation of Borrowed parameter
    return sum(items)
```

### `InOut` — 可变引用

函数可以读取和修改值。调用者保留所有权。

```python
def normalise(values: Annotated[list[float], InOut]) -> None:
    total = sum(values)
    for i, v in enumerate(values):
        values[i] = v / total  # OK——声明了 InOut
```

### `Owned` — 所有权已转移

函数获得值的所有权。调用者在调用后不得使用该值。

```python
def into_sorted(items: Annotated[list[int], Owned]) -> list[int]:
    items.sort()
    return items

data = [3, 1, 2]
result = into_sorted(data)
data.append(4)  # error[BSK-E0031]: use after ownership transfer
```

---

## 默认不可变性

Basilisk 为所有函数参数强制执行**默认不可变性**。即使没有明确的 `Borrowed` 注解，被变异的参数也会产生 **BSK-E0040**：

```python
def process(items: list[int]) -> list[int]:
    items.append(0)  # error[BSK-E0040]: mutation of immutable parameter
    return items
```

要允许变异，您必须明确声明：

```python
from typing import Annotated
from basilisk import InOut

def process(items: Annotated[list[int], InOut]) -> list[int]:
    items.append(0)  # OK——变异被明确声明
    return items
```

这使函数契约在调用点可见。当您看到函数调用时，您可以立即从注解中判断被调用方是否会修改传递的值。

---

## 强制转换安全

Python 执行几种隐式数字转换，可能隐藏错误。Basilisk 标记所有这些：

### BSK-E0060 — `int` → `float`

```python
def area(radius: float) -> float:
    return 3.14159 * radius * radius

area(5)       # error[BSK-E0060]: implicit int→float coercion
area(5.0)     # OK
area(float(5))  # OK——显式转换
```

### BSK-E0061 — `bool` → `int`

```python
def count(flags: list[bool]) -> int:
    total: int = 0
    for f in flags:
        total += f  # error[BSK-E0061]: implicit bool→int coercion
    return total

# 正确
total += int(f)
```

### BSK-E0062 — `bytes` → `str`

```python
def log(message: str) -> None:
    print(message)

data: bytes = b"hello"
log(data)  # error[BSK-E0062]: implicit bytes→str coercion
log(data.decode("utf-8"))  # OK
```

---

## 冻结数据类

可变数据类是常见的错误来源。Basilisk 将没有 `frozen=True` 的 `@dataclass` 标记为 **BSK-E0042**：

```python
from dataclasses import dataclass

@dataclass
class Point:      # warning[BSK-E0042]: prefer frozen=True
    x: float
    y: float

@dataclass(frozen=True)
class Point:      # OK——按设计不可变
    x: float
    y: float
```

---

## Mojo 兼容性矩阵

如果您计划最终针对 Mojo，Basilisk 的注解在结构上是兼容的。以下是 Mojo 概念如何映射到 Basilisk 的静态分析：

| Mojo 概念 | Basilisk 注解 | 静态检查 |
|---|---|---|
| `borrowed` 参数 | `Annotated[T, Borrowed]` | 不允许变异（BSK-E0030） |
| `inout` 参数 | `Annotated[T, InOut]` | 允许变异；调用者保留所有权 |
| `owned` 参数 | `Annotated[T, Owned]` | 调用者调用后不得使用值（BSK-E0031） |
| `fn` 函数 | 任何 Basilisk 检查的函数 | 所有参数必须有类型 |
| `alias` 声明 | `Final[T]` | 变异是 BSK-E0043 |
| `let` 绑定 | `Final` 注解 | 重赋值是 BSK-E0043 |
| `struct`（值类型） | `@dataclass(frozen=True)` | 无动态属性（BSK-E0050） |
| 无隐式强制转换 | 强制转换规则 E0060–E0063 | 需要显式转换 |

通过 Basilisk 所有权和不可变性检查的 Python 代码在结构上已准备好用于 Mojo。注解直接转换为 Mojo 的函数签名约定。

---

## 启用和禁用

默认情况下所有 Mojo 安全检查都已启用。它们可以在 `pyproject.toml` 中单独禁用：

```toml
[tool.basilisk.mojo-safety]
ownership = true           # BSK-E0030–E0035
immutability = true        # BSK-E0040–E0043
no-implicit-coercion = true  # BSK-E0060–E0063
```

要在项目范围内禁用特定规则：

```toml
[tool.basilisk]
rules.disable = ["BSK-E0042"]  # 不要求数据类上的 frozen=True
```

要禁用特定路径：

```toml
[tool.basilisk.per-path-overrides."legacy/**"]
rules.ignore = ["BSK-E0040", "BSK-E0041"]
```
