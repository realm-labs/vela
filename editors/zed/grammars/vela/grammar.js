const PREC = {
  assign: 1,
  range: 2,
  or: 3,
  and: 4,
  equality: 5,
  comparison: 6,
  additive: 7,
  multiplicative: 8,
  unary: 9,
  postfix: 10,
  call: 11,
};

const commaSep = (rule) => optional(seq(rule, repeat(seq(",", rule)), optional(",")));
const commaSep1 = (rule) => seq(rule, repeat(seq(",", rule)), optional(","));

module.exports = grammar({
  name: "vela",

  extras: ($) => [/[ \t\r\n]+/, $.shebang, $.line_comment, $.block_comment],

  word: ($) => $.identifier,

  conflicts: ($) => [
    [$.path, $.identifier],
    [$.path_expression, $.record_literal],
    [$.map_literal, $.block],
    [$.record_pattern, $.path_pattern],
    [$.tuple_pattern, $.path_pattern],
    [$.record_field_value, $.map_entry],
    [$.named_argument, $.assignment_expression],
  ],

  rules: {
    source_file: ($) => repeat($._item),

    shebang: (_) => token(seq("#!", /[^\n]*/)),
    line_comment: (_) => token(seq("//", /[^\n]*/)),
    block_comment: ($) =>
      seq(
        "/*",
        repeat(choice(/[^/*]+/, /\/[^*]/, /\*[^/]/, $.block_comment)),
        "*/",
      ),
    _item: ($) =>
      seq(
        repeat($.attribute),
        optional($.visibility),
        choice(
          $.use_declaration,
          $.const_declaration,
          $.global_declaration,
          $.function_declaration,
          $.struct_declaration,
          $.enum_declaration,
          $.trait_declaration,
          $.impl_declaration,
        ),
        optional(";"),
      ),

    visibility: (_) => "pub",

    use_declaration: ($) =>
      seq("use", field("path", $.path), optional(seq("as", field("alias", $.identifier)))),

    const_declaration: ($) =>
      seq(
        "const",
        field("name", $.identifier),
        optional($.type_annotation),
        "=",
        field("value", $._expression),
      ),

    global_declaration: ($) =>
      seq("global", field("name", $.identifier), field("type", $.type_annotation)),

    function_declaration: ($) =>
      seq(
        "fn",
        field("name", $.identifier),
        field("parameters", $.parameter_list),
        optional(field("return_type", $.return_type)),
        field("body", $.block),
      ),

    parameter_list: ($) => seq("(", commaSep($.parameter), ")"),

    parameter: ($) =>
      seq(
        field("name", $.identifier),
        optional(field("type", $.type_annotation)),
        optional(field("default", $.default_value)),
      ),

    default_value: ($) => seq("=", $._expression),

    return_type: ($) => seq("->", $._type_hint),

    struct_declaration: ($) =>
      seq("struct", field("name", $.identifier), field("body", $.field_declaration_list)),

    field_declaration_list: ($) => seq("{", repeat(seq($.field_declaration, optional(","))), "}"),

    field_declaration: ($) =>
      seq(
        repeat($.attribute),
        field("name", $.identifier),
        optional(field("type", $.type_annotation)),
        optional(field("default", $.default_value)),
      ),

    enum_declaration: ($) =>
      seq("enum", field("name", $.identifier), field("body", $.variant_declaration_list)),

    variant_declaration_list: ($) => seq("{", repeat(seq($.variant_declaration, optional(","))), "}"),

    variant_declaration: ($) =>
      seq(
        repeat($.attribute),
        field("name", $.identifier),
        optional(choice($.tuple_field_list, $.record_field_list)),
      ),

    tuple_field_list: ($) => seq("(", commaSep($.parameter), ")"),

    record_field_list: ($) => seq("{", repeat(seq($.field_declaration, optional(","))), "}"),

    trait_declaration: ($) =>
      seq("trait", field("name", $.identifier), field("body", $.trait_body)),

    trait_body: ($) => seq("{", repeat($.trait_item), "}"),

    trait_item: ($) =>
      seq(
        repeat($.attribute),
        "fn",
        field("name", $.identifier),
        field("parameters", $.parameter_list),
        optional(field("return_type", $.return_type)),
        choice(field("body", $.block), ";"),
      ),

    impl_declaration: ($) =>
      seq(
        "impl",
        field("target", $.path),
        optional(seq("for", field("for_type", $.path))),
        field("body", $.impl_body),
      ),

    impl_body: ($) => seq("{", repeat($.impl_item), "}"),

    impl_item: ($) => seq(repeat($.attribute), $.function_declaration),

    attribute: ($) => seq("#", "[", field("path", $.path), optional($.attribute_arguments), "]"),

    attribute_arguments: ($) => seq("(", commaSep($.attribute_argument), ")"),

    attribute_argument: ($) =>
      choice(seq(field("name", $.identifier), "=", field("value", $.attribute_value)), $.attribute_value),

    attribute_value: ($) =>
      choice($.literal, $.path, $.attribute_array, $.attribute_map),

    attribute_array: ($) => seq("[", commaSep($.attribute_value), "]"),

    attribute_map: ($) => seq("{", commaSep($.attribute_map_entry), "}"),

    attribute_map_entry: ($) => seq(field("key", $.identifier), ":", field("value", $.attribute_value)),

    type_annotation: ($) => seq(":", $._type_hint),

    _type_hint: ($) => $.type_path,

    builtin_type: (_) =>
      choice(
        "Any",
        "null",
        "bool",
        "char",
        "i8",
        "i16",
        "i32",
        "i64",
        "u8",
        "u16",
        "u32",
        "u64",
        "f32",
        "f64",
        "String",
        "Bytes",
        "Array",
        "Map",
        "Set",
        "Range",
        "Iterator",
        "Function",
        "Closure",
        "Option",
        "Result",
      ),

    type_path: ($) => seq(choice($.builtin_type, $.path), optional($.type_arguments)),

    type_arguments: ($) => seq("<", commaSep1($._type_hint), ">"),

    block: ($) => seq("{", repeat($._statement), "}"),

    _statement: ($) =>
      seq(
        repeat($.attribute),
        choice(
          $.let_statement,
          $.return_statement,
          $.break_statement,
          $.continue_statement,
          $.for_statement,
          $.expression_statement,
        ),
      ),

    let_statement: ($) =>
      seq(
        "let",
        field("name", $.identifier),
        optional(field("type", $.type_annotation)),
        optional(seq("=", field("value", $._expression))),
        optional(";"),
      ),

    return_statement: ($) => prec.right(seq("return", optional($._expression), optional(";"))),

    break_statement: (_) => prec.right(seq("break", optional(";"))),

    continue_statement: (_) => prec.right(seq("continue", optional(";"))),

    for_statement: ($) =>
      seq(
        "for",
        field("binding", $.for_binding),
        "in",
        field("iterable", $._expression),
        field("body", $.block),
      ),

    for_binding: ($) => choice($._pattern, seq($._pattern, ",", $._pattern)),

    expression_statement: ($) => seq($._expression, optional(";")),

    _expression: ($) =>
      choice(
        $.assignment_expression,
        $.binary_expression,
        $.unary_expression,
        $.try_expression,
        $.call_expression,
        $.field_expression,
        $.index_expression,
        $.path_expression,
        $.record_literal,
        $.array_literal,
        $.map_literal,
        $.lambda_expression,
        $.if_expression,
        $.match_expression,
        $.block,
        $.parenthesized_expression,
        $.literal,
      ),

    assignment_expression: ($) =>
      prec.right(
        PREC.assign,
        seq(
          field("left", $._expression),
          field("operator", choice("=", "+=", "-=", "*=", "/=", "%=")),
          field("right", $._expression),
        ),
      ),

    binary_expression: ($) => {
      const table = [
        [PREC.or, "||"],
        [PREC.and, "&&"],
        [PREC.equality, choice("==", "!=", "===", "!==")],
        [PREC.comparison, choice("<", "<=", ">", ">=")],
        [PREC.range, choice("..", "..=")],
        [PREC.additive, choice("+", "-")],
        [PREC.multiplicative, choice("*", "/", "%")],
      ];
      return choice(
        ...table.map(([precedence, operator]) =>
          prec.left(precedence, seq(field("left", $._expression), field("operator", operator), field("right", $._expression))),
        ),
      );
    },

    unary_expression: ($) =>
      prec(PREC.unary, seq(field("operator", choice("!", "-")), field("argument", $._expression))),

    try_expression: ($) => prec.left(PREC.postfix, seq(field("argument", $._expression), "?")),

    call_expression: ($) =>
      prec.left(PREC.call, seq(field("function", $._expression), field("arguments", $.argument_list))),

    argument_list: ($) => seq("(", commaSep($.argument), ")"),

    argument: ($) => choice($.named_argument, $._expression),

    named_argument: ($) => prec(1, seq(field("name", $.identifier), "=", field("value", $._expression))),

    field_expression: ($) =>
      prec.left(PREC.postfix, seq(field("object", $._expression), ".", field("field", $.identifier))),

    index_expression: ($) =>
      prec.left(PREC.postfix, seq(field("object", $._expression), "[", field("index", $._expression), "]")),

    path_expression: ($) => $.path,

    parenthesized_expression: ($) => seq("(", $._expression, ")"),

    array_literal: ($) => seq("[", commaSep($._expression), "]"),

    map_literal: ($) => seq("{", commaSep($.map_entry), "}"),

    map_entry: ($) =>
      prec(
        1,
        seq(
          field("key", choice($.identifier, $.string_literal, $.char_literal, $.integer_literal, $.float_literal, $.path)),
          ":",
          field("value", $._expression),
        ),
      ),

    record_literal: ($) => seq(field("type", $.path), field("body", $.record_literal_body)),

    record_literal_body: ($) => seq("{", commaSep($.record_field_value), "}"),

    record_field_value: ($) => seq(field("name", $.identifier), optional(seq(":", field("value", $._expression)))),

    lambda_expression: ($) =>
      seq(
        "|",
        commaSep($.lambda_parameter),
        "|",
        field("body", $._expression),
      ),

    lambda_parameter: ($) => seq(field("name", $.identifier), optional(field("type", $.type_annotation))),

    if_expression: ($) =>
      seq(
        "if",
        field("condition", $._expression),
        field("consequence", $.block),
        optional(seq("else", field("alternative", choice($.if_expression, $.block)))),
      ),

    match_expression: ($) =>
      seq("match", field("value", $._expression), "{", repeat($.match_arm), "}"),

    match_arm: ($) =>
      seq(
        field("pattern", $._pattern),
        optional(seq("if", field("guard", $._expression))),
        "=>",
        field("body", choice($.return_statement, $.break_statement, $.continue_statement, $._expression)),
        optional(choice(",", ";")),
      ),

    literal: ($) =>
      choice(
        $.boolean_literal,
        $.null_literal,
        $.integer_literal,
        $.float_literal,
        $.string_literal,
        $.multiline_string_literal,
        $.interpolated_string_literal,
        $.char_literal,
      ),

    boolean_literal: (_) => choice("true", "false"),

    null_literal: (_) => "null",

    integer_literal: (_) =>
      token(choice(/0x[0-9a-fA-F_]+([iu](8|16|32|64))?/, /0b[01_]+([iu](8|16|32|64))?/, /[0-9][0-9_]*([iu](8|16|32|64))?/)),

    float_literal: (_) =>
      token(/[0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9][0-9_]*)?(f32|f64)?/),

    string_literal: (_) => token(seq('"', repeat(choice(/[^"\\\n]+/, /\\./)), '"')),

    multiline_string_literal: (_) => token(seq('"""', repeat(choice(/[^"]+/, /"[^"]/, /""[^"]/)), '"""')),

    interpolated_string_literal: (_) =>
      token(
        seq(
          "f",
          choice(
            seq('"', repeat(choice(/[^"\\\n]+/, /\\./)), '"'),
            seq('"""', repeat(choice(/[^"]+/, /"[^"]/, /""[^"]/)), '"""'),
          ),
        ),
      ),

    char_literal: (_) => token(seq("'", choice(/[^'\\\n]/, /\\./), "'")),

    _pattern: ($) =>
      choice(
        $.wildcard_pattern,
        $.literal_pattern,
        $.binding_pattern,
        $.tuple_pattern,
        $.record_pattern,
        $.path_pattern,
      ),

    wildcard_pattern: (_) => "_",

    literal_pattern: ($) =>
      choice($.boolean_literal, $.null_literal, $.integer_literal, $.float_literal, $.string_literal, $.char_literal),

    binding_pattern: ($) => prec(1, $.identifier),

    path_pattern: ($) => $.path,

    tuple_pattern: ($) => seq(field("variant", $.path), "(", commaSep($._pattern), ")"),

    record_pattern: ($) => seq(field("variant", $.path), "{", commaSep($.record_pattern_field), "}"),

    record_pattern_field: ($) => seq(field("name", $.identifier), optional(seq(":", field("pattern", $._pattern)))),

    path: ($) => seq($.identifier, repeat(seq("::", $.identifier))),

    identifier: (_) => /[A-Za-z_][A-Za-z0-9_]*/,
  },
});
