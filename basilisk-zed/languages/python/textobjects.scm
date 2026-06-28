; Vim text objects (functions, classes, arguments, comments). Implements [ZED-TREESITTER].

; Function text objects
(function_definition) @function.around
(function_definition body: (block) @function.inside)

; Class text objects
(class_definition) @class.around
(class_definition body: (block) @class.inside)

; Comment text objects
(comment) @comment.around

; Parameter / argument text objects
(parameters (_) @parameter.inside) @parameter.around
(argument_list (_) @parameter.inside) @parameter.around
