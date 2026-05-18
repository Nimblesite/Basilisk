//! Refactoring code actions: extract variable/constant/function, convert type
//! syntax, implement abstract methods, inline variable/function, `NamedTuple`
//! conversions, and construct conversions.

mod abstract_methods;
mod change_signature;
mod extract;
mod extract_function;
mod fstring;
mod helpers;
mod inline;
mod inline_function;
mod literals;
mod move_symbol;
mod namedtuple;
mod ternary;
mod type_syntax;

pub(super) use abstract_methods::implement_abstract_methods;
pub(super) use change_signature::{add_parameter, remove_parameter, reorder_parameters};
pub(super) use extract::{extract_constant, extract_variable};
pub(super) use extract_function::extract_function;
pub(super) use fstring::convert_fstring;
pub(super) use inline::inline_variable;
pub(super) use inline_function::inline_function_call;
pub(super) use literals::convert_literals;
pub(super) use move_symbol::{move_symbol_to_existing_file, move_symbol_to_new_file};
pub(super) use namedtuple::convert_namedtuple;
pub(super) use ternary::convert_ternary;
pub(super) use type_syntax::{convert_optional_syntax, convert_union_syntax};

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test-only code: unwrap/expect/indexing acceptable in unit tests"
)]
mod tests {
    use tower_lsp::lsp_types::{CodeActionKind, Position, Range, Url};

    use super::*;

    fn test_uri() -> Url {
        Url::parse("file:///test.py").unwrap()
    }

    fn range(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Range {
        Range {
            start: Position {
                line: start_line,
                character: start_char,
            },
            end: Position {
                line: end_line,
                character: end_char,
            },
        }
    }

    // ── Extract Variable ────────────────────────────────────────────────────

    #[test]
    fn test_extract_variable_produces_correct_edits() {
        let source = "result = some_func(42) + other_func(7)\n";
        let range = range(0, 9, 0, 22);
        let uri = test_uri();
        let actions = extract_variable(&uri, source, &range);
        assert!(!actions.is_empty(), "should produce at least one action");
        let action = &actions[0];

        assert_eq!(action.title, "Extract variable (basilisk)");
        assert_eq!(
            action.kind.as_ref().map(CodeActionKind::as_str),
            Some("refactor.extract.variable")
        );

        let edit = action.edit.as_ref().unwrap();
        let changes = edit.changes.as_ref().unwrap();
        let edits = changes.get(&uri).unwrap();
        assert_eq!(edits.len(), 2);

        // First edit: insert variable assignment.
        assert_eq!(edits[0].new_text, "extracted_value = some_func(42)\n");
        assert_eq!(edits[0].range.start.line, 0);
        assert_eq!(edits[0].range.start.character, 0);

        // Second edit: replace the selection with variable name (shifted down).
        assert_eq!(edits[1].new_text, "extracted_value");
        assert_eq!(edits[1].range.start.line, 1);
        assert_eq!(edits[1].range.start.character, 9);
    }

    #[test]
    fn test_extract_variable_empty_selection_returns_empty() {
        let source = "x = 1\n";
        let range = range(0, 2, 0, 2);
        assert!(extract_variable(&test_uri(), source, &range).is_empty());
    }

    #[test]
    fn test_extract_variable_multiline_returns_empty() {
        let source = "x = (\n    1 + 2\n)\n";
        let range = range(0, 4, 2, 1);
        assert!(extract_variable(&test_uri(), source, &range).is_empty());
    }

    #[test]
    fn test_extract_variable_replace_all_with_multiple_occurrences() {
        let source = "a = foo(1) + foo(1)\nb = foo(1)\n";
        // Select the first "foo(1)" at line 0, chars 4..10.
        let range = range(0, 4, 0, 10);
        let uri = test_uri();
        let actions = extract_variable(&uri, source, &range);

        assert_eq!(actions.len(), 2, "should have single + replace-all actions");
        assert_eq!(actions[0].title, "Extract variable (basilisk)");
        assert_eq!(
            actions[1].title,
            "Extract variable \u{2014} replace all (basilisk)"
        );

        let edit = actions[1].edit.as_ref().unwrap();
        let changes = edit.changes.as_ref().unwrap();
        let edits = changes.get(&uri).unwrap();

        // 1 insertion + 3 replacements (three occurrences of "foo(1)").
        assert_eq!(edits.len(), 4);
        assert_eq!(edits[0].new_text, "extracted_value = foo(1)\n");

        for replacement in &edits[1..] {
            assert_eq!(replacement.new_text, "extracted_value");
        }
    }

    #[test]
    fn test_extract_variable_replace_all_not_offered_for_single_occurrence() {
        let source = "a = unique_expr()\n";
        let range = range(0, 4, 0, 17);
        let actions = extract_variable(&test_uri(), source, &range);

        assert_eq!(actions.len(), 1, "should only have single action");
        assert_eq!(actions[0].title, "Extract variable (basilisk)");
    }

    // ── Extract Constant ────────────────────────────────────────────────────

    #[test]
    fn test_extract_constant_places_at_top_of_file() {
        let source = "import os\n\ndef f():\n    return 42\n";
        let range = range(3, 11, 3, 13);
        let uri = test_uri();
        let action = extract_constant(&uri, source, &range).expect("should produce action");

        assert_eq!(action.title, "Extract constant (basilisk)");

        let edit = action.edit.unwrap();
        let changes = edit.changes.unwrap();
        let edits = changes.get(&uri).unwrap();
        assert_eq!(edits.len(), 2);

        // Constant inserted after imports (line 1).
        assert_eq!(edits[0].new_text, "EXTRACTED_VALUE = 42\n");
        assert_eq!(edits[0].range.start.line, 1);

        // Selection replacement is shifted down by 1.
        assert_eq!(edits[1].new_text, "EXTRACTED_VALUE");
        assert_eq!(edits[1].range.start.line, 4);
    }

    // ── Convert Union ───────────────────────────────────────────────────────

    #[test]
    fn test_convert_union_to_pipe() {
        let source = "x: Union[int, str] = 1\n";
        let range = range(0, 3, 0, 3);
        let uri = test_uri();
        let actions = convert_union_syntax(&uri, source, &range);

        let union_to_pipe = actions
            .iter()
            .find(|a| a.title.contains("Union[X, Y] to X | Y"))
            .expect("should have Union->pipe action");

        let edit = union_to_pipe.edit.as_ref().unwrap();
        let changes = edit.changes.as_ref().unwrap();
        let edits = changes.get(&uri).unwrap();
        assert_eq!(edits[0].new_text, "int | str");
    }

    #[test]
    fn test_convert_pipe_to_union() {
        let source = "x: int | str = 1\n";
        let range = range(0, 3, 0, 3);
        let uri = test_uri();
        let actions = convert_union_syntax(&uri, source, &range);

        let pipe_to_union = actions
            .iter()
            .find(|a| a.title.contains("X | Y to Union[X, Y]"))
            .expect("should have pipe->Union action");

        let edit = pipe_to_union.edit.as_ref().unwrap();
        let changes = edit.changes.as_ref().unwrap();
        let edits = changes.get(&uri).unwrap();
        assert_eq!(edits[0].new_text, "Union[int, str]");
    }

    // ── Convert Optional ────────────────────────────────────────────────────

    #[test]
    fn test_convert_optional_to_pipe_none() {
        let source = "x: Optional[int] = None\n";
        let range = range(0, 3, 0, 3);
        let uri = test_uri();
        let actions = convert_optional_syntax(&uri, source, &range);
        assert_eq!(actions.len(), 1);

        let action = &actions[0];
        assert_eq!(action.title, "Convert Optional[X] to X | None (basilisk)");

        let edit = action.edit.as_ref().unwrap();
        let changes = edit.changes.as_ref().unwrap();
        let edits = changes.get(&uri).unwrap();
        assert_eq!(edits[0].new_text, "int | None");
    }

    // ── Extract Function ──────────────────────────────────────────────────

    #[test]
    fn test_extract_function_simple() {
        let source = "def main() -> None:\n    x: int = 1\n    y: int = x + 1\n    print(y)\n";
        // Select lines 1-2 (the two assignments).
        let range = range(1, 0, 3, 0);
        let uri = test_uri();
        let action = extract_function(&uri, source, &range).expect("should produce action");

        assert_eq!(action.title, "Extract function (basilisk)");
        assert_eq!(
            action.kind.as_ref().map(CodeActionKind::as_str),
            Some("refactor.extract.function")
        );

        let edit = action.edit.unwrap();
        let changes = edit.changes.unwrap();
        let edits = changes.get(&uri).unwrap();
        assert_eq!(edits.len(), 2, "should have insert + replace edits");
    }

    #[test]
    fn test_extract_function_empty_selection_returns_none() {
        let source = "x = 1\n";
        let range = range(0, 0, 0, 0);
        assert!(extract_function(&test_uri(), source, &range).is_none());
    }

    // ── f-string conversion ─────────────────────────────────────────────────

    #[test]
    fn test_convert_fstring_to_format() {
        let source = "x = f\"hello {name}\"\n";
        let range = range(0, 4, 0, 4);
        let uri = test_uri();
        let actions = convert_fstring(&uri, source, &range);

        let to_format = actions.iter().find(|a| a.title.contains(".format()"));
        assert!(to_format.is_some(), "should offer f-string to .format()");
    }

    #[test]
    fn test_convert_format_to_fstring() {
        let source = "x = \"hello {}\".format(name)\n";
        let range = range(0, 4, 0, 4);
        let uri = test_uri();
        let actions = convert_fstring(&uri, source, &range);

        let to_fstring = actions.iter().find(|a| a.title.contains("f-string"));
        assert!(to_fstring.is_some(), "should offer .format() to f-string");
    }

    // ── Literal conversions ─────────────────────────────────────────────────

    #[test]
    fn test_convert_dict_to_literal() {
        let source = "x = dict(a=1, b=2)\n";
        let range = range(0, 4, 0, 4);
        let uri = test_uri();
        let actions = convert_literals(&uri, source, &range);

        let dict_action = actions.iter().find(|a| a.title.contains("dict"));
        assert!(dict_action.is_some(), "should offer dict() to literal");
    }

    #[test]
    fn test_convert_list_to_literal() {
        let source = "x = list()\n";
        let range = range(0, 4, 0, 4);
        let uri = test_uri();
        let actions = convert_literals(&uri, source, &range);

        let list_action = actions.iter().find(|a| a.title.contains("list"));
        assert!(list_action.is_some(), "should offer list() to literal");
    }

    // ── Ternary conversion ──────────────────────────────────────────────────

    #[test]
    fn test_convert_ternary_to_if_else() {
        let source = "    x = val_a if condition else val_b\n";
        let range = range(0, 4, 0, 4);
        let uri = test_uri();
        let actions = convert_ternary(&uri, source, &range);

        let to_block = actions.iter().find(|a| a.title.contains("if/else"));
        assert!(to_block.is_some(), "should offer ternary to if/else block");
    }

    // ── Inline variable ─────────────────────────────────────────────────────

    #[test]
    fn test_inline_variable_simple() {
        let source = "temp = calculate()\nresult = temp + 1\n";
        let range = range(0, 0, 0, 0);
        let uri = test_uri();
        let action = inline_variable(&uri, source, &range);
        assert!(
            action.is_some(),
            "should offer inline variable for simple assignment"
        );

        let action = action.unwrap();
        assert_eq!(action.title, "Inline variable (basilisk)");
    }

    #[test]
    fn test_inline_variable_no_usages_returns_none() {
        let source = "temp = calculate()\n";
        let range = range(0, 0, 0, 0);
        assert!(inline_variable(&test_uri(), source, &range).is_none());
    }

    // ── Helper tests ────────────────────────────────────────────────────────

    #[test]
    fn test_find_matching_bracket() {
        let text = "Union[int, list[str]]";
        let after = "Union[".len();
        let close = helpers::find_matching_bracket(text, after);
        assert_eq!(close, Some(20));
    }

    #[test]
    fn test_split_type_args_nested() {
        let text = "int, list[str], dict[str, int]";
        let parts = helpers::split_type_args(text);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].trim(), "int");
        assert_eq!(parts[1].trim(), "list[str]");
        assert_eq!(parts[2].trim(), "dict[str, int]");
    }
}
