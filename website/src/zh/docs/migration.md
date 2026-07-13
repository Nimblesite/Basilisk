---
layout: layouts/docs.njk
title: 从 Pyright 或 mypy 迁移到 Basilisk
description: 使用逐规则严重性、路径覆盖和显式采用例外，将现有 Python 项目逐步迁移到 Basilisk。
keywords: 迁移到basilisk, 从pyright, 从mypy, python类型检查器迁移
lang: zh
dateModified: 2026-07-14
---

# 迁移指南

Basilisk 的默认配置启用完整的核心 PEP 规则集。Basilisk 自有的注解、
`@override`、样式、依赖和存根规则默认关闭，需要项目显式启用。迁移的
核心是先确定目标规则，再只为当前无法解决的债务记录小范围例外。

> **当前工具状态：** `basilisk migrate`、`basilisk stats` 和
> `basilisk check --only` 尚未实现。请不要在脚本中使用这些命令。下面
> 只使用现有配置与命令；可视化配置编辑器的设计见本文末尾。

## 1. 添加项目配置

```toml
[tool.basilisk]
python-version = "3.12"
include = ["src/", "tests/"]
exclude = [
  "__pycache__", ".venv", "site-packages", "build", "dist",
  "**/migrations/**", "**/generated/**",
]
```

设置 `exclude` 会替换内置列表，因此请保留仍然需要的默认项。旧版根目录
`basilisk.json` 已不再被读取；若该文件仍然存在，请将其键翻译为
`[tool.basilisk]`（驼峰式 → 短横线式，例如 `typeshedPath` →
`typeshed-path`）后删除该文件——配置编辑器会将遗留的 `basilisk.json`
报告为被忽略的遮蔽来源。

## 2. 选择目标规则

核心 PEP 规则无需开关。Basilisk 扩展规则通过明确的逐规则严重性启用：

```toml
[tool.basilisk.rules]
"BSK-E0001" = "error"
"BSK-E0025" = "error"
"BSK-W0011" = "warning"
"BSK-E0152" = "error"
```

规则按实时的来源、PEP 类别和描述性标签组织。Basilisk 没有
basic/standard/strict 运行模式或规则族开关；策略由配置文件中的逐规则严重性组成。

## 3. 先应用安全修复

```bash
basilisk check src/ tests/
basilisk fix src/ tests/
```

`basilisk fix` 默认只应用确定性的 Safe 修复。随后重新检查，剩余项目
才是需要显式决策的债务。

## 4. 只降低当前无法修复的规则

优先保留可见的 warning/info，而不是完全隐藏规则：

```toml
[tool.basilisk.rules]
"returns_compatibility" = "warning"
"imports_unresolved" = "info"
```

支持 `error`、`warning`、`info` 和 `disabled`。显式的非 disabled 严重性
也会启用一个默认关闭的 Basilisk 规则。

将遗留债务限制在路径中：

```toml
[tool.basilisk.per-path-overrides."legacy/**"]
rules."returns_compatibility" = "warning"
rules."imports_unresolved" = "info"
```

项目级 `disabled` 也会隐藏以后新增的问题；若债务只存在于旧文件，
路径或精确文件例外更安全。

## 5. 保留并审计内联忽略

Basilisk 使用标准语法：

```python
value = legacy_api()  # type: ignore[returns_compatibility]
value = legacy_api()  # type: warning[calls_argument_type]
```

裸 `# type: ignore` 会忽略该行的所有诊断。外部检查器的错误码无法映射
时，也按 PEP 484 的宽泛忽略处理；应尽可能替换成真实的 Basilisk 规则码。

配置编辑器中的 `suppressions` 规则族会把有效特定、宽泛、未使用和格式
错误的指令变成可搜索诊断。该规则族默认关闭，每条审计规则都可独立设为
error、warning、info 或 disabled。

## 从 Pyright 迁移

请复制实际需要的设置，不要翻译模式名称：

| Pyright | Basilisk |
|---|---|
| `pythonVersion` | `[tool.basilisk].python-version` |
| `include` / `exclude` | `[tool.basilisk].include` / `exclude` |
| `stubPath` | `[tool.basilisk].stub-paths` |
| `typeshedPath` | `[tool.basilisk].typeshed-path` |
| `report…` 严重性 | `[tool.basilisk.rules]."RULE_CODE"` |
| 执行环境例外 | 语义相符时使用 `per-path-overrides` |

不要机械映射 `typeCheckingMode`；Basilisk 将策略保存为显式规则严重性，
而不是模式。

## 从 mypy 迁移

| mypy | Basilisk |
|---|---|
| `python_version` | `[tool.basilisk].python-version` |
| `exclude` | `[tool.basilisk].exclude` |
| `mypy_path` | 包含存根时使用 `[tool.basilisk].stub-paths` |
| `custom_typeshed_dir` | `[tool.basilisk].typeshed-path` |
| 每模块放宽 | 适用时使用每路径覆盖 |
| `# type: ignore[code]` | 保留语法并换成 Basilisk 规则码 |

mypy 插件不能直接加载到 Basilisk。对于框架特有债务，应使用有针对性的
路径或规则例外，而不是全局关闭无关检查。

## 可视化严格优先迁移

VS Code 配置编辑器采用标签优先界面，并由可复用 LSP API 驱动：

1. 按权威标签浏览实时规则目录；
2. 预览 LSP 提供的 **Strict preset**，把所有规则按各自原生严重性启用；
3. 先运行 Safe 修复；
4. 按标签、规则、文件和可修复性审查剩余债务；
5. 只降低选中的规则，或为受影响文件记录精确例外；
6. 检查准确影响后，通过版本令牌应用；
7. 持续查找采用例外和可选的忽略审计诊断。

Preset 是一次性的配置配方，不是运行模式。应用 Strict 后，LSP 会把展开
后的逐规则严重性写入当前有效配置文件。精确文件采用同样写入该文件的
`per-path-overrides`，不会创建 `.basilisk/adoptions.toml` 或隐藏状态。

参见权威的
[规范](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/LSP-CONFIGURATION-EDITOR-SPEC.md)
与
[实施计划](https://github.com/Nimblesite/Basilisk/blob/main/docs/plans/LSP-CONFIGURATION-EDITOR-PLAN.md)。
