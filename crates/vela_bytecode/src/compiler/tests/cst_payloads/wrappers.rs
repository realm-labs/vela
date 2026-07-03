use super::*;

#[test]
fn semantic_function_wrapper_operands_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
enum Result {
    Ok(value)
    Err(message)
}

fn checked(value) {
    return Result::Ok(value);
}

fn take(first, second) {
    return first;
}

fn wrapper_values() {
    let flag = !{
        let ready = false;
        ready
    };
    let amount = -{
        let value = 1;
        value
    };
    amount = -{
        let assigned = 2;
        assigned
    };
    take(!{
        let arg = false;
        arg
    }, {
        let inner = checked(10);
        inner
    }?);
    return {
        let result = checked(amount);
        result
    }?;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("wrapper_values")
        .expect("wrapper_values function");

    assert_cst_let_initializer_unary_operand_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let ready = false;"),
                (SyntaxStatementKind::Expr, "ready"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let value = 1;"),
                (SyntaxStatementKind::Expr, "value"),
            ],
        ],
    );
    assert_cst_assignment_value_unary_operand_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let assigned = 2;"),
            (SyntaxStatementKind::Expr, "assigned"),
        ]],
    );
    assert_cst_call_argument_unary_operand_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let arg = false;"),
            (SyntaxStatementKind::Expr, "arg"),
        ]],
    );
    assert_cst_call_argument_try_operand_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let inner = checked(10);"),
            (SyntaxStatementKind::Expr, "inner"),
        ]],
    );
    assert_cst_return_value_try_operand_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let result = checked(amount);"),
            (SyntaxStatementKind::Expr, "result"),
        ]],
    );

    compile_program_source(source, text).expect("CST-backed wrapper operands should compile");
}

#[test]
fn semantic_function_parenthesized_values_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn paren_values() {
    let value = ({
        let inner = 1;
        inner
    });
    let assigned: i64 = 0;
    assigned = ({
        let updated = 2;
        updated
    });
    return value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("paren_values")
        .expect("paren_values function");
    let statements = payload.body.statement_payloads();
    let value_fallback =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));
    assert!(
        value_fallback.is_err(),
        "parenthesized simple block initializer should not require owned fallback"
    );
    assert_cst_assignment_value_paren_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let updated = 2;"),
            (SyntaxStatementKind::Expr, "updated"),
        ]],
    );

    compile_program_source(source, text)
        .expect("CST-backed parenthesized expression should compile");
}

#[test]
fn block_tail_parenthesized_values_compile_with_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn block_tail_paren() {
    let value = {
        ({
            let inner = 1;
            inner
        })
    };
    return value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("block_tail_paren")
        .expect("block_tail_paren function");
    let statements = payload.body.statement_payloads();
    let let_fallback =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));
    assert!(
        let_fallback.is_err(),
        "parenthesized block tail initializer should not require owned fallback"
    );

    compile_program_source(source, text)
        .expect("CST-backed parenthesized block tail should compile");
}

#[test]
fn missing_parenthesized_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let value = ();
}
"#;
    let legacy_text = r#"
fn make(value) {
    return value;
}

fn main(input) {
    let value = (input && make(input));
}
"#;
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_paren = cst_parse
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
    assert_eq!(cst_paren.expression_kind(), SyntaxExpressionKind::Paren);

    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_paren = legacy_payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("legacy parenthesized payload");
    let missing = body_payloads::CompilerExpressionPayload::syntax(
        source,
        cst_paren,
        legacy_paren.fallback(),
    );
    let inner = missing
        .paren_inner_payload()
        .expect("parenthesized inner payload");
    assert!(inner.syntax_expression().is_none());

    let error = compiler
        .compile_expr_with_payload(legacy_paren.fallback(), Some(&missing))
        .expect_err("missing parenthesized payload must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST parenthesized expression")
    ));
}

#[test]
fn missing_unary_expression_payload_does_not_use_legacy_unary() {
    let source = SourceId::new(1);
    let text = r#"
fn main(input, other) {
    let value = -(input + other);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_unary = legacy_payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("legacy unary payload");
    let missing_unary =
        body_payloads::CompilerExpressionPayload::missing_syntax(source, legacy_unary.fallback());

    assert_eq!(missing_unary.syntax_unary_operator(), None);

    let error = compiler
        .compile_expr_with_payload(legacy_unary.fallback(), Some(&missing_unary))
        .expect_err("missing CST unary payload must not compile legacy unary");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST expression payload")
    ));
}

#[test]
fn missing_try_expression_payload_does_not_use_legacy_try() {
    let source = SourceId::new(1);
    let text = r#"
enum Result {
    Ok(value)
    Err(message)
}

fn checked(value) {
    return Result::Ok(value);
}

fn main() {
    let value = checked(1)?;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_try = legacy_payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("legacy try payload");
    let missing_try =
        body_payloads::CompilerExpressionPayload::missing_syntax(source, legacy_try.fallback());

    let error = compiler
        .compile_expr_with_payload(legacy_try.fallback(), Some(&missing_try))
        .expect_err("missing CST try payload must not compile legacy try");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST expression payload")
    ));
}

fn assert_cst_let_initializer_unary_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .filter_map(|payload| {
            let operand = payload.fallback_unary_operand()?;
            payload.unary_operand_payload(operand)
        })
        .filter_map(|operand| {
            let body = operand.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_assignment_value_unary_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .filter_map(|payload| {
            let operand = payload.fallback_unary_operand()?;
            payload.unary_operand_payload(operand)
        })
        .filter_map(|operand| {
            let body = operand.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_call_argument_unary_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .flat_map(|statement| statement.call_argument_value_payloads().unwrap_or_default())
        .filter_map(|payload| {
            let operand = payload.fallback_unary_operand()?;
            payload.unary_operand_payload(operand)
        })
        .filter_map(|operand| {
            let body = operand.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_call_argument_try_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .flat_map(|statement| statement.call_argument_value_payloads().unwrap_or_default())
        .filter_map(|payload| {
            let operand = payload.fallback_try_operand()?;
            payload.try_operand_payload(operand)
        })
        .filter_map(|operand| {
            let body = operand.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_return_value_try_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.return_value_expression_payload())
        .filter_map(|payload| {
            let operand = payload.fallback_try_operand()?;
            payload.try_operand_payload(operand)
        })
        .filter_map(|operand| {
            let body = operand.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_assignment_value_paren_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .filter(|payload| payload.kind() == Some(SyntaxExpressionKind::Paren))
        .filter_map(|payload| payload.paren_inner_payload())
        .filter_map(|inner| {
            let body = inner.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}
