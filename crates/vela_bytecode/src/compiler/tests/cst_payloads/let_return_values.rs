use super::*;

// Temporary 1200-line exception: this suite owns paired let/return CST payload
// and old-body-fallback sentinels during the body-payload hard switch. Splitting
// it before the fallback side is deleted would separate shared mismatch
// fixtures from the helper assertions they are meant to protect.

#[test]
fn mismatched_let_initializer_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn take(value) {
    return value;
}

fn main() {
    let cst_value = take(1);
    let legacy_value = [1];
    return 0;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_let = statements[0]
        .syntax_statement()
        .expect("CST let statement")
        .clone();
    let legacy_array_let = statements[1].fallback();
    let mismatched =
        body_payloads::CompilerStatementPayload::syntax(source, cst_let, legacy_array_let);

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("mismatched let initializer payload must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST let initializer payload")
    ));
}

#[test]
fn mismatched_path_let_initializer_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn main(value) {
    let cst_value = value;
    let legacy_value = value + value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_self_let = statements[0]
        .syntax_statement()
        .expect("CST let statement")
        .clone();
    let legacy_path_let = statements[1].fallback();
    let mismatched =
        body_payloads::CompilerStatementPayload::syntax(source, cst_self_let, legacy_path_let);

    compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect("CST path let payload should compile without legacy expression");

    assert!(
        compiler
            .code
            .instructions
            .iter()
            .all(|instruction| !matches!(instruction.kind, UnlinkedInstructionKind::Add { .. })),
        "CST path let must not compile the legacy binary expression"
    );
}

#[test]
fn syntax_only_binary_let_initializer_compiles_without_owned_statement_fallback() {
    let source = SourceId::new(1);
    let text = r#"
fn main(value) {
    let cst_value = value + 1;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let cst_binary_let = payload.body.statement_payloads()[0]
        .syntax_statement()
        .expect("CST binary let statement")
        .clone();
    let syntax_only =
        body_payloads::CompilerStatementPayload::syntax_only_for_test(source, cst_binary_let);

    compiler
        .compile_statement_payload_for_test(&syntax_only)
        .expect("CST binary let should compile without legacy statement fallback");

    assert!(
        compiler
            .code
            .instructions
            .iter()
            .any(|instruction| matches!(
                instruction.kind,
                UnlinkedInstructionKind::Add { .. }
                    | UnlinkedInstructionKind::BinaryIntLiteral { .. }
            )),
        "CST binary let should emit the syntax operator without owned fallback"
    );
}

#[test]
fn missing_let_initializer_payload_uses_cst_empty_let() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let cst_value;
    let legacy_value = [1];
    return 0;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_let_without_initializer = statements[0]
        .syntax_statement()
        .expect("CST let statement")
        .clone();
    let legacy_array_let = statements[1].fallback();
    let mismatched = body_payloads::CompilerStatementPayload::syntax(
        source,
        cst_let_without_initializer,
        legacy_array_let,
    );

    compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect("CST empty let payload must not compile legacy array expression");

    assert_empty_let_without_i64_fallback(&compiler);
}

#[test]
fn syntax_only_empty_let_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let cst_value;
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");

    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only empty let body should compile");

    assert_empty_let_without_i64_fallback(&compiler);
}

#[test]
fn syntax_only_empty_let_in_mixed_body_drops_owned_statement_fallback() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let cst_value;
    return 1;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (_, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only empty let should not retain an owned statement fallback"
    );
    assert_eq!(
        statements[1].return_value_kind(),
        Some(SyntaxExpressionKind::Literal)
    );
}

#[test]
fn syntax_only_bare_return_in_mixed_body_drops_owned_statement_fallback() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    return;
    return 1;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (_, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only bare return should not retain an owned statement fallback"
    );
    assert_eq!(
        statements[1].return_value_kind(),
        Some(SyntaxExpressionKind::Literal)
    );
}

#[test]
fn syntax_only_literal_return_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    return 1;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");

    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only literal return body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(1)),
        "CST literal return should emit the literal constant"
    );
}

#[test]
fn syntax_only_typed_numeric_literal_return_body_uses_contextual_type() {
    let source = SourceId::new(1);
    let text = r#"
fn main() -> i8 {
    return 12;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");

    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed literal return body should compile");

    assert!(
        compiler
            .code
            .constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I8(12))),
        "typed CST literal return should use the function return type"
    );
}

#[test]
fn syntax_only_negated_numeric_return_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    return -(12);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only negated numeric return should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only negated numeric return body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(-12)),
        "CST negated numeric return should emit the negated constant"
    );
}

#[test]
fn syntax_only_typed_negated_numeric_return_body_uses_contextual_type() {
    let source = SourceId::new(1);
    let text = r#"
fn main() -> i8 {
    return -12;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only typed negated numeric return should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed negated numeric return body should compile");

    assert!(
        compiler
            .code
            .constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I8(-12))),
        "typed CST negated numeric return should use the function return type"
    );
}

#[test]
fn syntax_only_boolean_not_return_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() -> bool {
    return !(true);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only boolean-not return should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only boolean-not return body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::Bool(false)),
        "CST boolean-not return should emit the evaluated boolean constant"
    );
}

#[test]
fn syntax_only_boolean_equality_return_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() -> bool {
    return (true) == false;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only boolean equality return should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only boolean equality return body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::Bool(false)),
        "CST boolean equality return should emit the evaluated boolean constant"
    );
}

#[test]
fn syntax_only_boolean_logical_return_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() -> bool {
    return true && !(false) && (1 < 2);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only boolean logical return should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only boolean logical return body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::Bool(true)),
        "CST boolean logical return should emit the evaluated boolean constant"
    );
}

#[test]
fn syntax_only_numeric_comparison_return_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() -> bool {
    return (1) < 2;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only numeric comparison return should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only numeric comparison return body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::Bool(true)),
        "CST numeric comparison return should emit the evaluated boolean constant"
    );
}

#[test]
fn syntax_only_path_numeric_comparison_preserves_record_trait_diagnostic() {
    let source = SourceId::new(1);
    let text = r#"
struct Reward { amount: i64 }

fn main(left: Reward) {
    return left < 1;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only path numeric comparison should not retain an owned statement fallback"
    );
    let error = compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect_err("known record comparison without PartialOrd should be a compile error");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::missing_comparison_trait"]
    );
}

#[test]
fn syntax_only_numeric_arithmetic_return_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() -> i64 {
    return 8 % 3;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only numeric arithmetic return should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only numeric arithmetic return body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(2)),
        "CST numeric arithmetic return should emit the evaluated integer constant"
    );
}

#[test]
fn syntax_only_numeric_division_return_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() -> i64 {
    return 8 / 2;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only numeric division return should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only numeric division return body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(4)),
        "CST numeric division return should emit the evaluated integer constant"
    );
}

#[test]
fn syntax_only_numeric_multiplication_return_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() -> i64 {
    return 3 * 4;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only numeric multiplication return should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only numeric multiplication return body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(12)),
        "CST numeric multiplication return should emit the evaluated integer constant"
    );
}

#[test]
fn syntax_only_numeric_subtraction_return_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() -> i64 {
    return 8 - 3;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only numeric subtraction return should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only numeric subtraction return body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(5)),
        "CST numeric subtraction return should emit the evaluated integer constant"
    );
}

#[test]
fn syntax_only_numeric_addition_return_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() -> i64 {
    return 1 + 2;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only numeric addition return should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only numeric addition return body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(3)),
        "CST numeric addition return should emit the evaluated integer constant"
    );
}

#[test]
fn syntax_only_literal_let_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value = 1;
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    assert!(
        statements[0].let_initializer_syntax_literal().is_some(),
        "CST literal let should expose a syntax literal payload"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only literal let body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(1)),
        "CST literal let should emit the literal constant"
    );
}

#[test]
fn syntax_only_negated_numeric_let_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value = -(12);
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only negated numeric let should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only negated numeric let body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(-12)),
        "CST negated numeric let should emit the negated constant"
    );
}

#[test]
fn syntax_only_typed_negated_numeric_let_body_uses_contextual_type() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value: i8 = -12;
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only typed negated numeric let should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed negated numeric let body should compile");

    assert!(
        compiler
            .code
            .constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I8(-12))),
        "typed CST negated numeric let should use the local type hint"
    );
}

#[test]
fn syntax_only_typed_boolean_not_let_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value: bool = !(false);
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only typed boolean-not let should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed boolean-not let body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::Bool(true)),
        "CST typed boolean-not let should emit the evaluated boolean constant"
    );
}

#[test]
fn syntax_only_typed_boolean_inequality_let_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value: bool = true != (false);
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only typed boolean inequality let should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed boolean inequality let body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::Bool(true)),
        "CST typed boolean inequality let should emit the evaluated boolean constant"
    );
}

#[test]
fn syntax_only_typed_boolean_logical_let_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value: bool = false || (true == true);
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only typed boolean logical let should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed boolean logical let body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::Bool(true)),
        "CST typed boolean logical let should emit the evaluated boolean constant"
    );
}

#[test]
fn syntax_only_typed_literal_equality_let_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value: bool = "ready" == ("ready");
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only typed literal equality let should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed literal equality let body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::Bool(true)),
        "CST typed literal equality let should emit the evaluated boolean constant"
    );
}

#[test]
fn syntax_only_typed_numeric_arithmetic_let_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value: i64 = 8 % 3;
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only typed numeric arithmetic let should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed numeric arithmetic let body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(2)),
        "CST typed numeric arithmetic let should emit the evaluated integer constant"
    );
}

#[test]
fn syntax_only_typed_numeric_division_let_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value: i64 = 8 / 2;
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only typed numeric division let should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed numeric division let body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(4)),
        "CST typed numeric division let should emit the evaluated integer constant"
    );
}

#[test]
fn syntax_only_typed_numeric_multiplication_let_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value: i64 = 3 * 4;
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only typed numeric multiplication let should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed numeric multiplication let body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(12)),
        "CST typed numeric multiplication let should emit the evaluated integer constant"
    );
}

#[test]
fn syntax_only_typed_numeric_subtraction_let_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value: i64 = 8 - 3;
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only typed numeric subtraction let should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed numeric subtraction let body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(5)),
        "CST typed numeric subtraction let should emit the evaluated integer constant"
    );
}

#[test]
fn syntax_only_typed_numeric_addition_let_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value: i64 = 1 + 2;
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only typed numeric addition let should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed numeric addition let body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(3)),
        "CST typed numeric addition let should emit the evaluated integer constant"
    );
}

#[test]
fn syntax_only_typed_numeric_literal_let_body_uses_contextual_type() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value: i8 = 12;
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");

    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed literal let body should compile");

    assert!(
        compiler
            .code
            .constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I8(12))),
        "typed CST literal let should use the local type hint"
    );
}

#[test]
fn syntax_only_literal_let_in_mixed_body_drops_owned_statement_fallback() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value = 1;
    return value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (_, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only literal let should not retain an owned statement fallback"
    );
    assert_eq!(
        statements[1].return_value_kind(),
        Some(SyntaxExpressionKind::Path)
    );
}

#[test]
fn syntax_only_path_let_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main(input) {
    let value = input;
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");

    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only path let body should compile");

    assert!(
        compiler.locals.contains_key("value"),
        "CST path let should bind the local"
    );
}

#[test]
fn syntax_only_typed_path_let_body_emits_runtime_guard() {
    let source = SourceId::new(1);
    let text = r#"
fn main(input) {
    let value: i64 = input;
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");

    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed path let body should compile");

    assert!(
        compiler
            .code
            .instructions
            .iter()
            .any(|instruction| matches!(
                instruction.kind,
                UnlinkedInstructionKind::GuardType { .. }
            )),
        "dynamic CST path assigned to a typed let should emit a runtime guard"
    );
}

#[test]
fn syntax_only_path_let_in_mixed_body_drops_owned_statement_fallback() {
    let source = SourceId::new(1);
    let text = r#"
fn main(input) {
    let value = input;
    return value + value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (_, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only path let should not retain an owned statement fallback"
    );
    assert_eq!(
        statements[1].return_value_kind(),
        Some(SyntaxExpressionKind::Binary)
    );
}

#[test]
fn syntax_only_path_return_body_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main(input) {
    return input;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");

    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only path return body should compile");

    assert_eq!(
        compiler
            .code
            .instructions
            .iter()
            .filter(|instruction| matches!(
                instruction.kind,
                UnlinkedInstructionKind::Return { .. }
            ))
            .count(),
        1
    );
}

#[test]
fn syntax_only_path_return_in_mixed_body_drops_owned_statement_fallback() {
    let source = SourceId::new(1);
    let text = r#"
fn main(input) {
    return input;
    return input + input;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (_, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only path return should not retain an owned statement fallback"
    );
    assert_eq!(
        statements[1].return_value_kind(),
        Some(SyntaxExpressionKind::Binary)
    );
}

#[test]
fn syntax_only_self_let_and_return_method_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
struct CstBox {}

impl CstBox {
    fn id(self) {
        let copy = self;
        return copy;
    }
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let method = semantic
        .script_impl_methods()
        .into_iter()
        .find(|method| method.method_name == "id")
        .expect("id method");

    compile_program_source(source, text).expect("CST self let and return method should compile");

    assert!(
        !method.body.has_fallback_statements(),
        "syntax-only self let/return method should not retain an owned body fallback"
    );
}

#[test]
fn syntax_only_block_let_and_return_compiles_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn block_let() {
    let value = {
        let nested;
        return;
    };
}

fn block_return() {
    return {
        let nested;
        return;
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (block_let, _, _) = semantic.function("block_let").expect("block_let");
    let (block_return, _, _) = semantic.function("block_return").expect("block_return");

    compile_program_source(source, text).expect("CST block let/return should compile");

    assert!(
        !block_let.body.has_fallback_statements(),
        "syntax-only block let body should not retain an owned body fallback"
    );
    assert!(
        !block_return.body.has_fallback_statements(),
        "syntax-only block return body should not retain an owned body fallback"
    );
}

#[test]
fn syntax_only_parenthesized_simple_let_and_return_compile_without_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn parenthesized_simple_values(input) {
    let literal = (1);
    let local = (input);
    return (local);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("parenthesized_simple_values")
        .expect("parenthesized_simple_values");

    compile_program_source(source, text).expect("CST parenthesized simple values should compile");

    assert!(
        !payload.body.has_fallback_statements(),
        "syntax-only parenthesized simple values should not retain an owned body fallback"
    );
}

#[test]
fn unclassified_let_initializer_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value = [1];
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statement = payload.body.statement_payloads()[0].fallback();
    let vela_syntax::ast::StmtKind::Let {
        value: Some(value), ..
    } = &statement.kind
    else {
        panic!("expected let statement");
    };
    let missing_payload = body_payloads::CompilerExpressionPayload::missing_syntax(source, value);

    let error = compiler
        .compile_let_initializer_value_payload_for_test(value, Some(&missing_payload))
        .expect_err("unclassified CST let payload must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST let initializer payload")
    ));
}

#[test]
fn let_initializer_kind_without_expression_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value = [1];
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statement = payload.body.statement_payloads()[0].fallback();
    let vela_syntax::ast::StmtKind::Let {
        value: Some(value), ..
    } = &statement.kind
    else {
        panic!("expected let statement");
    };

    let error = compiler
        .compile_let_initializer_kind_without_expression_payload_for_test(
            value,
            SyntaxExpressionKind::Array,
        )
        .expect_err("kind-only CST let payload must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST let initializer payload")
    ));
}

#[test]
fn mismatched_return_value_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn take(value) {
    return value;
}

fn main() {
    let value = 1;
    return take(value);
    return [value];
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_return = statements[1]
        .syntax_statement()
        .expect("CST return statement")
        .clone();
    let legacy_array_return = statements[2].fallback();
    let mismatched =
        body_payloads::CompilerStatementPayload::syntax(source, cst_return, legacy_array_return);

    let error = compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect_err("mismatched return value payload must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST return value payload")
    ));
}

#[test]
fn unclassified_return_value_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn main(value) {
    return value + value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statement = payload.body.statement_payloads()[0].fallback();
    let vela_syntax::ast::StmtKind::Return(Some(value)) = &statement.kind else {
        panic!("expected return statement");
    };
    let missing_payload = body_payloads::CompilerExpressionPayload::missing_syntax(source, value);

    let error = compiler
        .compile_return_value_payload_for_test(value, Some(&missing_payload))
        .expect_err("unclassified CST return payload must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST return value payload")
    ));
}

#[test]
fn return_value_kind_without_expression_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn main(value) {
    return value + value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statement = payload.body.statement_payloads()[0].fallback();
    let vela_syntax::ast::StmtKind::Return(Some(value)) = &statement.kind else {
        panic!("expected return statement");
    };

    let error = compiler
        .compile_return_kind_without_expression_payload_for_test(
            value,
            SyntaxExpressionKind::Binary,
        )
        .expect_err("kind-only CST return payload must not compile legacy expression");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST return value payload")
    ));
}

#[test]
fn mismatched_path_return_value_payload_does_not_use_legacy_expression() {
    let source = SourceId::new(1);
    let text = r#"
fn main(value) {
    return value;
    return value + value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_self_return = statements[0]
        .syntax_statement()
        .expect("CST return statement")
        .clone();
    let legacy_path_return = statements[1].fallback();
    let mismatched = body_payloads::CompilerStatementPayload::syntax(
        source,
        cst_self_return,
        legacy_path_return,
    );

    compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect("CST path return payload should compile without legacy expression");

    assert!(
        compiler
            .code
            .instructions
            .iter()
            .all(|instruction| !matches!(instruction.kind, UnlinkedInstructionKind::Add { .. })),
        "CST path return must not compile the legacy binary expression"
    );
}

#[test]
fn missing_simple_let_initializer_payload_uses_cst_empty_let() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let cst_value;
    let legacy_value = [1];
    return 0;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_let_without_initializer = statements[0]
        .syntax_statement()
        .expect("CST let statement")
        .clone();
    let legacy_array_let = statements[1].fallback();
    let mismatched = body_payloads::CompilerStatementPayload::syntax(
        source,
        cst_let_without_initializer,
        legacy_array_let,
    );

    compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect("CST empty let payload must not compile legacy literal expression");

    assert_empty_let_without_i64_fallback(&compiler);
}

#[test]
fn missing_let_initializer_block_body_payload_does_not_use_legacy_block() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value = {
        let nested = 1;
        nested + 1
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statement = payload.body.statement_payloads()[0]
        .syntax_statement()
        .expect("CST let")
        .clone();
    let missing_child = body_payloads::CompilerStatementPayload::missing_child_payload_context(
        statement,
        payload.body.statement_payloads()[0].fallback(),
    );

    let error = compiler
        .compile_statement_payload_for_test(&missing_child)
        .expect_err("missing CST let block body must not compile legacy block");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST let initializer block body payload")
    ));
}

#[test]
fn syntax_only_let_initializer_block_drops_owned_body_fallback() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value = {
        let nested;
        return;
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (_, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let body = payload.body.statement_payloads()[0]
        .let_initializer_block_body_payload()
        .expect("let initializer block body payload");

    assert!(
        !body.has_fallback_statements(),
        "syntax-only let initializer block should not retain an owned body fallback"
    );
}

#[test]
fn empty_return_payload_with_array_fallback_uses_cst_empty_return() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value = 1;
    return;
    return [value];
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_return_without_value = statements[1]
        .syntax_statement()
        .expect("CST return statement")
        .clone();
    let legacy_array_return = statements[2].fallback();
    let mismatched = body_payloads::CompilerStatementPayload::syntax(
        source,
        cst_return_without_value,
        legacy_array_return,
    );

    compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect("CST empty return payload must not compile legacy array expression");

    assert_empty_return_without_i64_fallback(&compiler);
}

#[test]
fn missing_return_block_body_payload_does_not_use_legacy_block() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    return {
        let nested = 1;
        nested + 1
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statement = payload.body.statement_payloads()[0]
        .syntax_statement()
        .expect("CST return")
        .clone();
    let missing_child = body_payloads::CompilerStatementPayload::missing_child_payload_context(
        statement,
        payload.body.statement_payloads()[0].fallback(),
    );

    let error = compiler
        .compile_statement_payload_for_test(&missing_child)
        .expect_err("missing CST return block body must not compile legacy block");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST return block body payload")
    ));
}

#[test]
fn syntax_only_return_value_block_drops_owned_body_fallback() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    return {
        let nested;
        return;
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (_, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let body = payload.body.statement_payloads()[0]
        .return_value_block_body_payload()
        .expect("return value block body payload");

    assert!(
        !body.has_fallback_statements(),
        "syntax-only return value block should not retain an owned body fallback"
    );
}

#[test]
fn empty_return_statement_payload_uses_cst_kind() {
    let source = SourceId::new(1);
    let text = r#"
fn cst_body() {
    return;
}

fn fallback_body() {
    let value = 1;
    return value + value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (cst_payload, _, _) = semantic.function("cst_body").expect("cst function");
    let (fallback_payload, _, _) = semantic
        .function("fallback_body")
        .expect("fallback function");
    let cst_statement = cst_payload.body.statement_payloads()[0]
        .syntax_statement()
        .expect("cst statement syntax")
        .clone();
    let fallback_statement = fallback_payload.body.statement_payloads()[1].fallback();
    let mismatched =
        body_payloads::CompilerStatementPayload::syntax(source, cst_statement, fallback_statement);
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "fallback_body");

    compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect("CST empty return payload must not compile fallback return value");

    assert_empty_return_without_i64_fallback(&compiler);
}

#[test]
fn empty_return_payload_with_literal_fallback_uses_cst_empty_return() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    return;
    let value = 1;
    return value + value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();
    let cst_return_without_value = statements[0]
        .syntax_statement()
        .expect("CST return statement")
        .clone();
    let legacy_literal_return = statements[2].fallback();
    let mismatched = body_payloads::CompilerStatementPayload::syntax(
        source,
        cst_return_without_value,
        legacy_literal_return,
    );

    compiler
        .compile_statement_payload_for_test(&mismatched)
        .expect("CST empty return payload must not compile legacy literal expression");

    assert_empty_return_without_i64_fallback(&compiler);
}

fn assert_empty_return_without_i64_fallback(compiler: &Compiler<'_, '_>) {
    assert_eq!(
        compiler
            .code
            .instructions
            .iter()
            .filter(|instruction| matches!(
                instruction.kind,
                UnlinkedInstructionKind::Return { .. }
            ))
            .count(),
        1
    );
    assert!(
        compiler
            .code
            .constants
            .iter()
            .all(|constant| *constant != Constant::i64(1)),
        "fallback return value must not be emitted"
    );
}

fn assert_empty_let_without_i64_fallback(compiler: &Compiler<'_, '_>) {
    assert!(
        compiler.code.constants.contains(&Constant::Null),
        "CST empty let must emit null"
    );
    assert!(
        compiler
            .code
            .constants
            .iter()
            .all(|constant| *constant != Constant::i64(1)),
        "fallback let initializer must not be emitted"
    );
}
