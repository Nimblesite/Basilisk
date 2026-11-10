---
layout: layouts/docs.njk
title: 从 Pyright 或 mypy 迁移到 Basilisk
description: 使用逐规则严重性、路径覆盖和显式采用例外，将现有 Python 项目逐步迁移到 Basilisk。
keywords: 迁移到basilisk, 从pyright, 从mypy, python类型检查器迁移
lang: zh
dateModified: 2026-07-14
---

# 迁移指南

Basilisk 的默认配置启用当前已注册的所有 PEP 标签规则。这只是配置属性，
并不表示实现已经完整或符合规范；在[完整性修复](/zh/docs/conformance/)期间，
实际符合性水平暂时未知。Basilisk 自有的注解、
`@override`、样式、依赖和存根规则默认关闭，需要项目显式启用。迁移的
核心是先确定目标规则，再只为当前无法解决的债务记录小范围例外。

> **当前工具状态：** `basilisk migrate`、`basilisk stats` 和
> `basilisk check --only` 尚未实现。请不要在脚本中使用这些命令。下面
> 只使用现有配置与命令；可视化配置编辑器的设计见本文末尾。

## 1. 添加项目配置

先配置你拥有的路径。只有要覆盖项目或解释器证据时才添加 `python-version`；
固定提交的 typing 规范定义版本判断，而不是默认目标
（[`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst)）。

```toml
[tool.basilisk]
include = ["src/", "tests/"]
exclude = [
  "__pycache__", ".venv", "site-packages", "build", "dist",
  "**/migrations/**", "**/generated/**",
]
```

设置 `exclude` 会替换内置列表，因此请保留仍然需要的默认项。旧版根目录
`basilisk.json` 不会被任何组件读取；若该文件仍然存在，它也完全无效——没有
任何工具会加载它，配置编辑器也不会以任何方式呈现它。请将其键翻译为
`[tool.basilisk]`（驼峰式 → 短横线式，例如 `typeshedPath` →
`typeshed-path`），把逐规则与逐标签的严重性移入 `[tool.basilisk.rules]`
与 `[tool.basilisk.rule-tags]`，然后删除该文件。

## 2. 选择目标规则

核心 PEP 规则无需开关。Basilisk 扩展规则通过明确的逐规则严重性启用：

```toml
[tool.basilisk.rule-tags]
"basilisk" = "error"     # 一行启用所有可选自有规则

[tool.basilisk.rules]
"BSK-0011" = "warning"   # 再对个别规则作出高于标签条目的定级
```

规则按实时的来源、PEP 类别和描述性标签组织。Basilisk 没有
basic/standard/strict 运行模式；策略就是配置文件中持久化的规则条目与
标签条目。参见[规则：两个扁平映射](/zh/docs/configuration/#规则-两个扁平映射)。

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

将遗留债务限制在路径中：在那个文件夹放置带 `[tool.basilisk]` 表的
`pyproject.toml`——最近作出决定的表按规则胜出，树的其余部分保持严格定级：

```toml
# legacy/pyproject.toml
[tool.basilisk.rules]
"returns_compatibility" = "warning"
"imports_unresolved" = "info"
```

规则配置中没有 glob 路径模式或逐路径覆盖表——作用域策略始终是放在
对应文件夹里的配置文件。项目级 `disabled` 也会隐藏以后新增的问题
（且对 PEP 规则无效——它们只能定级）；若债务只存在于旧代码，
`basilisk adopt` 是更安全的工具。

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
| 执行环境例外 | 在对应文件夹放置带 `[tool.basilisk]` 表的 `pyproject.toml` |

不要机械映射 `typeCheckingMode`；Basilisk 将策略保存为显式规则严重性，
而不是模式。

## 从 mypy 迁移

| mypy | Basilisk |
|---|---|
| `python_version` | `[tool.basilisk].python-version` |
| `exclude` | `[tool.basilisk].exclude` |
| `mypy_path` | 包含存根时使用 `[tool.basilisk].stub-paths` |
| `custom_typeshed_dir` | `[tool.basilisk].typeshed-path` |
| 每模块放宽 | 例外基于源码路径时，使用文件夹作用域的 `pyproject.toml` |
| `# type: ignore[code]` | 保留语法并换成 Basilisk 规则码 |

mypy 插件不能直接加载到 Basilisk。对于框架特有债务，应使用有针对性的
路径或规则例外，而不是全局关闭无关检查。

## 可视化严格优先迁移

VS Code 配置编辑器采用标签优先界面，并由可复用 LSP API 驱动：

1. 按权威标签浏览实时规则目录；
2. 用一条 `rule-tags` 条目（例如 `"basilisk" = "error"`）启用整组规则，
   并在应用前预览效果；
3. 先运行 Safe 修复（独立的根作用域 LSP 操作）；
4. 按标签和规则审查剩余债务；
5. 显式降低选中的规则，或用 `basilisk adopt` 记录债务；
6. 持续跟踪例外与可选的抑制审计诊断，直至清零。

采用债务保存为同一个活动配置文件中普通的 warning 严重性规则条目，
不会创建第二个配置文件或持久模式。

参见权威的
[规范](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/LSP-CONFIGURATION-EDITOR-SPEC.md)
与
[实施计划](https://github.com/Nimblesite/Basilisk/blob/main/docs/plans/LSP-CONFIGURATION-EDITOR-PLAN.md)。
