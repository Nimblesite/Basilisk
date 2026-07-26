# LSP Rename String-Literal Filter — Bug-Fix Working Doc

**Status: IN PROGRESS — Step 1 (understanding) of the [fix-bug skill](../../.claude/skills/fix-bug/SKILL.md) workflow.**
This doc is the live handoff record: any agent can resume from the "Next step" section at the bottom.

## The bug

`is_in_string_or_comment` ([crates/basilisk-lsp/src/references.rs:456-479](../../crates/basilisk-lsp/src/references.rs))
is documented — and used everywhere — as the guard that rejects matches "inside a `#` comment
**or string literal**", but its body only implements the `#`-comment half. The single
`return true` is inside the `if let Some(hash_pos) = line_before.find('#')` branch; no code
path inspects string context. Any identifier occurrence inside a plain string literal or
docstring is therefore treated as a real reference.

Found by a 19-agent adversarial workflow (run `wf_60d402d8-ccf`); all verifiers confirmed
(confidence 9–10), reproduced end-to-end over JSON-RPC.

### User-visible failure (reproduced)

```python
total: int = 1
msg: str = "total is big"
print(total)
```

F2-rename `total` → `amount` at (0,0) returns **three** edits, including `(1,12)-(1,17)`
inside the string → the client silently rewrites the user's data to `"amount is big"`.
Docstring prose is corrupted the same way. The `#`-comment half of the guard demonstrably
works (a `# total is big` line is correctly skipped), isolating the missing string half.

### Blast radius

`is_in_string_or_comment` is the **only** string/comment filter for all raw text sweeps:

| Call site | Feeds |
|---|---|
| `scope_tree::find_identifier_in_slice` ([scope_tree.rs:374](../../crates/basilisk-lsp/src/scope_tree.rs)) | `textDocument/rename`, `textDocument/references`, `documentHighlight` ([highlight.rs:23](../../crates/basilisk-lsp/src/highlight.rs)), code-lens ref counts ([code_lens.rs:33](../../crates/basilisk-lsp/src/code_lens.rs)) |
| `references::find_identifier_occurrences` (references.rs:417) | cross-file reference sweep in `server/handlers/navigation.rs` |
| `references::find_kwarg_in_line` (references.rs:172) — NOTE: passes a **single line**, not full source | keyword-arg rename sites |
| `references::find_attr_in_span` (references.rs:381) | `self.attr` / `cls.attr` rename sites |

## Constraints on the fix (must NOT regress)

1. **f-string interpolation fields** — `f"Hello, {name}!"`: `name` is a real reference and
   must keep being renamed. (Reportedly pinned by `test_ws_rename_parameter_scope` in
   `ws_test_scope_rename.rs`; VERIFY — see open questions.)
2. **PEP 563 string annotations** — `def f(x: "MyClass")`: real reference, must keep renaming.
3. **`__all__` entries** — rename adds them separately via `find_dunder_all_entries`
   (references.rs:118/192), so the text-sweep no longer matching inside `__all__` strings is
   fine **for rename**; check no *references/highlight* test pins them.
4. **No regex / quote-scanning** — CLAUDE.md forbids regex parsing; use ruff. The crate
   already depends on `ruff_python_parser` + `ruff_python_ast` + `ruff_text_size`
   (Cargo.toml:24-26); the only current in-crate lexer user is `import_hygiene/sort.rs`.
5. Docstring `:param name:` tags are added intentionally by `find_docstring_param_references`
   (references.rs:102) — unaffected; only the raw sweep must stop matching docstring prose.

## Fix direction (draft — not final until failing test confirmed)

Rewrite `is_in_string_or_comment` on ruff's lexer instead of the line heuristic:

- Offset inside a `Comment` token → `true`.
- Offset inside a plain `String` token → `true` **unless** the string is an annotation
  (PEP 563) — annotation detection needs AST context, see open question 3.
- F-strings: ruff lexes `FStringStart`/`FStringMiddle`/`FStringEnd` with normal tokens for
  interpolation fields, so interpolation offsets fall in no string token → `false` naturally.
- `find_kwarg_in_line` passes a single line as `source` — lexing a fragment is
  error-tolerant in ruff, same heuristic status as today, but keep in mind.

## Open questions — RESOLVED

1. `ws_test_scope_rename.rs` pins scope behavior only. `test_ws_rename_parameter_scope`
   (line 99) has the f-string fixture but only asserts "edits within lines 2-3" — it would
   still pass if the `{name}` interpolation edit vanished. F-string interpolation rename is
   therefore NOT hard-pinned; must be preserved anyway (pin it in follow-up hardening).
2. No references/highlight/code-lens test pins `__all__`-string, docstring, or
   string-annotation matches. `ws_test_find_references.rs` fixtures contain f-strings but no
   occurrence of the searched name inside string *content*.
3. **Key discovery:** `ResolvedModule` has `source`, `path`, and `lazy_ast: LazyAst`
   ([resolved_module.rs:42-56,298-306](../../crates/basilisk-resolver/src/scope/resolved_module.rs))
   — a `OnceLock`-cached `ParsedModule` (`ast: ModModule` + precomputed
   `comment_ranges: Vec<(u32, u32)>`, [basilisk-parser/src/lib.rs:17-25](../../crates/basilisk-parser/src/lib.rs)).
   AST-based classification is ~free wherever `resolved` is in hand — which is every
   in-process caller: `scope_tree::find_scoped_references`, `highlight::document_highlights`,
   `code_lens::code_lenses`, `references::rename_symbol`/`find_self_attr_references`.
   Cross-file sweeps in `navigation.rs` use `entry.text` — check at fix time whether workspace
   entries carry a resolved module.
4. Perf: prefer building a skip-mask once per public entry point (from the cached parse)
   rather than lexing per match. `find_identifier_occurrences` may build its own as fallback
   (O(n) per call, same class as its existing scan).

## fix-bug workflow state

- [x] Step 1: Understand — root cause read and confirmed in source (this doc).
- [x] Step 2: Failing test written — `test_ws_rename_skips_string_literals_and_docstring_prose`
      in `crates/basilisk-lsp/tests/lsp/ws_test_scope_rename.rs` (request id 507). One fixture
      covers both manifestations: docstring prose (line 1) + plain string literal (line 2);
      asserts edits only on lines 0/3 and exactly 2 edits.
- [x] Step 3: Ran `cargo test -p basilisk-lsp --test ws_navigation_tests
      test_ws_rename_skips_string_literals_and_docstring_prose` → FAILED at ws_test_scope_rename.rs:262
      with `rename must not edit string content on line 1: {"newText":"amount","range":
      {"start":{"line":1,"character":19},"end":{"line":1,"character":24}}}` — the word `total`
      inside the docstring. Fails precisely on the bug. ✅
- [ ] Step 4: Show failure to user; STOP for explicit confirmation. ⚠️ Do not fix before this.
      **← CURRENT STATE: awaiting user confirmation.**
- [ ] Step 5: Minimal fix in `is_in_string_or_comment` (+ helpers it needs).
- [ ] Step 6: New test passes.
- [ ] Step 7: Full suite (`make test` — fail-fast, coverage-enforced) + clippy/fmt.

## Next step

Answer open questions 1–3 (read `ws_test_scope_rename.rs`, grep reference/highlight/code-lens
tests for string expectations, inspect `basilisk_parser::ParsedModule` for retained AST/tokens),
then write the Step-2 failing test.

## Session context (for a resuming agent)

- Workflow transcripts: `~/.claude/projects/-Users-christianfindlay-Documents-Code-Basilisk/da06eaae-f70a-49d1-a387-1ca1ceda4722/subagents/workflows/wf_60d402d8-ccf/` (full winner dossier in `journal.jsonl`; ranked runner-ups: `didChangeWorkspaceFolders` wipes `WorkspaceIndex`; `remove_parameter` overlapping TextEdits; parse-error early-return bypasses exclude gate in `workspace.rs`; extract-variable line-shift; preview-cache eviction bug in `configuration_editor/state.rs` — all verified real, candidates for follow-up fixes).
- Working tree was clean at session start; mid-run a workflow verify agent reverted other
  agents' *temporary probe edits* in `workspace.rs` via `git checkout` — no user work lost,
  no verdicts corrupted (all landed confirmed).
- Git: per CLAUDE.md — no commits/pushes unless the user asks; PR-only via one feature branch.
