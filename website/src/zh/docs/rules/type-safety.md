---
layout: layouts/docs.njk
title: 类型安全 — E0010–E0025
description: 捕获类型不匹配、错误注解和不健全类型使用的规则。
keywords: basilisk, 类型安全, 类型不匹配, BSK-E0012, BSK-E0013, BSK-E0016
lang: zh
---

# 类型安全 — E0010–E0025

捕获类型不匹配、错误注解和不健全类型使用的规则。

← [缺失注解](/zh/docs/rules/missing-annotations/) | 下一个：[所有权安全](/zh/docs/rules/ownership-safety/) →

---

### BSK-E0010 — 从未类型化模块导入

从没有类型存根的模块导入会为所有导入的名称产生隐式 `Any`。

```python
# 错误——legacy_module 没有存根
from legacy_module import process_data

# 正确——提供或生成存根
# basilisk stubs generate legacy_module
```

---

### BSK-E0011 — 隐式 `Any`

`Any` 必须用抑制原因明确注解。不允许来自推断的隐式 `Any`。

```python
# 错误
def handle(data: Any) -> bool:
    ...

# 正确（带理由）
def handle(
    data: Any,  # basilisk: ignore[BSK-E0011] -- awaiting stubs for third-party SDK
) -> bool:
    ...
```

---

### BSK-E0012 — 参数类型不匹配

用错误类型的参数调用函数。

```python
def greet(name: str) -> str:
    return f"Hello, {name}"

# 错误——int 不是 str
greet(42)
```

---

### BSK-E0013 — 返回类型不匹配

返回值的类型与声明的返回类型不匹配。

```python
def get_count() -> int:
    return "many"  # 错误——str 不是 int
```

---

### BSK-E0014 — 赋值不兼容

将错误类型的值赋给注解变量。

```python
count: int = 0
count = "zero"  # 错误——str 不是 int
```

---

### BSK-E0015 — 无效的类型参数数量

泛型类型使用了错误数量的类型参数。

```python
x: dict[str]        # 错误——dict 需要 2 个类型参数
y: dict[str, int]   # 正确
```

---

### BSK-E0016 — 不兼容的方法覆盖

子类中的覆盖方法具有不兼容的签名。

```python
class Base:
    def process(self, data: str) -> str: ...

class Child(Base):
    def process(self, data: int) -> str:  # 错误——参数类型已更改
        ...
```

---

### BSK-E0017 — 不兼容的变量覆盖

类变量在子类中以不兼容的类型被覆盖。

---

### BSK-E0018 — 未定义变量

使用了在当前范围中未定义的名称。

---

### BSK-E0019 — 未绑定变量

在所有代码路径中赋值之前使用了变量。

```python
def check(flag: bool) -> str:
    if flag:
        result = "yes"
    return result  # 错误——result 可能未绑定
```

---

### BSK-E0020 — 缺少重载实现

`@overload` 组没有具体的实现函数。

---

### BSK-E0021 — 重叠的重载

两个 `@overload` 签名从调用者的角度看无法区分。

---

### BSK-E0022 — 哈希上下文中的不可哈希类型

可变类型（如 `list`）用作字典键或集合元素。

```python
d: dict[list[int], str] = {}  # 错误——list 不可哈希
```

---

### BSK-E0023 — 非穷举模式匹配

`match` 语句没有涵盖匹配类型的所有可能情况。

```python
def classify(x: int | str) -> str:
    match x:
        case int():
            return "number"
    # 错误——未处理 str 情况
```

---

### BSK-E0024 — 无效类型形式

类型注解使用了无效的语法。

```python
x: int | = None  # 错误——格式错误的联合
```

---

### BSK-E0025 — 缺少 `@override` 装饰器

覆盖父类方法的方法缺少 `@override` 装饰器（PEP 698）。

```python
class Child(Base):
    def process(self) -> str:  # 错误——缺少 @override
        ...
```
