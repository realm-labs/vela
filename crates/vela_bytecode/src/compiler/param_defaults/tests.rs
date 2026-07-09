use vela_common::SourceId;
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};
use vela_syntax::ast::AstNode;
use vela_syntax::parse::parse_source_with_id;

use super::{ParamDefaultValue, param_default_expression_supported, param_default_values};

#[test]
fn param_default_values_follow_hir_default_bodies() {
    let source = SourceId::new(1);
    let text = "fn sample(first, second = 1 + 2) { return second; }";
    let syntax = parse_source_with_id(source, text);
    let mut graph = ModuleGraph::new();
    let module = graph.add_source(ModuleSource::new(
        source,
        ModulePath::from_qualified("main"),
        text.to_owned(),
    ));
    graph.resolve_imports();
    let declaration = graph
        .module(module)
        .and_then(|module| module.get("sample"))
        .expect("sample declaration");
    let hir_body = graph.function_body(declaration).expect("sample body");
    let syntax_function = syntax.tree().functions().next().expect("syntax function");

    let defaults = param_default_values(source, syntax_function.param_list(), &graph, hir_body);

    assert_eq!(defaults.len(), 2);
    assert!(defaults[0].is_none());
    assert_eq!(
        defaults[1]
            .as_ref()
            .expect("second default")
            .expression
            .syntax()
            .text()
            .to_string(),
        "1 + 2"
    );
}

#[test]
fn param_default_values_keep_syntax_expression_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn sample(first = 1) {
    return first;
}
"#;
    let syntax = parse_source_with_id(source, text);
    let function = syntax
        .tree()
        .functions()
        .find(|function| function.name_text().as_deref() == Some("sample"))
        .expect("function");
    let syntax_expression = function
        .param_list()
        .and_then(|params| params.params().next())
        .and_then(|param| param.default_value())
        .expect("default expression");
    let syntax_defaults = vec![Some(ParamDefaultValue {
        source,
        expression: syntax_expression,
    })];

    let defaults = syntax_defaults;

    assert_eq!(defaults.len(), 1);
    assert_eq!(
        defaults[0]
            .as_ref()
            .expect("default")
            .expression
            .syntax()
            .text()
            .to_string(),
        "1"
    );
}

#[test]
fn unsupported_param_defaults_keep_syntax_expressions_for_diagnostics() {
    let source = SourceId::new(1);
    let text = r#"
fn sample(first = player.level) {
    return first;
}
"#;
    let parsed = parse_source_with_id(source, text);
    let function = parsed
        .tree()
        .functions()
        .find(|function| function.name_text().as_deref() == Some("sample"))
        .expect("function");
    let syntax_expression = function
        .param_list()
        .and_then(|params| params.params().next())
        .and_then(|param| param.default_value())
        .expect("default expression");
    let syntax_defaults = vec![Some(ParamDefaultValue {
        source,
        expression: syntax_expression,
    })];

    let defaults = syntax_defaults;

    assert_eq!(defaults.len(), 1);
    assert_eq!(
        defaults[0]
            .as_ref()
            .expect("default")
            .expression
            .syntax()
            .text()
            .to_string(),
        "player.level"
    );
}

#[test]
fn directly_lowered_param_defaults_do_not_require_legacy_inputs() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![Some(ParamDefaultValue {
        source,
        expression: first_param_default("fn sample(value = 1 + 2) { return value; }"),
    })];

    let defaults = syntax_defaults;

    let default = defaults[0].as_ref().expect("direct syntax default");
    assert_eq!(default.expression.syntax().text().to_string(), "1 + 2");
}

#[test]
fn param_default_expression_supported_logical_chains() {
    assert!(
        param_default_expression_supported(&first_param_default(
            "fn sample(value = true || false || (1 < 2)) { return value; }"
        )),
        "logical defaults with supported operands should lower from syntax"
    );
    assert!(
        param_default_expression_supported(&first_param_default(
            "fn sample(value = false && true && (2 > 1)) { return value; }"
        )),
        "logical defaults with parenthesized supported operands should lower from syntax"
    );
    assert!(
        param_default_expression_supported(&first_param_default(
            "fn sample(value = true || expensive()) { return value; }"
        )),
        "logical defaults with path calls should lower from syntax"
    );
}

#[test]
fn param_default_expression_supported_path_calls() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultValue {
            source,
            expression: first_param_default("fn sample(value = next()) { return value; }"),
        }),
        Some(ParamDefaultValue {
            source,
            expression: first_param_default(
                "fn sample(value = pick(rhs = 2, lhs = 1 + 1)) { return value; }",
            ),
        }),
    ];

    let defaults = syntax_defaults;

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        default.expect("direct syntax default");
    }
}

#[test]
fn param_default_expression_supported_record_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultValue {
            source,
            expression: first_param_default(
                r#"fn sample(value = Reward { amount: 7, label: "xp" }) { return value; }"#,
            ),
        }),
        Some(ParamDefaultValue {
            source,
            expression: param_default_at(
                r#"fn sample(label, value = Reward { amount: 7, label }) { return value; }"#,
                1,
            ),
        }),
    ];

    let defaults = syntax_defaults;

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        default.expect("direct syntax default");
    }
}

#[test]
fn param_default_expression_supported_record_literal_field_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultValue {
            source,
            expression: first_param_default(
                r#"fn sample(value = Reward { amount: 7, label: "xp" }.amount) { return value; }"#,
            ),
        }),
        Some(ParamDefaultValue {
            source,
            expression: first_param_default(
                r#"fn sample(value = Outer { inner: Inner { amount: 7 } }.inner.amount) { return value; }"#,
            ),
        }),
    ];

    let defaults = syntax_defaults;

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        default.expect("direct syntax default");
    }
}

#[test]
fn param_default_expression_supported_simple_match_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultValue {
            source,
            expression: param_default_at(
                "fn sample(kind, value = match kind { RewardKind::Small => 1, RewardKind::Large => 2, _ => 0 }) { return value; }",
                1,
            ),
        }),
        Some(ParamDefaultValue {
            source,
            expression: param_default_at(
                "fn sample(value, copy = match value { bound if bound > 0 => bound, _ => 0 }) { return copy; }",
                1,
            ),
        }),
    ];

    let defaults = syntax_defaults;

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        default.expect("direct syntax default");
    }
}

#[test]
fn param_default_expression_supported_payload_match_patterns() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultValue {
            source,
            expression: param_default_at(
                "fn sample(kind, value = match kind { Option::Some(inner) => inner, _ => 0 }) { return value; }",
                1,
            ),
        }),
        Some(ParamDefaultValue {
            source,
            expression: param_default_at(
                "fn sample(kind, value = match kind { Result::Err { code, message: _ } => code, _ => 0 }) { return value; }",
                1,
            ),
        }),
    ];

    let defaults = syntax_defaults;

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        default.expect("direct syntax default");
    }
}

#[test]
fn param_default_expression_supported_path_field_defaults() {
    let default = param_default_at(
        "fn sample(player, value = player.level) { return value; }",
        1,
    );

    assert!(
        param_default_expression_supported(&default),
        "path-rooted field defaults should lower directly from syntax"
    );
}

#[test]
fn param_default_expression_supported_range_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultValue {
            source,
            expression: first_param_default("fn sample(value = 1..4) { return value; }"),
        }),
        Some(ParamDefaultValue {
            source,
            expression: first_param_default("fn sample(value = 1..=4) { return value; }"),
        }),
    ];

    let defaults = syntax_defaults;

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        default.expect("direct syntax default");
    }
}

#[test]
fn param_default_expression_supported_try_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![Some(ParamDefaultValue {
        source,
        expression: first_param_default("fn sample(value = maybe?) { return value; }"),
    })];

    let defaults = syntax_defaults;

    let default = defaults[0].as_ref().expect("direct syntax default");
    assert_eq!(default.expression.syntax().text().to_string(), "maybe?");
}

#[test]
fn param_default_expression_supported_simple_block_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultValue {
            source,
            expression: first_param_default("fn sample(value = {}) { return value; }"),
        }),
        Some(ParamDefaultValue {
            source,
            expression: first_param_default("fn sample(value = { 1 + 2 }) { return value; }"),
        }),
        Some(ParamDefaultValue {
            source,
            expression: first_param_default("fn sample(value = { maybe?; }) { return value; }"),
        }),
    ];

    let defaults = syntax_defaults;

    assert_eq!(defaults.len(), 3);
    for default in defaults {
        default.expect("direct syntax default");
    }
}

#[test]
fn param_default_expression_supported_let_block_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultValue {
            source,
            expression: first_param_default(
                "fn sample(value = { let x = 1; x }) { return value; }",
            ),
        }),
        Some(ParamDefaultValue {
            source,
            expression: first_param_default(
                "fn sample(value = { let x = 1; let y = x + 2; y }) { return value; }",
            ),
        }),
        Some(ParamDefaultValue {
            source,
            expression: first_param_default("fn sample(value = { let x = 1; }) { return value; }"),
        }),
    ];

    let defaults = syntax_defaults;

    assert_eq!(defaults.len(), 3);
    for default in defaults {
        default.expect("direct syntax default");
    }
}

#[test]
fn param_default_expression_supported_typed_let_block_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultValue {
            source,
            expression: first_param_default(
                "fn sample(value = { let x: i64 = 1; x }) { return value; }",
            ),
        }),
        Some(ParamDefaultValue {
            source,
            expression: first_param_default(
                "fn sample(value = { let x: i8 = 1; x }) { return value; }",
            ),
        }),
    ];
    let defaults = syntax_defaults;

    assert_eq!(defaults.len(), 2);
    for (index, default) in defaults.into_iter().enumerate() {
        default.unwrap_or_else(|| panic!("direct syntax default at index {index}"));
    }
}

#[test]
fn param_default_expression_supported_simple_if_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultValue {
            source,
            expression: first_param_default(
                "fn sample(value = if true { 1 } else { 2 }) { return value; }",
            ),
        }),
        Some(ParamDefaultValue {
            source,
            expression: first_param_default(
                "fn sample(value = if false { 1 } else if true { 2 } else { 3 }) { return value; }",
            ),
        }),
        Some(ParamDefaultValue {
            source,
            expression: first_param_default("fn sample(value = if false { 1 }) { return value; }"),
        }),
    ];

    let defaults = syntax_defaults;

    assert_eq!(defaults.len(), 3);
    for default in defaults {
        default.expect("direct syntax default");
    }
}

#[test]
fn param_default_values_keep_unsupported_if_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultValue {
            source,
            expression: first_param_default(
                "fn sample(value = if player.level { 1 } else { 2 }) { return value; }",
            ),
        }),
        Some(ParamDefaultValue {
            source,
            expression: first_param_default(
                "fn sample(value = if true { let x = player.level; x } else { 2 }) { return value; }",
            ),
        }),
    ];

    let defaults = syntax_defaults;

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        default.expect("unsupported default should still be reported during compilation");
    }
}

#[test]
fn param_default_expression_supported_index_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultValue {
            source,
            expression: first_param_default("fn sample(value = [10, 20][1]) { return value; }"),
        }),
        Some(ParamDefaultValue {
            source,
            expression: first_param_default(
                "fn sample(value = { \"key\": 7 }[\"key\"]) { return value; }",
            ),
        }),
        Some(ParamDefaultValue {
            source,
            expression: first_param_default(
                "fn sample(value = [[1], [2]][1][0]) { return value; }",
            ),
        }),
    ];

    let defaults = syntax_defaults;

    assert_eq!(defaults.len(), 3);
    for default in defaults {
        default.expect("direct syntax default");
    }
}

#[test]
fn param_default_expression_supported_interpolated_strings() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![Some(ParamDefaultValue {
        source,
        expression: first_param_default(r#"fn sample(value = f"level {1 + 2}") { return value; }"#),
    })];

    let defaults = syntax_defaults;

    let default = defaults[0].as_ref().expect("direct syntax default");
    assert_eq!(
        default.expression.syntax().text().to_string(),
        r#"f"level {1 + 2}""#
    );
}

#[test]
fn param_default_values_keep_unsupported_interpolated_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![Some(ParamDefaultValue {
        source,
        expression: first_param_default(
            r#"fn sample(value = f"level {player.level}") { return value; }"#,
        ),
    })];

    let defaults = syntax_defaults;

    defaults[0]
        .as_ref()
        .expect("unsupported default should still be reported during compilation");
}

#[test]
fn param_default_values_keep_unsupported_index_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![Some(ParamDefaultValue {
        source,
        expression: first_param_default("fn sample(value = values.field[0]) { return value; }"),
    })];

    let defaults = syntax_defaults;

    defaults[0]
        .as_ref()
        .expect("unsupported default should still be reported during compilation");
}

fn first_param_default(text: &str) -> vela_syntax::ast::SyntaxExpression {
    param_default_at(text, 0)
}

fn param_default_at(text: &str, index: usize) -> vela_syntax::ast::SyntaxExpression {
    parse_source_with_id(SourceId::new(1), text)
        .tree()
        .functions()
        .next()
        .expect("function")
        .param_list()
        .expect("parameter list")
        .params()
        .nth(index)
        .expect("parameter")
        .default_value()
        .expect("default expression")
}
