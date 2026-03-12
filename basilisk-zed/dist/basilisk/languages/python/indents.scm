; Blocks that increase indentation
[
  (if_statement)
  (elif_clause)
  (else_clause)
  (for_statement)
  (while_statement)
  (with_statement)
  (try_statement)
  (except_clause)
  (finally_clause)
  (function_definition)
  (class_definition)
  (match_statement)
  (case_clause)
] @indent

; Brackets also indent
(parenthesized_expression) @indent
(list) @indent
(dictionary) @indent
(set) @indent
(tuple) @indent
(argument_list) @indent
(parameters) @indent

; Dedent after return/break/continue/pass/raise
[
  (return_statement)
  (break_statement)
  (continue_statement)
  (pass_statement)
  (raise_statement)
] @dedent
