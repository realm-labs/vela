use super::*;

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

    assert_cst_let_initializer_field_base_body_payloads(
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
    assert_cst_let_initializer_index_operand_body_payloads(
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
    assert_cst_assignment_value_field_base_body_payloads(
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
    assert_cst_assignment_value_index_operand_body_payloads(
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
    assert_cst_let_initializer_field_names(&payload.body, &["value", "value"]);
    assert_cst_assignment_value_field_names(&payload.body, &["value", "value"]);

    compile_program_source(source, text).expect("CST-backed field/index operands should compile");
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

    assert_cst_assignment_target_field_base_body_payloads(
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
    assert_cst_assignment_target_index_operand_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let offset = 0;"),
            (SyntaxStatementKind::Expr, "offset"),
        ]],
    );
    assert_cst_assignment_target_field_names(&payload.body, &["value", "value"]);

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
            let statements = payload.body.statement_payloads();
            compiler
                .compile_statement(statements[0].fallback())
                .expect("cst local should compile");
            compiler
                .compile_statement(statements[1].fallback())
                .expect("legacy local should compile");
            let cst_field = statements[2]
                .let_initializer_expression_payload()
                .expect("CST field payload");
            let legacy_field = statements[3]
                .let_initializer_expression_payload()
                .expect("legacy field fallback");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_field
                    .syntax_expression()
                    .expect("CST field expression")
                    .clone(),
                legacy_field.fallback(),
            );

            compiler
                .compile_expr_with_payload(mismatched_payload.fallback(), Some(&mismatched_payload))
                .expect("CST-backed field read should compile");
            let slot = compiler
                .code
                .instructions
                .iter()
                .rev()
                .find_map(|instruction| {
                    let UnlinkedInstructionKind::GetRecordSlot { field, slot, .. } =
                        &instruction.kind
                    else {
                        return None;
                    };
                    (field == "amount").then_some(*slot)
                });
            assert_eq!(slot, Some(1));
        },
    );
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
            let statements = payload.body.statement_payloads();
            compiler
                .compile_statement(statements[0].fallback())
                .expect("cst local should compile");
            compiler
                .compile_statement(statements[1].fallback())
                .expect("legacy local should compile");
            let cst_target = statements[2]
                .assignment_target_expression_payload()
                .expect("CST assignment target payload");
            let legacy_statement = statements[3]
                .expression_payload()
                .expect("legacy assignment expression");
            let legacy_target = statements[3]
                .assignment_target_expression_payload()
                .expect("legacy assignment target fallback");
            let mismatched_target = body_payloads::CompilerExpressionPayload::syntax(
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
                    crate::compiler::assignments::AssignmentValueSyntax::new(
                        None,
                        None,
                        crate::compiler::assignments::AssignmentValuePayloads::new(
                            None, None, None, None,
                        ),
                    ),
                )
                .expect_err("CST target amount expects i64, not bool");
            let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
                panic!("expected semantic diagnostics: {:?}", error.kind);
            };
            let diagnostic = diagnostics
                .iter()
                .find(|diagnostic| {
                    diagnostic.code.as_deref() == Some("compiler::type_contract_mismatch")
                })
                .expect("type contract mismatch diagnostic");
            assert!(
                diagnostic
                    .labels
                    .iter()
                    .any(|label| label.message.contains("expected `i64`")),
                "{diagnostic:?}"
            );
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
            let statements = payload.body.statement_payloads();
            compiler
                .compile_statement(statements[0].fallback())
                .expect("cst_items local should compile");
            compiler
                .compile_statement(statements[1].fallback())
                .expect("legacy_items local should compile");
            let cst_target = statements[2]
                .assignment_target_expression_payload()
                .expect("CST indexed assignment target");
            let legacy_statement = statements[3]
                .expression_payload()
                .expect("legacy indexed assignment expression");
            let legacy_target = statements[3]
                .assignment_target_expression_payload()
                .expect("legacy indexed assignment target");
            let mismatched_target = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_target
                    .syntax_expression()
                    .expect("CST target expression")
                    .clone(),
                legacy_target.fallback(),
            );

            compiler
                .compile_assignment_with_payloads(
                    legacy_statement.fallback(),
                    crate::compiler::assignments::AssignmentTargetSyntax::new(Some(
                        &mismatched_target,
                    )),
                    crate::compiler::assignments::AssignmentValueSyntax::new(
                        None,
                        None,
                        crate::compiler::assignments::AssignmentValuePayloads::new(
                            None, None, None, None,
                        ),
                    ),
                )
                .expect("CST-backed indexed assignment should compile");
            let slot = compiler
                .code
                .instructions
                .iter()
                .rev()
                .find_map(|instruction| {
                    let UnlinkedInstructionKind::SetRecordSlot { field, slot, .. } =
                        &instruction.kind
                    else {
                        return None;
                    };
                    (field == "amount").then_some(*slot)
                });
            assert_eq!(slot, Some(1));
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
            let statements = payload.body.statement_payloads();
            compiler
                .compile_statement(statements[0].fallback())
                .expect("cst map should compile");
            compiler
                .compile_statement(statements[1].fallback())
                .expect("legacy map should compile");
            let cst_index = statements[2]
                .let_initializer_expression_payload()
                .expect("CST string-key index payload");
            let legacy_index = statements[3]
                .let_initializer_expression_payload()
                .expect("legacy numeric index fallback");
            let mismatched_index = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_index
                    .syntax_expression()
                    .expect("CST index syntax")
                    .clone(),
                legacy_index.fallback(),
            );

            compiler
                .compile_expr_with_payload(mismatched_index.fallback(), Some(&mismatched_index))
                .expect("CST-backed string-key index read should compile");
            let key = compiler
                .code
                .instructions
                .iter()
                .rev()
                .find_map(|instruction| match instruction.kind {
                    UnlinkedInstructionKind::GetStringKeyIndex { key, .. } => Some(key),
                    _ => None,
                })
                .expect("string-key index read should be emitted");
            assert_eq!(
                compiler.code.constants[key.0],
                crate::Constant::String("alpha".to_owned())
            );
        },
    );
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
            let statements = payload.body.statement_payloads();
            compiler
                .compile_statement(statements[0].fallback())
                .expect("cst map should compile");
            compiler
                .compile_statement(statements[1].fallback())
                .expect("legacy map should compile");
            let cst_target = statements[2]
                .assignment_target_expression_payload()
                .expect("CST string-key assignment target payload");
            let legacy_statement = statements[3]
                .expression_payload()
                .expect("legacy numeric assignment expression");
            let legacy_target = statements[3]
                .assignment_target_expression_payload()
                .expect("legacy numeric assignment target fallback");
            let mismatched_target = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_target
                    .syntax_expression()
                    .expect("CST target syntax")
                    .clone(),
                legacy_target.fallback(),
            );

            compiler
                .compile_assignment_with_payloads(
                    legacy_statement.fallback(),
                    crate::compiler::assignments::AssignmentTargetSyntax::new(Some(
                        &mismatched_target,
                    )),
                    crate::compiler::assignments::AssignmentValueSyntax::new(
                        None,
                        None,
                        crate::compiler::assignments::AssignmentValuePayloads::new(
                            None, None, None, None,
                        ),
                    ),
                )
                .expect("CST-backed string-key index assignment should compile");
            let key = compiler
                .code
                .instructions
                .iter()
                .rev()
                .find_map(|instruction| match instruction.kind {
                    UnlinkedInstructionKind::SetStringKeyIndex { key, .. } => Some(key),
                    _ => None,
                })
                .expect("string-key index write should be emitted");
            assert_eq!(
                compiler.code.constants[key.0],
                crate::Constant::String("alpha".to_owned())
            );
        },
    );
}

#[test]
fn host_index_validation_prefers_cst_receiver_payloads() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    registry
        .register_type(
            vela_registry::TypeDef::new(vela_def::DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "CstMap",
            ))
            .host_runtime_id(77),
        )
        .expect("CstMap host type should register");
    registry
        .register_type(
            vela_registry::TypeDef::new(vela_def::DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "LegacyMap",
            ))
            .host_runtime_id(78),
        )
        .expect("LegacyMap host type should register");

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(cst: CstMap, legacy: LegacyMap) {
    let cst_value = cst[1];
    let legacy_value = legacy[false];
}
"#,
    )
    .expect("semantic source should parse");
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
        options: CompilerOptions::new()
            .with_host_index_capability(
                "CstMap",
                crate::compiler::options::HostIndexCapabilityInfo {
                    readable: true,
                    writable: true,
                    addable: true,
                    removable: true,
                    key_type: Some("i64".to_owned()),
                    value_type: Some("i64".to_owned()),
                },
            )
            .with_host_index_capability(
                "LegacyMap",
                crate::compiler::options::HostIndexCapabilityInfo {
                    readable: true,
                    writable: true,
                    addable: true,
                    removable: true,
                    key_type: Some("bool".to_owned()),
                    value_type: Some("i64".to_owned()),
                },
            ),
        registry: Some(registry.compile_view()),
    };
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let statements = payload.body.statement_payloads();
    let cst_index = statements[0]
        .let_initializer_expression_payload()
        .expect("CST index initializer");
    let legacy_index = statements[1]
        .let_initializer_expression_payload()
        .expect("legacy index initializer");
    let mismatched_index = body_payloads::CompilerExpressionPayload::syntax(
        source,
        cst_index
            .syntax_expression()
            .expect("CST index syntax")
            .clone(),
        legacy_index.fallback(),
    );
    let (base_payload, index_payload) = mismatched_index
        .index_operand_payloads()
        .expect("mismatched index payloads");
    let ExprKind::Index { base, index } = &mismatched_index.fallback().kind else {
        panic!("expected legacy index fallback");
    };
    let mut compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    compiler
        .reject_invalid_host_index_read_with_payload(
            mismatched_index.fallback(),
            base,
            index,
            Some(&base_payload),
            Some(&index_payload),
        )
        .expect("CST receiver payload should select CstMap key contract");
    compiler
        .compile_expr_with_payload(mismatched_index.fallback(), Some(&mismatched_index))
        .expect("CST-backed host index read should compile");
    let target = compiler
        .code
        .instructions
        .iter()
        .rev()
        .find_map(|instruction| match instruction.kind {
            UnlinkedInstructionKind::HostRead { target, .. } => Some(target),
            _ => None,
        })
        .expect("host index read should be emitted");
    let plan = compiler
        .code
        .host_target(target)
        .expect("host index read target should exist");
    assert_eq!(plan.root_type, HostTypeId::new(77));
    assert_eq!(
        plan.parts.as_slice(),
        [vela_host::target::HostPathPart::DynIndex { arg: 0 }]
    );
}

#[test]
fn read_only_host_assignment_prefers_cst_target_payloads() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let readonly = registry
        .register_type(
            vela_registry::TypeDef::new(vela_def::DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "ReadOnlyHost",
            ))
            .host_runtime_id(77),
        )
        .expect("ReadOnlyHost host type should register");
    let writable = registry
        .register_type(
            vela_registry::TypeDef::new(vela_def::DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "WritableHost",
            ))
            .host_runtime_id(78),
        )
        .expect("WritableHost host type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                vela_def::DefPath::field(
                    "host",
                    std::iter::empty::<&str>(),
                    "ReadOnlyHost",
                    "amount",
                ),
                readonly,
            )
            .host_runtime_id(vela_def::FieldId::new(3).get())
            .writable(false),
        )
        .expect("ReadOnlyHost amount field should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                vela_def::DefPath::field(
                    "host",
                    std::iter::empty::<&str>(),
                    "WritableHost",
                    "amount",
                ),
                writable,
            )
            .host_runtime_id(vela_def::FieldId::new(4).get())
            .writable(true),
        )
        .expect("WritableHost amount field should register");

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(readonly: ReadOnlyHost, writable: WritableHost) {
    readonly.amount = 1;
    writable.amount = 2;
}
"#,
    )
    .expect("semantic source should parse");
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
        registry: Some(registry.compile_view()),
    };
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let statements = payload.body.statement_payloads();
    let readonly_target = statements[0]
        .assignment_target_expression_payload()
        .expect("CST read-only assignment target");
    let writable_target = statements[1]
        .assignment_target_expression_payload()
        .expect("legacy writable assignment target");
    let writable_statement = statements[1]
        .expression_payload()
        .expect("legacy writable assignment expression");
    let mismatched_target = body_payloads::CompilerExpressionPayload::syntax(
        source,
        readonly_target
            .syntax_expression()
            .expect("CST read-only target syntax")
            .clone(),
        writable_target.fallback(),
    );
    let mut compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    let error = compiler
        .compile_assignment_with_payloads(
            writable_statement.fallback(),
            crate::compiler::assignments::AssignmentTargetSyntax::new(Some(&mismatched_target)),
            crate::compiler::assignments::AssignmentValueSyntax::new(
                None,
                None,
                crate::compiler::assignments::AssignmentValuePayloads::new(None, None, None, None),
            ),
        )
        .expect_err("CST read-only assignment target should be rejected");
    assert_eq!(
        semantic_diagnostic_codes(error),
        ["analysis::field_not_writable"]
    );
}

fn assert_cst_let_initializer_field_base_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(field_base_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_let_initializer_index_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(index_block_operand_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_assignment_value_field_base_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .flat_map(field_base_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_assignment_value_index_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .flat_map(index_block_operand_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_assignment_target_field_base_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_target_expression_payload())
        .flat_map(field_base_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_assignment_target_index_operand_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_target_expression_payload())
        .flat_map(index_block_operand_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_let_initializer_field_names(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&str],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .filter_map(|payload| payload.syntax_field_name())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_strings(expected));
}

fn assert_cst_assignment_value_field_names(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&str],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .filter_map(|payload| payload.syntax_field_name())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_strings(expected));
}

fn assert_cst_assignment_target_field_names(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&str],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_target_expression_payload())
        .filter_map(|payload| payload.syntax_field_name())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_strings(expected));
}

fn expected_strings(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|name| (*name).to_owned()).collect()
}

fn field_base_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    payload
        .field_base_payload()
        .map(nested_block_payloads)
        .unwrap_or_default()
}

fn index_block_operand_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    let Some(field_base) = payload.field_base_payload() else {
        return Vec::new();
    };
    let Some((base, index)) = field_base.index_operand_payloads() else {
        return Vec::new();
    };
    [base, index]
        .into_iter()
        .flat_map(index_operand_block_payloads)
        .collect()
}

fn index_operand_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    nested_block_payloads(payload)
}

fn nested_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    if let Some(body) = payload.block_body_payload() {
        return vec![cst_statement_texts(&body)];
    }
    if let Some((base, index)) = payload.index_operand_payloads() {
        return [base, index]
            .into_iter()
            .flat_map(nested_block_payloads)
            .collect();
    }
    if let Some(base) = payload.field_base_payload() {
        return nested_block_payloads(base);
    }
    payload
        .call_argument_payloads()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|argument| {
            let value = argument.value_expression_payload();
            let body = value.block_body_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect()
}
