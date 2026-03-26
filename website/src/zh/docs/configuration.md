---
layout: layouts/docs.njk
title: 配置参考
description: Basilisk pyproject.toml 配置选项的完整参考。
keywords: basilisk, 配置, pyproject.toml, 设置
lang: zh
eleventyNavigation:
  key: Configuration
  order: 4
---

# 配置参考

Basilisk 通过 `pyproject.toml` 进行配置。所有设置都在 `[tool.basilisk]` 下。

## 最小配置

```toml
[tool.basilisk]
python-version = "3.12"
```

这就是您所需要的全部。Basilisk 从当前目录查找 Python 文件并应用所有规则。

## 完整配置示例

```toml
[tool.basilisk]
python-version = "3.12"
python-platform = "All"
stub-paths = ["stubs/"]
include = ["src/", "tests/"]
exclude = ["**/migrations/**", "**/generated/**"]

[tool.basilisk.mojo-safety]
ownership = true
immutability = true
no-implicit-coercion = true

[tool.basilisk.migration]
enabled = true
started = "2025-06-01"
enforce_after = "2025-12-01"

[tool.basilisk.per-path-overrides."legacy/**"]
strict = false
deadline = "2026-12-31"
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

用于搜索 `.pyi` 存根文件的额外目录。在捆绑的 typeshed 存根之前按顺序搜索。对于内部库的自定义存根很有用。

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

## `[tool.basilisk.mojo-safety]`

控制 Mojo 启发的安全分析。请参阅 [Mojo 风格安全](/zh/docs/mojo-safety/) 以获取完整文档。

### `ownership`

**类型：** `boolean`
**默认值：** `true`

启用所有权分析：`Borrowed`、`InOut`、`Owned` 注解检查。
标记 `Borrowed` 参数的变异（BSK-E0030）和移动后使用（BSK-E0031）。

### `immutability`

**类型：** `boolean`
**默认值：** `true`

强制执行未标注 `InOut` 的参数的不可变性。
标记未标注参数的变异（BSK-E0040）。

### `no-implicit-coercion`

**类型：** `boolean`
**默认值：** `true`

标记隐式类型强制转换：`int` → `float`、`bool` → `int`、`bytes` → `str`。
需要显式转换函数（BSK-E0060 到 BSK-E0063）。

---

## `[tool.basilisk.migration]`

迁移模式在定义的时间段内将选定的错误软化为警告，使现有代码库更容易采用 Basilisk。

### `enabled`

**类型：** `boolean`
**默认值：** `false`

启用迁移模式。当为 `true` 时，错误在 `enforce_after` 之前报告为警告。

### `started`

**类型：** `string`（ISO 日期）
**示例：** `"2025-06-01"`

信息性：迁移开始的时间。用于进度报告。

### `enforce_after`

**类型：** `string`（ISO 日期）
**示例：** `"2025-12-01"`

在此日期之后，迁移模式中的所有警告再次变为错误。随着截止日期临近，Basilisk 会警告您。

---

## `[tool.basilisk.per-path-overrides."<glob>"]`

将不同的设置应用于特定路径。glob 与相对于项目根目录的文件路径匹配。

```toml
[tool.basilisk.per-path-overrides."legacy/**"]
strict = false
deadline = "2026-12-31"

[tool.basilisk.per-path-overrides."tests/**"]
# 测试可以更自由地使用 Any
rules.ignore = ["BSK-E0011"]
```

### `strict`

**类型：** `boolean`
**默认值：** `true`

设置为 `false` 以禁用匹配文件的严格模式。所有错误变为警告。

### `deadline`

**类型：** `string`（ISO 日期）

`strict = false` 不再生效并强制执行错误的日期。随着截止日期临近，Basilisk 会打印提醒。

### `rules.ignore`

**类型：** `string[]`
**示例：** `["BSK-E0011", "BSK-W0080"]`

在匹配文件中忽略的特定规则。尽可能选择狭窄的忽略而不是 `strict = false`。

---

## 内联抑制

要在特定行上抑制诊断，请添加带有规则代码和强制原因的注释：

```python
result: Any = get_legacy_value()  # basilisk: ignore[BSK-E0011] -- no stub available, tracked in #123
```

要抑制一行上的所有诊断：

```python
data = unsafe_cast(value)  # basilisk: ignore -- third-party code, cannot type
```

要抑制文件中的所有诊断，请在顶部添加：

```python
# basilisk: relaxed
```

> **注意：** 没有原因注释的内联抑制本身被标记为 BSK-W0095。原因不检查内容——它只需要存在。

---

## 配置发现

Basilisk 从被检查文件的目录开始搜索 `pyproject.toml`，向上遍历到文件系统根目录。使用第一个包含 `[tool.basilisk]` 部分的 `pyproject.toml`。

如果未找到配置文件，Basilisk 使用默认值：所有规则启用，`python-version = "3.12"`，检查当前目录。
