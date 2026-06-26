<p align="center"><a href="README.md">English</a> · <strong>简体中文</strong></p>

> 📝 本文档由机器翻译生成，欢迎母语者校对改进。

# Basilisk 示例

真实的 Python 脚本，展示 Basilisk 能捕获哪些问题，以及无错误（干净）、完整类型注解的代码是什么样子。

## 运行示例

```bash
# 检查单个文件
basilisk check examples/bad.py

# 一次性检查所有示例
basilisk check examples/

# JSON 输出（用于编辑器 / CI）
basilisk check examples/bad.py --output json
```

## 文件

### 违规展示（包含大量诊断）

| 文件 | 领域 | 主要违规 |
|---|---|---|
| [bad.py](bad.py) | 最小化示例 | E0001, E0002, E0004, E0005 |
| [mixed.py](mixed.py) | 混合：有类型 / 无类型 | E0001, E0002 |
| [api_server.py](api_server.py) | REST API 处理器 | E0001–E0003, E0011, E0014, E0017, E0019, E0025 |
| [data_pipeline.py](data_pipeline.py) | ETL 管道 | E0001–E0003, E0011, E0014, E0017, E0019, E0022, E0025 |
| [ml_trainer.py](ml_trainer.py) | 机器学习训练循环 | E0001–E0003, E0011, E0014, E0017, E0018, E0023, E0025 |
| [finance.py](finance.py) | 财务计算 | E0001–E0003, E0011, E0014, E0017, E0018, E0019, E0023, E0025 |
| [cli_tool.py](cli_tool.py) | CLI 应用程序 | E0001–E0003, E0011, E0014, E0017, E0019, E0023, E0025 |
| [weird_violations.py](weird_violations.py) | 微妙的边界情况 | E0003, E0011, E0014, E0017, E0019, E0021, E0023, E0025 |

### 无错误对照版本（零诊断）

| 文件 | 对照 |
|---|---|
| [good.py](good.py) | 最小化示例的修复版 |
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

## 错误代码参考

| 代码 | 含义 |
|---|---|
| E0001 | 缺少参数类型注解 |
| E0002 | 缺少返回值类型注解 |
| E0003 | 无法推断空集合或 `None` 的类型 |
| E0004 | 缺少 `*args` / `**kwargs` 类型注解 |
| E0005 | 缺少类属性类型注解 |
| E0010 | 未类型化的导入 |
| E0011 | 使用显式 `Any` 但缺少说明注释 |
| E0012 | 传递给函数的参数类型错误 |
| E0013 | 声明 `-> None` 的函数返回了非 None 值 |
| E0014 | 赋值类型不匹配 |
| E0015 | 无效的类型参数 |
| E0016 | 方法签名不兼容的重写 |
| E0017 | 属性类型不兼容的重写 |
| E0018 | 变量在定义之前被使用 |
| E0019 | 变量在某些代码路径上可能未绑定 |
| E0020 | `@overload` 组缺少实现 |
| E0021 | 重叠的重载签名 |
| E0022 | 不可哈希的类型被用作字典键 |
| E0023 | 非穷尽的 `match` 语句 |
| E0024 | 无效的类型形式 |
| E0025 | 重写缺少 `@override` 装饰器 |
