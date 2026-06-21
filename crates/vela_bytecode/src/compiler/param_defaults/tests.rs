use vela_common::SourceId;
use vela_syntax::ast::AstNode;
use vela_syntax::parse::parse_source_with_id as parse_syntax_source;

use crate::compiler::syntax_payloads::ParamDefaultExpression;

use super::{param_default_cst_lowering_covers, param_default_values};

#[test]
fn param_default_values_keep_cst_expression_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn cst(first = 1) {
    return first;
}
"#;
    let syntax = parse_syntax_source(source, text);
    let cst_function = syntax
        .tree()
        .functions()
        .find(|function| function.name_text().as_deref() == Some("cst"))
        .expect("CST function");
    let syntax_expression = cst_function
        .param_list()
        .and_then(|params| params.params().next())
        .and_then(|param| param.default_value())
        .expect("CST default expression");
    let syntax_defaults = vec![Some(ParamDefaultExpression {
        source,
        expression: syntax_expression,
    })];

    let defaults = param_default_values(&syntax_defaults);

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
fn unsupported_param_defaults_keep_cst_payloads_without_legacy_pairing() {
    let source = SourceId::new(1);
    let text = r#"
fn cst(first = player.level) {
    return first;
}
"#;
    let parsed = parse_syntax_source(source, text);
    let cst_function = parsed
        .tree()
        .functions()
        .find(|function| function.name_text().as_deref() == Some("cst"))
        .expect("CST function");
    let syntax_expression = cst_function
        .param_list()
        .and_then(|params| params.params().next())
        .and_then(|param| param.default_value())
        .expect("default expression");
    let syntax_defaults = vec![Some(ParamDefaultExpression {
        source,
        expression: syntax_expression,
    })];

    let defaults = param_default_values(&syntax_defaults);

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
    let syntax_defaults = vec![Some(ParamDefaultExpression {
        source,
        expression: first_param_default("fn cst(value = 1 + 2) { return value; }"),
    })];

    let defaults = param_default_values(&syntax_defaults);

    let default = defaults[0].as_ref().expect("direct CST default");
    assert_eq!(default.expression.syntax().text().to_string(), "1 + 2");
}

#[test]
fn param_default_cst_lowering_covers_logical_chains() {
    assert!(
        param_default_cst_lowering_covers(&first_param_default(
            "fn cst(value = true || false || (1 < 2)) { return value; }"
        )),
        "logical defaults with supported operands should lower from CST"
    );
    assert!(
        param_default_cst_lowering_covers(&first_param_default(
            "fn cst(value = false && true && (2 > 1)) { return value; }"
        )),
        "logical defaults with parenthesized supported operands should lower from CST"
    );
    assert!(
        param_default_cst_lowering_covers(&first_param_default(
            "fn cst(value = true || expensive()) { return value; }"
        )),
        "logical defaults with path calls should lower from CST"
    );
}

#[test]
fn param_default_cst_lowering_covers_path_calls() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default("fn cst(value = next()) { return value; }"),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default(
                "fn cst(value = pick(rhs = 2, lhs = 1 + 1)) { return value; }",
            ),
        }),
    ];

    let defaults = param_default_values(&syntax_defaults);

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        default.expect("direct CST default");
    }
}

#[test]
fn param_default_cst_lowering_covers_record_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default(
                r#"fn cst(value = Reward { amount: 7, label: "xp" }) { return value; }"#,
            ),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: param_default_at(
                r#"fn cst(label, value = Reward { amount: 7, label }) { return value; }"#,
                1,
            ),
        }),
    ];

    let defaults = param_default_values(&syntax_defaults);

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        default.expect("direct CST default");
    }
}

#[test]
fn param_default_cst_lowering_covers_record_literal_field_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default(
                r#"fn cst(value = Reward { amount: 7, label: "xp" }.amount) { return value; }"#,
            ),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default(
                r#"fn cst(value = Outer { inner: Inner { amount: 7 } }.inner.amount) { return value; }"#,
            ),
        }),
    ];

    let defaults = param_default_values(&syntax_defaults);

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        default.expect("direct CST default");
    }
}

#[test]
fn param_default_cst_lowering_covers_simple_match_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultExpression {
            source,
            expression: param_default_at(
                "fn cst(kind, value = match kind { RewardKind::Small => 1, RewardKind::Large => 2, _ => 0 }) { return value; }",
                1,
            ),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: param_default_at(
                "fn cst(value, copy = match value { bound if bound > 0 => bound, _ => 0 }) { return copy; }",
                1,
            ),
        }),
    ];

    let defaults = param_default_values(&syntax_defaults);

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        default.expect("direct CST default");
    }
}

#[test]
fn param_default_cst_lowering_covers_payload_match_patterns() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultExpression {
            source,
            expression: param_default_at(
                "fn cst(kind, value = match kind { Option::Some(inner) => inner, _ => 0 }) { return value; }",
                1,
            ),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: param_default_at(
                "fn cst(kind, value = match kind { Result::Err { code, message: _ } => code, _ => 0 }) { return value; }",
                1,
            ),
        }),
    ];

    let defaults = param_default_values(&syntax_defaults);

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        default.expect("direct CST default");
    }
}

#[test]
fn param_default_cst_lowering_rejects_path_field_defaults() {
    let default = param_default_at("fn cst(player, value = player.level) { return value; }", 1);

    assert!(
        !param_default_cst_lowering_covers(&default),
        "path-rooted field defaults are unsupported until they lower directly from CST"
    );
}

#[test]
fn param_default_cst_lowering_covers_range_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default("fn cst(value = 1..4) { return value; }"),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default("fn cst(value = 1..=4) { return value; }"),
        }),
    ];

    let defaults = param_default_values(&syntax_defaults);

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        default.expect("direct CST default");
    }
}

#[test]
fn param_default_cst_lowering_covers_try_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![Some(ParamDefaultExpression {
        source,
        expression: first_param_default("fn cst(value = maybe?) { return value; }"),
    })];

    let defaults = param_default_values(&syntax_defaults);

    let default = defaults[0].as_ref().expect("direct CST default");
    assert_eq!(default.expression.syntax().text().to_string(), "maybe?");
}

#[test]
fn param_default_cst_lowering_covers_simple_block_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default("fn cst(value = {}) { return value; }"),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default("fn cst(value = { 1 + 2 }) { return value; }"),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default("fn cst(value = { maybe?; }) { return value; }"),
        }),
    ];

    let defaults = param_default_values(&syntax_defaults);

    assert_eq!(defaults.len(), 3);
    for default in defaults {
        default.expect("direct CST default");
    }
}

#[test]
fn param_default_cst_lowering_covers_let_block_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default("fn cst(value = { let x = 1; x }) { return value; }"),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default(
                "fn cst(value = { let x = 1; let y = x + 2; y }) { return value; }",
            ),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default("fn cst(value = { let x = 1; }) { return value; }"),
        }),
    ];

    let defaults = param_default_values(&syntax_defaults);

    assert_eq!(defaults.len(), 3);
    for default in defaults {
        default.expect("direct CST default");
    }
}

#[test]
fn param_default_cst_lowering_covers_typed_let_block_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default(
                "fn cst(value = { let x: i64 = 1; x }) { return value; }",
            ),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default(
                "fn cst(value = { let x: i8 = 1; x }) { return value; }",
            ),
        }),
    ];
    let defaults = param_default_values(&syntax_defaults);

    assert_eq!(defaults.len(), 2);
    for (index, default) in defaults.into_iter().enumerate() {
        default.unwrap_or_else(|| panic!("direct CST default at index {index}"));
    }
}

#[test]
fn param_default_cst_lowering_covers_simple_if_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default(
                "fn cst(value = if true { 1 } else { 2 }) { return value; }",
            ),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default(
                "fn cst(value = if false { 1 } else if true { 2 } else { 3 }) { return value; }",
            ),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default("fn cst(value = if false { 1 }) { return value; }"),
        }),
    ];

    let defaults = param_default_values(&syntax_defaults);

    assert_eq!(defaults.len(), 3);
    for default in defaults {
        default.expect("direct CST default");
    }
}

#[test]
fn param_default_values_keep_unsupported_if_cst_payloads() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default(
                "fn cst(value = if player.level { 1 } else { 2 }) { return value; }",
            ),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default(
                "fn cst(value = if true { let x = player.level; x } else { 2 }) { return value; }",
            ),
        }),
    ];

    let defaults = param_default_values(&syntax_defaults);

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        default.expect("unsupported CST default should still be reported during compilation");
    }
}

#[test]
fn param_default_cst_lowering_covers_index_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default("fn cst(value = [10, 20][1]) { return value; }"),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default(
                "fn cst(value = { \"key\": 7 }[\"key\"]) { return value; }",
            ),
        }),
        Some(ParamDefaultExpression {
            source,
            expression: first_param_default("fn cst(value = [[1], [2]][1][0]) { return value; }"),
        }),
    ];

    let defaults = param_default_values(&syntax_defaults);

    assert_eq!(defaults.len(), 3);
    for default in defaults {
        default.expect("direct CST default");
    }
}

#[test]
fn param_default_cst_lowering_covers_interpolated_strings() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![Some(ParamDefaultExpression {
        source,
        expression: first_param_default(r#"fn cst(value = f"level {1 + 2}") { return value; }"#),
    })];

    let defaults = param_default_values(&syntax_defaults);

    let default = defaults[0].as_ref().expect("direct CST default");
    assert_eq!(
        default.expression.syntax().text().to_string(),
        r#"f"level {1 + 2}""#
    );
}

#[test]
fn param_default_values_keep_unsupported_interpolated_cst_payloads() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![Some(ParamDefaultExpression {
        source,
        expression: first_param_default(
            r#"fn cst(value = f"level {player.level}") { return value; }"#,
        ),
    })];

    let defaults = param_default_values(&syntax_defaults);

    defaults[0]
        .as_ref()
        .expect("unsupported CST default should still be reported during compilation");
}

#[test]
fn param_default_values_keep_unsupported_index_cst_payloads() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![Some(ParamDefaultExpression {
        source,
        expression: first_param_default("fn cst(value = values.field[0]) { return value; }"),
    })];

    let defaults = param_default_values(&syntax_defaults);

    defaults[0]
        .as_ref()
        .expect("unsupported CST default should still be reported during compilation");
}

fn first_param_default(text: &str) -> vela_syntax::ast::SyntaxExpression {
    param_default_at(text, 0)
}

fn param_default_at(text: &str, index: usize) -> vela_syntax::ast::SyntaxExpression {
    parse_syntax_source(SourceId::new(1), text)
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
