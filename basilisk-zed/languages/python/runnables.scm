; Run buttons for entry points and pytest functions. Implements [ZED-TREESITTER].

; if __name__ == "__main__": — script entry point
(if_statement
  condition: (comparison_operator
    (identifier) @_name
    (string) @_main)
  (#eq? @_name "__name__")
  (#eq? @_main "\"__main__\"")) @run

; pytest test functions (def test_*)
(function_definition
  name: (identifier) @_test_name
  (#match? @_test_name "^test_")) @run

; pytest test classes (class Test*)
(class_definition
  name: (identifier) @_test_class
  (#match? @_test_class "^Test")) @run

; unittest test methods
(class_definition
  body: (block
    (function_definition
      name: (identifier) @_test_method
      (#match? @_test_method "^test_")) @run))
