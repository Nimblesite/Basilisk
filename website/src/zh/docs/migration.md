---
layout: layouts/docs.njk
title: 迁移指南
description: 如何从 Pyright 或 mypy 逐步迁移到 Basilisk，包括配置导入、每路径覆盖和截止日期。
keywords: 迁移到basilisk, 从pyright, 从mypy, python类型检查器迁移
lang: zh
---

# 迁移指南

Basilisk 包含帮助现有代码库逐步采用它的工具。在大型代码库上完整迁移可能需要数周时间；以下工具使这个过程可行。

---

<h2 id="from-pyright">从 Pyright 迁移</h2>

### 第 1 步——导入您的配置

如果您有 `pyrightconfig.json`，Basilisk 可以读取它并生成等效的 `[tool.basilisk]` 配置：

```bash
basilisk migrate --from pyright
```

这从当前目录读取 `pyrightconfig.json` 并将等效设置追加到您的 `pyproject.toml`。

**示例输入（`pyrightconfig.json`）：**

```json
{
  "include": ["src"],
  "exclude": ["**/migrations/**"],
  "typeCheckingMode": "strict",
  "pythonVersion": "3.12"
}
```

**生成的输出（`pyproject.toml`）：**

```toml
[tool.basilisk]
python-version = "3.12"
include = ["src/"]
exclude = ["**/migrations/**"]
```

### 第 2 步——运行 Basilisk

```bash
basilisk check src/
```

如果您已经将 Pyright 与 `typeCheckingMode = "strict"` 一起使用，您会看到相对较少的新错误。最常见的添加：

| Pyright 规则 | Basilisk 等效 |
|---|---|
| `reportMissingTypeArgument` | BSK-E0015 |
| `reportReturnType` | BSK-E0002 |
| `reportArgumentType` | BSK-E0012 |
| `reportAttributeAccessIssue` | BSK-E0050 |
| `reportUndefinedVariable` | BSK-E0018 |
| `reportOperatorIssue` | BSK-E0014 |

Basilisk 添加了即使在严格模式下 Pyright 也不标记的错误：
- **BSK-E0001** — 缺少参数类型注解（严格强制执行）
- **BSK-E0002** — 缺少返回类型注解（严格强制执行）
- **BSK-E0011** — 参数和返回位置中的显式 `Any`——需要说明理由
- **BSK-E0023** — 非穷举 `match` 语句（缺少情况）
- **BSK-E0025** — 覆盖方法缺少 `@override` 装饰器

### 第 3 步——处理 `# type: ignore` 注释

Pyright 使用 `# type: ignore` 进行内联抑制。Basilisk 需要不同的格式，带有强制原因：

```python
# Pyright
x = get_value()  # type: ignore[reportArgumentType]

# Basilisk
x = get_value()  # basilisk: ignore[BSK-E0012] -- third-party API mismatch, tracked in #456
```

Basilisk 将标记裸 `# type: ignore` 注释，因为它不识别它们。迁移工具建议正确的 `# basilisk: ignore[CODE] -- reason` 格式。

### 第 4 步——通过迁移模式逐步执行

对于大型代码库，启用迁移模式首先将错误分阶段作为警告：

```toml
[tool.basilisk]
python-version = "3.12"
include = ["src/"]

[tool.basilisk.migration]
enabled = true
started = "2025-06-01"
enforce_after = "2025-12-01"
```

在迁移模式下，所有类型错误报告为警告。在 `enforce_after` 之后，它们再次变为错误。

### 严格模式差异

如果您将 Pyright 与 `typeCheckingMode = "basic"` 或 `"standard"` 一起使用，您会看到 Basilisk 报告的错误明显更多——因为这些模式允许未类型化的代码。这是预期的，也正是关键所在。使用带截止日期的每路径覆盖，逐目录地分阶段执行。

---

<h2 id="from-mypy">从 mypy 迁移</h2>

### 第 1 步——导入您的配置

如果您有 `mypy.ini` 或带有 `[mypy]` 部分的 `setup.cfg`：

```bash
basilisk migrate --from mypy
```

**示例输入（`mypy.ini`）：**

```ini
[mypy]
python_version = 3.12
strict = True
ignore_missing_imports = True
exclude = migrations/
```

**生成的输出：**

```toml
[tool.basilisk]
python-version = "3.12"
exclude = ["**/migrations/**"]
# 注意：ignore_missing_imports → BSK-E0010 仍然激活；
# 对没有存根的特定包使用每路径覆盖
```

### 第 2 步——mypy `--strict` 标志是子集

mypy 的 `--strict` 启用一组特定的标志。Basilisk 强制执行所有这些标志以及更多。从 `mypy --strict` 迁移时，预期：

- **来自 BSK-E0011 的更多错误** — mypy 在某些 Basilisk 不允许的位置允许 `Any`
- **来自 BSK-E0023 的更多错误** — mypy 不检查的非穷举 `match` 语句
- **来自 BSK-E0025 的更多错误** — mypy 不要求的缺失 `@override` 装饰器

### 第 3 步——mypy 插件

mypy 插件（Django、SQLAlchemy、Pydantic）不能与 Basilisk 一起使用。Basilisk 的 WASM 插件系统计划在第 5 阶段。在此之前，您可能会看到框架特定模式的错误，这些错误以前被 mypy 插件抑制。

等待 WASM 插件期间的解决方法：

```toml
[tool.basilisk.per-path-overrides."models/**"]
rules.ignore = ["BSK-E0011"]  # Django 模型字段广泛使用 Any
```

### 第 4 步——更新内联抑制

mypy 使用 `# type: ignore[error-code]`。Basilisk 需要 `# basilisk: ignore[BSK-EXXXX] -- reason`。

迁移工具生成代码库中每个 `# type: ignore` 注释的列表，以及建议的 Basilisk 等效和添加原因的提醒。

### 第 5 步——删除守护进程

mypy 的守护进程（`dmypy`）之所以需要，是因为 mypy 很慢。Basilisk 的增量计算是内置的。替换您的 mypy CI 步骤：

```yaml
# 旧（mypy）
- run: dmypy run -- src/

# 新（Basilisk）
- run: basilisk check src/
```

---

## 一般迁移建议

### 从缺失注解开始

BSK-E0001（缺失参数注解）和 BSK-E0002（缺失返回注解）往往会级联。首先修复它们通常会自动解决下游错误。

最初只使用这些规则运行：

```bash
basilisk check --only E0001,E0002 src/
```

### 对遗留目录使用每路径覆盖

不要试图一次性类型化所有内容。使用带有截止日期的每路径覆盖：

```toml
[tool.basilisk.per-path-overrides."legacy/**"]
strict = false
deadline = "2026-06-01"

[tool.basilisk.per-path-overrides."new_modules/**"]
# 新代码立即完全严格
```

### 使用 `basilisk stats` 跟踪进度

```bash
basilisk stats src/
```

输出：

```
Type coverage report — src/
  Files:     142 total, 98 fully typed, 44 partially typed
  Functions: 1,847 total, 1,203 typed (65%)
  Classes:   318 total, 241 typed (76%)

  Top offenders (most errors):
    src/legacy/data_pipeline.py   — 47 errors
    src/utils/converters.py       — 23 errors
    src/models/legacy_models.py   — 19 errors
```

每周使用此报告跟踪迁移进度。

### 优先考虑关键路径

在实践中，导入最多的模块中的错误是最高优先级的，因为它们也可能在调用者中导致类型错误。首先修复最深层的共享工具。
