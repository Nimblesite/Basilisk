<p align="center"><a href="README.md">English</a> · <strong>简体中文</strong></p>

> 📝 本文档由机器翻译生成，欢迎母语者校对改进。

# Basilisk 示例

真实的 Python 脚本，展示 Basilisk 能捕获哪些问题，以及无错误（干净）、完整类型注解的代码是什么样子。

## 运行示例

```bash
# 单个文件中的类型规范错误
basilisk check examples/bad.py

# 同一文件上可选启用的自定规则
basilisk analyze examples/bad.py

# 一次性检查所有示例
basilisk check examples/

# JSON 输出（用于编辑器 / CI）
basilisk check examples/bad.py --output json
```

`check` 与 `analyze` 读取的是同一套规则，只是按来源做了划分（[CHKARCH-COMMANDS]）：
`check` 只报告带 `pep` 标签的类型规范规则，而 `analyze` 报告由配置表选用的、不带
`pep` 标签的自定规则；两者都遵循下文表格所描述的严重级别。因此下文列出的 `BSK-`
代码只会出现在 `analyze` 中——`check` 永远不会输出它们。

## 规范规则 vs 自定规则

下面的每个 **PEP** 诊断都是对
[Python 类型规范](https://typing.python.org/en/latest/spec/index.html)的真实违反。
这些规则开箱即用——在您自己的项目中，无需任何配置，您得到的正是它们，级别为
`error`。配置文件可以把其中某条降级为 `warning` 或 `info`，但任何表都无法把
它关掉。

其余的都是 Basilisk 的可选自定规则（处处要求注解、要求 `@override` 等）。
在某个 `[tool.basilisk]` 表选中它们之前，它们保持沉默。Basilisk 针对每个被
检查的文件，从该文件所在目录逐级向上查找配置，最近的、对某条规则作出决定的表
直接胜出。这里有两个表在起作用：根 `pyproject.toml` 为整个仓库选中这些自定
规则，而 `examples/pyproject.toml`——对 `examples/` 下的一切来说更近的那个
表——把 `BSK-0001`–`BSK-0005` 与 `BSK-0025` 降级为 `warning`。这正是文档教授
的渐进式采纳方式：警告意味着"这段代码通过类型检查，但严格度还没有拉满"。
examples 的表没有提到的规则（例如 `BSK-0014` 与 `BSK-0050`）仍由根表决定。

这就是全部的作用域机制。若要让某条规则在目录树的某一部分以不同严重级别运行，
请在该文件夹放一个带有自己的 `[tool.basilisk]` 表的 `pyproject.toml`；这里
没有 glob 路径模式，没有按模块的表，也没有预设或模式。

## 文件

### 违规展示（包含大量诊断）

| 文件 | 领域 | PEP 规则（始终启用，此处为错误） | Basilisk 自定规则（可选启用） |
|---|---|---|---|
| [bad.py](bad.py) | 最小化导览 | `calls_argument_type`, `returns_compatibility`, `assignment_compatibility`, `calls_argument_count`, `classes_override`, `names_unbound`, `match_exhaustiveness` | BSK-0001, BSK-0002, BSK-0004 |
| [mixed.py](mixed.py) | 混合：有类型 / 无类型 | `calls_argument_type` | BSK-0001, BSK-0002 |
| [api_server.py](api_server.py) | REST API 处理器 | `assignment_compatibility`, `overloads_consistency`, `names_unbound`, `dict_key_hashable`, `classes_override_2` | BSK-0001–BSK-0003, BSK-0025, BSK-0014, BSK-0050 |
| [data_pipeline.py](data_pipeline.py) | ETL 管道 | `assignment_compatibility`, `overloads_consistency`, `names_unbound`, `dict_key_hashable`, `classes_override_2` | BSK-0001–BSK-0003, BSK-0025, BSK-0014 |
| [ml_trainer.py](ml_trainer.py) | 机器学习训练循环 | `assignment_compatibility`, `overloads_consistency`, `match_exhaustiveness`, `dict_key_hashable`, `classes_override_2` | BSK-0001–BSK-0003, BSK-0025, BSK-0014, BSK-0050 |
| [finance.py](finance.py) | 财务计算 | `assignment_compatibility`, `classes_override_2`, `overloads_consistency`, `names_unbound`, `match_exhaustiveness`, `dict_key_hashable` | BSK-0001–BSK-0003, BSK-0025, BSK-0014, BSK-0050 |
| [cli_tool.py](cli_tool.py) | CLI 应用程序 | `assignment_compatibility`, `classes_override_2`, `overloads_consistency`, `names_unbound`, `match_exhaustiveness`, `dict_key_hashable` | BSK-0001–BSK-0003, BSK-0025, BSK-0014, BSK-0050 |
| [weird_violations.py](weird_violations.py) | 微妙的边界情况 | `overloads_consistency`, `names_unbound`, `classes_override_2`, `assignment_compatibility`, `match_exhaustiveness`, `dict_key_hashable` | BSK-0001–BSK-0003, BSK-0014, BSK-0050 |

### 无错误对照版本（零诊断）

| 文件 | 对照 |
|---|---|
| [good.py](good.py) | `bad.py` 的修复版——在完全严格模式下通过 |
| [api_server_clean.py](api_server_clean.py) | `api_server.py` 的修复版 |

### 调试器与性能分析器演示（按 F5 启动）

这些是无错误（干净）、完整类型注解的脚本，旨在 Basilisk 调试器下*运行*，而非进行静态检查。打开其中一个并按 F5。

| 文件 | 演示内容 | 使用方式 |
|---|---|---|
| [debug_demo.py](debug_demo.py) | 断点、Watch 面板、Locals、Debug Console | 设置一个断点并单步执行 |
| [profile_demo.py](profile_demo.py) | CPU 性能分析——几秒钟具有明显热点的 CPU 密集型工作，让火焰图和热点行热力图填充起来 | 一键操作：**Run & Profile CPU (Current File)** |
| [cpu_demo.py](cpu_demo.py) | CPU 采样——热/温/冷火焰图、热点行提示 | 将 CPU 性能分析器附加到正在运行的会话 |
| [memory_demo.py](memory_demo.py) | 内存——持续泄漏、瞬时峰值、引用循环；该运行会在退出时捕获最终快照，因此结束时会生成可查看的热力图 / `.heapprofile` | 一键操作：**Run & Track Memory (Current File)** |
| [heap_demo.py](heap_demo.py) | 内存——约 70 MB 的大块温缓存，分布在约 40 个不同的分配位置，使 `.heapprofile` 火焰图和 Self-Size 表格填满多样、真实的数据切片 | 一键操作：**Run & Track Memory (Current File)** |

## 规则参考

每条诊断末尾都带有指向其文档页面的 `see:` 链接。完整目录见
[basilisk-python.dev/docs/rules](https://www.basilisk-python.dev/docs/rules/)。

### 此处展示的 PEP 类型规范规则（始终启用，此处为错误）

| 代码 | 含义 |
|---|---|
| `calls_argument_type` | 实参与形参声明的类型不兼容 |
| `calls_argument_count` | 调用时参数数量错误 |
| `returns_compatibility` / `returns_compatibility_2` | 返回值不能赋值给声明的返回类型 |
| `assignment_compatibility` | 赋的值不能赋值给注解类型 |
| `classes_override` | `@override` 方法与基类方法不兼容 |
| `classes_override_2` | 属性重写与基类不兼容 |
| `names_unbound` | 变量在某些执行路径上可能未绑定 |
| `match_exhaustiveness` | 非穷尽的 `match`——缺少通配 `case _:` 分支 |
| `dict_key_hashable` | 不可哈希的类型被用作字典键 |
| `overloads_consistency` | `@overload` 组不一致或相互重叠 |

### 此处展示的 Basilisk 自定规则（可选启用）

严重级别一列是管辖 `examples/` 的那些表所选中的值，而不是代码自身的属性——
规则代码不携带任何严重级别类别。在未作配置的项目中，这些规则根本不会运行。

| 代码 | 含义 | 此处的严重级别 |
|---|---|---|
| BSK-0001 | 缺少参数类型注解 | warning |
| BSK-0002 | 缺少返回值类型注解 | warning |
| BSK-0003 | 无法推断空集合或 `None` 的类型 | warning |
| BSK-0004 | 缺少 `*args` / `**kwargs` 类型注解 | warning |
| BSK-0025 | 重写缺少 `@override` 装饰器 | warning |
| BSK-0014 | 使用显式 `Any` 但缺少说明 | warning |
| BSK-0050 | 冗余的类型注解 | warning |
