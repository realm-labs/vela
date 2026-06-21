use super::*;

mod binary;
mod calls;
mod constructors;
mod containers;
mod expression_statements;
mod field_index;
mod for_iterables;
mod if_conditions;
mod interpolated;
mod lambdas;
mod literals;
mod match_arms;
mod path_expressions;
mod wrappers;

fn with_cst_payload_compiler(
    source: &str,
    inspect: impl for<'ast> FnOnce(
        &mut Compiler<'ast, 'static>,
        legacy_payloads::FunctionBodyPayload<'ast>,
    ),
) {
    let semantic =
        parse_semantic_source(SourceId::new(1), source).expect("semantic source should parse");
    let script_function_symbols = semantic.script_function_symbols();
    let script_function_signatures = semantic.script_function_signatures();
    let type_symbols = semantic.type_symbols();
    let global_symbols = semantic.global_symbols();
    let global_slots = global_slots(&global_symbols);
    let global_type_symbols = semantic.global_type_symbols();
    let script_field_slots = semantic.script_field_slots(&type_symbols);
    let const_values = semantic.const_values().expect("const values should lower");
    let schema_defaults = semantic.schema_defaults(&type_symbols, &const_values);
    let facts = CompilerFacts {
        script_function_symbols,
        script_function_signatures,
        script_method_ids: std::collections::BTreeMap::new(),
        script_method_signatures: std::collections::BTreeMap::new(),
        derived_operator_traits: std::collections::BTreeMap::new(),
        script_field_slots,
        schema_defaults,
        type_symbols,
        global_symbols,
        global_slots,
        global_type_symbols,
        const_values,
        options: CompilerOptions::default(),
        registry: None,
    };
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let mut compiler = Compiler::new(
        payload.function.name.clone(),
        payload.function,
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");
    inspect(&mut compiler, payload);
}

#[test]
fn semantic_function_defaults_are_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn grant(base, amount = 10, bonus = amount + 1) {
    return base + amount + bonus;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("grant").expect("grant function");
    assert_cst_body(
        &payload.body,
        source,
        "{\n    return base + amount + bonus;\n}",
    );
    assert_cst_statements(
        &payload.body,
        &[(SyntaxStatementKind::Return, "return base + amount + bonus;")],
    );
    assert!(payload.param_defaults[0].is_none());
    assert_cst_param_default(&payload.param_defaults[1], source, "10");
    assert_cst_param_default(&payload.param_defaults[2], source, "amount + 1");
    let bonus_default = payload.param_defaults[2]
        .as_ref()
        .expect("bonus default payload");
    let compiler_payload = bonus_default
        .compiler_payload()
        .expect("CST-backed default should produce compiler payload");
    assert_eq!(
        compiler_payload
            .syntax_expression()
            .expect("default compiler payload syntax")
            .syntax()
            .text()
            .to_string(),
        "amount + 1",
    );

    compile_program_source(source, text).expect("CST-backed defaults should compile");
}

#[test]
fn semantic_script_method_defaults_are_cst_payloads() {
    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
struct Counter { value: i64 }
impl Counter {
    fn add(self, amount = 1) {
        self.value += amount;
    }
}
"#,
    )
    .expect("source should parse");
    let methods = semantic.script_impl_methods();
    let method = methods
        .iter()
        .find(|method| method.method_name == "add")
        .expect("script method");
    assert_cst_body(
        &method.body,
        source,
        "{\n        self.value += amount;\n    }",
    );
    assert_cst_statements(
        &method.body,
        &[(SyntaxStatementKind::Expr, "self.value += amount;")],
    );
    assert_cst_expr_statements(
        &method.body,
        &[(SyntaxExpressionKind::Assign, "self.value += amount")],
    );
    assert!(method.default_values[0].is_none());
    assert_cst_param_default(&method.default_values[1], source, "1");
    let amount_default = method.default_values[1]
        .as_ref()
        .expect("amount default payload");
    let compiler_payload = amount_default
        .compiler_payload()
        .expect("CST-backed method default should produce compiler payload");
    assert_eq!(
        compiler_payload
            .syntax_expression()
            .expect("method default compiler payload syntax")
            .syntax()
            .text()
            .to_string(),
        "1",
    );
}

#[test]
fn semantic_function_assignment_statement_expression_is_cst_payload() {
    let source = SourceId::new(1);
    let text = r#"
fn assign() {
    let total = 1;
    total += 2;
    return total;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("assign").expect("assign function");
    assert_cst_statements(
        &payload.body,
        &[
            (SyntaxStatementKind::Let, "let total = 1;"),
            (SyntaxStatementKind::Expr, "total += 2;"),
            (SyntaxStatementKind::Return, "return total;"),
        ],
    );
    assert_cst_expr_statements(
        &payload.body,
        &[(SyntaxExpressionKind::Assign, "total += 2")],
    );

    compile_program_source(source, text).expect("CST-backed assignment body should compile");
}

#[test]
fn semantic_function_assignment_value_expressions_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn assign_values() {
    let total = 0;
    total = {
        let start = 1;
        start + 1
    };
    total = if total > 0 {
        let next = total + 1;
        next
    } else {
        0
    };
    total = match total {
        0 => {
            let zero = 1;
            zero
        },
        _ => {
            total
        },
    };
    return total;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("assign_values")
        .expect("assign_values function");
    assert_cst_assignment_values(
        &payload.body,
        &[
            (
                SyntaxExpressionKind::Block,
                "{\n        let start = 1;\n        start + 1\n    }",
            ),
            (
                SyntaxExpressionKind::If,
                "if total > 0 {\n        let next = total + 1;\n        next\n    } else {\n        0\n    }",
            ),
            (
                SyntaxExpressionKind::Match,
                "match total {\n        0 => {\n            let zero = 1;\n            zero\n        },\n        _ => {\n            total\n        },\n    }",
            ),
        ],
    );
    assert_cst_assignment_value_block_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let start = 1;"),
            (SyntaxStatementKind::Expr, "start + 1"),
        ]],
    );
    assert_cst_assignment_value_if_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let next = total + 1;"),
            (SyntaxStatementKind::Expr, "next"),
        ]],
        &[vec![(SyntaxStatementKind::Expr, "0")]],
    );
    assert_cst_assignment_value_match_arm_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let zero = 1;"),
                (SyntaxStatementKind::Expr, "zero"),
            ],
            vec![(SyntaxStatementKind::Expr, "total")],
        ],
    );

    compile_program_source(source, text).expect("CST-backed assignment values should compile");
}

#[test]
fn semantic_function_call_argument_values_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn take(first, second, third) {
    return first;
}

fn call_values() {
    take(
        {
            let start = 1;
            start
        },
        if true {
            let next = 2;
            next
        } else {
            0
        },
        match 0 {
            0 => {
                let zero = 1;
                zero
            },
            _ => {
                2
            },
        },
    );
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("call_values")
        .expect("call_values function");
    assert_cst_call_argument_values(
        &payload.body,
        &[
            (
                SyntaxExpressionKind::Block,
                "{\n            let start = 1;\n            start\n        }",
            ),
            (
                SyntaxExpressionKind::If,
                "if true {\n            let next = 2;\n            next\n        } else {\n            0\n        }",
            ),
            (
                SyntaxExpressionKind::Match,
                "match 0 {\n            0 => {\n                let zero = 1;\n                zero\n            },\n            _ => {\n                2\n            },\n        }",
            ),
        ],
    );
    assert_cst_call_argument_block_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let start = 1;"),
            (SyntaxStatementKind::Expr, "start"),
        ]],
    );
    assert_cst_call_argument_if_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let next = 2;"),
            (SyntaxStatementKind::Expr, "next"),
        ]],
        &[vec![(SyntaxStatementKind::Expr, "0")]],
    );
    assert_cst_call_argument_match_arm_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let zero = 1;"),
                (SyntaxStatementKind::Expr, "zero"),
            ],
            vec![(SyntaxStatementKind::Expr, "2")],
        ],
    );

    compile_program_source(source, text).expect("CST-backed call argument values should compile");
}

#[test]
fn semantic_function_let_initializer_expression_is_cst_payload() {
    let source = SourceId::new(1);
    let text = r#"
fn choose() {
    let total = if true {
        1
    } else {
        2
    };
    return total;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("choose").expect("choose function");
    assert_cst_statements(
        &payload.body,
        &[
            (
                SyntaxStatementKind::Let,
                "let total = if true {\n        1\n    } else {\n        2\n    };",
            ),
            (SyntaxStatementKind::Return, "return total;"),
        ],
    );
    assert_cst_let_initializers(
        &payload.body,
        &[(
            SyntaxExpressionKind::If,
            "if true {\n        1\n    } else {\n        2\n    }",
        )],
    );

    compile_program_source(source, text).expect("CST-backed let initializer body should compile");
}

#[test]
fn semantic_function_return_value_expression_is_cst_payload() {
    let source = SourceId::new(1);
    let text = r#"
fn choose() {
    return if true {
        1
    } else {
        2
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("choose").expect("choose function");
    assert_cst_statements(
        &payload.body,
        &[(
            SyntaxStatementKind::Return,
            "return if true {\n        1\n    } else {\n        2\n    };",
        )],
    );
    assert_cst_return_values(
        &payload.body,
        &[(
            SyntaxExpressionKind::If,
            "if true {\n        1\n    } else {\n        2\n    }",
        )],
    );

    compile_program_source(source, text).expect("CST-backed return value body should compile");
}

#[test]
fn semantic_function_if_value_expressions_have_cst_body_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn choose() {
    let value: i64 = 2;
    let total = if value > 0 {
        let base = value;
        base
    } else {
        let fallback = 0;
        fallback
    };
    return if total > 1 {
        let next = total + 1;
        next
    } else {
        total
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("choose").expect("choose function");
    assert_cst_let_initializers(
        &payload.body,
        &[
            (SyntaxExpressionKind::Literal, "2"),
            (
                SyntaxExpressionKind::If,
                "if value > 0 {\n        let base = value;\n        base\n    } else {\n        let fallback = 0;\n        fallback\n    }",
            ),
        ],
    );
    assert_cst_return_values(
        &payload.body,
        &[(
            SyntaxExpressionKind::If,
            "if total > 1 {\n        let next = total + 1;\n        next\n    } else {\n        total\n    }",
        )],
    );
    assert_cst_let_initializer_if_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let base = value;"),
            (SyntaxStatementKind::Expr, "base"),
        ]],
        &[vec![
            (SyntaxStatementKind::Let, "let fallback = 0;"),
            (SyntaxStatementKind::Expr, "fallback"),
        ]],
    );
    assert_cst_return_value_if_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let next = total + 1;"),
            (SyntaxStatementKind::Expr, "next"),
        ]],
        &[vec![(SyntaxStatementKind::Expr, "total")]],
    );

    compile_program_source(source, text).expect("CST-backed if value bodies should compile");
}

#[test]
fn semantic_function_else_if_value_expressions_have_cst_body_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn choose() {
    let value: i64 = 2;
    let total = if value > 10 {
        let high = value;
        high
    } else if value > 0 {
        let mid = value + 1;
        mid
    } else {
        let low = 0;
        low
    };
    return if total > 10 {
        total
    } else if total > 0 {
        let next = total + 1;
        next
    } else {
        0
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("choose").expect("choose function");
    assert_cst_let_initializer_if_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let high = value;"),
            (SyntaxStatementKind::Expr, "high"),
        ]],
        &[],
    );
    assert_cst_let_initializer_else_if_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let mid = value + 1;"),
            (SyntaxStatementKind::Expr, "mid"),
        ]],
        &[vec![
            (SyntaxStatementKind::Let, "let low = 0;"),
            (SyntaxStatementKind::Expr, "low"),
        ]],
    );
    assert_cst_return_value_if_body_payloads(
        &payload.body,
        &[vec![(SyntaxStatementKind::Expr, "total")]],
        &[],
    );
    assert_cst_return_value_else_if_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let next = total + 1;"),
            (SyntaxStatementKind::Expr, "next"),
        ]],
        &[vec![(SyntaxStatementKind::Expr, "0")]],
    );

    compile_program_source(source, text).expect("CST-backed else-if value bodies should compile");
}

#[test]
fn semantic_function_block_value_expressions_have_cst_body_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn block_values() {
    let total = {
        let start = 1;
        start + 1
    };
    return {
        let value = total;
        value
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("block_values")
        .expect("block_values function");
    assert_cst_let_initializers(
        &payload.body,
        &[(
            SyntaxExpressionKind::Block,
            "{\n        let start = 1;\n        start + 1\n    }",
        )],
    );
    assert_cst_return_values(
        &payload.body,
        &[(
            SyntaxExpressionKind::Block,
            "{\n        let value = total;\n        value\n    }",
        )],
    );
    assert_cst_let_initializer_block_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let start = 1;"),
            (SyntaxStatementKind::Expr, "start + 1"),
        ]],
    );
    assert_cst_return_value_block_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let value = total;"),
            (SyntaxStatementKind::Expr, "value"),
        ]],
    );

    compile_program_source(source, text).expect("CST-backed block value bodies should compile");
}

#[test]
fn semantic_function_block_tail_control_flow_expressions_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn block_tail_values() {
    let value = {
        let seed = 1;
        if seed > 0 {
            let high = seed;
            high
        } else {
            0
        }
    };
    let matched = {
        let input = value;
        match input {
            0 => {
                let zero = 1;
                zero
            },
            _ => {
                let fallback = input;
                fallback
            },
        }
    };
    return matched;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("block_tail_values")
        .expect("block_tail_values function");
    assert_cst_let_initializer_block_tail_if_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let high = seed;"),
            (SyntaxStatementKind::Expr, "high"),
        ]],
        &[vec![(SyntaxStatementKind::Expr, "0")]],
    );
    assert_cst_let_initializer_block_tail_match_arm_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let zero = 1;"),
                (SyntaxStatementKind::Expr, "zero"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let fallback = input;"),
                (SyntaxStatementKind::Expr, "fallback"),
            ],
        ],
    );

    compile_program_source(source, text)
        .expect("CST-backed block tail control-flow values should compile");
}

#[test]
fn semantic_function_match_value_expressions_have_cst_arm_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn choose(input) {
    let total = match input {
        0 => {
            let base = 1;
            base
        },
        _ => {
            let fallback = input;
            fallback
        },
    };
    return match total {
        1 => {
            let next = total + 1;
            next
        },
        _ => {
            total
        },
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("choose").expect("choose function");
    assert_cst_let_initializers(
        &payload.body,
        &[(
            SyntaxExpressionKind::Match,
            "match input {\n        0 => {\n            let base = 1;\n            base\n        },\n        _ => {\n            let fallback = input;\n            fallback\n        },\n    }",
        )],
    );
    assert_cst_return_values(
        &payload.body,
        &[(
            SyntaxExpressionKind::Match,
            "match total {\n        1 => {\n            let next = total + 1;\n            next\n        },\n        _ => {\n            total\n        },\n    }",
        )],
    );
    assert_cst_let_initializer_match_arm_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let base = 1;"),
                (SyntaxStatementKind::Expr, "base"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let fallback = input;"),
                (SyntaxStatementKind::Expr, "fallback"),
            ],
        ],
    );
    assert_cst_return_value_match_arm_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let next = total + 1;"),
                (SyntaxStatementKind::Expr, "next"),
            ],
            vec![(SyntaxStatementKind::Expr, "total")],
        ],
    );

    compile_program_source(source, text).expect("CST-backed match value arms should compile");
}

#[test]
fn semantic_function_for_iterable_expression_is_cst_payload() {
    let source = SourceId::new(1);
    let text = r#"
fn sum() {
    let total = 0;
    for value in 0..3 {
        total += value;
    }
    return total;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("sum").expect("sum function");
    assert_cst_statements(
        &payload.body,
        &[
            (SyntaxStatementKind::Let, "let total = 0;"),
            (
                SyntaxStatementKind::For,
                "for value in 0..3 {\n        total += value;\n    }",
            ),
            (SyntaxStatementKind::Return, "return total;"),
        ],
    );
    assert_cst_for_iterables(
        &payload.body,
        &[(SyntaxExpressionKind::Binary, Some(BinaryOp::Range), "0..3")],
    );
    assert_cst_for_body_payloads(
        &payload.body,
        &[vec![(SyntaxStatementKind::Expr, "total += value;")]],
    );

    let program =
        compile_program_source(source, text).expect("CST-backed range for body should compile");
    let function = program.function("sum").expect("sum bytecode");
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::I64RangeNext { .. }
    )));
}

#[test]
fn semantic_function_if_condition_expression_is_cst_payload() {
    let source = SourceId::new(1);
    let text = r#"
fn check() {
    let value: i64 = 10;
    if value > 5 {
        return 1;
    } else {
        return 0;
    }
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("check").expect("check function");
    assert_cst_statements(
        &payload.body,
        &[
            (SyntaxStatementKind::Let, "let value: i64 = 10;"),
            (
                SyntaxStatementKind::If,
                "if value > 5 {\n        return 1;\n    } else {\n        return 0;\n    }",
            ),
        ],
    );
    assert_cst_if_conditions(
        &payload.body,
        &[(
            SyntaxExpressionKind::Binary,
            Some(BinaryOp::Greater),
            "value > 5",
        )],
    );
    assert_cst_if_body_payloads(
        &payload.body,
        &[vec![(SyntaxStatementKind::Return, "return 1;")]],
        &[vec![(SyntaxStatementKind::Return, "return 0;")]],
    );

    let program =
        compile_program_source(source, text).expect("CST-backed if condition should compile");
    let function = program.function("check").expect("check bytecode");
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
            op: crate::I64CompareOp::Greater,
            imm: 5,
            ..
        }
    )));
}

#[test]
fn semantic_function_else_if_statements_have_cst_body_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn check() {
    let value: i64 = 10;
    if value > 10 {
        let high = value;
        return high;
    } else if value > 5 {
        let mid = value - 1;
        return mid;
    } else {
        return 0;
    }
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("check").expect("check function");
    assert_cst_if_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let high = value;"),
            (SyntaxStatementKind::Return, "return high;"),
        ]],
        &[],
    );
    assert_cst_statement_else_if_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let mid = value - 1;"),
            (SyntaxStatementKind::Return, "return mid;"),
        ]],
        &[vec![(SyntaxStatementKind::Return, "return 0;")]],
    );

    compile_program_source(source, text).expect("CST-backed else-if statement body should compile");
}

#[test]
fn semantic_function_block_statement_body_is_cst_payload() {
    let source = SourceId::new(1);
    let text = r#"
fn scoped() {
    let total = 0;
    {
        total += 1;
    }
    return total;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("scoped").expect("scoped function");
    assert_cst_statements(
        &payload.body,
        &[
            (SyntaxStatementKind::Let, "let total = 0;"),
            (SyntaxStatementKind::Block, "{\n        total += 1;\n    }"),
            (SyntaxStatementKind::Return, "return total;"),
        ],
    );
    assert_cst_block_statement_payloads(
        &payload.body,
        &[vec![(SyntaxStatementKind::Expr, "total += 1;")]],
    );

    compile_program_source(source, text).expect("CST-backed block statement body should compile");
}

#[test]
fn semantic_function_control_flow_statements_are_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn flow() {
    let total = 0;
    if total == 0 {
        return 1;
    }
    match total {
        0 => { return 0; },
        _ => { return total; },
    }
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("flow").expect("flow function");
    assert_cst_statements(
        &payload.body,
        &[
            (SyntaxStatementKind::Let, "let total = 0;"),
            (
                SyntaxStatementKind::If,
                "if total == 0 {\n        return 1;\n    }",
            ),
            (
                SyntaxStatementKind::Match,
                "match total {\n        0 => { return 0; },\n        _ => { return total; },\n    }",
            ),
        ],
    );
    assert_cst_match_arm_body_payloads(
        &payload.body,
        &[
            vec![(SyntaxStatementKind::Return, "return 0;")],
            vec![(SyntaxStatementKind::Return, "return total;")],
        ],
    );

    compile_program_source(source, text).expect("CST-backed control-flow body should compile");
}
