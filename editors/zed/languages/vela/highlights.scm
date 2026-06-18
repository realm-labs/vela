; Keywords
[
  "as"
  "break"
  "const"
  "continue"
  "else"
  "enum"
  "fn"
  "for"
  "global"
  "if"
  "impl"
  "in"
  "let"
  "match"
  "return"
  "struct"
  "trait"
  "use"
] @keyword

(visibility) @keyword

(boolean_literal) @boolean
(null_literal) @constant.builtin

; Variables
(let_statement name: (identifier) @variable)
(binding_pattern (identifier) @variable)
(path_expression (path (identifier) @variable))

((identifier) @variable.special
  (#eq? @variable.special "self"))

; Comments
(line_comment) @comment
(block_comment) @comment
(shebang) @preproc

; Literals
(string_literal) @string
(multiline_string_literal) @string
(interpolated_string_literal) @string.special
(char_literal) @string.special
(integer_literal) @number
(float_literal) @number

; Declarations
(function_declaration name: (identifier) @function)
(trait_item name: (identifier) @function)
(field_declaration name: (identifier) @property)
(variant_declaration name: (identifier) @constant)
(struct_declaration name: (identifier) @type)
(enum_declaration name: (identifier) @type)
(trait_declaration name: (identifier) @type)
(const_declaration name: (identifier) @constant)
(global_declaration name: (identifier) @constant)
(parameter name: (identifier) @variable.parameter)
(lambda_parameter name: (identifier) @variable.parameter)

; Attributes
(attribute "#" @attribute)
(attribute path: (path) @attribute)
(attribute_argument name: (identifier) @property)
(attribute_map_entry key: (identifier) @property)

; Types
(builtin_type) @type.builtin
(type_path (path) @type)
(impl_declaration target: (path) @type)
(impl_declaration for_type: (path) @type)
(record_literal type: (path) @constructor)
(tuple_pattern variant: (path) @constructor)
(record_pattern variant: (path) @constructor)

; Member and call sites
(use_declaration path: (path (identifier) @namespace))
(call_expression function: (field_expression field: (identifier) @function.method))
(call_expression function: (path_expression (path (identifier) @function)))
(field_expression field: (identifier) @property)
(named_argument name: (identifier) @variable.parameter)
(record_field_value name: (identifier) @property)
(record_pattern_field name: (identifier) @property)
(map_entry key: (identifier) @property)

; Operators and punctuation
[
  "="
  "+="
  "-="
  "*="
  "/="
  "%="
  "||"
  "&&"
  "=="
  "!="
  "==="
  "!=="
  "<"
  "<="
  ">"
  ">="
  ".."
  "..="
  "+"
  "-"
  "*"
  "/"
  "%"
  "!"
  "?"
  "=>"
  "->"
] @operator

[
  "::"
  "."
  ","
  ":"
  ";"
] @punctuation.delimiter

[
  "("
  ")"
  "{"
  "}"
  "["
  "]"
  "|"
] @punctuation.bracket
