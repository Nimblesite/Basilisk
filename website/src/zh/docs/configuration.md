---
layout: layouts/docs.njk
title: 配置参考
description: Basilisk pyproject.toml 配置选项的完整参考。严重性覆盖、每路径规则、内联抑制和 Ruff 集成。
keywords: basilisk, 配置, pyproject.toml, 设置
lang: zh
dateModified: 2026-07-14
---

# 配置参考

`pyproject.toml` 中的 `[tool.basilisk]` 是唯一的配置来源。对于每个被检查的
文件，Basilisk 从该文件所在目录向上遍历，读取每一个带有 `[tool.basilisk]`
表的祖先 `pyproject.toml`。这些表会累积合并：同一个键在多个文件中都有设置
时，**最近**的文件生效——子目录中的 `pyproject.toml` 只是细化根配置，
绝不会将其整体替换。

> **正在从 `basilisk.json` 迁移？** 旧版根目录 `basilisk.json` 文件已不再
> 被读取。请将其键翻译为 `[tool.basilisk]`（驼峰式 → 短横线式，例如
> `typeshedPath` → `typeshed-path`），然后删除该文件。配置编辑器会将遗留的
> `basilisk.json` 报告为被忽略的遮蔽来源。

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
**规范：** [`STUBRES-CUSTOM-TYPESHED`](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED)

指向包含 typeshed 标准库存根的自定义或修改版本的目录路径。设置后，该目录将成为**标准库类型的规范来源**——[typing 规范的导入解析顺序](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)中的第 3 步，该规范指出类型检查器"SHOULD use this as the canonical source for standard-library types in this step"（应将其用作此步骤中标准库类型的规范来源）。Basilisk 优先针对它解析标准库模块，而不是捆绑的 typeshed；目录中缺失的标准库模块将继续进入后续的解析步骤。

该目录必须遵循 typeshed 的布局——标准库存根位于顶层 `stdlib/` 子目录下，因此 Basilisk 将每个模块解析为 `<typeshed-path>/stdlib/<module>.pyi`。[python/typeshed](https://github.com/python/typeshed) 仓库的克隆，或任何你已用作 Pyright 的 [`typeshedPath`](https://microsoft.github.io/pyright/#/configuration) 或 mypy 的 [`custom_typeshed_dir`](https://mypy.readthedocs.io/en/stable/config_file.html) 的目录，都可原样使用。相对路径相对于项目根目录解析。

使用此选项可针对替代标准库进行类型检查——例如 MicroPython 的 [`micropython-stdlib-stubs`](https://github.com/Josverl/micropython-stubs)，其 `os`、`time` 和 `machine` 签名与 CPython 不同。从自定义 typeshed 解析的符号在悬停时会带有 `(custom typeshed)` 标记——区别于捆绑 typeshed 的 `(typeshed)`——因此你可以确认覆盖已生效，并确保 MicroPython 的签名绝不会被误报为 CPython 的签名。

`stub-paths` *前置*额外的存根目录；`typeshed-path` 则*整体替换*捆绑的标准库。二者相互独立，可以组合使用。有关分步演练，请参见下方的"如何使用自定义 typeshed"一节。

### `include`

**类型：** `string[]`
**默认值：** `["."]`（当前目录）
**示例：** `["src/", "tests/"]`

要分析的目录或文件。相对于项目根目录的普通路径——与 `exclude` 不同，`include` **不**接受 glob 模式。只处理 `.py` 文件。

### `exclude`

**类型：** `string[]`
**默认值：**

```toml
exclude = [
    "__pycache__", "node_modules", "venv", ".venv", "env", ".env",
    ".tox", ".mypy_cache", ".ruff_cache", ".pytest_cache",
    "site-packages", "__pypackages__", "build", "dist", ".eggs",
    "bundled", "_vendored",
]
```

**示例：** `["py-gen", "**/generated/**", "*.pb.py"]`

用于跳过路径的 gitignore 风格 glob 模式。隐藏目录（名称以 `.` 开头）无论此设置如何都始终会被跳过。

> **`exclude` 会_替换_默认值——而不是在其基础上追加。** 一旦你设置了 `exclude`，上面的内置列表便不再生效。请将你仍需要的任何默认值与自己的模式一并重新列出，否则它们会被重新分析。

模式语法，针对每个相对于项目根目录的路径进行匹配：

| 模式 | 匹配 |
| --- | --- |
| `build` | **裸名称**——在**任意**深度处的该目录或文件段 |
| `**/generated/**` | `**`——零个或多个目录段（任意位置的 `generated` 目录） |
| `*.pb.py` | `*`——单个段内任意长度的字符（文件 glob） |
| `gen?.py` | `?`——段内恰好一个字符 |
| `src/generated` | **锚定**模式（包含 `/`）——该路径或其任意祖先目录，及其子树 |

Basilisk 在发现文件的所有场景中都遵循相同的模式：LSP 工作区扫描、`basilisk check` / `fix` / `adopt` CLI，以及你打开或编辑文件时编辑器的逐文件检查——因此在 CLI 上被排除的文件在编辑器中同样静默。规范语义请参见架构规范中的 `CHKARCH-CONFIG-EXCLUDE`。

---

## 如何使用自定义 typeshed

`typeshed-path` 会将 Basilisk 捆绑的标准库存根替换为你自己的副本。当你面向标准库与 CPython 不同的替代 Python（MicroPython、打过补丁的 CPython、厂商 SDK），或需要比 Basilisk 发行版内置版本更新或分叉的 typeshed 时，就可以使用它。

### 1. 按 typeshed 的方式组织目录

将 `typeshed-path` 指向 typeshed 布局目录的**根**。标准库存根必须位于顶层 `stdlib/` 子目录下，与 [python/typeshed](https://github.com/python/typeshed) 仓库完全一致——Basilisk 将每个模块解析为 `<typeshed-path>/stdlib/<module>.pyi`：

```
vendor/typeshed/
└── stdlib/
    ├── os.pyi
    ├── time.pyi
    └── ...
```

任何你已用作 Pyright 的 [`typeshedPath`](https://microsoft.github.io/pyright/#/configuration) 或 mypy 的 [`custom_typeshed_dir`](https://mypy.readthedocs.io/en/stable/config_file.html) 的目录都采用同样的布局，因此可与 Basilisk 原样配合使用。

### 2a. 指向分叉或更新的 typeshed

克隆 typeshed 仓库（或你的分叉），然后将 `typeshed-path` 指向该克隆，并修补你需要的 `.pyi` 文件：

```sh
git clone https://github.com/python/typeshed vendor/typeshed
```

```toml
[tool.basilisk]
typeshed-path = "vendor/typeshed"
```

现在 Basilisk 会针对 `vendor/typeshed/stdlib/` 而不是其捆绑副本对标准库进行类型检查。

### 2b. 指向 MicroPython 的标准库

MicroPython 的标准库与 CPython 存在差异——`os`、`time` 和 `machine` 的签名各不相同。安装 [`micropython-stdlib-stubs`](https://github.com/Josverl/micropython-stubs)（一份带有 MicroPython 特定改动的 typeshed 布局标准库副本）并将其指向它：

```toml
[tool.basilisk]
python-version = "3.12"
typeshed-path = ".venv/lib/python3.12/site-packages/micropython_stdlib_stubs"
```

由于 `micropython-stdlib-stubs` 是**部分**标准库，它未包含的模块（例如开发板上并不存在的 `tkinter`）**不会**由捆绑的 CPython 存根来兜底——自定义 typeshed 是第 3 步的规范来源，因此该导入会被报告为无法解析。对于嵌入式目标而言，这才是诚实的结果。

### 3. 在活动项目文件中配置

`typeshed-path` 与所有其他设置一样位于 `[tool.basilisk]` 中——不存在第二种
拼写，也不存在第二个文件。请在管辖被检查文件的 `pyproject.toml`（带有
`[tool.basilisk]` 表的最近祖先）中设置它。编辑器不会保存第二份副本；
该项目配置文件始终是唯一来源。若你正在迁移旧版 `basilisk.json`，其驼峰式
键 `typeshedPath` 在这里写作 `typeshed-path`。

### 4. 确认已生效——悬停溯源

从自定义 typeshed 解析的符号在悬停时会带有 `(custom typeshed)` 标记，区别于捆绑 typeshed 的 `(typeshed)` 标记。将鼠标悬停在导入的标准库符号上：看到 `(custom typeshed)` 即可确认覆盖已生效，且该签名来自你的目录——MicroPython 的 `os.uname` 绝不会被误报为 CPython 的。

### `typeshed-path` 与 `stub-paths` 的区别

它们解决不同的问题，并且可以组合使用：

| | `stub-paths`（第 1 步） | `typeshed-path`（第 3 步） |
| --- | --- | --- |
| 作用 | 在搜索路径最前端*前置*额外的 `.pyi` 目录 | *整体替换*捆绑的标准库 typeshed |
| 范围 | 可遮蔽任意单个模块，无论标准库还是第三方 | 整个标准库的规范来源 |
| 典型用途 | 修补某个损坏的存根；为内部库提供存根 | 面向替代或分叉的标准库（MicroPython、更新的 typeshed） |
| 优先级 | 更高——`stub-paths` 中的模块仍会遮蔽自定义 typeshed | 位于 `stub-paths` 之下、已安装包之上 |

---

## 规则选择与全局严重性

未配置时，Basilisk 启用完整的核心 PEP 规则集；带 `basilisk` 标签的扩展
规则默认关闭。不存在 basic/standard/strict 模式，也不存在规则族开关。
配置编辑器中的 **Strict 预设**只是一次性配方：它把每条实时规则的原生
严重性显式写入活动配置文件，之后每条规则仍可独立调整。

任何非 `disabled` 的显式严重性都会启用相应的可选规则：

```toml
[tool.basilisk.rules]
"BSK-E0001" = "error"
"BSK-W0011" = "warning"
"BSK-E0152" = "error"
"BSK-I0060" = "info"
```

可用值为 `"error"`、`"warning"`、`"info"` 与 `"disabled"`。标签只用于
浏览和批量选择，不会充当隐藏开关。所有项目策略都保存在这一个活动配置
文件中。

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

使用标准的 `# type: ignore` 拼写。指定 Basilisk 规则代码可让抑制保持精确：

```python
result: Any = get_legacy_value()  # type: ignore[returns_compatibility]
```

裸 ignore（以及其他检查器的代码）按 PEP 484 兼容语义抑制整行：

```python
data = unsafe_cast(value)  # type: ignore
```

同一语法也可以降低严重性而不隐藏诊断：

```python
value = legacy_call()  # type: warning[returns_compatibility]
value = legacy_call()  # type: info[returns_compatibility]
value = legacy_call()  # type: disabled[returns_compatibility]
```

文件级指令必须位于文件顶部并单独成行：

```python
# basilisk: relaxed
# basilisk: file-warning[returns_compatibility]
# basilisk: file-disabled[imports_unresolved]
```

`suppressions` 标签下的四条审计规则默认全部关闭：`BSK-I0060`（有效且
精确）、`BSK-W0061`（有效但宽泛）、`BSK-W0062`（未使用）与
`BSK-E0063`（格式错误）。它们和其他规则一样可分别配置为 error、warning、
info 或 disabled。格式错误的指令可以被审计，但绝不会真正抑制诊断。

---

## 配置发现

Basilisk 按被检查的文件逐一发现配置：从该文件所在目录**向上**遍历，每一个
带有 `[tool.basilisk]` 表的祖先 `pyproject.toml` 都会参与，且这些表会累积
合并——同一个键在多个文件中都有设置时，**最近**的文件生效。子表未设置的键
继续沿用祖先的值，因此嵌套的 `pyproject.toml` 只是细化根配置，绝不会将
根配置整体清除。

旧版根目录 `basilisk.json` **绝不**会被读取。若该文件仍然存在，配置编辑器
会将其报告为被忽略的遮蔽来源；请将其键翻译为 `[tool.basilisk]` 后删除该
文件。

如果没有任何祖先 `pyproject.toml` 带有 `[tool.basilisk]` 表，Basilisk 使用默认值：启用**核心 PEP 符合性规则集**（额外的 Basilisk 规则保持可选），`python-version = "3.12"`，检查当前目录。

---

## 可视化配置编辑器

VS Code 中的标签优先配置编辑器直接读取 LSP 的实时规则目录。它按来源、
PEP 分类与策略标签浏览规则，支持逐规则及批量严重性、路径覆盖、精确的
前后变更预览和分页出现位置。

严格优先采用流程由三个显式操作组成：应用 Strict 或 Maximum 预设；运行
限定到当前根目录的安全修复；刷新后用 `WithoutSafeFix` 检查剩余债务，再
显式选择 disabled、较低严重性或更窄的路径覆盖。抑制审计预设只会把
`suppressions` 标签展开为普通规则条目，不会写入模式标志。

采用债务也保存在同一个配置文件的精确文件 `per-path-overrides` 中，并带
`adoption = true` 来源标记；不会创建 `.basilisk/adoptions.toml` 或任何隐藏
状态。VSIX 本身不解析或写入配置，所有操作均由可复用的 LSP API 完成。

![Basilisk 的标签优先 VS Code 配置编辑器，展示实时规则分类和逐规则严重性控制](/assets/images/vscode-configuration-editor.png)
