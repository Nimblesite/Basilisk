---
layout: layouts/docs.njk
title: 缺失注解 — E0001–E0009
description: Basilisk 的缺失注解规则 BSK-E0001 至 E0009，标记缺少类型信息的代码——包括参数与返回类型注解、无法解析的变量类型、`*args` 与 `**kwargs` 以及类属性注解。
keywords: basilisk, 缺失注解, 类型注解, BSK-E0001, BSK-E0002
lang: zh
---

# 缺失注解 — E0001–E0009

标记缺少类型信息的代码的规则。

← [所有规则](/zh/docs/rules/) | 下一个：[类型安全](/zh/docs/rules/type-safety/) →

---

### BSK-E0001 — 缺少参数类型注解

每个函数参数都必须有明确的类型注解。

```python
# 错误
def process(data):
    return data.upper()

# 正确
def process(data: str) -> str:
    return data.upper()
```

---

### BSK-E0002 — 缺少返回类型注解

每个函数都必须声明其返回类型。

```python
# 错误
def get_user(user_id: int):
    return {"id": user_id}

# 正确
def get_user(user_id: int) -> dict[str, int]:
    return {"id": user_id}
```

---

### BSK-E0003 — 无法解析变量类型

变量被赋予一个无法推断类型的值。需要明确的注解。

```python
# 错误——无法确定 result 的类型
result = some_dynamic_function()

# 正确
result: list[str] = some_dynamic_function()
```

---

### BSK-E0004 — 缺少 `*args` 或 `**kwargs` 注解

可变参数必须被注解。

```python
# 错误
def log(*args, **kwargs):
    print(args, kwargs)

# 正确
def log(*args: str, **kwargs: int) -> None:
    print(args, kwargs)
```

---

### BSK-E0005 — 缺少类属性注解

类级别的属性必须明确注解。

```python
# 错误
class Config:
    host = "localhost"
    port = 8080

# 正确
class Config:
    host: str = "localhost"
    port: int = 8080
```
