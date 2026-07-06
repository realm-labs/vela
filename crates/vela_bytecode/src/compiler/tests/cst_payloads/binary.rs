use super::*;

fn binary_statement_payloads<'ast>(
    body: &body_payloads::CompilerBodyPayload<'ast>,
) -> Vec<body_payloads::CompilerStatementPayload<'ast>> {
    paired_statement_payloads_for_body(SourceId::new(1), body)
}

#[test]
fn semantic_function_binary_operands_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn take(value) {
    return value;
}

fn binary_values() {
    let amount = (-{
        let left = 1;
        left
    }) + {
        let right = 2;
        right
    };
    let plus_right = (-{
        let base = 3;
        base
    }) + 1;
    let plus_left = 1 + (-{
        let tail = 4;
        tail
    });
    amount = (-{
        let assigned_left = 5;
        assigned_left
    }) + {
        let assigned_right = 6;
        assigned_right
    };
    take((-{
        let arg_left = 7;
        arg_left
    }) + {
        let arg_right = 8;
        arg_right
    });
    return (-{
        let return_left = 9;
        return_left
    }) + {
        let return_right = 10;
        return_right
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("binary_values")
        .expect("binary_values function");

    assert_cst_let_initializer_binary_operand_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let left = 1;"),
                (SyntaxStatementKind::Expr, "left"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let right = 2;"),
                (SyntaxStatementKind::Expr, "right"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let base = 3;"),
                (SyntaxStatementKind::Expr, "base"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let tail = 4;"),
                (SyntaxStatementKind::Expr, "tail"),
            ],
        ],
    );
    assert_cst_assignment_value_binary_operand_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let assigned_left = 5;"),
                (SyntaxStatementKind::Expr, "assigned_left"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let assigned_right = 6;"),
                (SyntaxStatementKind::Expr, "assigned_right"),
            ],
        ],
    );
    assert_cst_call_argument_binary_operand_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let arg_left = 7;"),
                (SyntaxStatementKind::Expr, "arg_left"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let arg_right = 8;"),
                (SyntaxStatementKind::Expr, "arg_right"),
            ],
        ],
    );
    assert_cst_return_value_binary_operand_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let return_left = 9;"),
                (SyntaxStatementKind::Expr, "return_left"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let return_right = 10;"),
                (SyntaxStatementKind::Expr, "return_right"),
            ],
        ],
    );

    compile_program_source(source, text).expect("CST-backed binary operands should compile");
}

#[test]
fn semantic_function_logical_chain_operands_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn logical_values() {
    let both = ({
        let and_left = true;
        and_left
    }) && ({
        let and_middle = true;
        and_middle
    }) && ({
        let and_right = true;
        and_right
    });
    let either = ({
        let or_left = false;
        or_left
    }) || ({
        let or_middle = false;
        or_middle
    }) || ({
        let or_right = true;
        or_right
    });
    return both || either;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("logical_values")
        .expect("logical_values function");

    let initializers = binary_statement_payloads(&payload.body)
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .collect::<Vec<_>>();

    let and_payload = initializers
        .iter()
        .find(|payload| {
            payload
                .logical_chain_operand_payloads(BinaryOp::And)
                .is_some_and(|operands| operands.len() == 3)
        })
        .expect("&& initializer should expose flattened logical operands");
    assert_logical_chain_block_payloads(
        and_payload,
        BinaryOp::And,
        &[
            vec![
                (SyntaxStatementKind::Let, "let and_left = true;"),
                (SyntaxStatementKind::Expr, "and_left"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let and_middle = true;"),
                (SyntaxStatementKind::Expr, "and_middle"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let and_right = true;"),
                (SyntaxStatementKind::Expr, "and_right"),
            ],
        ],
    );

    let or_payload = initializers
        .iter()
        .find(|payload| {
            payload
                .logical_chain_operand_payloads(BinaryOp::Or)
                .is_some_and(|operands| operands.len() == 3)
        })
        .expect("|| initializer should expose flattened logical operands");
    assert_logical_chain_block_payloads(
        or_payload,
        BinaryOp::Or,
        &[
            vec![
                (SyntaxStatementKind::Let, "let or_left = false;"),
                (SyntaxStatementKind::Expr, "or_left"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let or_middle = false;"),
                (SyntaxStatementKind::Expr, "or_middle"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let or_right = true;"),
                (SyntaxStatementKind::Expr, "or_right"),
            ],
        ],
    );

    compile_program_source(source, text).expect("CST-backed logical operands should compile");
}

#[test]
fn mismatched_logical_chain_payload_does_not_use_legacy_operands() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn make(value) {
    return value;
}

fn main(left, middle, right) {
    let value = left && make(right);
}
"#;
    let cst_semantic = parse_semantic_source(source, cst_text).expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("main function");
    let cst_binary = binary_statement_payloads(&cst_payload.body)[0]
        .let_initializer_expression_payload()
        .expect("CST logical payload");

    with_cst_payload_compiler(
        r#"
fn make(value) {
    return value;
}

fn main(left, middle, right) {
    let value = left && make(middle) && right;
}
"#,
        |compiler, payload| {
            let legacy_binary = binary_statement_payloads(&payload.body)[0]
                .let_initializer_expression_payload()
                .expect("legacy logical payload");
            let mismatched_payload = expression_payload_with_fallback(
                source,
                cst_binary
                    .syntax_expression()
                    .expect("CST logical expression")
                    .clone(),
                legacy_binary.fallback(),
            );

            let error = compiler
                .compile_expr_with_payload(legacy_binary.fallback(), Some(&mismatched_payload))
                .expect_err("mismatched CST logical payload must not compile legacy operands");

            assert!(
                matches!(
                    error.kind,
                    CompileErrorKind::UnsupportedSyntax("mismatched CST logical chain payload")
                ),
                "expected mismatched CST logical chain payload, got {error:?}"
            );
        },
    );
}

#[test]
fn binary_expression_lowering_prefers_cst_operator_payload() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main(left, right) {
    let value = left + right;
    return value;
}
"#;
    let cst_semantic = parse_semantic_source(source, cst_text).expect("CST source should parse");
    let (cst_payload, _, _) = cst_semantic.function("main").expect("main function");
    let cst_body = cst_payload.body.syntax_payload().body.clone();

    with_cst_payload_compiler(
        r#"
fn make(value) {
    return value;
}

fn main(left, right) {
    let value = left && make(right);
    return value;
}
"#,
        |compiler, payload| {
            let statements = body_payloads::CompilerBodyPayload::paired_statement_payloads_for_test(
                source,
                cst_body,
                fallback_statements_for_body(source, &payload.body),
            );

            compiler
                .compile_statement_payloads(&statements)
                .expect("CST-backed binary expression should compile");

            assert!(
                compiler
                    .code
                    .instructions
                    .iter()
                    .any(|instruction| matches!(
                        instruction.kind,
                        UnlinkedInstructionKind::Add { .. }
                    )),
                "binary expression should use the CST operator"
            );
            assert!(
                compiler
                    .code
                    .instructions
                    .iter()
                    .all(|instruction| !matches!(
                        instruction.kind,
                        UnlinkedInstructionKind::Sub { .. }
                    )),
                "binary expression should not use the legacy fallback body"
            );
        },
    );
}

#[test]
fn identity_comparison_diagnostics_prefer_cst_operand_payloads() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst_binary = true === false;
    let legacy_binary = 1 === 2;
}
"#,
        |compiler, payload| {
            let statements = binary_statement_payloads(&payload.body);
            let cst_binary = statements[0]
                .let_initializer_expression_payload()
                .expect("CST binary payload");
            let legacy_binary = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy binary fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_binary
                    .syntax_expression()
                    .expect("CST expression")
                    .clone(),
                legacy_binary.fallback(),
            );

            let error = compiler
                .compile_expr_with_payload(mismatched_payload.fallback(), Some(&mismatched_payload))
                .expect_err("mismatched CST binary payload must not compile");
            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("mismatched CST binary expression payload")
            ));
        },
    );
}

#[test]
fn missing_binary_operand_payload_does_not_use_legacy_operand() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main(input) {
    let value = input +;
}
"#;
    let legacy_text = r#"
fn make(value) {
    return value;
}

fn main(input, other) {
    let value = input && make(other);
}
"#;
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_binary = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST function body")
        .statements()
        .next()
        .expect("CST let statement")
        .as_let()
        .expect("CST let")
        .initializer()
        .expect("CST initializer");
    assert_eq!(cst_binary.expression_kind(), SyntaxExpressionKind::Binary);

    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_binary = binary_statement_payloads(&legacy_payload.body)[0]
        .let_initializer_expression_payload()
        .expect("legacy binary payload");
    let missing = expression_payload_with_fallback(source, cst_binary, legacy_binary.fallback());
    let (_left, right) = missing
        .binary_operand_payloads()
        .expect("binary operand payloads");

    assert!(right.syntax_expression().is_none());

    let error = compiler
        .compile_expr_with_payload(legacy_binary.fallback(), Some(&missing))
        .expect_err("missing binary operand payload must not compile legacy operand");

    assert!(matches!(error.kind, CompileErrorKind::UnsupportedSyntax(_)));
}

#[test]
fn missing_binary_expression_payload_does_not_use_legacy_binary() {
    let source = SourceId::new(1);
    let text = r#"
fn make(value) {
    return value;
}

fn main(input, other) {
    let value = input && make(other);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_binary = binary_statement_payloads(&legacy_payload.body)[0]
        .let_initializer_expression_payload()
        .expect("legacy binary payload");
    let missing_binary =
        body_payloads::CompilerExpressionPayload::missing_syntax(source, legacy_binary.fallback());

    let error = compiler
        .compile_expr_with_payload(legacy_binary.fallback(), Some(&missing_binary))
        .expect_err("missing CST binary payload must not compile legacy binary");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST expression payload")
    ));
}

#[test]
fn bare_logical_expression_requires_cst_operand_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn make(value) {
    return value;
}

fn main(input, other) {
    let value = input && make(other);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_binary = binary_statement_payloads(&legacy_payload.body)[0]
        .let_initializer_expression_payload()
        .expect("legacy binary payload");

    let error = compiler
        .compile_expr(legacy_binary.fallback())
        .expect_err("bare logical expression must not compile legacy operands");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST logical operand payload")
    ));
}

#[test]
fn binary_value_type_inference_rejects_mismatched_cst_payloads() {
    with_cst_payload_compiler(
        r#"
fn make(value) {
    return value;
}

fn main(input) {
    let lhs = 1;
    let rhs = 2;
    let cst_sum = lhs && make(rhs);
    let cst_diff = lhs || make(rhs);
    let legacy_bool = !(input == rhs);
}
"#,
        |compiler, payload| {
            let statements = binary_statement_payloads(&payload.body);
            let cst_sum = statements[2]
                .let_initializer_expression_payload()
                .expect("CST binary payload");
            let cst_diff = statements[3]
                .let_initializer_expression_payload()
                .expect("CST binary path payload");
            let legacy_bool = statements[4]
                .let_initializer_expression_payload()
                .expect("legacy literal fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_sum
                    .syntax_expression()
                    .expect("CST binary expression")
                    .clone(),
                legacy_bool.fallback(),
            );

            assert_eq!(
                compiler.static_type_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                value_types::StaticExprType::Dynamic
            );

            compiler.value_types.set_name(
                "lhs",
                Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::I64)),
            );
            compiler.value_types.set_name(
                "rhs",
                Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::I64)),
            );
            let mismatched_path_operand_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_diff
                    .syntax_expression()
                    .expect("CST binary path expression")
                    .clone(),
                legacy_bool.fallback(),
            );
            assert_eq!(
                compiler.static_type_for_expr_with_payload(
                    mismatched_path_operand_payload.fallback(),
                    Some(&mismatched_path_operand_payload),
                ),
                value_types::StaticExprType::Dynamic
            );
        },
    );
}

#[test]
fn binary_value_type_inference_rejects_child_path_payload() {
    with_cst_payload_compiler(
        r#"
fn make(value) {
    return value;
}

fn main(lhs, rhs) {
    let value = lhs && make(rhs);
}
"#,
        |compiler, payload| {
            compiler.value_types.set_name(
                "lhs",
                Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::I64)),
            );
            compiler.value_types.set_name(
                "rhs",
                Some(RuntimeTypeFact::primitive(vela_common::PrimitiveTag::I64)),
            );
            let binary = binary_statement_payloads(&payload.body)[0]
                .let_initializer_expression_payload()
                .expect("binary initializer payload");
            let (left, _) = binary
                .binary_operand_payloads()
                .expect("binary operand payloads");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                left.syntax_expression()
                    .expect("left path CST expression")
                    .clone(),
                binary.fallback(),
            );

            assert_eq!(
                compiler.static_type_for_expr_with_payload(
                    mismatched_payload.fallback(),
                    Some(&mismatched_payload),
                ),
                value_types::StaticExprType::Dynamic,
                "child path CST payload must not type the whole legacy binary expression"
            );
        },
    );
}

#[test]
fn inline_binary_numeric_literals_prefer_cst_payloads() {
    with_cst_payload_compiler(
        r#"
fn take(value) {
    return value;
}

fn main() {
    take(5);
    take(99);
}
"#,
        |_compiler, payload| {
            let statements = binary_statement_payloads(&payload.body);
            let cst_literal = statements[0]
                .expression_payload()
                .and_then(|payload| payload.call_argument_value_payloads())
                .expect("CST call argument payloads")
                .into_iter()
                .next()
                .expect("CST call argument");
            let fallback_literal = statements[1]
                .expression_payload()
                .and_then(|payload| payload.call_argument_value_payloads())
                .expect("fallback call argument payloads")
                .into_iter()
                .next()
                .expect("fallback call argument");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_literal
                    .syntax_expression()
                    .expect("CST literal expression")
                    .clone(),
                fallback_literal.fallback(),
            );

            let literal =
                crate::compiler::expression_checks::unsuffixed_numeric_literal_with_payload(Some(
                    &mismatched_payload,
                ));

            assert_eq!(
                literal,
                Some(
                    crate::compiler::expression_checks::UnsuffixedNumericLiteral::Integer(
                        "5".to_owned()
                    )
                )
            );
        },
    );
}

#[test]
fn inline_binary_numeric_literals_require_cst_payload() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let fallback_literal = 99;
}
"#,
        |_compiler, _payload| {
            let literal =
                crate::compiler::expression_checks::unsuffixed_numeric_literal_with_payload(None);

            assert_eq!(
                literal, None,
                "old literal fallback must not drive CST inline literal lowering"
            );
        },
    );
}

#[test]
fn multiline_return_binary_method_operands_lower_from_cst() {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn main() {
    let names: Array = ["gold", "xp"];
    let rewards: Map = {"gold": 4};
    let tags: Set = set::from_array(["daily"]);
    let some: Option = option::some(1);
    let err: Result = result::err("bad");
    if some.is_some() && err.is_err() {
        return "gold".len()
            + names.len()
            + rewards.len()
            + tags.len()
            + (1..4).len();
    }
    return 0;
}
"#,
        registry.compile_view(),
    )
    .expect("CST multiline binary method operands should compile");
    let main = program.function("main").expect("main function");
    let len_method_ids = main
        .instructions
        .iter()
        .filter(|instruction| {
            matches!(
                &instruction.kind,
                UnlinkedInstructionKind::CallMethodId { method, .. } if method == "len"
            )
        })
        .count();
    let dynamic_len_methods = main
        .instructions
        .iter()
        .filter(|instruction| {
            matches!(
                &instruction.kind,
                UnlinkedInstructionKind::CallDynamicMethod { method, .. } if method == "len"
            )
        })
        .count();

    assert_eq!(len_method_ids, 5);
    assert_eq!(dynamic_len_methods, 0);
}

fn assert_cst_let_initializer_binary_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = binary_statement_payloads(body);
    let actual = statements
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(binary_block_operand_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_assignment_value_binary_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = binary_statement_payloads(body);
    let actual = statements
        .iter()
        .filter_map(|statement| {
            statement
                .expression_payload()
                .and_then(|payload| payload.assignment_value_payload())
        })
        .flat_map(binary_block_operand_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_call_argument_binary_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = binary_statement_payloads(body);
    let actual = statements
        .iter()
        .flat_map(|statement| {
            statement
                .expression_payload()
                .and_then(|payload| payload.call_argument_value_payloads())
                .unwrap_or_default()
        })
        .flat_map(binary_block_operand_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_return_value_binary_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = binary_statement_payloads(body);
    let actual = statements
        .iter()
        .filter_map(|statement| statement.return_value_expression_payload())
        .flat_map(binary_block_operand_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_logical_chain_block_payloads(
    payload: &body_payloads::CompilerExpressionPayload<'_>,
    op: BinaryOp,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = payload
        .logical_chain_operand_payloads(op)
        .expect("logical chain should expose operand payloads")
        .into_iter()
        .flat_map(block_operand_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn binary_block_operand_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    if let Some(operand) = payload.paren_inner_payload() {
        return binary_block_operand_payloads(operand);
    }
    if let Some(operand) = payload.unary_operand_payload() {
        return binary_block_operand_payloads(operand);
    }
    let Some((left, right)) = payload.binary_operand_payloads() else {
        return Vec::new();
    };
    [left, right]
        .into_iter()
        .flat_map(block_operand_payloads)
        .collect()
}

fn block_operand_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    if let Some(body) = payload.block_body_payload() {
        return vec![cst_statement_texts(&body)];
    }
    if let Some(operand) = payload.paren_inner_payload() {
        return block_operand_payloads(operand);
    }
    if let Some(operand) = payload.unary_operand_payload() {
        return block_operand_payloads(operand);
    }
    Vec::new()
}
