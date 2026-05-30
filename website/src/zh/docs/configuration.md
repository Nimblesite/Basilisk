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

这就是您所需要的全部。Basilisk 从当前目录查找 Python 文件并应用所有规则。

## 完整配置示例

```toml
[tool.basilisk]
python-version = "3.12"
python-platform = "All"
stub-paths = ["stubs/"]
include = ["src/", "tests/"]
exclude = ["**/migrations/**", "**/generated/**"]

[tool.basilisk.per-path-overrides."legacy/**"]
disabled = ["BSK-E0011"]
rules."BSK-E0010" = "warning"
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

## `[tool.basilisk.per-path-overrides."<glob>"]`

将不同的设置应用于特定路径。glob 与相对于项目根目录的文件路径匹配。

```toml
[tool.basilisk.per-path-overrides."legacy/**"]
# 为匹配的文件完全禁用规则
disabled = ["BSK-E0011"]

[tool.basilisk.per-path-overrides."tests/**"]
# 或降低规则的严重性而不是完全禁用
rules."BSK-E0011" = "warning"
```

### `disabled`

**类型：** `string[]`
**示例：** `["BSK-E0011", "BSK-E0001"]`

为匹配此 glob 的文件完全禁用的规则代码。

### `rules`

**类型：** 规则代码 → 严重性的表
**严重性：** `"error"`、`"warning"`、`"info"`、`"disabled"`
**示例：** `rules."BSK-E0011" = "warning"`

为匹配的文件覆盖特定规则的严重性。尽可能选择降低或禁用单个规则，而不是放宽大范围的检查。

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

> **注意：** 没有原因注释的内联抑制本身会被标记为警告。原因不检查内容——它只需要存在。

---

## 配置发现

Basilisk 从被检查文件的目录开始搜索 `pyproject.toml`，向上遍历到文件系统根目录。使用第一个包含 `[tool.basilisk]` 部分的 `pyproject.toml`。

如果未找到配置文件，Basilisk 使用默认值：所有规则启用，`python-version = "3.12"`，检查当前目录。
