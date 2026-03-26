---
layout: layouts/docs.njk
title: 可选安全 — E0070–E0073
description: 防止对 Optional 值进行不安全访问的规则。
keywords: basilisk, optional, none, nullable, BSK-E0070, BSK-E0073
lang: zh
eleventyNavigation:
  key: Optional Safety
  parent: Rules
  order: 7
---

# 可选安全 — E0070–E0073

← [强制转换安全](/zh/docs/rules/coercion-safety/) | 下一个：[未使用代码](/zh/docs/rules/unused-code/) →

---

### BSK-E0070 — 对 `Optional` 的属性访问

对可能为 `None` 的值访问方法或属性。

```python
def get_name(user: User | None) -> str:
    return user.name  # 错误——user 可能是 None
```

---

### BSK-E0071 — `Optional` 传递到期望非 `Optional` 的地方

---

### BSK-E0072 — `Optional` 在声明非 `Optional` 的地方返回

---

### BSK-E0073 — 与 `None` 比较而不缩窄
