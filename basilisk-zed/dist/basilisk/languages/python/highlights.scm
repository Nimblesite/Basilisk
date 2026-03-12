; Keywords
[
  "and" "as" "assert" "async" "await" "break" "class" "continue"
  "del" "elif" "else" "except" "finally" "for" "from" "global"
  "if" "import" "in" "is" "lambda" "nonlocal" "not" "or" "pass"
  "raise" "try" "while" "with" "yield"
] @keyword

"def" @keyword.function
"return" @keyword.return
"match" @keyword
"case" @keyword
"type" @keyword

; Builtins
((identifier) @function.builtin
  (#any-of? @function.builtin
    "abs" "all" "any" "bin" "bool" "breakpoint" "bytes" "callable"
    "chr" "classmethod" "compile" "complex" "delattr" "dict" "dir"
    "divmod" "enumerate" "eval" "exec" "filter" "float" "format"
    "frozenset" "getattr" "globals" "hasattr" "hash" "help" "hex"
    "id" "input" "int" "isinstance" "issubclass" "iter" "len"
    "list" "locals" "map" "max" "memoryview" "min" "next" "object"
    "oct" "open" "ord" "pow" "print" "property" "range" "repr"
    "reversed" "round" "set" "setattr" "slice" "sorted"
    "staticmethod" "str" "sum" "super" "tuple" "type" "vars" "zip"))

; Type builtins
((identifier) @type.builtin
  (#any-of? @type.builtin
    "int" "float" "str" "bool" "bytes" "list" "dict" "set"
    "tuple" "frozenset" "complex" "range" "bytearray" "memoryview"
    "object" "type" "None" "NotImplemented" "Ellipsis"))

; Constants
((identifier) @constant
  (#match? @constant "^[A-Z][A-Z_0-9]+$"))

(none) @constant.builtin
[(true) (false)] @boolean
(ellipsis) @constant.builtin

; Functions
(function_definition name: (identifier) @function)
(call function: (identifier) @function.call)
(call function: (attribute attribute: (identifier) @function.method.call))

; Decorators
(decorator "@" @attribute)
(decorator (identifier) @attribute)
(decorator (attribute attribute: (identifier) @attribute))

; Parameters
(parameters (identifier) @variable.parameter)
(parameters (typed_parameter (identifier) @variable.parameter))
(parameters (default_parameter name: (identifier) @variable.parameter))
(parameters (typed_default_parameter name: (identifier) @variable.parameter))
(parameters (list_splat_pattern (identifier) @variable.parameter))
(parameters (dictionary_splat_pattern (identifier) @variable.parameter))
(keyword_argument name: (identifier) @variable.parameter)

; Types (annotations)
(type (identifier) @type)
(type (attribute attribute: (identifier) @type))
(type (subscript value: (identifier) @type))

; Class definitions
(class_definition name: (identifier) @type)
(class_definition superclasses: (argument_list (identifier) @type))

; String literals
(string) @string
(string (escape_sequence) @string.escape)
(interpolation) @string.special
(format_expression) @string.special

; Numeric literals
(integer) @number
(float) @number

; Comments
(comment) @comment

; Operators
[
  "+" "-" "*" "**" "/" "//" "%" "@"
  "<<" ">>" "&" "|" "^" "~"
  "<" ">" "<=" ">=" "==" "!="
  "=" "+=" "-=" "*=" "/=" "//=" "%=" "**=" ">>=" "<<=" "&=" "|=" "^=" "@="
  "->" ":"
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," "." ";" ":"] @punctuation.delimiter

; Self / cls
((identifier) @variable.builtin
  (#any-of? @variable.builtin "self" "cls"))

; Variables (catch-all — last so specific captures take priority)
(identifier) @variable
