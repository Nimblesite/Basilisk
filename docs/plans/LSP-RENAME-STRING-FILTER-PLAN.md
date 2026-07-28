# LSP Rename String-Literal Filter — Bug-Fix Working Doc

**Status: FIXED and VERIFIED.** Fix landed in commit `f6dd07d6` (branch `fixes`);
hardening tests added on top (working tree). All steps of the
[fix-bug skill](../../.claude/skills/fix-bug/SKILL.md) workflow completed.

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
- [x] Step 4: Failure shown; user confirmed ("go ahead").
- [x] Step 5: Fix implemented — new module `crates/basilisk-lsp/src/source_mask.rs`
      (`SourceMask`, Implements [LSPARCH-FEATURES-REFS]): built once per sweep via
      `ruff_python_parser::parse_module`; masks `Comment`, `String` (excluding
      annotation-span strings, collected by an AST `Visitor` over parameter/return/
      `AnnAssign` annotations), and `FStringMiddle`/`TStringMiddle` literal chunks —
      so f-string interpolation fields and PEP 563 string annotations stay renameable.
      Parse failure falls back to the old comment-only heuristic. The broken
      `is_in_string_or_comment` was deleted; all four sweep sites now take
      `&SourceMask` (`scope_tree::find_identifier_in_slice`,
      `references::find_identifier_occurrences` / `find_kwarg_in_line` /
      `find_attr_in_span`); `find_keyword_arg_sites` now tracks absolute line
      offsets (CRLF-exact via `split_inclusive`). Callers build one mask per file:
      rename, scoped refs, highlight, code-lens, 4 cross-file sweeps in navigation.
- [x] Step 6: Regression test passes.
- [x] Step 7 (scoped per user instruction — focused verification instead of full CI):
      all suites around the touched code green — basilisk-lsp lib 666, ws_navigation 84,
      ws_features 133, ws_core 89, lsp_stdio e2e 98; clippy clean; fmt clean.

## Verification (inside-out and backwards)

Backwards check: sources temporarily reverted to pre-fix (`git checkout HEAD~1 --
crates/basilisk-lsp/src/`), new tests run, then restored. On the OLD code exactly the
seven bug-catching tests fail and every non-regression pin passes — proving the tests
target the bug and nothing else:

| Test | Old code | Fixed |
|---|---|---|
| rename skips docstring + string literal (507) | FAIL | pass |
| original repro: module var + string (510) | FAIL | pass |
| kwarg site renamed, string lookalike skipped (512) | FAIL | pass |
| self-attr rename skips docstring mention (513) | FAIL | pass |
| find-references skips strings/docstrings (1106) | FAIL | pass |
| documentHighlight skips string content (631) | FAIL | pass |
| code-lens count skips string/docstring (406) | FAIL | pass |
| comment text still skipped (511) | pass | pass |
| f-string interpolation still renamed (508) | pass | pass |
| PEP 563 string annotations still renamed (509) | pass | pass |
| `__all__` entry still renamed (514) | pass | pass |

Plus 5 unit tests in `source_mask.rs` (strings/docstrings/comments masked; f-string
fields and annotation strings exempt; parse-failure fallback).

## Follow-up round: four more bugs found and fixed in the same neighbourhood

A 5-finder adversarial hunt (run `wf_aa2ba10c-67c`) over references/scope_tree/source_mask/
navigation/highlight surfaced 10 candidates. Four were both obvious and egregious and were
fixed here, each test-first via the fix-bug workflow:

1. **Duplicate (overlapping) TextEdits** — `rename_symbol` unioned four sweeps and never
   de-duplicated, so a defaulted parameter (`def f(x=1)`) had its definition range emitted
   twice; LSP forbids overlapping ranges, so VS Code rejected the whole rename. Fixed by
   sorting + `dedup()` on the range tuple before building edits (references.rs).
   Test: `test_ws_rename_defaulted_parameter_has_no_duplicate_edits`.
2. **`__all__` block never terminated** — the line scanner opened on `__all__ … =` and only
   closed on a line *ending* with `]`, so `__all__ = ["run"]  # comment` and the tuple form
   left it open for the rest of the file, rewriting the first quoted match on every later
   line. Replaced the whole line-scanning heuristic with an AST-based finder in the new
   [dunder_all.rs](../../crates/basilisk-lsp/src/dunder_all.rs) (Assign / AnnAssign /
   AugAssign targets named `__all__`; list, tuple, set and `+`-concatenated values), which
   also lifted references.rs back under the 500-LOC cap.
   Test: `test_ws_rename_dunder_all_with_trailing_comment_spares_string_literals` + 8 unit tests.
3. **Byte offsets emitted as LSP `character`** — `find_kwarg_in_line` and the old `__all__`
   scanner built `Position.character` from raw byte indices while every other range went
   through `byte_offset_to_position`. Any non-ASCII character earlier on the line shifted the
   edit right (+2 for an en dash) and corrupted the file. Both now use absolute offsets and
   `push_name_range`/`byte_offset_to_position`.
   Test: `test_ws_rename_kwarg_range_uses_utf16_columns`.
4. **Annotation exemption was too broad (regression introduced by the fix above)** — the mask
   exempted *every* string inside an annotation span, so `Literal["Mode"]` values and
   `Annotated[T, "meta"]` metadata were treated as forward references and rewritten. The
   exemption now walks the type expression and records only strings in true type-expression
   position, skipping `Literal` arguments entirely and taking only the first argument of
   `Annotated`.
   Test: `test_ws_rename_skips_literal_string_values` + 2 unit tests.

### Verified-real but NOT fixed (need design changes, deliberately out of scope)

- **Non-ASCII identifiers are split.** `is_ident_byte`/`is_ident`/`is_ident_char` all use
  `is_ascii_alphanumeric() || b'_'`, so every byte of a non-ASCII UTF-8 char reads as a word
  boundary. Renaming `x` in a file containing `xé` rewrites the `x` prefix of `xé`. PEP 3131
  allows these identifiers and ruff parses them. Fixing means char-based identifier
  classification across three helpers plus every raw sweep.
- **Cross-file rename is not scope-aware.** The importer sweep in navigation.rs is a raw
  whole-word text match, so `self.greet`, class attributes and shadowing locals in importer
  files are rewritten. The importer's `entry.resolved` is already in hand, so the scope tree
  is available — but wiring it in is a design change.
- **`defining_scope` walks through class scopes.** Python skips class scopes when resolving
  names inside nested functions (this file's own doc comment says so at scope_tree.rs:24-26),
  but the lookup walks the parent chain unconditionally, so a method reading a module-level
  name binds to the class attribute instead. Changing name resolution risks broad fallout.

## Known accepted limitations (documented, not regressions)

- `cast("MyClass", x)` / implicit string type aliases outside annotation position are
  masked (not renamed) — the old code renamed them only by accident of renaming ALL
  string content; PEP 563 annotation positions are the supported exemption.
- On files with syntax errors the mask degrades to comment-only filtering (exactly the
  pre-fix behaviour).

## Session context (for a resuming agent)

- Workflow transcripts: `~/.claude/projects/-Users-christianfindlay-Documents-Code-Basilisk/da06eaae-f70a-49d1-a387-1ca1ceda4722/subagents/workflows/wf_60d402d8-ccf/` (full winner dossier in `journal.jsonl`; ranked runner-ups: `didChangeWorkspaceFolders` wipes `WorkspaceIndex`; `remove_parameter` overlapping TextEdits; parse-error early-return bypasses exclude gate in `workspace.rs`; extract-variable line-shift; preview-cache eviction bug in `configuration_editor/state.rs` — all verified real, candidates for follow-up fixes).
- Working tree was clean at session start; mid-run a workflow verify agent reverted other
  agents' *temporary probe edits* in `workspace.rs` via `git checkout` — no user work lost,
  no verdicts corrupted (all landed confirmed).
- Git: per CLAUDE.md — no commits/pushes unless the user asks; PR-only via one feature branch.
