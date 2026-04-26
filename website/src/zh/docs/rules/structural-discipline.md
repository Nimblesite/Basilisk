---
layout: layouts/docs.njk
title: 结构纪律 — E0050–E0054
description: 强制执行类结构、slots 和密封类的规则。
keywords: basilisk, 结构纪律, slots, final, BSK-E0050, BSK-E0054
lang: zh
---

# 结构纪律 — E0050–E0054

← [不可变性](/zh/docs/rules/immutability/) | 下一个：[强制转换安全](/zh/docs/rules/coercion-safety/) →

---

### BSK-E0050 — 类型化类上的动态属性

在类主体中未声明的类实例上设置属性。

```python
class Config:
    host: str

c = Config()
c.port = 8080  # 错误——port 不是声明的属性
```

---

### BSK-E0051 — 缺少 `__init__`

类定义实例属性但没有 `__init__` 方法。

---

### BSK-E0052 — 缺少资源清理

打开资源（文件、连接）的类没有 `__exit__` 方法或 `close()` 实现。

---

### BSK-E0053 — 缺少 `__slots__`

类可以使用 `__slots__` 提高内存效率，但没有使用。（在严格模式下为警告。）

---

### BSK-E0054 — 密封类被子类化

用 `typing` 的 `@final` 装饰的类被子类化。
