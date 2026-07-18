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
# 标准库 typeshed 会自动克隆并刷新；如需调整：
typeshed-commit = "83c2518a9e6abbda0c44592c3483de459198f887"  # 可选：固定并冻结某个提交
typeshed-cache-path = ".cache/typeshed"                       # 可选：克隆存放位置
typeshed-refresh-interval = "24h"                             # 可选：未固定时的刷新 TTL（默认）
# typeshed-path = "typeshed-micropython"                      # 可选：提供你自己的树，禁用自动克隆
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

用于类型检查的目标 Python 版本（例如 `"3.11"`）。Basilisk 将其作为求值输入——应用于 typeshed 的 `stdlib/VERSIONS`，以及类型规范要求检查器理解的 `sys.version_info` / `sys.platform` 条件判断（[规范：版本与平台检查，`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst)）。它在单个 typeshed 快照*内*进行选择，绝不选择某个 typeshed 提交。

### `python-platform`

**类型：** `"Linux" | "macOS" | "Windows" | "All"`
**默认值：** `"All"`

目标平台。影响平台特定的类型存根和条件导入。

### `stub-paths`

**类型：** `string[]`
**默认值：** `[]`
**示例：** `["stubs/", "typings/"]`

用于搜索 `.pyi` 存根文件的额外目录。它们位于导入搜索路径的**最前端**——[typing 规范的导入解析顺序](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)中的第 1 步——因此可以修补或遮蔽任何后续模块，无论是标准库还是第三方。对于内部库的自定义存根很有用。

### 标准库 typeshed 的自动获取

**规范：** [`STUBRES-TYPESHED`](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED)

默认情况下，Basilisk 针对磁盘上 [python/typeshed](https://github.com/python/typeshed) 的**实时克隆**解析标准库。LSP 在启动时于后台获取它，CLI 则在首次检查之前获取它，二者都存入操作系统缓存目录；随后标准库模块针对其真实的 `stdlib/*.pyi` 及其版本门控的 `stdlib/VERSIONS` 模块集解析，`types-<distribution>` 映射则从其 `stubs/<DIST>/` 树读取。无需任何配置——自动克隆即开箱即用的默认行为。

包中随附一份小巧的**捆绑基线**，作为离线首日兜底。它仅携带标准库模块名称集（typeshed 的 `VERSIONS` 格式）与 `types-<distribution>` 映射——绝不含标准库 `.pyi` 主体——因此在克隆落地之前 `import os` 绝不会闪现为无法解析。克隆成功后会**整体覆盖**基线；基线**仅**在从未获取过任何克隆时（离线首次运行，或初次克隆失败）才被参考。新鲜度在每次运行时都会报告，暗淡而低调：已克隆且为最新的缓存打印暗绿色 `typeshed <short-sha> · <date>`；已存在但无法刷新（失败或离线，早于 TTL）的克隆打印暗琥珀色 `typeshed <short-sha> · <date> — stale (refresh failed/offline); connect to refresh`；而真正回退到捆绑基线的运行则打印暗琥珀色 `typeshed: bundled baseline <date> — not updated; connect to refresh`。克隆或刷新失败绝不致命：Basilisk 保留上一次成功的缓存并针对它静默解析（即上面的 *stale* 行），仅当没有任何缓存时才回退到基线并发出警告。LSP 在其**服务信息树**中呈现同一状态——获取期间显示旋转指示，随后是已解析的缓存路径与新鲜度。

**确定性。** 设置了 `typeshed-commit` 时，缓存会检出到该精确的 SHA 并冻结——绝不运行任何更新检查。未固定时，缓存跟踪 `python/typeshed@main`，并每隔 `typeshed-refresh-interval`（默认 `24h`）重新检查一次。每次获取与每次刷新都以 `git fetch`、`git clean -x -f -d` 和 `git reset --hard` 收尾，因此工作树与上游提交逐字节一致，任何本地修改的文件都不会残留。克隆由纯 Rust 的 `gix` 库驱动，因此 Basilisk 始终是单一原生二进制，既无外部 `git` 二进制，也无 Python 运行时依赖。

#### `typeshed-commit`

**类型：** `string`
**默认值：** _（未设置——克隆跟踪 `python/typeshed@main`）_
**示例：** `"83c2518a9e6abbda0c44592c3483de459198f887"`

将自动克隆固定到某个精确的提交 SHA 并**冻结**它：绝不运行任何 TTL 轮询，因此每次检出都完全可复现。

#### `typeshed-cache-path`

**类型：** `string`
**默认值：** _（操作系统缓存目录）_
**示例：** `".cache/typeshed"`

重定位**自动克隆的存放位置**。它只移动自动克隆——并不关闭克隆（那是 `typeshed-path` 的作用）。可视化配置编辑器将其呈现为**文件夹选择器**。

#### `typeshed-refresh-interval`

**类型：** `string`
**默认值：** `"24h"`
**示例：** `"6h"`

未固定的克隆每隔多久重新检查 `python/typeshed@main` 是否有更新。当 `typeshed-commit` 固定了检出时此项被忽略。

### `typeshed-path`

**类型：** `string`
**默认值：** _（未设置——标准库针对自动克隆的 typeshed 缓存解析）_
**示例：** `"typeshed-micropython"`
**规范：** [`STUBRES-CUSTOM-TYPESHED`](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED)

指向包含 typeshed 标准库存根的自定义或修改版本的目录路径。设置后，该目录将成为**标准库类型的规范来源**——[typing 规范的导入解析顺序](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)中的第 3 步，该规范指出类型检查器"SHOULD use this as the canonical source for standard-library types in this step"（应将其用作此步骤中标准库类型的规范来源）。设置它会**完全禁用自动克隆**：Basilisk 针对你的目录解析标准库模块，对于该目录提供的模块绝不再参考运行时克隆或捆绑基线。目录中缺失的标准库模块将继续进入后续的解析步骤。这也是让 Basilisk 指向磁盘上已有的 typeshed 树、而非让它自行克隆的方式。

该目录必须遵循 typeshed 的布局——标准库存根位于顶层 `stdlib/` 子目录下，因此 Basilisk 将每个模块解析为 `<typeshed-path>/stdlib/<module>.pyi`。[python/typeshed](https://github.com/python/typeshed) 仓库的克隆，或任何你已用作 Pyright 的 [`typeshedPath`](https://microsoft.github.io/pyright/#/configuration) 或 mypy 的 [`custom_typeshed_dir`](https://mypy.readthedocs.io/en/stable/config_file.html) 的目录，都可原样使用。相对路径相对于项目根目录解析。可视化配置编辑器将其呈现为**文件夹选择器**。

使用此选项可针对替代标准库进行类型检查——例如 MicroPython 的 [`micropython-stdlib-stubs`](https://github.com/Josverl/micropython-stubs)，其 `os`、`time` 和 `machine` 签名与 CPython 不同。从你的目录解析的符号在悬停时会带有 `(custom typeshed)` 标记——区别于自动克隆 typeshed 的 `(typeshed)`——因此你可以确认覆盖已生效，并知晓 MicroPython 的签名绝不会被误报为 CPython 的。

`typeshed-path` 与 `typeshed-cache-path` 不同：`typeshed-cache-path` 只重定位*自动克隆的存放位置*；`typeshed-path` 提供你*自己的*树并关闭克隆。而 `stub-paths` *前置*额外的存根目录，`typeshed-path` 则*整体替换*自动克隆的标准库 typeshed——二者相互独立，可以组合使用。有关分步演练，请参见下方的"如何使用自定义 typeshed"一节。

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

`typeshed-path` 会将自动克隆的标准库 typeshed 替换为你自己的副本，并关闭克隆。当你面向标准库与 CPython 不同的替代 Python（MicroPython、打过补丁的 CPython、厂商 SDK），当你想要一个分叉或手动修补的 typeshed 而非上游 `python/typeshed` 检出，或当你必须让 Basilisk 指向磁盘上已有的 typeshed 树而非让它自行克隆时，就可以使用它。（若只需在上游 typeshed 上保持更新鲜或固定它，请改用自动克隆的键 `typeshed-commit` / `typeshed-refresh-interval`——无需 `typeshed-path`。）

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

### 2a. 指向分叉或手动修补的 typeshed

克隆 typeshed 仓库（或你的分叉），然后将 `typeshed-path` 指向该克隆，并修补你需要的 `.pyi` 文件：

```sh
git clone https://github.com/python/typeshed vendor/typeshed
```

```toml
[tool.basilisk]
typeshed-path = "vendor/typeshed"
```

现在 Basilisk 会针对 `vendor/typeshed/stdlib/` 而不是自动克隆的缓存对标准库进行类型检查，并停止管理自己的自动克隆——这份检出由你来更新。（若只需更新鲜的上游 typeshed 而非分叉，请不要设置 `typeshed-path`，让自动克隆跟踪 `main`，或用 `typeshed-commit` 固定它。）

### 2b. 指向 MicroPython 的标准库

MicroPython 的标准库与 CPython 存在差异——`os`、`time` 和 `machine` 的签名各不相同。安装 [`micropython-stdlib-stubs`](https://github.com/Josverl/micropython-stubs)（一份带有 MicroPython 特定改动的 typeshed 布局标准库副本）并将其指向它：

```toml
[tool.basilisk]
python-version = "3.12"
typeshed-path = ".venv/lib/python3.12/site-packages/micropython_stdlib_stubs"
```

由于 `micropython-stdlib-stubs` 是**部分**标准库，它未包含的模块（例如开发板上并不存在的 `tkinter`）**不会**由自动克隆的 CPython typeshed 来兜底——设置 `typeshed-path` 已关闭克隆，而自定义 typeshed 是第 3 步的规范来源，因此该导入会被报告为无法解析。对于嵌入式目标而言，这才是诚实的结果。

### 3. 在活动项目文件中配置

`typeshed-path` 与所有其他设置一样位于 `[tool.basilisk]` 中——不存在第二种
拼写，也不存在第二个文件。请在管辖被检查文件的 `pyproject.toml`（带有
`[tool.basilisk]` 表的最近祖先）中设置它。编辑器不会保存第二份副本；
该项目配置文件始终是唯一来源。若你正在迁移旧版 `basilisk.json`，其驼峰式
键 `typeshedPath` 在这里写作 `typeshed-path`。

### 4. 确认已生效——悬停溯源

从自定义 typeshed 解析的符号在悬停时会带有 `(custom typeshed)` 标记，区别于自动克隆 typeshed 的 `(typeshed)` 标记。将鼠标悬停在导入的标准库符号上：看到 `(custom typeshed)` 即可确认覆盖已生效，且该签名来自你的目录——MicroPython 的 `os.uname` 绝不会被误报为 CPython 的。

### `typeshed-path` 与 `stub-paths` 的区别

它们解决不同的问题，并且可以组合使用：

| | `stub-paths`（第 1 步） | `typeshed-path`（第 3 步） |
| --- | --- | --- |
| 作用 | 在搜索路径最前端*前置*额外的 `.pyi` 目录 | *整体替换*自动克隆的标准库 typeshed，并禁用克隆 |
| 范围 | 可遮蔽任意单个模块，无论标准库还是第三方 | 整个标准库的规范来源 |
| 典型用途 | 修补某个损坏的存根；为内部库提供存根 | 面向替代或分叉的标准库（MicroPython、打过补丁的分叉），或复用磁盘上已有的 typeshed 树 |
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
"BSK-0001" = "error"
"BSK-0011" = "warning"
"BSK-0152" = "error"
"BSK-0060" = "info"
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
**示例：** `["returns_compatibility", "BSK-0001"]`

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

`suppressions` 标签下的四条审计规则默认全部关闭：`BSK-0060`（有效且
精确）、`BSK-0061`（有效但宽泛）、`BSK-0062`（未使用）与
`BSK-0063`（格式错误）。它们和其他规则一样可分别配置为 error、warning、
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
