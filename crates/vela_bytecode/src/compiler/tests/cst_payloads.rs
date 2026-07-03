use super::*;

mod assignment_targets;
mod binary;
mod block_values;
mod call_counts;
mod calls;
mod constant_expressions;
mod constructors;
mod containers;
mod control_flow_values;
mod expression_statements;
mod field_index;
mod for_iterables;
mod host_paths;
mod if_conditions;
mod interpolated;
mod lambdas;
mod let_return_values;
mod literals;
mod match_arms;
mod match_counts;
mod match_guards;
mod match_payloads;
mod param_defaults;
mod path_expressions;
mod pattern_counts;
mod shape_inference;
mod source_identity;
mod statement_bodies;
mod statement_counts;
mod typed_call_args;
mod wrappers;

fn with_cst_payload_compiler(
    source: &str,
    inspect: impl for<'ast> FnOnce(
        &mut Compiler<'ast, 'static>,
        function_payloads::FunctionBodyPayload<'ast>,
    ),
) {
    let semantic =
        parse_semantic_source(SourceId::new(1), source).expect("semantic source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    inspect(&mut compiler, payload);
}

fn cst_payload_compiler_for_function<'ast>(
    semantic: &'ast semantic::SemanticSource,
    function: &str,
) -> (
    Compiler<'ast, 'static>,
    function_payloads::FunctionBodyPayload<'ast>,
) {
    let facts = cst_payload_compiler_facts(semantic);
    let (payload, signature, bindings) = semantic.function(function).expect("script function");
    let compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");
    (compiler, payload)
}

fn cst_payload_compiler_facts(semantic: &semantic::SemanticSource) -> CompilerFacts<'static> {
    cst_payload_compiler_facts_with_options(semantic, CompilerOptions::default(), None)
}

fn cst_payload_compiler_facts_with_options<'registry>(
    semantic: &semantic::SemanticSource,
    options: CompilerOptions,
    registry: Option<vela_registry::RegistryCompileView<'registry>>,
) -> CompilerFacts<'registry> {
    let script_function_symbols = semantic.script_function_symbols();
    let script_function_signatures = semantic.script_function_signatures();
    let type_symbols = semantic.type_symbols();
    let global_symbols = semantic.global_symbols();
    let global_slots = global_slots(&global_symbols);
    let global_type_symbols = semantic.global_type_symbols();
    let script_field_slots = semantic.script_field_slots(&type_symbols);
    let const_values = semantic.const_values().expect("const values should lower");
    let schema_defaults = semantic.schema_defaults(&type_symbols, &const_values);
    CompilerFacts {
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
        options,
        registry,
    }
}

#[test]
fn equal_count_body_payloads_pair_statements_by_position_not_legacy_span() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_body() {
    let cst_value = 1;
    return cst_value;
}

fn legacy_body() {
    let legacy_value = 2;
    return legacy_value + legacy_value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_body").expect("cst function");
    let (legacy_payload, _, _) = semantic.function("legacy_body").expect("legacy function");
    let mismatched = body_payloads::CompilerBodyPayload::syntax(
        source,
        cst_payload.body.syntax_payload().body.clone(),
        legacy_payload.body.fallback(),
    );

    let statements = mismatched.statement_payloads();
    assert_eq!(statements.len(), 2);
    assert_eq!(
        statements[0]
            .syntax_statement()
            .map(|statement| statement.syntax().text().to_string())
            .as_deref(),
        Some("let cst_value = 1;")
    );
    assert_eq!(
        statements[1]
            .syntax_statement()
            .map(|statement| statement.syntax().text().to_string())
            .as_deref(),
        Some("return cst_value;")
    );
}

#[test]
fn mismatched_statement_body_payloads_compile_cst_values_by_position() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_body() {
    let cst_value = 1;
    return cst_value;
}

fn legacy_body() {
    let legacy_value = 2;
    return legacy_value + legacy_value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_body").expect("cst function");
    let (legacy_payload, _, _) = semantic.function("legacy_body").expect("legacy function");
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "legacy_body");
    let mismatched = body_payloads::CompilerBodyPayload::syntax(
        source,
        cst_payload.body.syntax_payload().body.clone(),
        legacy_payload.body.fallback(),
    );
    let statements = mismatched.statement_payloads();

    compiler
        .compile_statement_payload_for_test(&statements[0])
        .expect("positionally paired CST statement payload should compile");

    assert!(
        compiler
            .code
            .constants
            .iter()
            .any(|constant| *constant == Constant::i64(1)),
        "CST initializer value must be compiled"
    );
    assert!(
        compiler
            .code
            .constants
            .iter()
            .all(|constant| *constant != Constant::i64(2)),
        "legacy fallback initializer value must not be compiled"
    );
}

#[test]
fn extra_map_entry_payloads_do_not_compile_fallback_entries() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_body() {
    let value = {
        first: 1,
        second: 2,
    };
}

fn fallback_body() {
    let value = {
        first: 1,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_body").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_body")
        .expect("fallback function");
    let cst_map = cst_payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("cst map payload");
    let fallback_map = fallback_payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("fallback map payload");
    let mismatched = body_payloads::CompilerExpressionPayload::syntax(
        source,
        cst_map.syntax_expression().expect("cst map syntax").clone(),
        fallback_map.fallback(),
    );
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_body");

    let error = compiler
        .compile_expr_with_payload(fallback_map.fallback(), Some(&mismatched))
        .expect_err("extra CST map entries must not be ignored");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST map entries")
    ));
}

#[test]
fn extra_array_element_payloads_do_not_compile_fallback_items() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_body() {
    let value = [1, 2];
}

fn fallback_body() {
    let value = [1];
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_body").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_body")
        .expect("fallback function");
    let cst_array = cst_payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("cst array payload");
    let fallback_array = fallback_payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("fallback array payload");
    let mismatched = body_payloads::CompilerExpressionPayload::syntax(
        source,
        cst_array
            .syntax_expression()
            .expect("cst array syntax")
            .clone(),
        fallback_array.fallback(),
    );
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_body");

    let error = compiler
        .compile_expr_with_payload(fallback_array.fallback(), Some(&mismatched))
        .expect_err("extra CST array elements must not be ignored");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST array elements")
    ));
}

#[test]
fn extra_record_field_payloads_do_not_compile_fallback_fields() {
    let source = SourceId::new(1);
    let text = r#"
struct Pair {
    first
    second
}

fn cst_body() {
    let value = Pair {
        first: 1,
        second: 2,
    };
}

fn fallback_body() {
    let value = Pair {
        first: 1,
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_body").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_body")
        .expect("fallback function");
    let cst_record = cst_payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("cst record payload");
    let fallback_record = fallback_payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("fallback record payload");
    let mismatched = body_payloads::CompilerExpressionPayload::syntax(
        source,
        cst_record
            .syntax_expression()
            .expect("cst record syntax")
            .clone(),
        fallback_record.fallback(),
    );
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_body");

    let error = compiler
        .compile_expr_with_payload(fallback_record.fallback(), Some(&mismatched))
        .expect_err("extra CST record fields must not be ignored");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST record fields")
    ));
}

#[test]
fn mismatched_statement_payloads_do_not_compile_legacy_statement() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value = [1];
    return value + value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let let_statement = &statements[0];
    let return_syntax = statements[1]
        .syntax_statement()
        .expect("return CST statement")
        .clone();
    let mismatched = body_payloads::CompilerStatementPayload::syntax(
        source,
        return_syntax,
        let_statement.fallback(),
    );

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("mismatched statement payload must not use legacy fallback");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST statement payload")
    ));
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
