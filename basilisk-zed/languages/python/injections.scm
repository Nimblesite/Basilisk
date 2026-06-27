; Language injections (SQL in strings, regex patterns). Implements [ZED-TREESITTER].

; SQL in string literals (heuristic: strings starting with SELECT, INSERT, etc.)
((string
  (string_content) @injection.content)
  (#match? @injection.content "^\\s*(SELECT|INSERT|UPDATE|DELETE|CREATE|ALTER|DROP|WITH)\\b")
  (#set! injection.language "sql"))

; Regex patterns in re.compile() and re.match() etc.
(call
  function: (attribute
    object: (identifier) @_re
    attribute: (identifier) @_method)
  arguments: (argument_list
    (string (string_content) @injection.content))
  (#eq? @_re "re")
  (#any-of? @_method "compile" "match" "search" "findall" "finditer" "sub" "subn" "fullmatch" "split")
  (#set! injection.language "regex"))
