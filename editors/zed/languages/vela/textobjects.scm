(function_declaration
  body: (block
    "{"
    (_)*
    "}") @function.inside) @function.around

(trait_item
  body: (block
    "{"
    (_)*
    "}") @function.inside) @function.around

(struct_declaration
  body: (field_declaration_list
    "{"
    (_)*
    "}") @class.inside) @class.around

(enum_declaration
  body: (variant_declaration_list
    "{"
    (_)*
    "}") @class.inside) @class.around

(trait_declaration
  body: (trait_body
    "{"
    (_)*
    "}") @class.inside) @class.around

(impl_declaration
  body: (impl_body
    "{"
    (_)*
    "}") @class.inside) @class.around

(line_comment)+ @comment.around
(block_comment) @comment.around
