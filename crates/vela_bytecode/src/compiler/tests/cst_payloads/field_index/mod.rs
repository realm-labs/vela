use super::*;

mod helpers;
mod host_access;

fn field_index_statement_payloads<'ast>(
    body: &body_payloads::CompilerBodyPayload<'ast>,
) -> Vec<body_payloads::CompilerStatementPayload<'ast>> {
    paired_statement_payloads_for_body(body.syntax_payload().source, body)
}

#[test]
fn semantic_function_field_and_index_operands_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
struct Counter {
    value: i64,
}

fn make_counter(value) {
    return Counter { value: value };
}

fn make_counters(value) {
    return [Counter { value: value }];
}

fn field_and_index_values() {
    let field = make_counter({
        let current = 2;
        current
    }).value;
    let indexed = make_counters({
        let all = 3;
        all
    })[{
        let offset = 0;
        offset
    }].value;
    let assigned = 0;
    assigned = make_counter({
        let assigned_current = 4;
        assigned_current
    }).value;
    assigned = make_counters({
        let assigned_all = 5;
        assigned_all
    })[{
        let assigned_offset = 0;
        assigned_offset
    }].value;
    return field + indexed;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("field_and_index_values")
        .expect("field_and_index_values function");

    helpers::assert_cst_let_initializer_field_base_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let current = 2;"),
                (SyntaxStatementKind::Expr, "current"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let all = 3;"),
                (SyntaxStatementKind::Expr, "all"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let offset = 0;"),
                (SyntaxStatementKind::Expr, "offset"),
            ],
        ],
    );
    helpers::assert_cst_let_initializer_index_operand_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let all = 3;"),
                (SyntaxStatementKind::Expr, "all"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let offset = 0;"),
                (SyntaxStatementKind::Expr, "offset"),
            ],
        ],
    );
    helpers::assert_cst_assignment_value_field_base_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let assigned_current = 4;"),
                (SyntaxStatementKind::Expr, "assigned_current"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let assigned_all = 5;"),
                (SyntaxStatementKind::Expr, "assigned_all"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let assigned_offset = 0;"),
                (SyntaxStatementKind::Expr, "assigned_offset"),
            ],
        ],
    );
    helpers::assert_cst_assignment_value_index_operand_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let assigned_all = 5;"),
                (SyntaxStatementKind::Expr, "assigned_all"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let assigned_offset = 0;"),
                (SyntaxStatementKind::Expr, "assigned_offset"),
            ],
        ],
    );
    helpers::assert_cst_let_initializer_field_names(&payload.body, &["value", "value"]);
    helpers::assert_cst_assignment_value_field_names(&payload.body, &["value", "value"]);

    compile_program_source(source, text).expect("CST-backed field/index operands should compile");
}

#[test]
fn syntax_only_index_receiver_field_reads_compile() {
    let source = SourceId::new(1);
    let text = r#"
fn main(fields) {
    return fields[0].name == "now" && fields[1].name == "tick";
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");

    assert!(
        body_has_no_statement_fallbacks(&payload.body),
        "syntax-only index receiver field return should not retain statement fallbacks"
    );

    let program = compile_program_source(source, text)
        .expect("CST-backed index receiver field reads should compile");
    let main = program.function("main").expect("main function");

    assert!(
        main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetIndex { .. }
        )),
        "CST index receiver should emit index reads"
    );
    assert!(
        main.instructions
            .iter()
            .filter(|instruction| {
                matches!(
                    instruction.kind,
                    UnlinkedInstructionKind::GetRecordField { ref field, .. } if field == "name"
                )
            })
            .count()
            >= 2,
        "CST index receiver fields should emit dynamic field reads"
    );
}

#[test]
fn field_name_payload_comes_from_cst_without_field_fallback() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let object = { value: 1 };
    let cst_field = make(object).value;
    let fallback_path = make(object);
}

fn make(value) {
    return value;
}
"#,
        |_compiler, payload| {
            let statements = field_index_statement_payloads(&payload.body);
            let cst_field = statements[1]
                .let_initializer_expression_payload()
                .expect("CST field payload");
            let fallback_path = statements[2]
                .let_initializer_expression_payload()
                .expect("non-field fallback payload");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_field
                    .syntax_expression()
                    .expect("CST field expression")
                    .clone(),
                fallback_path.fallback(),
            );

            assert_eq!(
                mismatched_payload.syntax_field_name().as_deref(),
                Some("value")
            );
            assert!(
                mismatched_payload.field_base_payload().is_none(),
                "field child payloads still require field-shaped fallback children"
            );
        },
    );
}

#[test]
fn semantic_function_assignment_targets_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
struct Counter {
    value: i64,
}

struct CounterBox {
    counter: Counter,
}

fn make_box(value) {
    return CounterBox { counter: Counter { value: value } };
}

fn assignment_targets() {
    make_box({
        let seed = 1;
        seed
    }).counter.value = 2;
    let counters = [Counter { value: 0 }];
    counters[{
        let offset = 0;
        offset
    }].value = 3;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("assignment_targets")
        .expect("assignment_targets function");

    helpers::assert_cst_assignment_target_field_base_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let seed = 1;"),
                (SyntaxStatementKind::Expr, "seed"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let offset = 0;"),
                (SyntaxStatementKind::Expr, "offset"),
            ],
        ],
    );
    helpers::assert_cst_assignment_target_index_operand_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let offset = 0;"),
            (SyntaxStatementKind::Expr, "offset"),
        ]],
    );
    helpers::assert_cst_assignment_target_field_names(&payload.body, &["value", "value"]);

    compile_program_source(source, text).expect("CST-backed assignment targets should compile");
}

#[test]
fn field_read_slots_prefer_cst_receiver_payloads() {
    with_cst_payload_compiler(
        r#"
struct CstBox {
    alpha: i64,
    amount: i64,
}

struct LegacyBox {
    amount: i64,
    zed: i64,
}

fn main() {
    let cst = CstBox { alpha: 0, amount: 1 };
    let legacy = LegacyBox { amount: 2, zed: 3 };
    let cst_amount = cst.amount;
    let legacy_amount = legacy.amount;
}
"#,
        |compiler, payload| {
            let statements = field_index_statement_payloads(&payload.body);
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("cst local should compile");
            compiler
                .compile_statement_payload_for_test(&statements[1])
                .expect("legacy local should compile");
            let cst_field = statements[2]
                .let_initializer_expression_payload()
                .expect("CST field payload");
            let legacy_field = statements[3]
                .let_initializer_expression_payload()
                .expect("legacy field fallback");
            let mismatched_payload = expression_payload_with_fallback(
                SourceId::new(1),
                cst_field
                    .syntax_expression()
                    .expect("CST field expression")
                    .clone(),
                legacy_field.fallback(),
            );

            let register = compiler
                .compile_expr_with_payload(mismatched_payload.fallback(), Some(&mismatched_payload))
                .expect("mismatched fallback should not block CST field compilation");

            assert!(
                compiler
                    .code
                    .instructions
                    .iter()
                    .any(|instruction| matches!(
                        &instruction.kind,
                        UnlinkedInstructionKind::GetRecordSlot {
                            dst,
                            field,
                            slot: 1,
                            ..
                        } if *dst == register && field == "amount"
                    ))
            );
        },
    );
}

#[test]
fn missing_field_receiver_payload_does_not_use_legacy_receiver() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let object = { amount: 1 };
    let value = .amount;
}
"#;
    let legacy_text = r#"
fn main() {
    let object = { amount: 1 };
    let value = make(object).amount;
}

fn make(value) {
    return value;
}
"#;
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_field = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST function body")
        .statements()
        .nth(1)
        .expect("CST let statement")
        .as_let()
        .expect("CST let")
        .initializer()
        .expect("CST initializer");
    assert_eq!(cst_field.expression_kind(), SyntaxExpressionKind::Field);

    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = field_index_statement_payloads(&legacy_payload.body);
    compiler
        .compile_statement_payload_for_test(&statements[0])
        .expect("object local should compile");
    let legacy_field = statements[1]
        .let_initializer_expression_payload()
        .expect("legacy field payload");
    let missing = expression_payload_with_fallback(source, cst_field, legacy_field.fallback());
    let receiver = missing
        .field_base_payload()
        .expect("field receiver payload");
    assert!(receiver.syntax_expression().is_none());

    let error = compiler
        .compile_expr_with_payload(legacy_field.fallback(), Some(&missing))
        .expect_err("missing CST field receiver must not compile legacy receiver");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST field receiver")
    ));
}

#[test]
fn missing_field_name_payload_does_not_use_legacy_field_name() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let object = { amount: 1 };
    let value = object.;
}
"#;
    let legacy_text = r#"
fn main() {
    let object = { amount: 1 };
    let value = make(object).amount;
}

fn make(value) {
    return value;
}
"#;
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_field = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST function body")
        .statements()
        .nth(1)
        .expect("CST let statement")
        .as_let()
        .expect("CST let")
        .initializer()
        .expect("CST initializer");
    assert_eq!(cst_field.expression_kind(), SyntaxExpressionKind::Field);

    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = field_index_statement_payloads(&legacy_payload.body);
    compiler
        .compile_statement_payload_for_test(&statements[0])
        .expect("object local should compile");
    let legacy_field = statements[1]
        .let_initializer_expression_payload()
        .expect("legacy field payload");
    let missing = expression_payload_with_fallback(source, cst_field, legacy_field.fallback());
    assert_eq!(missing.syntax_field_name(), None);

    let error = compiler
        .compile_expr_with_payload(legacy_field.fallback(), Some(&missing))
        .expect_err("missing CST field name must not compile legacy field name");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("field expression")
    ));
}

#[test]
fn missing_field_expression_payload_does_not_use_legacy_field() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let object = { amount: 1 };
    let value = make(object).amount;
}

fn make(value) {
    return value;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = field_index_statement_payloads(&legacy_payload.body);
    compiler
        .compile_statement_payload_for_test(&statements[0])
        .expect("object local should compile");
    let legacy_field = statements[1]
        .let_initializer_expression_payload()
        .expect("legacy field payload");
    let missing_field =
        body_payloads::CompilerExpressionPayload::missing_syntax(source, legacy_field.fallback());

    let error = compiler
        .compile_expr_with_payload(legacy_field.fallback(), Some(&missing_field))
        .expect_err("missing CST field payload must not compile legacy field");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST expression payload")
    ));
}

#[test]
fn record_field_assignment_target_facts_prefer_cst_root_payloads() {
    with_cst_payload_compiler(
        r#"
struct CstBox {
    amount: i64,
}

struct LegacyBox {
    amount: bool,
}

fn main() {
    let cst = CstBox { amount: 0 };
    let legacy = LegacyBox { amount: false };
    cst.amount = true;
    legacy.amount = true;
}
"#,
        |compiler, payload| {
            let statements = field_index_statement_payloads(&payload.body);
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("cst local should compile");
            compiler
                .compile_statement_payload_for_test(&statements[1])
                .expect("legacy local should compile");
            let cst_target = statements[2]
                .expression_payload()
                .and_then(|payload| payload.assignment_target_payload())
                .expect("CST assignment target payload");
            let legacy_statement = statements[3]
                .expression_payload()
                .expect("legacy assignment expression");
            let legacy_target = statements[3]
                .expression_payload()
                .and_then(|payload| payload.assignment_target_payload())
                .expect("legacy assignment target fallback");
            let mismatched_target = expression_payload_with_fallback(
                SourceId::new(1),
                cst_target
                    .syntax_expression()
                    .expect("CST target expression")
                    .clone(),
                legacy_target.fallback(),
            );

            let error = compiler
                .compile_assignment_with_payloads(
                    legacy_statement.fallback(),
                    crate::compiler::assignments::AssignmentTargetSyntax::new(Some(
                        &mismatched_target,
                    )),
                    crate::compiler::assignments::AssignmentValueSyntax::new(None, None, None),
                )
                .expect_err("mismatched CST assignment target must not compile");
            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("mismatched CST assignment target")
            ));
        },
    );
}

#[test]
fn indexed_record_assignment_slots_prefer_cst_collection_payloads() {
    with_cst_payload_compiler(
        r#"
struct CstBox {
    alpha: i64,
    amount: i64,
}

struct LegacyBox {
    amount: i64,
    zed: i64,
}

fn main() {
    let cst_items = [CstBox { alpha: 0, amount: 1 }];
    let legacy_items = [LegacyBox { amount: 2, zed: 3 }];
    cst_items[0].amount = 10;
    legacy_items[0].amount = 20;
}
"#,
        |compiler, payload| {
            let statements = field_index_statement_payloads(&payload.body);
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("cst_items local should compile");
            compiler
                .compile_statement_payload_for_test(&statements[1])
                .expect("legacy_items local should compile");
            let cst_target = statements[2]
                .expression_payload()
                .and_then(|payload| payload.assignment_target_payload())
                .expect("CST indexed assignment target");
            let legacy_statement = statements[3]
                .expression_payload()
                .expect("legacy indexed assignment expression");
            let legacy_target = statements[3]
                .expression_payload()
                .and_then(|payload| payload.assignment_target_payload())
                .expect("legacy indexed assignment target");
            let mismatched_target = expression_payload_with_fallback(
                SourceId::new(1),
                cst_target
                    .syntax_expression()
                    .expect("CST target expression")
                    .clone(),
                legacy_target.fallback(),
            );

            let error = compiler
                .compile_assignment_with_payloads(
                    legacy_statement.fallback(),
                    crate::compiler::assignments::AssignmentTargetSyntax::new(Some(
                        &mismatched_target,
                    )),
                    crate::compiler::assignments::AssignmentValueSyntax::new(None, None, None),
                )
                .expect_err("mismatched CST indexed assignment target must not compile");
            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("mismatched CST assignment target")
            ));
        },
    );
}

#[test]
fn string_key_index_reads_prefer_cst_index_literal_payloads() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst = { "alpha": 1 };
    let legacy = { "legacy": 2 };
    let cst_value = cst["alpha"];
    let legacy_value = legacy[0];
}
"#,
        |compiler, payload| {
            let statements = field_index_statement_payloads(&payload.body);
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("cst map should compile");
            compiler
                .compile_statement_payload_for_test(&statements[1])
                .expect("legacy map should compile");
            let cst_index = statements[2]
                .let_initializer_expression_payload()
                .expect("CST string-key index payload");
            let legacy_index = statements[3]
                .let_initializer_expression_payload()
                .expect("legacy numeric index fallback");
            let mismatched_index = expression_payload_with_fallback(
                SourceId::new(1),
                cst_index
                    .syntax_expression()
                    .expect("CST index syntax")
                    .clone(),
                legacy_index.fallback(),
            );

            let register = compiler
                .compile_expr_with_payload(mismatched_index.fallback(), Some(&mismatched_index))
                .expect("mismatched fallback should not block CST index compilation");

            assert!(
                compiler
                    .code
                    .instructions
                    .iter()
                    .any(|instruction| matches!(
                        &instruction.kind,
                        UnlinkedInstructionKind::GetStringKeyIndex { dst, .. } if *dst == register
                    ))
            );
        },
    );
}

#[test]
fn string_key_index_reads_require_cst_index_literal_payload() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let values = { "legacy": 1 };
    let value = values["legacy"];
}
"#,
        |_compiler, payload| {
            let statement = field_index_statement_payloads(&payload.body)[1]
                .let_initializer_expression_payload()
                .expect("index payload");
            assert!(
                statement.index_operand_payloads().is_some(),
                "index operand payloads should be present"
            );

            let key = crate::compiler::expressions::literal_string_with_payload(None);

            assert_eq!(
                key, None,
                "old string literal fallback must not drive string-key index lowering"
            );
        },
    );
}

#[test]
fn missing_index_operand_payload_does_not_use_legacy_index() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    let values = [1];
    let value = values[];
}
"#;
    let legacy_text = r#"
fn main() {
    let values = [1];
    let value = values[0];
}
"#;
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_index = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST function body")
        .statements()
        .nth(1)
        .expect("CST let statement")
        .as_let()
        .expect("CST let")
        .initializer()
        .expect("CST initializer");
    assert_eq!(cst_index.expression_kind(), SyntaxExpressionKind::Index);

    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = field_index_statement_payloads(&legacy_payload.body);
    compiler
        .compile_statement_payload_for_test(&statements[0])
        .expect("values local should compile");
    let legacy_index = statements[1]
        .let_initializer_expression_payload()
        .expect("legacy index payload");
    let missing = expression_payload_with_fallback(source, cst_index, legacy_index.fallback());
    let (_base, index) = missing
        .index_operand_payloads()
        .expect("index operand payloads");
    assert!(index.syntax_expression().is_none());

    let error = compiler
        .compile_expr_with_payload(legacy_index.fallback(), Some(&missing))
        .expect_err("missing CST index operand must not compile legacy operand");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST index operand")
    ));
}

#[test]
fn missing_index_expression_payload_does_not_use_legacy_index() {
    let source = SourceId::new(1);
    let text = r#"
fn main() {
    let values = [1];
    let value = values[0];
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let statements = field_index_statement_payloads(&legacy_payload.body);
    compiler
        .compile_statement_payload_for_test(&statements[0])
        .expect("values local should compile");
    let legacy_index = statements[1]
        .let_initializer_expression_payload()
        .expect("legacy index payload");
    let missing_index =
        body_payloads::CompilerExpressionPayload::missing_syntax(source, legacy_index.fallback());

    let error = compiler
        .compile_expr_with_payload(legacy_index.fallback(), Some(&missing_index))
        .expect_err("missing CST index payload must not compile legacy index");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST expression payload")
    ));
}

#[test]
fn string_key_index_writes_prefer_cst_index_literal_payloads() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let cst = { "alpha": 1 };
    let legacy = { "legacy": 2 };
    cst["alpha"] = 3;
    legacy[0] = 4;
}
"#,
        |compiler, payload| {
            let statements = field_index_statement_payloads(&payload.body);
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("cst map should compile");
            compiler
                .compile_statement_payload_for_test(&statements[1])
                .expect("legacy map should compile");
            let cst_target = statements[2]
                .expression_payload()
                .and_then(|payload| payload.assignment_target_payload())
                .expect("CST string-key assignment target payload");
            let legacy_statement = statements[3]
                .expression_payload()
                .expect("legacy numeric assignment expression");
            let legacy_target = statements[3]
                .expression_payload()
                .and_then(|payload| payload.assignment_target_payload())
                .expect("legacy numeric assignment target fallback");
            let mismatched_target = expression_payload_with_fallback(
                SourceId::new(1),
                cst_target
                    .syntax_expression()
                    .expect("CST target syntax")
                    .clone(),
                legacy_target.fallback(),
            );

            let error = compiler
                .compile_assignment_with_payloads(
                    legacy_statement.fallback(),
                    crate::compiler::assignments::AssignmentTargetSyntax::new(Some(
                        &mismatched_target,
                    )),
                    crate::compiler::assignments::AssignmentValueSyntax::new(None, None, None),
                )
                .expect_err("mismatched CST index assignment target must not compile");
            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("mismatched CST assignment target")
            ));
        },
    );
}
