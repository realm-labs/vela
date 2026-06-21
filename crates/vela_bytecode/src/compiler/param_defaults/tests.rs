use vela_common::{SourceId, Span};
use vela_syntax::ast::{AstNode, Expr, ExprKind};
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
    let fallback_expr = Expr {
        kind: ExprKind::Error,
        span: Span::new(source, 16, 17),
    };

    let defaults = param_default_values(&syntax_defaults, &[Some(&fallback_expr)]);

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
    assert!(
        defaults[0].as_ref().expect("default").fallback.is_none(),
        "directly lowered CST defaults should not retain a legacy expression fallback"
    );
}

#[test]
fn mismatched_param_defaults_do_not_pair_by_index() {
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
    let fallback_expr = Expr {
        kind: ExprKind::Error,
        span: Span::new(source, 1000, 1001),
    };

    let defaults = param_default_values(&syntax_defaults, &[Some(&fallback_expr)]);

    assert_eq!(defaults.len(), 1);
    assert!(
        defaults[0].is_none(),
        "unsupported defaults must not receive mismatched legacy fallbacks by index"
    );
}

#[test]
fn directly_lowered_param_defaults_do_not_require_legacy_fallbacks() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![Some(ParamDefaultExpression {
        source,
        expression: first_param_default("fn cst(value = 1 + 2) { return value; }"),
    })];

    let defaults = param_default_values(&syntax_defaults, &[]);

    let default = defaults[0].as_ref().expect("direct CST default");
    assert_eq!(default.expression.syntax().text().to_string(), "1 + 2");
    assert!(
        default.fallback.is_none(),
        "directly lowered CST defaults should not depend on a legacy expression"
    );
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

    let defaults = param_default_values(&syntax_defaults, &[]);

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        assert!(
            default.expect("direct CST default").fallback.is_none(),
            "path call defaults with supported arguments should lower directly from CST"
        );
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

    let defaults = param_default_values(&syntax_defaults, &[]);

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        assert!(
            default.expect("direct CST default").fallback.is_none(),
            "record defaults with supported fields should lower directly from CST"
        );
    }
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

    let defaults = param_default_values(&syntax_defaults, &[]);

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        assert!(
            default.expect("direct CST default").fallback.is_none(),
            "range defaults should be directly lowerable from CST"
        );
    }
}

#[test]
fn param_default_cst_lowering_covers_try_expressions() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![Some(ParamDefaultExpression {
        source,
        expression: first_param_default("fn cst(value = maybe?) { return value; }"),
    })];

    let defaults = param_default_values(&syntax_defaults, &[]);

    let default = defaults[0].as_ref().expect("direct CST default");
    assert_eq!(default.expression.syntax().text().to_string(), "maybe?");
    assert!(
        default.fallback.is_none(),
        "try defaults should be directly lowerable from CST"
    );
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

    let defaults = param_default_values(&syntax_defaults, &[]);

    assert_eq!(defaults.len(), 3);
    for default in defaults {
        assert!(
            default.expect("direct CST default").fallback.is_none(),
            "simple block defaults should be directly lowerable from CST"
        );
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

    let defaults = param_default_values(&syntax_defaults, &[]);

    assert_eq!(defaults.len(), 3);
    for default in defaults {
        assert!(
            default.expect("direct CST default").fallback.is_none(),
            "let block defaults should be directly lowerable from CST"
        );
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
    let defaults = param_default_values(&syntax_defaults, &[]);

    assert_eq!(defaults.len(), 2);
    for (index, default) in defaults.into_iter().enumerate() {
        assert!(
            default
                .unwrap_or_else(|| panic!("direct CST default at index {index}"))
                .fallback
                .is_none(),
            "typed let block defaults should be directly lowerable from CST"
        );
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

    let defaults = param_default_values(&syntax_defaults, &[]);

    assert_eq!(defaults.len(), 3);
    for default in defaults {
        assert!(
            default.expect("direct CST default").fallback.is_none(),
            "simple if defaults should be directly lowerable from CST"
        );
    }
}

#[test]
fn param_default_cst_lowering_keeps_unsupported_if_fallbacks() {
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

    let defaults = param_default_values(&syntax_defaults, &[]);

    assert_eq!(defaults.len(), 2);
    for default in defaults {
        assert!(
            default.is_none(),
            "unsupported if defaults still require the temporary legacy fallback"
        );
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

    let defaults = param_default_values(&syntax_defaults, &[]);

    assert_eq!(defaults.len(), 3);
    for default in defaults {
        assert!(
            default.expect("direct CST default").fallback.is_none(),
            "index defaults with supported operands should lower directly from CST"
        );
    }
}

#[test]
fn param_default_cst_lowering_covers_interpolated_strings() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![Some(ParamDefaultExpression {
        source,
        expression: first_param_default(r#"fn cst(value = f"level {1 + 2}") { return value; }"#),
    })];

    let defaults = param_default_values(&syntax_defaults, &[]);

    let default = defaults[0].as_ref().expect("direct CST default");
    assert_eq!(
        default.expression.syntax().text().to_string(),
        r#"f"level {1 + 2}""#
    );
    assert!(
        default.fallback.is_none(),
        "interpolated string defaults with supported expressions should lower directly from CST"
    );
}

#[test]
fn param_default_cst_lowering_keeps_unsupported_interpolated_fallbacks() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![Some(ParamDefaultExpression {
        source,
        expression: first_param_default(
            r#"fn cst(value = f"level {player.level}") { return value; }"#,
        ),
    })];

    let defaults = param_default_values(&syntax_defaults, &[]);

    assert!(
        defaults[0].is_none(),
        "interpolated defaults still require the temporary legacy fallback when an expression is unsupported"
    );
}

#[test]
fn param_default_cst_lowering_keeps_unsupported_index_fallbacks() {
    let source = SourceId::new(1);
    let syntax_defaults = vec![Some(ParamDefaultExpression {
        source,
        expression: first_param_default("fn cst(value = values.field[0]) { return value; }"),
    })];

    let defaults = param_default_values(&syntax_defaults, &[]);

    assert!(
        defaults[0].is_none(),
        "index defaults still require the temporary legacy fallback when an operand is unsupported"
    );
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
