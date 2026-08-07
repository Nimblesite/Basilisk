---
layout: layouts/docs.njk
title: "Basilisk 正在审计并删除自己的检查器规则"
description: "Basilisk 已撤回 Python typing 符合性声明，正在逐条审计规则，并删除那些只匹配源文本、并未做真正类型检查的规则。"
keywords: basilisk 符合性更正, python typing 符合性, python/typing 结果, 变异测试
dateModified: 2026-08-08
lang: zh
---

# 我们正在审计检查器，并删除站不住脚的代码

<p class="bench-caveat"><strong>更正：</strong>Basilisk 已撤回此前的满分声明。该结果并不能可信地衡量规范符合性。我们请求将 Basilisk 从 <code>python/typing</code> 结果表中移除，现已<a href="https://github.com/python/typing/blob/main/conformance/results/results.html">完成移除</a>。Basilisk 当前的符合性百分比<strong>暂时未知</strong>，而且我们并不打算把它恢复回去。</p>

我们发现，检查器中的一些逻辑针对符合性测试文件的确切内容进行了适配，而不是普遍实现类型规范。那些规则匹配的是代码的**写法**，而不是代码的含义：类型别名验证曾对原始源代码文本执行前缀和子字符串判断，其中甚至专门判断了 `eval(`，仅仅因为某个测试文件使用了这种写法。改一个导入别名或重新格式化文件，结论就会变，尽管被测试的类型行为并没有改变。

针对固定测试用例开发出的代码即使通过，也不能作为证据；因此，解决办法不是拿到一个更好的分数。

## 我们正在做什么

**我们正在逐条审计规则，并删除那些没有做真正类型检查的规则。** 不是重写，不是打补丁，也不是标一个 TODO —— 是删除，并留下一个失败的测试，让这个缺口可见而不是被掩盖。一条规则只有在依据已解析的语法树做判断、并且同一个程序换一种写法时给出相同诊断的情况下，才会保留。

由此带来的后果是我们主动选择的，与其让你自己发现，不如先讲清楚：

- **Basilisk 会先变小，再变好。** 规则会更少，诊断也会更少。
- **符合性数字会下降。** 删除本来就没有在做分析的逻辑，本就该有这个结果；每一次下降我们都会如实报告，而不是设法回避。
- **对我们来说，一个失败的测试比一个由不做分析的代码撑起来的通过用例更有价值。** 前者如实记录了 Basilisk 做不到什么，后者则是在宣称它做得到。

留下来的，将是对自己所做之事诚实的代码 —— 仅此而已。

被删掉的分析是按规范重建，还是让扩展由一个成熟的开源检查器驱动，我们尚未决定。无论走哪条路，在通过下文所述的稳健性验证之前，都不会发布替代百分比。

<p class="conf-links">
  <a href="https://github.com/python/typing/blob/main/conformance/results/results.html" target="_blank" rel="noopener"><strong>当前 python/typing 结果 ↗</strong></a>
  <a href="https://github.com/Nimblesite/Basilisk/blob/main/docs/CONFORMANCE-INTEGRITY-AUDIT.md" target="_blank" rel="noopener">完整性审计全文 ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/issues/379" target="_blank" rel="noopener">最初的缺陷报告 ↗</a>
  <a href="https://github.com/Nimblesite/Basilisk/issues/408" target="_blank" rel="noopener">完整性修复跟踪 ↗</a>
  <a href="https://typing.python.org/en/latest/spec/" target="_blank" rel="noopener">Python 类型规范 ↗</a>
  <a href="https://github.com/python/typing/blob/main/conformance/README.md" target="_blank" rel="noopener">符合性 README ↗</a>
</p>

## 审计范围

审查覆盖每一处可能用狭窄测试用例代替通用实现的地方：源文本判断与子字符串匹配、硬编码的符号写法、围绕某个测试文件而不是围绕规范概念组织的规则、重复逻辑，以及用来顶替从未写出的检查的"全部接受"兜底分支。

每一处发现都按同样的方式处理 —— 先写一个因这段代码而失败的测试，然后删除这段代码，然后记录这次删除。不会在原地悄悄修补，因为修补会保留"这条规则本来是有效的"这个说法。

这是正在进行的修复，并非无限期撤回。如果可以辩护的结果低于此前的声明，我们会如实发布较低的结果。

## 今后发布结果的门槛

未来的符合性结果必须通过以下全部检查：

1. 使用 Basilisk 默认配置运行官方、未经修改的 `python/typing` 评分工具。
2. 进行保持 AST 语义的变异，例如一致地重命名类型变量和采用等价写法。如果这些变化会改变结果，该规则就不能算作已实现。
3. 通过依据类型规范和真实代码独立设计的套件外用例，而不是从上游测试文本衍生用例。
4. 为审计发现的每一处针对测试的实现添加回归测试和变异测试。
5. 将稳健性与套件外验证结果同套件百分比一并发布，并保证方法可复现。
6. 在 Basilisk 再次提交给 `python/typing` 之前，先通过一次由项目之外的人进行的审计。

在这项工作完成之前，旧的符合性表格、图表、分类得分、通过数量和误报统计均已撤回，不应被引用为 Basilisk 的当前状态。我们同样不会引用一个当前数字 —— 问题的根源并不是某个数字，在审计完成前发布一个新数字，只会重蹈覆辙。

## 相关性能数据

同样的审查失效意味着已公开的基准测试数据也必须重新验证。为保持透明，这些数据仅作为明确标注的历史记录保留在[基准测试页面](/docs/benchmarks/)上，不得用于将 Basilisk 与其他工具进行比较。只有在方法和结果通过完整性审查后，我们才会发布新的性能数据。
