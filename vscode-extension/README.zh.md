<p align="center"><a href="README.md">English</a> · <strong>简体中文</strong></p>

> 📝 本文档由机器翻译生成，欢迎母语者校对改进。

<p align="center">
  <img src="https://basilisk-python.dev/assets/images/favicon.png" alt="Basilisk" width="140">
</p>

<h1 align="center">VS Code 版 Basilisk</h1>

<p align="center">
  <strong>面向 VS Code 的开源 Pylance 替代品。</strong><br>
  完整的语言服务器：诊断、自动补全、悬停信息、跳转到定义、<br>
  重构、调试、性能分析。默认严格。使用 Rust 构建。
</p>

<p align="center">
  <a href="https://www.basilisk-python.dev">官网</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/quick-start/">快速开始</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/zh/docs/rules/">规则</a> &nbsp;&bull;&nbsp;
  <a href="https://github.com/Nimblesite/Basilisk">GitHub</a>
</p>

---

<p align="center">
  <img src="images/screenshot.png" alt="Basilisk 实战 —— 在 VS Code 中进行类型检查、诊断与重构" width="900">
</p>

## Basilisk 的功能

Basilisk 是一个**完整的 Python 语言服务器和 VS Code 扩展**，可替代 Pylance 和 Pyright。它不仅仅是一个类型检查器 —— 还提供自动补全、跳转到定义、悬停信息、代码操作、重构、集成调试和性能分析。全部完全开源。

其他类型检查器默认采取宽松策略，寄希望于你主动选择严格模式。Basilisk **从一开始就严格**，并始终保持严格。如果你的代码没有类型标注，那就是一个错误 —— 正如上面的截图所示。

修复一次，永远以带类型的方式发布：

```python
def greet(name: str) -> str:
    return "Hello " + name
```

---

## 功能特性

### 实时诊断

错误会在你输入时内联显示 —— 由 Basilisk LSP 服务器驱动，通过 Salsa 框架（与 rust-analyzer 相同的技术）实现亚 10 毫秒级的增量分析。

### 自动补全、悬停信息、跳转到定义

完整的语言智能 —— 补全、悬停文档、跳转到定义、查找引用、重命名符号。

### 代码操作与重构

提取函数/变量、重命名、移动符号、内联、组织导入 —— 全部内置于 LSP 中。

### 集成调试

按 F5 即可调试 Python。Basilisk 会启动 debugpy 并代理 DAP 连接 —— 断点、单步执行、变量检查、监视表达式。无需单独的调试扩展。

### 集成性能分析

直接在编辑器中使用 py-spy 对 Python 代码进行性能分析。查看热力图并定位性能瓶颈，无需离开 VS Code。

### 活动面板

Basilisk 侧边栏在活动栏中提供两个可访问的面板：

**Modules** —— 浏览工作区的 Python 模块树，并融合类型健康度信息。每个模块都会显示覆盖率条、覆盖率百分比、错误/警告计数以及 `[adopted]` 徽章，其图标会根据覆盖率染成绿色/黄色/红色；展开某个模块即可查看其顶层符号（函数、类、变量）及其标注状态。整个工作区的覆盖率摘要显示在面板标题中（消息 + 数字徽章）。右键单击可复制导入路径。可在树状视图和扁平视图之间切换；在扁平视图中，可按最差优先、最佳优先或字母顺序排序。可使用 glob 模式进行筛选。当服务器运行时，工具栏还提供 **Fix All**、**Organize Imports** 和 **Restart Server**。

**Basilisk Info** —— 功能开关（类型检查、uv 集成）以及紧凑的只读服务器信息（版本、分析模式、Python、uv —— 工具提示中包含自动同步和存根建议详情 —— 以及二进制文件路径）。实时服务器状态显示在状态栏中，点击状态栏会打开 Basilisk 输出日志；uv 操作（sync/add/lock/create-env）位于命令面板中。

当文件发生更改时，两个面板都会自动更新（防抖 300 毫秒）。Modules 面板在打开工作区时显示；Info 面板始终可见。

### 内嵌提示

直接在编辑器中查看推断出的类型和参数名称：

- 调用处的**参数名称**
- 未标注局部变量的**变量类型**

### Ruff 集成

内置对 Ruff 格式化和导入组织的支持。一个扩展，两个工具。

### 单一二进制文件，零依赖

Basilisk 以单个 Rust 二进制文件的形式发布。无需 Python 运行时、无需 Node.js、无需 pip、无需 npm。安装即用。

---

## 诊断规则

所有规则**默认开启**。无法在全局范围内放宽它们。

### 标注规则

| Code | 检测内容 |
|------|----------------|
| `BSK-E0001` | 参数没有类型标注 |
| `BSK-E0002` | 函数缺少返回类型 |
| `BSK-E0003` | 变量缺少类型标注 |
| `BSK-E0004` | `*args` / `**kwargs` 未标注 |
| `BSK-E0005` | 类属性未标注 |

### 类型正确性

| Code | 检测内容 |
|------|----------------|
| `imports_unresolved` | 从未标注类型的模块导入 |
| `returns_compatibility` | 显式 `Any` 标注（警告） |
| `calls_argument_type` | 参数类型不匹配 |
| `returns_compatibility_2` | 返回类型不匹配 |
| `assignment_compatibility` | 赋值类型不匹配 |
| `callables_annotation` | 类型参数数量错误 |
| `classes_override` | 不兼容的方法重写 |
| `classes_override_2` | 不兼容的类变量重写 |
| `names_undefined` | 未定义的名称 |
| `names_unbound` | 在赋值前使用 |
| `overloads_definitions` | `@overload` 缺少实现 |
| `overloads_consistency` | 重叠的 `@overload` 签名 |
| `dict_key_hashable` | 不可哈希的字典键 |
| `match_exhaustiveness` | 非穷尽的 `match` |
| `annotations_typeexpr` | 无效的类型表达式 |
| `BSK-E0025` | 缺少 `@override` 装饰器 |

---

## 横向对比

| | Basilisk | Pyright | mypy |
|---|:---:|:---:|:---:|
| **默认严格** | 是 | 否 | 否 |
| **编写语言** | Rust | TypeScript | Python |
| **所需运行时** | 无 | Node.js | Python |
| **增量速度** | <10ms | ~50ms | ~200ms |
| **所有权分析** | 是 | 否 | 否 |
| **单一二进制文件** | 是 | 否 | 否 |

---

## 扩展设置

| Setting | Default | 说明 |
|---------|---------|-------------|
| `basilisk.enabled` | `true` | 启用/禁用类型检查器 |
| `basilisk.executablePath` | `""` | basilisk 二进制文件的显式路径。留空则使用 VSIX 内置的二进制文件 |
| `basilisk.binaries.path` | `""` | 包含 Basilisk 运行时二进制文件的目录 |
| `basilisk.binaries.basilisk` | `""` | Basilisk 语言服务器二进制文件的显式路径 |
| `basilisk.useLsp` | `true` | 使用 LSP 服务器（禁用则回退到子进程模式） |
| `basilisk.trace.server` | `"off"` | LSP 跟踪级别：`off`、`messages`、`verbose` |
| `basilisk.inlayHints.parameterNames` | `true` | 保留项 —— 提示始终显示；服务器尚未读取此项 |
| `basilisk.inlayHints.variableTypes` | `true` | 保留项 —— 提示始终显示；服务器尚未读取此项 |
| `basilisk.formatter` | `"ruff"` | 格式化引擎 —— `"ruff"` 使用内嵌于 Basilisk 二进制文件中的 Ruff 格式化器（进程内运行；无需任何外部 `ruff` 二进制文件），`"none"` 则禁用格式化 |

---

## 命令

| Command | 说明 |
|---------|-------------|
| `Basilisk: Restart Language Server` | 重启 LSP 服务器 |
| `Basilisk: Show Output` | 打开 Basilisk 输出通道 |
| `Basilisk: Organize Imports` | 通过 Ruff 排序并清理导入 |
| `Basilisk: Fix File` | 对当前文件应用所有可用的自动修复 |
| `Basilisk: Adopt File` | 为未标注类型的文件添加类型标注 |
| `Basilisk: uv sync` | 在工作区中运行 uv sync |
| `Basilisk: uv add` | 通过 uv 添加一个包 |
| `Basilisk: Refresh Module Explorer` | 刷新模块树 |
| `Basilisk: Toggle Module Explorer View` | 在树状视图和扁平视图之间切换 |
| `Basilisk: Toggle Sort Order` | 循环切换扁平视图排序（最差/最佳/字母） |
| `Basilisk: Copy Import Path` | 为选定符号复制 `from x import y` |
| `Basilisk: Open Walkthrough` | 打开 Basilisk 入门引导 |

---

## 要求

无 —— Basilisk 二进制文件已随本扩展一起捆绑，支持 macOS（Apple Silicon）、Linux（x86_64 和 aarch64）以及 Windows（x86_64 和 aarch64）。安装扩展即可使用。

### 单独安装 CLI

如果你还想将 `basilisk` CLI 加入到 PATH 中（用于 CI、脚本编写或终端使用），请使用你所在平台的包管理器进行安装：

```bash
# macOS, Linux
brew tap Nimblesite/tap
brew install basilisk
```

```powershell
# Windows
scoop bucket add nimblesite https://github.com/Nimblesite/scoop-bucket
scoop install basilisk
```

或从 [GitHub Releases](https://github.com/Nimblesite/Basilisk/releases) 下载预构建的二进制文件。

要让本扩展使用你单独安装的 CLI，请将 `basilisk.executablePath` 或 `basilisk.binaries.basilisk` 设置为该二进制文件的绝对路径。从源码构建同样可行 —— 请参阅 [GitHub 仓库](https://github.com/Nimblesite/Basilisk)。

---

## Basilisk 的组成部分

这是 [Basilisk](https://github.com/Nimblesite/Basilisk) 项目的 VS Code 扩展。Basilisk 还支持 [Neovim](https://github.com/Nimblesite/Basilisk/tree/main/basilisk.nvim) 和 [Zed](https://github.com/Nimblesite/Basilisk/tree/main/basilisk-zed)。

## 许可证

MIT。

由 [NIMBLESITE PTY LTD](https://www.nimblesite.co) 构建。
