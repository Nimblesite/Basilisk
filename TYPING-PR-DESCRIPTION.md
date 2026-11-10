# PR description — python/typing: Remove Basilisk from conformance results

> **DRAFT — not posted.** Target: a PR against `python/typing` reverting #2316.
> Local branch prepared: `remove-basilisk-conformance-results` (revert of `f051625`, 147 files, −2302).

---

Reverts #2316. I'm the author of Basilisk, and I'm asking for it to be taken out of the conformance results. Our published score doesn't measure what the table says it measures, so it needs to be removed.

## Why

Many of Basilisk's rules match against raw source text and hard-coded typing symbol names instead of resolved symbols on the AST. The result is that our score reflected the exact spelling of the test files rather than conformance to the specification.

A [mutation harness](https://gist.github.com/sharkdp/3f3266fd9c67d22137e2b6c015c5f206) makes this unambiguous. It renames imported typing symbols and adjusts whitespace — the same programs, spelled differently — so it should not move any checker's score. Ours moves. I reproduced this myself in a current development branch before opening this PR: 113 of the 141 test files fail. We will investigate exactly how much work is necessary to fix this.

## How it happened

Despite our best efforts to enforce accuracy, we relied too heavily on AI to validate the work we did, and we focused too heavily on conformance results instead of thoroughly testing real world scenarios. It looks as though we didn't pick up code that clearly should have only been just a placeholder and that code proliferated. This should have been predictable and I take full responsibility for letting this happen. 

I am sorry for breaking trust, especially when the whole aim of Basilisk as a tool is to build trust. We have an absolute commitment to building the trust of the Python community so requesting this removal is a necessary step until we can work through the issues. 

## Resubmission

I'm asking that you please leave the door open to make another attempt in future. 

Before we submit again, we'll hold ourselves to the test that proved the issues. If a rename or a whitespace change moves our score, the rule isn't implemented and the number doesn't count. We'll publish that measurement alongside any resubmission, and if the honest score is lower, we'll publish the lower score.

We will do a full audit to remove any code that pattern-matches source text. We will track it publicly in our repo. We will take as long as it takes to get this right and not submit again until we've got proof that we're not making the same mistakes again.

Sincere apologies to the maintainers here. You accepted our submission in good faith and our carelessness led to misleading results being posted here. 
