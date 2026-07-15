---
layout: layouts/docs.njk
title: 类型安全 — E0010–E0029
description: Basilisk 的类型安全规则 imports_unresolved 至 E0029，捕获参数与返回类型不匹配、赋值不兼容、不兼容的方法覆盖、不可哈希类型、非穷举模式匹配以及不健全的类型使用。
keywords: basilisk, 类型安全, 类型不匹配, calls_argument_type, returns_compatibility_2, classes_override
lang: zh
---

# 类型安全 — E0010–E0029

捕获类型不匹配、错误注解和不健全类型使用的规则。完整代码列表请参见[规则总览](/zh/docs/rules/)。

← [缺失注解](/zh/docs/rules/missing-annotations/) | [所有规则](/zh/docs/rules/) →

---

### imports_unresolved — 无法解析的导入 (Unresolved import)

`import` 语句引用了在已配置的搜索路径中无法找到的模块。

```python
# 错误——找不到该模块
from legacy_module import process_data

# 修复——安装对应包或将其加入工作区，
# 也可通过 stub-paths 指向对应的 .pyi 文件
```

---

### returns_compatibility — 显式 `Any` 注解 / 返回类型不匹配 (Explicit `Any` / return type mismatch)

此代码覆盖两种检查：显式 `Any` 注解会屏蔽类型检查，必须附带理由；返回值与声明的返回类型明显不兼容也会触发此报告。

```python
from typing import Any

# 警告——显式 `Any` 必须注明原因
def handle(
    data: Any,  # basilisk: ignore[returns_compatibility] -- awaiting stubs for third-party SDK
) -> bool:
    ...

# 错误——int 字面量不能赋值给 `str`
def name() -> str:
    return 42
```

---

### calls_argument_type — 参数类型不匹配

用错误类型的参数调用函数。

```python
def greet(name: str) -> str:
    return f"Hello, {name}"

# 错误——int 不是 str
greet(42)
```

---

### returns_compatibility_2 — 返回类型不匹配

返回值的类型与声明的返回类型不匹配。

```python
def get_count() -> int:
    return "many"  # 错误——str 不是 int
```

---

### assignment_compatibility — 赋值不兼容

将错误类型的值赋给注解变量。

```python
count: int = 0
count = "zero"  # 错误——str 不是 int
```

---

### callables_annotation — 无效的类型参数数量

泛型类型使用了错误数量的类型参数。

```python
x: dict[str]        # 错误——dict 需要 2 个类型参数
y: dict[str, int]   # 正确
```

---

### classes_override — 不兼容的方法覆盖

子类中的覆盖方法具有不兼容的签名。

```python
class Base:
    def process(self, data: str) -> str: ...

class Child(Base):
    def process(self, data: int) -> str:  # 错误——参数类型已更改
        ...
```

---

### classes_override_2 — 不兼容的变量覆盖

类变量在子类中以不兼容的类型被覆盖。

---

### names_undefined — 未定义变量

使用了在当前范围中未定义的名称。

---

### names_unbound — 未绑定变量

在所有代码路径中赋值之前使用了变量。

```python
def check(flag: bool) -> str:
    if flag:
        result = "yes"
    return result  # 错误——result 可能未绑定
```

---

### overloads_definitions — 缺少重载实现

`@overload` 组没有具体的实现函数。

---

### overloads_consistency — 重叠的重载

两个 `@overload` 签名从调用者的角度看无法区分。

---

### dict_key_hashable — 哈希上下文中的不可哈希类型

可变类型（如 `list`）用作字典键或集合元素。

```python
d: dict[list[int], str] = {}  # 错误——list 不可哈希
```

---

### match_exhaustiveness — 非穷举模式匹配

`match` 语句没有涵盖匹配类型的所有可能情况。

```python
def classify(x: int | str) -> str:
    match x:
        case int():
            return "number"
    # 错误——未处理 str 情况
```

---

### annotations_typeexpr — 无效类型形式

在类型位置使用了非有效类型的值，例如将数字字面量用作注解。

```python
x: 42 = 0   # 错误——`42` 不是类型
y: int = 0  # 正确
```

---

### BSK-0025 — 缺少 `@override` 装饰器

覆盖父类方法的方法缺少 `@override` 装饰器（PEP 698）。

```python
class Child(Base):
    def process(self) -> str:  # 错误——缺少 @override
        ...
```

---

### generics_basic — `TypeVar` 只有一个约束

声明仅有一个约束的 `TypeVar` 没有意义——约束需要两个或更多。

```python
from typing import TypeVar

T = TypeVar("T", int)        # 错误——只有一个约束
U = TypeVar("U", int, str)   # 正确——两个或更多
```

---

### generics_base_class — `Generic[...]` 基类中重复的 `TypeVar`

同一 `TypeVar` 在 `Generic[...]`（或 `Protocol[...]`）基类中出现了不止一次。

```python
from typing import Generic, TypeVar

T = TypeVar("T")
class Box(Generic[T, T]):  # 错误——`T` 出现两次
    ...
```

---

### typeddicts_class_syntax — 在 `TypedDict` 类中定义方法

`TypedDict` 类仅描述数据结构，不允许定义方法。

```python
from typing import TypedDict

class Movie(TypedDict):
    title: str
    def play(self) -> None:  # 错误——TypedDict 中不允许定义方法
        ...
```
