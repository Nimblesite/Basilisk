; Top-level functions
(function_definition
  name: (identifier) @name) @item

; Top-level async functions
(function_definition
  "async"
  name: (identifier) @name) @item

; Classes
(class_definition
  name: (identifier) @name) @item

; Methods inside classes
(class_definition
  body: (block
    (function_definition
      name: (identifier) @name) @item))

; Decorated definitions
(decorated_definition
  (function_definition
    name: (identifier) @name)) @item

(decorated_definition
  (class_definition
    name: (identifier) @name)) @item
