---
layout: layouts/docs.njk
title: "Basilisk 符合性结果已撤回"
description: "Basilisk 已撤回此前的 Python typing 符合性声明。在相关逻辑完成全新实现并通过独立稳健性验证之前，当前百分比暂时未知。"
keywords: basilisk 符合性更正, python typing 符合性, python/typing 结果, 变异测试
dateModified: 2026-08-06
lang: zh
---

# 符合性结果已撤回

<p class="bench-caveat"><strong>更正：</strong>Basilisk 已撤回此前的满分声明。该结果并不能可信地衡量规范符合性。我们请求将 Basilisk 从 <code>python/typing</code> 结果表中移除，现已<a href="https://github.com/python/typing/blob/main/conformance/results/results.html">完成移除</a>。Basilisk 当前的符合性百分比<strong>暂时未知</strong>。</p>

我们发现，检查器中的一些逻辑针对符合性测试文件的确切内容进行了适配，而不是普遍实现类型规范。例如，类型别名验证曾对原始源代码文本执行前缀和子字符串判断，其中甚至专门判断了 `eval(`，仅仅因为某个测试使用了这种写法。因此，即使被测试的类型行为没有改变，对套件进行语义等价的变异也可能让 Basilisk 的结果发生变化。

官方套件仍然有价值，但针对固定测试用例开发出的代码即使通过，也不足以证明实现正确。在受影响逻辑完成全新实现并通过稳健性测试之前，我们不会发布替代百分比。

<p class="conf-links">
  <a href="https://github.com/python/typing/blob/main/conformance/results/results.html" target="_blank" rel="noopener"><strong>当前 python/typing 结果 ↗</strong></a>
  <a href="https://github.com/Nimblesite/Basilisk/blob/main/docs/CONFORMANCE-INTEGRITY-AUDIT.md" target="_blank" rel="noopener">完整性审计全文 ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/issues/379" target="_blank" rel="noopener">最初的缺陷报告 ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/issues/408" target="_blank" rel="noopener">完整性修复跟踪 ↗</a>
  <a href="https://typing.python.org/en/latest/spec/" target="_blank" rel="noopener">Python 类型规范 ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/README.md" target="_blank" rel="noopener">符合性 README ↗</a>
</p>

## 当前工作

有问题的实现正在被删除，相关行为将根据规范和结构化语法重新实现，不再依赖测试文件文本。审查范围也包括类似的源文本判断、重复逻辑、过度宽松的兜底分支，以及其他可能用狭窄测试用例代替通用实现的地方。

这是正在进行的修复，并非无限期撤回。我们预计在全新实现和验证完成后，很快会得到一个可以辩护的结果。如果新结果低于此前的声明，我们会如实发布较低的结果。

## 今后发布结果的门槛

未来的符合性结果必须通过以下全部检查：

1. 使用 Basilisk 默认配置运行官方、未经修改的 `python/typing` 评分工具。
2. 进行保持 AST 语义的变异，例如一致地重命名类型变量和采用等价写法。如果这些变化会改变结果，该规则就不能算作已实现。
3. 通过依据类型规范和真实代码独立设计的套件外用例，而不是从上游测试文本衍生用例。
4. 为审计发现的每一处针对测试的实现添加回归测试和变异测试。
5. 将稳健性与套件外验证结果同套件百分比一并发布，并保证方法可复现。

在这项工作完成之前，旧的符合性表格、图表、分类得分、通过数量和误报统计均已撤回，不应被引用为 Basilisk 的当前状态。

## 相关性能数据

同样的审查失效意味着已公开的基准测试数据也必须重新验证。为保持透明，这些数据仅作为明确标注的历史记录保留在[基准测试页面](/docs/benchmarks/)上，不得用于将 Basilisk 与其他工具进行比较。只有在方法和结果通过完整性审查后，我们才会发布新的性能数据。
