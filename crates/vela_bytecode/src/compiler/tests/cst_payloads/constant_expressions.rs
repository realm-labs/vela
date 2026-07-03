use super::*;

#[test]
fn syntax_only_constant_comparison_return_drops_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() -> bool {
    return (1 + 2) < (5 - 1);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only constant comparison return should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only constant comparison return body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::Bool(true)),
        "CST constant comparison return should emit the evaluated boolean constant"
    );
}

#[test]
fn syntax_only_typed_constant_equality_let_drops_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value: bool = (true && false) == !(true);
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
        "syntax-only typed constant equality let should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed constant equality let body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::Bool(true)),
        "CST typed constant equality let should emit the evaluated boolean constant"
    );
}

#[test]
fn syntax_only_constant_unary_return_drops_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() -> i64 {
    return -(1 + 2);
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only constant unary return should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only constant unary return body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(-3)),
        "CST constant unary return should emit the evaluated integer constant"
    );
}

#[test]
fn syntax_only_typed_constant_unary_let_drops_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value: bool = !(true && false);
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
        "syntax-only typed constant unary let should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed constant unary let body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::Bool(true)),
        "CST typed constant unary let should emit the evaluated boolean constant"
    );
}

#[test]
fn syntax_only_nested_constant_unary_arithmetic_return_drops_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() -> i64 {
    return -(1 + 2) * 4;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only nested constant unary arithmetic return should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only nested constant unary arithmetic return body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::i64(-12)),
        "CST nested constant unary arithmetic return should emit the evaluated integer constant"
    );
}

#[test]
fn syntax_only_typed_nested_constant_unary_logical_let_drops_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value: bool = !(true && false) || false;
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
        "syntax-only typed nested constant unary logical let should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only typed nested constant unary logical let body should compile");

    assert!(
        compiler.code.constants.contains(&Constant::Bool(true)),
        "CST typed nested constant unary logical let should emit the evaluated boolean constant"
    );
}

#[test]
fn syntax_only_empty_array_let_drops_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let values: Array<i64> = [];
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
        "syntax-only empty array let should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only empty array let body should compile");

    assert!(
        compiler
            .code
            .constants
            .contains(&Constant::Array(Vec::new())),
        "CST empty array let should emit the evaluated empty array constant"
    );
}

#[test]
fn syntax_only_empty_array_return_drops_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    return [];
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = payload.body.statement_payloads();

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| statements[0].fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only empty array return should not retain an owned statement fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only empty array return body should compile");

    assert!(
        compiler
            .code
            .constants
            .contains(&Constant::Array(Vec::new())),
        "CST empty array return should emit the evaluated empty array constant"
    );
}

#[test]
fn syntax_only_nested_empty_array_block_let_drops_owned_body_lookup() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let value = {
        let nested: Array<i64> = [];
    };
    return;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, payload) = cst_payload_compiler_for_function(&semantic, "main");

    let fallback_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| payload.body.fallback()));

    assert!(
        fallback_result.is_err(),
        "syntax-only nested empty array block let should not retain an owned body fallback"
    );
    compiler
        .compile_body_payload_statements_for_test(&payload.body)
        .expect("syntax-only nested empty array block let body should compile");

    assert!(
        compiler
            .code
            .constants
            .contains(&Constant::Array(Vec::new())),
        "CST nested empty array block let should emit the evaluated empty array constant"
    );
}
