---
layout: layouts/docs.njk
title: 配置参考
description: Basilisk pyproject.toml 配置选项的完整参考。规则与标签严重性、typeshed 来源固定、内联抑制以及按文件夹作用域的配置。
keywords: basilisk, 配置, pyproject.toml, 设置
lang: zh
dateModified: 2026-07-21
---

# 配置参考

`pyproject.toml` 中的 `[tool.basilisk]` 是唯一的配置来源。对于每个被检查的
文件，Basilisk 从该文件所在目录向上遍历，访问每一个带有 `[tool.basilisk]`
表的祖先 `pyproject.toml`。**不带**该表的 `pyproject.toml` 不参与配置，
也不会终止遍历。

被访问的表如何组合：

- **规则严重性从不合并。** 对每条规则，*最近*作出决定的表直接胜出——见
  [严重性解析](#严重性解析)。
- **非规则设置**（路径、版本、typeshed 键）按键解析：最近设置了该键的
  文件生效，更近的文件未设置的键继续沿用祖先的值。`stub-paths` 是唯一的
  追加型键——条目追加并去重。

> **正在从 `basilisk.json` 迁移？** 旧版根目录 `basilisk.json` 文件不会被
> 任何组件读取——它完全无效，配置编辑器也不会以任何方式呈现它。请将其键
> 翻译为 `[tool.basilisk]`（驼峰式 → 短横线式，例如 `typeshedPath` →
> `typeshed-path`），把逐规则与逐标签的严重性移入
> `[tool.basilisk.rules]` 与 `[tool.basilisk.rule-tags]`，然后删除该文件。

## 零配置

Basilisk 完全不需要配置文件。当遍历路径上任何地方都没有 `[tool.basilisk]`
表时：

- 每条**核心 PEP 符合性规则**以 `error` 严重性运行；Basilisk 自有的可选
  规则保持关闭——这正是 `basilisk check` 每次运行时的行为。
- 从当前目录发现文件。
- 目标 Python 版本从项目文件解析：`.python-version`，然后是
  `[project].requires-python` 下界，然后是 `uv.lock` 的
  `requires-python` 下界。
- 标准库存根来自编译进二进制的
  [python/typeshed](https://github.com/python/typeshed) 内置快照——完全
  离线，在你固定某个提交之前会附带 `typeshed_source_unpinned`
  提示——见[标准库存根](#标准库存根-typeshed)。

> **在编辑器中，这一状态会被一次性写入种子配置——CLI 从不写配置，但 LSP 会。**
> 当某个工作区根目录的遍历找不到任何 `[tool.basilisk]` 表时，语言服务器会在
> 首次分析之前，把这两行"默认严格"的种子配置写入该根目录的 `pyproject.toml`
> （若项目还没有该文件则创建它）
> （[`LSPARCH-CONFIG-SEEDING`](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-SEEDING)）：
>
> ```toml
> [tool.basilisk.rule-tags]
> "basilisk" = "error"
> ```
>
> 因此在编辑器中，所有自有规则一开始就以 `error` 启用——它明明白白写在你的
> 文件里，是可以降级或删除的一行。种子只写入**一次**：遍历路径上任何
> `[tool.basilisk]` 表都会阻止它，包括你删除该条目后残留的空表。这正是
> "没有表"与"空表"唯一表现不同的地方。

## 完整配置示例

```toml
[tool.basilisk]
python-version = "3.12"          # 仅当 PEP 依赖版本时才被查询
python-platform = "All"          # 显式跨平台分析
stub-paths = ["stubs/"]          # 解析第 1 步：前置额外的 .pyi 存根目录
include = ["src/", "tests/"]
exclude = ["**/migrations/**", "**/generated/**"]
# typeshed-commit = "<完整 40 位提交 SHA>"  # 固定标准库存根来源
# typeshed-path = "vendor/typeshed"          # 或：你自己的标准库存根树

[tool.basilisk.rules]
"imports_unresolved" = "warning"   # PEP 规则降级——绝不禁用
"BSK-0050" = "error"               # 单条自有规则提升到其标签条目之上

[tool.basilisk.rule-tags]
"basilisk" = "error"               # 一行启用所有自有规则——严格模式
```

---

## 规则：两个扁平映射

规则配置是两个扁平映射，除此之外别无其他
（[`CHKARCH-CONFIG-MODEL`](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)）：
逐规则条目和标签条目。规则代码本身不携带严重性——代码是 `BSK-nnnn` 或
符合性 snake_case 名称（如 `imports_unresolved`）；只有配置条目携带
严重性。

### `[tool.basilisk.rules]`

显式的逐规则条目，`"<代码>" = "<严重性>"`：

```toml
[tool.basilisk.rules]
"imports_unresolved" = "warning"
"BSK-0050" = "error"
"BSK-0001" = "info"
```

可用值为 `"error"`、`"warning"`、`"info"` 与 `"disabled"`。对于可选规则，
任何非 disabled 的值同时也*选择*该规则——无需第二个开关。可在生成的
[规则参考](/zh/docs/rules/)中浏览所有代码。

### `[tool.basilisk.rule-tags]`

显式的分组条目，`"<标签>" = "<严重性>"`——一行即可为**携带该标签的每条
规则**定级：

```toml
[tool.basilisk.rule-tags]
"basilisk" = "error"       # 所有可选自有规则全部启用
"suppressions" = "warning" # 抑制审计规则族设为 warning
```

标签条目是写在文件里的真实配置——绝不是隐式模式或隐藏开关。规范标签
词汇表：

- **来源标签：** `pep`（默认运行的核心符合性规则）与 `basilisk`
  （可选的自有规则）。
- **PEP 分类**，与[符合性测试套件](https://github.com/python/typing/tree/main/conformance/tests)
  的文件命名一致：`aliases`、`annotations`、`callables`、`classes`、
  `constructors`、`dataclasses`、`directives`、`enums`、`exceptions`、
  `generics`、`historical`、`literals`、`namedtuples`、`narrowing`、
  `overloads`、`protocols`、`qualifiers`、`specialtypes`、`tuples`、
  `typeddicts`、`typeforms`。
- **描述性标签**（自有规则）：`style`、`redundancy`、`strictness`、
  `dependencies`、`imports`、`stubs`、`suppressions`。

[规则参考](/zh/docs/rules/)列出每条规则的标签；配置编辑器的标签操作
写入的正是这些 `rule-tags` 行。

### 严重性解析

按规则、按被检查文件——一次遍历，首个决定胜出：

1. 从文件所在文件夹向根目录遍历。**最近**对该规则作出决定的
   `[tool.basilisk]` 表直接胜出。
2. 在同一个表内，逐规则条目优先于标签条目；多个匹配的标签条目中，
   **最严格**的严重性胜出（`error` > `warning` > `info` > `disabled`）。
3. 没有任何表对该规则作出决定时：带 `pep` 标签的规则以 `error` 运行；
   其他所有规则处于禁用状态。

这就是完整的模型——没有继承的规则状态，没有优先级分数，表与表之间
也没有合并规则。

### PEP 规则只能定级，绝不禁用

`disabled` 永远不适用于带 `pep` 标签的规则。任何将 PEP 规则解析为
`disabled` 的配置——无论通过规则条目还是标签条目——都是**无效的**：
CLI 与编辑器会将其呈现为配置错误，且检查器无论如何都会保持该规则运行，
确保符合性诊断绝不会被静默丢失。要让 PEP 规则安静，请将其定级为
`"warning"` 或 `"info"`，用 `# type: ignore` 抑制特定行
（[见下文](#内联抑制)），或 `exclude` 相关路径。

### 将规则作用于目录树的一部分

规则配置中**没有** glob 路径模式、逐路径覆盖表或逐模块例外。要让某个
子树使用不同的规则配置，就在那个文件夹放置带 `[tool.basilisk]` 表的
`pyproject.toml`——最近作出决定的表按规则胜出：

```toml
# pyproject.toml（仓库根目录）
[tool.basilisk.rule-tags]
"basilisk" = "error"

# tests/pyproject.toml
[tool.basilisk.rules]
"BSK-0001" = "disabled"    # tests/ 下重新关闭这条可选规则
```

---

## `[tool.basilisk]` 设置

### `python-version`

**类型：** `string`，例如 `"3.12"`
**默认值：** _（未设置——从项目文件解析：`.python-version` → `[project].requires-python` 下界 → `uv.lock` 的 `requires-python` 下界）_

被检查代码的目标 Python 版本。Basilisk 没有规范的 Python 发行版本：只有当
[typing 规范](https://typing.python.org/en/latest/spec/index.html)、已接受的
PEP 或 Python 语言语义使结果依赖版本时，规则才会查询此版本——例如
[PEP 695](https://peps.python.org/pep-0695/) 的 `type X = ...` /
`class C[T]` 语法在目标低于 3.12 时会被拒绝，因为目标解释器根本无法解析
它。与版本无关的规则从不依据此值分支。

### `python-platform`

**类型：** `"Linux" | "macOS" | "Windows" | "All"`
**默认值：** _（未设置——向所选项目解释器查询其 `sys.platform`）_

平台相关存根与 `sys.platform` 窄化的目标平台。未设置时，Basilisk 探测项目
解释器并使用该具体平台；探测失败则平台保持未知——Basilisk 绝不凭空捏造
平台。显式的 `"All"` 保持跨平台交集语义。

上面四种写法是规范写法，但该值**不做校验**：除 `"macOS"` 外，`"Darwin"`
与 `"MacOS"` 同样被接受，小写的 `"windows"`/`"all"` 以及原始 `sys.platform`
值（`linux`、`darwin`、`win32`）也可识别。其他任何字符串都会被原样当作具体
平台名，因此拼写错误不会报错，只会得到一个没有任何存根匹配的平台。请坚持
使用上述四种规范写法。

### `stub-paths`

**类型：** `string[]`
**默认值：** `[]`
**示例：** `["stubs/", "typings/"]`

用于搜索 `.pyi` 存根文件的额外目录。它们位于导入搜索路径的**最前端**——
[typing 规范的导入解析顺序](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)
中的第 1 步——因此可以修补或遮蔽任何后续模块，无论是标准库还是第三方。
对于内部库的自定义存根很有用。在嵌套配置文件之间，这是唯一的追加型键：
更近的条目追加到继承的条目上（去重）。

### `include`

**类型：** `string[]`
**默认值：** _（未设置——扫描当前目录）_
**示例：** `["src/", "tests/"]`

CLI 未给出路径时扫描的根目录。普通路径——与 `exclude` 不同，`include`
**不**接受 glob 模式——相对于**扫描根目录**解析：`basilisk check` 下是当前
目录，编辑器中则是工作区根目录。它们*不*相对于声明它们的那个配置文件所在
目录解析，因此在祖先 `pyproject.toml` 中设置的 `include` 仍然相对于扫描起点
解析。显式的 CLI 路径会覆盖它；`exclude` 在 include 根目录内生效。LSP 遵循
相同的根目录，因此编辑器分析的正是 `basilisk check` 会分析的那些文件。

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

用于跳过路径的 gitignore 风格 glob 模式。隐藏目录（名称以 `.` 开头）
无论此设置如何都始终会被跳过，且编辑器的批量工作区扫描始终额外跳过上面
内置的 vendored/缓存目录名。

> **`exclude` 会_替换_默认值——而不是在其基础上追加。** 一旦你设置了
> `exclude`，上面的内置列表便不再作用于 CLI 的文件发现。请将你仍需要的
> 任何默认值与自己的模式一并重新列出。

模式语法，针对每个相对于项目根目录的路径进行匹配：

| 模式 | 匹配 |
| --- | --- |
| `build` | **裸名称**——在**任意**深度处的该目录或文件段 |
| `**/generated/**` | `**`——零个或多个目录段（任意位置的 `generated` 目录） |
| `*.pb.py` | `*`——单个段内任意长度的字符（文件 glob） |
| `gen?.py` | `?`——段内恰好一个字符 |
| `src/generated` | **锚定**模式（包含 `/`）——该路径或其任意祖先目录，及其子树 |

所有入口共享同一个规范匹配器——LSP 工作区扫描、`basilisk check` /
`fix` / `adopt` CLI，以及编辑器的逐文件检查——因此在 CLI 上被排除的路径
在编辑器中同样静默。规范语义请参见
[`CHKARCH-CONFIG-EXCLUDE`](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-EXCLUDE)。

### `narrow-attributes-across-calls`

**类型：** `bool`
**默认值：** `true`
**状态：** _仅解析，尚未被读取——目前设置它不会产生任何效果。_

为"属性窄化（`if x.attr is not None:` 守卫）在中间函数调用后仍然保持"预留。
属性窄化尚未实现，因此没有任何检查器路径会读取该键；接受它只是为了让现有
配置文件继续正常解析。预期的默认值是*实用*的行为：调用**可能**使属性失效，
但把每次调用都当作失效会让属性窄化在实践中毫无用处——将来 `false` 会选择
健全但严格的行为：任何调用都会丢弃属性窄化。参见
[`TYPEINF-NARROWING-ATTR-CALLS`](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING-ATTR-CALLS)。

---

## 标准库存根（typeshed）

标准库类型来自 [typeshed](https://github.com/python/typeshed) 存根——
[typing 规范的导入解析顺序](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)
中的第 3 步。Basilisk 恰好选择**一个**第 3 步来源：

| 模式 | 生效来源 |
| --- | --- |
| 自定义文件夹 | 你的 `typeshed-path` 目录，原样使用 |
| 固定提交 | `typeshed-commit` 指定的 SHA，离线对照磁盘上的存储库校验（该提交不在本机时失败关闭） |

解析**完全离线**：`basilisk check`、`basilisk analyze` 和 LSP 绝不下载任何
东西。固定提交只做一件事——校验磁盘上的 typeshed 树与该提交的 SHA 一致。
如果固定的提交尚未下载到本机，检查会以 `NO SOURCE` 错误硬失败（退出码
3），并给出恢复命令；绝不静默替换为其他来源。

typeshed 的字节只能通过显式下载操作到达机器，而这些操作完全位于检查器
之外：

- 配置编辑器的 **Download latest**（下载最新）按钮：下载当前的
  `python/typeshed@main` 提交，并把解析出的 SHA 写入你的
  `typeshed-commit` 固定项（同时清除任何 `typeshed-path`）；
- `basilisk typeshed download [--commit <sha>]`——不带 `--commit` 时与按钮
  行为相同；带 `--commit` 时将该已配置的精确提交物化到存储库，不写入任何
  配置。

每次下载在任何字节落入内容寻址存储库之前，都要通过安全、结构、许可证与
内容验证关卡；条目写入后不可变，之后的每次解析都会离线地将存储的树重新
哈希并对照该固定提交的提交对象。完整细节：
[`STUBRES-TYPESHED`](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED)。

内置快照被编译进二进制文件，因此完全无需网络也能获得标准库类型——在飞机上、
在防火墙后、在隔离网络的 CI 中都一样。它是提交
[`83c2518`](https://github.com/python/typeshed/tree/83c2518a9e6abbda0c44592c3483de459198f887/stdlib)
处**完整的 typeshed `stdlib/` `.pyi` 存根集合**（不含第三方 `stubs/`，也不含
typeshed 自身在 `stdlib/` 下的非存根文件）：752 个 `.pyi` 文件，外加
`stdlib/VERSIONS` 与 `LICENSE`，未压缩约 2.85 MB。未设置 `typeshed-commit`
时，内置提交即为生效的固定项，编辑器的 Server Info 面板会显示 `typeshed_source_unpinned`
提示；显式固定任意提交——包括内置的 `83c2518…`——即可清除该提示。

| 键 | 类型 | 默认值 | 含义 |
| --- | --- | --- | --- |
| `typeshed-commit` | 完整 40 位 SHA | _（未设置——内置提交，附 `typeshed_source_unpinned` 提示）_ | 磁盘上的树必须匹配的精确 `python/typeshed` 提交。固定后**失败关闭**——绝不静默替换为其他提交。缩写 SHA 会被拒绝。 |
| `typeshed-store-path` | 路径 | 操作系统缓存目录 | 经验证的内容寻址存储库根目录：`basilisk typeshed download` 写入这里，固定提交从这里解析。 |
| `typeshed-path` | 路径 | _（未设置）_ | 你自己的标准库存根树——完全取代存储库与内置快照。 |

这就是全部配置面：不存在任何下载策略键，`check` 与 `analyze` 也没有任何
与下载相关的命令行开关——下载绝不是检查运行的一部分。另请注意：
`basilisk stubs` 用于为**第三方**未加类型的包生成存根，与 typeshed 无关。

`typeshed-path` 与 `typeshed-commit` 是**同一个来源选择**：设置了其中任一
键的嵌套配置文件会将继承的选择作为整体替换，绝不会把一个文件的路径和
另一个文件的固定提交混在一起。

### `typeshed-path`

**类型：** `string`
**默认值：** _（未设置——按上文使用固定或内置提交）_
**示例：** `"vendor/typeshed"`
**规范：** [`STUBRES-CUSTOM-TYPESHED`](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED)

指向包含 typeshed 标准库存根的自定义或修改版本的目录路径。设置后，该目录
将成为**标准库类型的规范来源**——typing 规范指出类型检查器"SHOULD use
this as the canonical source for standard-library types in this step"
（应将其用作此步骤中标准库类型的规范来源）。目录中缺失的标准库模块将继续
进入后续的解析步骤——**不会**由存储库或内置的 typeshed 来兜底。

## 如何使用自定义 typeshed

`typeshed-path` 会将标准库存根替换为你自己的副本。当你面向标准库与
CPython 不同的替代 Python（MicroPython、打过补丁的 CPython、厂商 SDK），
或需要分叉的 typeshed 而非官方提交时（官方提交请改用固定
`typeshed-commit`），就可以使用它。

### 1. 按 typeshed 的方式组织目录

将 `typeshed-path` 指向 typeshed 布局目录的**根**。标准库存根必须位于顶层
`stdlib/` 子目录下，与 [python/typeshed](https://github.com/python/typeshed)
仓库完全一致——Basilisk 将每个模块解析为
`<typeshed-path>/stdlib/<module>.pyi`：

```
vendor/typeshed/
└── stdlib/
    ├── os.pyi
    ├── time.pyi
    └── ...
```

任何你已用作 Pyright 的
[`typeshedPath`](https://microsoft.github.io/pyright/#/configuration) 或
mypy 的
[`custom_typeshed_dir`](https://mypy.readthedocs.io/en/stable/config_file.html)
的目录都采用同样的布局，因此可与 Basilisk 原样配合使用。

### 2a. 指向分叉的 typeshed

克隆 typeshed 仓库（或你的分叉），然后将 `typeshed-path` 指向该克隆，
并修补你需要的 `.pyi` 文件：

```sh
git clone https://github.com/python/typeshed vendor/typeshed
```

```toml
[tool.basilisk]
typeshed-path = "vendor/typeshed"
```

现在 Basilisk 会针对 `vendor/typeshed/stdlib/` 对标准库进行类型检查。

### 2b. 指向 MicroPython 的标准库

MicroPython 的标准库与 CPython 存在差异——`os`、`time` 和 `machine` 的
签名各不相同。安装
[`micropython-stdlib-stubs`](https://github.com/Josverl/micropython-stubs)
（一份带有 MicroPython 特定改动的 typeshed 布局标准库副本）并指向它：

```toml
[tool.basilisk]
python-version = "3.12"
typeshed-path = ".venv/lib/python3.12/site-packages"
```

该 wheel 会把 `stdlib/` **直接解压到 `site-packages` 下**——并不存在
`micropython_stdlib_stubs/` 目录——因此 `typeshed-path` 应指向包含 `stdlib/`
的 `site-packages` 本身。若多指向一层，自定义 typeshed 会失败关闭并报
`custom typeshed source is unavailable`（退出码 3），因为自定义 typeshed
绝不回退。

由于 `micropython-stdlib-stubs` 是**部分**标准库，它未包含的模块（例如
开发板上并不存在的 `tkinter`）**不会**由 CPython 存根来兜底——自定义
typeshed 是第 3 步的规范来源，因此该导入会被报告为无法解析。对于嵌入式
目标而言，这才是诚实的结果。

### 3. 在活动项目文件中配置

`typeshed-path` 与所有其他设置一样位于 `[tool.basilisk]` 中——不存在第二种
拼写，也不存在第二个文件。请在管辖被检查文件的 `pyproject.toml`（带有
`[tool.basilisk]` 表的最近祖先）中设置它。编辑器不会保存第二份副本；
该项目配置文件始终是唯一来源。若你正在迁移旧版 `basilisk.json`，其驼峰式
键 `typeshedPath` 在这里写作 `typeshed-path`。

### 4. 确认已生效——悬停溯源

从自定义 typeshed 解析的符号在悬停时会带有 `(custom typeshed)` 标记，
区别于官方来源的 `(typeshed)` 标记。将鼠标悬停在导入的标准库符号上：
看到 `(custom typeshed)` 即可确认覆盖已生效，且该签名来自你的目录——
MicroPython 的 `os.uname` 绝不会被误报为 CPython 的。

### `typeshed-path` 与 `stub-paths` 的区别

它们解决不同的问题，并且可以组合使用：

| | `stub-paths`（第 1 步） | `typeshed-path`（第 3 步） |
| --- | --- | --- |
| 作用 | 在搜索路径最前端*前置*额外的 `.pyi` 目录 | *整体替换*标准库 typeshed |
| 范围 | 可遮蔽任意单个模块，无论标准库还是第三方 | 整个标准库的规范来源 |
| 典型用途 | 修补某个损坏的存根；为内部库提供存根 | 面向替代或分叉的标准库（MicroPython、打过补丁的树） |
| 优先级 | 更高——`stub-paths` 中的模块仍会遮蔽自定义 typeshed | 位于 `stub-paths` 之下、已安装包之上 |

---

## 内联抑制

使用标准的
[`# type: ignore`](https://typing.python.org/en/latest/spec/directives.html#type-ignore-comments)
拼写。指定 Basilisk 规则代码可让抑制保持精确：

```python
result = get_legacy_value()  # type: ignore[returns_compatibility]
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

单独成行的 `warning`、`info` 或 `disabled` 指令会开启一个**块**，由匹配的
`end-` 指令关闭。（`ignore` 是例外：单独成行的 `# type: ignore` 是文件级的
全量忽略，并不开启块，也没有 `end-ignore`。）

```python
# type: disabled[imports_unresolved]
from fastmcp import FastMCP
from result import Result
# type: end-disabled[imports_unresolved]
```

文件级指令是单独成行的注释，且必须出现在文件中**任何代码之前**——出现在
语句之后的指令会被丢弃，并报告为 `BSK-0063`（格式错误）。`relaxed` 将整个
文件中的所有错误降级为警告；`file-` 形式将同一效果应用于特定代码（不写代码
则应用于所有规则）：

```python
# basilisk: relaxed
# basilisk: file-warning[returns_compatibility]
# basilisk: file-disabled[imports_unresolved]
```

抑制审计是**可选的**标签规则族（`[tool.basilisk.rule-tags]` 中的
`"suppressions"`），默认不产生任何输出。`BSK-0060`（有效且精确）、
`BSK-0061`（有效但宽泛）、`BSK-0062`（未使用）与 `BSK-0063`（格式错误）
可分别独立配置为 error、warning、info 或 disabled。

---

## 采用债务

`basilisk adopt` 将某个文件夹的现有诊断记录为活动配置文件中普通的
warning 严重性 `[tool.basilisk.rules]` 条目——没有边车文件、标记或隐藏
状态。`basilisk unadopt` 删除这些条目；重新运行 `adopt` 会重新计算它们，
因此不再触发的规则会自动恢复到完整严重性。

---

## 可视化配置编辑器

VS Code 中的标签优先配置编辑器从 LSP 读取实时规则目录，预览批量变更，
并展示每条规则的生效严重性及其决定位置。它的编辑是由 LSP 应用到活动
`pyproject.toml` 的类型化变更——设置或移除规则条目、标签条目或
typeshed 设置；对 PEP 规则请求 `disabled` 会作为错误被拒绝。扩展本身
从不解析或写入配置文件；文件夹配置支撑编辑器的作用域定级视图。

![Basilisk 的标签优先 VS Code 配置编辑器，展示实时规则分类和逐规则严重性控制](/assets/images/vscode-configuration-editor.png)

请追踪权威的
[规范](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/LSP-CONFIGURATION-EDITOR-SPEC.md)
与[实现计划](https://github.com/Nimblesite/Basilisk/blob/main/docs/plans/LSP-CONFIGURATION-EDITOR-PLAN.md)。
