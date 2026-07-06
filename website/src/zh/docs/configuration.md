---
layout: layouts/docs.njk
title: 配置参考
description: Basilisk pyproject.toml 配置选项的完整参考。严重性覆盖、每路径规则、内联抑制和 Ruff 集成。
keywords: basilisk, 配置, pyproject.toml, 设置
lang: zh
---

# 配置参考

Basilisk 通过 `pyproject.toml` 进行配置。所有设置都在 `[tool.basilisk]` 下。

## 最小配置

```toml
[tool.basilisk]
python-version = "3.12"
```

这就是您所需要的全部。Basilisk 从当前目录查找 Python 文件并应用其默认规则集——**核心 PEP 符合性规则**。超出规范的额外 Basilisk 规则是可选的；当你想要比规范更严格的检查时再启用它们。

## 完整配置示例

```toml
[tool.basilisk]
python-version = "3.12"
python-platform = "All"
stub-paths = ["stubs/"]
typeshed-path = "typeshed-micropython"   # 可选：替换捆绑的标准库 typeshed
include = ["src/", "tests/"]
exclude = ["**/migrations/**", "**/generated/**"]

[tool.basilisk.per-path-overrides."legacy/**"]
disabled = ["returns_compatibility"]
rules."imports_unresolved" = "warning"
```

---

## `[tool.basilisk]`

### `python-version`

**类型：** `string`
**默认值：** 从 PATH 上的解释器自动检测，如果未找到则为 `"3.12"`
**示例：** `"3.12"`

用于类型检查的目标 Python 版本。影响哪些 PEP 和类型功能可用。支持 `"3.9"` 到 `"3.14"` 的版本。

### `python-platform`

**类型：** `"Linux" | "macOS" | "Windows" | "All"`
**默认值：** `"All"`

目标平台。影响平台特定的类型存根和条件导入。

### `stub-paths`

**类型：** `string[]`
**默认值：** `[]`
**示例：** `["stubs/", "typings/"]`

用于搜索 `.pyi` 存根文件的额外目录。它们位于导入搜索路径的**最前端**——[typing 规范的导入解析顺序](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)中的第 1 步——因此可以修补或遮蔽任何后续模块，无论是标准库还是第三方。对于内部库的自定义存根很有用。

### `typeshed-path`

**类型：** `string`
**默认值：** _（未设置——使用捆绑的 typeshed）_
**示例：** `"typeshed-micropython"`

指向包含 typeshed 标准库存根的自定义或修改版本的目录路径。设置后，该目录将成为**标准库类型的规范来源**——[typing 规范的导入解析顺序](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)中的第 3 步，该规范指出类型检查器"SHOULD use this as the canonical source for standard-library types in this step"（应将其用作此步骤中标准库类型的规范来源）。Basilisk 优先针对它解析标准库模块，而不是捆绑的 typeshed；目录中缺失的标准库模块将继续进入后续的解析步骤。

使用此选项可针对替代标准库进行类型检查——例如 MicroPython 的 [`micropython-stdlib-stubs`](https://github.com/Josverl/micropython-stubs)，其 `os`、`time` 和 `machine` 签名与 CPython 不同。相对路径相对于项目根目录解析。

`stub-paths` *前置*额外的存根目录；`typeshed-path` 则*整体替换*捆绑的标准库。二者相互独立，可以组合使用。

### `include`

**类型：** `string[]`
**默认值：** `["."]`（当前目录）
**示例：** `["src/", "tests/"]`

要分析的目录或文件。接受路径和 glob 模式。只处理 `.py` 文件。

### `exclude`

**类型：** `string[]`
**默认值：** `["**/node_modules/**", "**/__pycache__/**"]`
**示例：** `["**/migrations/**", "**/generated/**"]`

要从分析中排除的 Glob 模式。在 `include` 之后应用。使用 `**` 进行递归匹配。

---

## `[tool.basilisk.per-path-overrides."<glob>"]`

将不同的设置应用于特定路径。glob 与相对于项目根目录的文件路径匹配。

```toml
[tool.basilisk.per-path-overrides."legacy/**"]
# 为匹配的文件完全禁用规则
disabled = ["returns_compatibility"]

[tool.basilisk.per-path-overrides."tests/**"]
# 或降低规则的严重性而不是完全禁用
rules."returns_compatibility" = "warning"
```

### `disabled`

**类型：** `string[]`
**示例：** `["returns_compatibility", "BSK-E0001"]`

为匹配此 glob 的文件完全禁用的规则代码。

### `rules`

**类型：** 规则代码 → 严重性的表
**严重性：** `"error"`、`"warning"`、`"info"`、`"disabled"`
**示例：** `rules."returns_compatibility" = "warning"`

为匹配的文件覆盖特定规则的严重性。尽可能选择降低或禁用单个规则，而不是放宽大范围的检查。

---

## 内联抑制

要在特定行上抑制诊断，请添加带有规则代码和强制原因的注释：

```python
result: Any = get_legacy_value()  # basilisk: ignore[returns_compatibility] -- no stub available, tracked in #123
```

要抑制一行上的所有诊断：

```python
data = unsafe_cast(value)  # basilisk: ignore -- third-party code, cannot type
```

要抑制文件中的所有诊断，请在顶部添加：

```python
# basilisk: relaxed
```

> **注意：** 没有原因注释的内联抑制本身会被标记为警告。原因不检查内容——它只需要存在。

---

## 配置发现

Basilisk 从被检查文件的目录开始搜索 `pyproject.toml`，向上遍历到文件系统根目录。使用第一个包含 `[tool.basilisk]` 部分的 `pyproject.toml`。

如果未找到配置文件，Basilisk 使用默认值：启用**核心 PEP 符合性规则集**（额外的 Basilisk 规则保持可选），`python-version = "3.12"`，检查当前目录。
