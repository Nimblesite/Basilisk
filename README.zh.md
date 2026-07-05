<p align="center"><a href="README.md">English</a> · <strong>简体中文</strong></p>

> 📝 本文档由机器翻译生成，欢迎母语者校对改进。

<p align="center">
  <img src="images/basilisk-logo.png" alt="Basilisk" width="160">
</p>

<h1 align="center">Basilisk</h1>

<p align="center">
  <strong>开源的 Python 语言服务器。</strong><br>
  完整的语言服务器、类型检查器、调试器与性能分析器 —— 默认严格。<br>
  VS Code、Cursor 与 Windsurf（Open VSX）&bull; Zed &bull; Neovim。使用 <strong>Rust</strong> 构建 —— 单一二进制文件，无需运行时。
</p>

<p align="center">
  <a href="https://www.basilisk-python.dev">官网</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/installation/">安装</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/quick-start/">快速开始</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/rules/">规则</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/refactoring/">重构</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/comparison/">对比</a>
</p>

<p align="center">
  <a href="https://www.basilisk-python.dev/zh/docs/conformance/"><strong>PEP 符合性 <!--g:score-->100.0%<!--/g:score--></strong></a> &mdash; 官方
  <a href="https://github.com/python/typing/tree/c94dfceff0af70c6626a1f86bc8f979135ae4652/conformance"><code>python/typing</code></a>
  符合性套件 <!--g:total-->141<!--/g:total--> 个测试中通过 <!--g:pass-->141<!--/g:pass--> 个（提交 <code><!--g:short-->c94dfce<!--/g:short--></code>），由
  <a href="https://github.com/Nimblesite/Basilisk/blob/main/conformance/score.py"><code>conformance/score.py</code></a>
  在默认配置下对真实二进制文件评分。我们对标 <code>python/typing@main</code>，得分只升不降。
</p>

---

<p align="center">
  <img src="images/screenshot.png" alt="Basilisk 实战 —— 在编辑器中进行类型检查、诊断与重构" width="900">
</p>

## 试用

`examples/` 文件夹中提供了可直接运行的 Python 文件：

```sh
basilisk check examples/bad.py    # everything flagged
basilisk check examples/good.py   # clean
basilisk check examples/mixed.py  # some errors, some clean
basilisk check examples/          # all three at once
```

---

## 快速示例

<table>
<tr>
<th>Basilisk 会拒绝这段代码</th>
<th>修复后</th>
</tr>
<tr>
<td>

```python
def greet(name):
    return "Hello " + name
```

</td>
<td>

```python
def greet(name: str) -> str:
    return "Hello " + name
```

</td>
</tr>
</table>

---

## 规则

所有规则默认开启，且无法在全局范围内放宽。

### 注解规则（E0001-E0005）

| Code | 触发条件 |
|------|---------------|
| `BSK-E0001` | 函数参数缺少类型注解 |
| `BSK-E0002` | 函数缺少返回类型注解 |
| `BSK-E0003` | 变量赋值缺少类型注解 |
| `BSK-E0004` | `*args` 或 `**kwargs` 缺少类型注解 |
| `BSK-E0005` | 类属性缺少类型注解 |

### 类型正确性（E0010-E0029）

| Code | 触发条件 |
|------|---------------|
| `imports_unresolved` | 无法解析导入 |
| `returns_compatibility` | 显式的 `Any` 注解（以警告形式发出），或返回类型不匹配 |
| `calls_argument_type` | 实参类型与形参类型不匹配 |
| `returns_compatibility_2` | 返回类型与声明的返回类型不匹配 |
| `assignment_compatibility` | 赋值类型与声明的变量类型不匹配 |
| `callables_annotation` | 类型参数数量错误（例如 `list[int, str]`） |
| `classes_override` | 方法重写的签名不兼容 |
| `classes_override_2` | 类变量重写的类型不兼容 |
| `names_undefined` | 引用了未定义的名称 |
| `names_unbound` | 变量在赋值前被使用 |
| `overloads_definitions` | `@overload` 分组缺少未加装饰器的实现 |
| `overloads_consistency` | 两个 `@overload` 签名相互重叠 |
| `dict_key_hashable` | 字典键类型不可哈希 |
| `match_exhaustiveness` | `match` 语句不完备 |
| `annotations_typeexpr` | 类型表达式无效（例如把数字字面量当作类型使用） |
| `BSK-E0025` | 重写方法缺少 `@override` 装饰器 |
| `generics_basic` | `TypeVar` 仅声明了单个约束 |
| `generics_base_class` | `Generic[...]` 基类中存在重复的 `TypeVar` |
| `typeddicts_class_syntax` | 在 `TypedDict` 类内部定义了方法 |

以上是最常见的规则。Basilisk 提供 **148 条 PEP 类型规范规则**（由符合性套件评分），外加 **13 条默认关闭的可选风格规则**，二者互不相干：总共 **161 个诊断代码**（155 个错误、6 个警告）—— 参见[完整诊断参考](https://www.basilisk-python.dev/zh/docs/rules/)（由 `scripts/gen_rules_reference.py` 从检查器源码生成）。

---

## 重构

Basilisk 提供了一整套重构代码操作 —— 在 VS Code、Cursor 与 Windsurf（通过 Open VSX）以及 Zed 和 Neovim 中，均可通过灯泡（代码操作）菜单使用。无需任何额外扩展。

| 操作 | 类别 | 功能说明 |
|--------|------|-------------|
| **提取变量** | `refactor.extract` | 将表达式提取为命名变量 |
| **提取变量（替换全部）** | `refactor.extract` | 替换所有相同的出现处 |
| **提取常量** | `refactor.extract` | 提取为模块级的 `SCREAMING_SNAKE` 常量 |
| **提取函数** | `refactor.extract` | 将选中的语句提取为新函数 |
| **内联变量** | `refactor.inline` | 用变量的值替换变量，并删除赋值 |
| **内联函数** | `refactor.inline` | 用函数体替换调用（单表达式） |
| **移动到新文件** | `refactor.move` | 将类/函数移动到新文件，并在原处留下导入语句 |
| **移动到现有文件** | `refactor.move` | 通过命令将类/函数移动到指定文件 |
| **重命名符号** | — | 作用域感知的重命名，同时更新关键字参数、`self.attr`、文档字符串与 `__all__` |
| **删除参数** | `refactor.rewrite` | 从函数及所有调用处删除参数 |
| **添加参数** | `refactor.rewrite` | 向函数签名添加 `new_param=None` |
| **排序参数** | `refactor.rewrite` | 按字母顺序排序参数（保持 `self`/`cls` 在前） |
| **实现抽象方法** | `refactor.rewrite` | 为抽象基类生成方法存根 |
| **转换 Union/Optional** | `refactor.rewrite` | `Union[X, Y]` ↔ `X \| Y`、`Optional[X]` ↔ `X \| None` |
| **转换语法结构** | `refactor.rewrite` | f-string ↔ `.format()`、`dict()` ↔ `{}`、`list()` ↔ `[]`、三元表达式 ↔ if/else、NamedTuple 类 ↔ 函数式写法 |

提取函数能够识别异步函数、方法（`self`/`cls`），并会拒绝包含 `yield`、`break` 或 `continue` 的选区。

---

## 输出格式

诊断采用 rustc 风格的输出：

```
error[BSK-E0001]: Missing parameter type annotation for `data`
  --> src/utils.py:14:13
   |
14 | def process(data):
   |             ^^^^
   |
   = help: Add a type annotation: `data: <type>`
   = note: In Basilisk, all function parameters require explicit types
   = see: https://www.basilisk-python.dev/errors/BSK-E0001
```

| Exit code | 含义 |
|-----------|---------|
| `0` | 通过 —— 无错误 |
| `1` | 发现类型错误 |
| `3` | 内部错误 |

---

## 架构

Basilisk 是一个 Cargo workspace，每个 crate 负责分析流水线中的一层。

> **流水线：** 源文本 &rarr; 解析器 &rarr; AST &rarr; 解析器（名称解析） &rarr; 作用域 &rarr; 检查器 &rarr; 诊断
>
> **增量：** `basilisk-db` 按内容哈希缓存 AST 与已解析的模块，因此只有发生变更的文件才会重新运行流水线。

### 分析流水线

| Crate | 功能 | 状态 |
|-------|-------------|--------|
| [basilisk-parser](crates/basilisk-parser/) | 封装 `ruff_python_parser`，将 `.py` 源码解析为带类型的 AST | 已完成 |
| [basilisk-resolver](crates/basilisk-resolver/) | 名称解析与作用域分析 —— 捕获未定义名称与赋值前使用的情况 | 已完成 |
| [basilisk-checker](crates/basilisk-checker/) | 核心类型检查器 —— 实现所有 E0001-E0025 规则 | 已完成 |
| [basilisk-cli](crates/basilisk-cli/) | `basilisk` 二进制文件 —— 将整条流水线串联起来 | 已完成 |

### LSP 与基础设施

| Crate | 功能 | 状态 |
|-------|-------------|--------|
| [basilisk-lsp](crates/basilisk-lsp/) | LSP 服务器 —— 诊断、悬停信息、跳转到定义、代码操作、重构、调试 | 运行中 |
| [basilisk-db](crates/basilisk-db/) | 基于 Salsa 的增量计算，实现低于 10ms 的延迟 | 运行中 |
| [basilisk-config](crates/basilisk-config/) | 配置解析（`pyproject.toml`、`basilisk.json`） | 已完成 |
| [basilisk-stubs](crates/basilisk-stubs/) | 内置类型存根（typeshed）—— 无需联网 | 运行中 |
| [basilisk-uv](crates/basilisk-uv/) | 为 LSP 提供的 uv 包管理器集成 | 运行中 |
| [basilisk-common](crates/basilisk-common/) | 共享的常量与类型 —— 零依赖，兼容 WASM | 已完成 |
| [basilisk-test-utils](crates/basilisk-test-utils/) | 共享的 E2E 测试辅助工具 | 已完成 |

### 未来能力

| Crate | 功能 | 状态 |
|-------|-------------|--------|
| [basilisk-mojo](crates/basilisk-mojo/) | 受 Mojo 启发的所有权/不可变性分析（`Borrowed`、`InOut`、`Owned`） | Phase 4 |
| [basilisk-compiler](crates/basilisk-compiler/) | 将带类型的 Python 编译为原生代码 | 未来 |
| [basilisk-plugin](crates/basilisk-plugin/) | 用于 Django、Pydantic、SQLAlchemy 类型扩展的 WASM 插件宿主 | Phase 5 |

### 编辑器扩展

| 扩展 | 编辑器 | 状态 |
|-----------|--------|--------|
| [vscode-extension](vscode-extension/) | VS Code | 运行中 |
| [basilisk.nvim](basilisk.nvim/) | Neovim 0.10+ | 运行中 |
| [basilisk-zed](basilisk-zed/) | Zed | Phase 2 |

---

## 开发

```sh
cargo build          # build all crates
cargo test           # run all tests
cargo clippy         # lint (zero warnings policy)
cargo fmt            # format
```

需要 Rust 1.87+。

---

## 贡献

Basilisk 由人类与 AI 的协作打造，并有意地划分了各自的工作。请参阅
[CONTRIBUTING.md](CONTRIBUTING.md) —— **For Humans**（测试、代码质量审查、
一致性/安全审计、IDE 功能对等、打磨 AI 指令）以及
**For AI**（在 [CLAUDE.md](CLAUDE.md) 既定规则下的技术执行）。

---

## 许可证

MIT。

由 [NIMBLESITE PTY LTD](https://www.nimblesite.co) 构建。
