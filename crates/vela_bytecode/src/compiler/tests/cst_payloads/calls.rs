use super::*;

#[test]
fn semantic_function_value_call_arguments_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn take(value) {
    return value;
}

fn take_typed(value: i64) {
    return value;
}

fn outer(value) {
    return value;
}

enum Boxed {
    Value(value)
}

fn call_values() {
    let result = take({
        let initial = 1;
        initial
    });
    let boxed = Boxed::Value({
        let enum_value = 5;
        enum_value
    });
    result = take({
        let assigned = 2;
        assigned
    });
    outer(take({
        let nested = 3;
        nested
    }));
    let named = take_typed(value = {
        let named_value = 8;
        named_value
    });
    outer(take_typed({
        let typed = 6;
        typed
    }));
    return take({
        let returned = 4;
        returned
    });
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("call_values")
        .expect("call_values function");

    assert_cst_let_initializer_call_argument_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let initial = 1;"),
                (SyntaxStatementKind::Expr, "initial"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let enum_value = 5;"),
                (SyntaxStatementKind::Expr, "enum_value"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let named_value = 8;"),
                (SyntaxStatementKind::Expr, "named_value"),
            ],
        ],
    );
    assert_cst_assignment_value_call_argument_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let assigned = 2;"),
            (SyntaxStatementKind::Expr, "assigned"),
        ]],
    );
    assert_cst_nested_call_argument_body_payloads(
        &payload.body,
        &[
            vec![
                (SyntaxStatementKind::Let, "let nested = 3;"),
                (SyntaxStatementKind::Expr, "nested"),
            ],
            vec![
                (SyntaxStatementKind::Let, "let typed = 6;"),
                (SyntaxStatementKind::Expr, "typed"),
            ],
        ],
    );
    assert_cst_return_value_call_argument_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let returned = 4;"),
            (SyntaxStatementKind::Expr, "returned"),
        ]],
    );
    assert_cst_call_argument_names(&payload.body, &["value"]);
    assert_cst_let_initializer_call_callee_path_segments(
        &payload.body,
        &[&["take"], &["Boxed", "Value"], &["take_typed"]],
    );

    compile_program_source(source, text).expect("CST-backed value call arguments should compile");
}

#[test]
fn equal_count_call_payloads_pair_arguments_by_position_not_legacy_span() {
    with_cst_payload_compiler(
        r#"
fn take(value) {
    return value;
}

fn main() {
    let cst_call = take(value = true);
    let legacy_call = take(value = 1);
}
"#,
        |_, payload| {
            let statements = payload.body.statement_payloads();
            let cst_call = statements[0]
                .let_initializer_expression_payload()
                .expect("CST call payload");
            let legacy_call = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy call fallback");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_call
                    .syntax_expression()
                    .expect("CST expression")
                    .clone(),
                legacy_call.fallback(),
            );

            let args = mismatched_payload
                .call_argument_payloads()
                .expect("call argument payloads");
            assert_eq!(args.len(), 1);
            assert_eq!(args[0].syntax_name().as_deref(), Some("value"));
            assert_eq!(
                args[0]
                    .value_expression_payload()
                    .syntax_expression()
                    .expect("CST argument value")
                    .syntax()
                    .text()
                    .to_string(),
                "true"
            );

            let ExprKind::Call {
                args: legacy_args, ..
            } = &legacy_call.fallback().kind
            else {
                panic!("expected legacy call fallback");
            };
            let arg_syntax = call_args::CallArgumentSyntax::new(legacy_args, Some(&args));
            assert_eq!(
                arg_syntax.name_for(&legacy_args[0]),
                Some("value".to_owned()),
                "argument names must come from the CST argument payload"
            );
        },
    );
}

#[test]
fn missing_call_argument_value_payload_does_not_use_legacy_value() {
    with_cst_payload_compiler(
        r#"
fn take(value) {
    return value;
}

fn main() {
    let legacy_call = take([1]);
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let legacy_call = statements[0]
                .let_initializer_expression_payload()
                .expect("legacy call payload");
            let ExprKind::Call {
                args: legacy_args, ..
            } = &legacy_call.fallback().kind
            else {
                panic!("expected legacy call fallback");
            };
            let missing_value_arg = body_payloads::CompilerArgumentPayload::missing_value_syntax(
                SourceId::new(1),
                &legacy_args[0],
            );
            let argument_payloads = [missing_value_arg];
            let arg_syntax =
                call_args::CallArgumentSyntax::new(legacy_args, Some(&argument_payloads));

            let error = compiler
                .compile_call_argument_value(&legacy_args[0], arg_syntax)
                .expect_err("unmatched CST argument value must not compile legacy argument");

            assert!(
                matches!(
                    error.kind,
                    CompileErrorKind::UnsupportedSyntax("missing CST call argument value")
                ),
                "expected missing CST call argument value, got {error:?}"
            );
        },
    );
}

#[test]
fn missing_call_argument_child_payload_does_not_use_legacy_value() {
    with_cst_payload_compiler(
        r#"
fn take(value) {
    return value;
}

fn main() {
    let legacy_call = take([1]);
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let legacy_call = statements[0]
                .let_initializer_expression_payload()
                .expect("legacy call payload");
            let ExprKind::Call {
                args: legacy_args, ..
            } = &legacy_call.fallback().kind
            else {
                panic!("expected legacy call fallback");
            };
            let argument_payloads = [];
            let arg_syntax =
                call_args::CallArgumentSyntax::new(legacy_args, Some(&argument_payloads));

            let error = compiler
                .compile_call_argument_value(&legacy_args[0], arg_syntax)
                .expect_err("missing CST argument child payload must not compile legacy value");

            assert!(
                matches!(
                    error.kind,
                    CompileErrorKind::UnsupportedSyntax("missing CST call argument value")
                ),
                "expected missing CST call argument value, got {error:?}"
            );
        },
    );
}

#[test]
fn missing_call_callee_payload_does_not_use_legacy_callee() {
    let source = SourceId::new(1);
    let text = r#"
fn make() {
    return 1;
}

fn main() {
    let value = make();
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_call = legacy_payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("legacy call payload");
    let missing_source_call =
        body_payloads::CompilerExpressionPayload::missing_child_payload_context(
            legacy_call
                .syntax_expression()
                .expect("call syntax expression")
                .clone(),
            legacy_call.fallback(),
        );
    assert_eq!(missing_source_call.syntax_call_callee_path_segments(), None);

    let ExprKind::Call { callee, args } = &legacy_call.fallback().kind else {
        panic!("expected legacy call fallback");
    };
    let missing_callee = body_payloads::CompilerExpressionPayload::missing_syntax(source, callee);

    let error = compiler
        .compile_call_expr_with_arg_payloads(
            legacy_call.fallback(),
            callee,
            args,
            Some(&missing_callee),
            None,
        )
        .expect_err("missing CST call callee must not compile legacy callee");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST call callee")
    ));
}

#[test]
fn missing_call_expression_payload_does_not_use_legacy_call() {
    let source = SourceId::new(1);
    let text = r#"
fn make() {
    return 1;
}

fn main() {
    let value = make();
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (mut compiler, legacy_payload) = cst_payload_compiler_for_function(&semantic, "main");
    let legacy_call = legacy_payload.body.statement_payloads()[0]
        .let_initializer_expression_payload()
        .expect("legacy call payload");
    let missing_call =
        body_payloads::CompilerExpressionPayload::missing_syntax(source, legacy_call.fallback());

    let error = compiler
        .compile_expr_with_payload(legacy_call.fallback(), Some(&missing_call))
        .expect_err("missing CST call payload must not compile legacy call");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST expression payload")
    ));
}

#[test]
fn path_call_with_non_path_cst_callee_does_not_use_legacy_callable_name() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let native = |value| value;
    let cst_call = ({
        let selected = native;
        selected
    })(1);
    let legacy_call = external_native(1);
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let cst_call = statements[1]
                .let_initializer_expression_payload()
                .expect("CST call payload");
            let legacy_call = statements[2]
                .let_initializer_expression_payload()
                .expect("legacy path call fallback");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_call
                    .syntax_expression()
                    .expect("CST expression")
                    .clone(),
                legacy_call.fallback(),
            );

            let error = compiler
                .compile_expr_with_payload(mismatched_payload.fallback(), Some(&mismatched_payload))
                .expect_err("mismatched non-path CST callee must not use the legacy callable name");

            assert!(
                matches!(
                    error.kind,
                    CompileErrorKind::UnsupportedSyntax("mismatched CST call callee payload")
                ),
                "expected mismatched CST call callee payload, got {error:?}"
            );
        },
    );
}

#[test]
fn script_path_call_with_non_path_cst_callee_does_not_use_legacy_function() {
    with_cst_payload_compiler(
        r#"
fn external_script(value) {
    return value;
}

fn main() {
    let callable = |value| value;
    let cst_call = ({
        let selected = callable;
        selected
    })(1);
    let legacy_call = external_script(1);
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let cst_call = statements[1]
                .let_initializer_expression_payload()
                .expect("CST call payload");
            let legacy_call = statements[2]
                .let_initializer_expression_payload()
                .expect("legacy script call fallback");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_call
                    .syntax_expression()
                    .expect("CST expression")
                    .clone(),
                legacy_call.fallback(),
            );

            let error = compiler
                .compile_expr_with_payload(mismatched_payload.fallback(), Some(&mismatched_payload))
                .expect_err(
                    "mismatched non-path CST callee must not use the legacy script function",
                );

            assert!(
                matches!(
                    error.kind,
                    CompileErrorKind::UnsupportedSyntax("mismatched CST call callee payload")
                ),
                "expected mismatched CST call callee payload, got {error:?}"
            );
        },
    );
}

#[test]
fn path_call_with_misaligned_path_cst_callee_does_not_use_wrong_function() {
    with_cst_payload_compiler(
        r#"
fn first(value) {
    return value;
}

fn second(value) {
    return value;
}

fn main() {
    let cst_call = first(1);
    let legacy_call = second(2);
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let cst_call = statements[0]
                .let_initializer_expression_payload()
                .expect("CST call payload");
            let legacy_call = statements[1]
                .let_initializer_expression_payload()
                .expect("legacy path call fallback");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_call
                    .syntax_expression()
                    .expect("CST expression")
                    .clone(),
                legacy_call.fallback(),
            );

            let error = compiler
                .compile_expr_with_payload(mismatched_payload.fallback(), Some(&mismatched_payload))
                .expect_err("misaligned path CST callee must not select the wrong function");

            assert!(
                matches!(
                    error.kind,
                    CompileErrorKind::UnsupportedSyntax("mismatched CST call callee payload")
                ),
                "expected mismatched CST call callee payload, got {error:?}"
            );
        },
    );
}

#[test]
fn method_call_with_non_field_cst_callee_does_not_use_legacy_method_name() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let callable = |value| value;
    let cst_call = ({
        let selected = callable;
        selected
    })(1);
    let legacy_call = "ready".len();
}
"#,
        |compiler, payload| {
            let statements = payload.body.statement_payloads();
            let cst_call = statements[1]
                .let_initializer_expression_payload()
                .expect("CST call payload");
            let legacy_call = statements[2]
                .let_initializer_expression_payload()
                .expect("legacy method call fallback");
            let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
                SourceId::new(1),
                cst_call
                    .syntax_expression()
                    .expect("CST expression")
                    .clone(),
                legacy_call.fallback(),
            );

            let error = compiler
                .compile_expr_with_payload(mismatched_payload.fallback(), Some(&mismatched_payload))
                .expect_err("mismatched non-field CST callee must not compile");

            assert!(
                matches!(
                    error.kind,
                    CompileErrorKind::UnsupportedSyntax("mismatched CST call callee payload")
                ),
                "expected mismatched CST call callee payload, got {error:?}"
            );
        },
    );
}

#[test]
fn host_path_push_with_non_field_cst_callee_does_not_use_legacy_method_name() {
    let inventory = FieldId::new(3);
    let rewards = FieldId::new(4);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), "Player"))
                .host_runtime_id(77),
        )
        .expect("Player host type should register");
    let inventory_type = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "Inventory",
            ))
            .host_runtime_id(78),
        )
        .expect("Inventory host type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "Player", "inventory"),
                player,
            )
            .host_runtime_id(inventory.get())
            .writable(true)
            .type_hint(Some("Inventory".to_owned())),
        )
        .expect("Player inventory field should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "Inventory", "rewards"),
                inventory_type,
            )
            .host_runtime_id(rewards.get())
            .writable(true),
        )
        .expect("Inventory rewards field should register");

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(player: Player) {
    let callable = |value| value;
    let cst_call = ({
        let selected = callable;
        selected
    })("silver");
    let legacy_call = player.inventory.rewards.push("gold");
}
"#,
    )
    .expect("semantic source should parse");
    let facts = cst_payload_compiler_facts_with_options(
        &semantic,
        CompilerOptions::default(),
        Some(registry.compile_view()),
    );
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let statements = payload.body.statement_payloads();
    let cst_call = statements[1]
        .let_initializer_expression_payload()
        .expect("CST call payload");
    let legacy_call = statements[2]
        .let_initializer_expression_payload()
        .expect("legacy host push call fallback");
    let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
        source,
        cst_call
            .syntax_expression()
            .expect("CST expression")
            .clone(),
        legacy_call.fallback(),
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
        .compile_expr_with_payload(mismatched_payload.fallback(), Some(&mismatched_payload))
        .expect_err("mismatched host push fallback must not compile");

    assert!(
        matches!(
            error.kind,
            CompileErrorKind::UnsupportedSyntax("mismatched CST call callee payload")
        ),
        "expected mismatched CST call callee payload, got {error:?}"
    );
}

#[test]
fn host_collection_method_targets_use_cst_receiver_roots() {
    fn host_type(
        registry: &mut vela_registry::DefinitionRegistry,
        name: &str,
        id: u32,
    ) -> vela_def::TypeId {
        registry
            .register_type(
                vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), name))
                    .host_runtime_id(id.into()),
            )
            .expect("host type should register")
    }

    fn host_field(
        registry: &mut vela_registry::DefinitionRegistry,
        owner: vela_def::TypeId,
        owner_name: &str,
        name: &str,
        id: FieldId,
        type_hint: Option<&str>,
    ) {
        let mut field = vela_registry::FieldDef::new(
            DefPath::field("host", std::iter::empty::<&str>(), owner_name, name),
            owner,
        )
        .host_runtime_id(id.get())
        .writable(true);
        if let Some(type_hint) = type_hint {
            field = field.type_hint(Some(type_hint.to_owned()));
        }
        registry
            .register_field(field)
            .expect("host field should register");
    }

    let cst_inventory = FieldId::new(3);
    let cst_items = FieldId::new(4);
    let legacy_inventory = FieldId::new(5);
    let legacy_items = FieldId::new(6);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let cst_player = host_type(&mut registry, "CstPlayer", 77);
    let cst_inventory_type = host_type(&mut registry, "CstInventory", 78);
    let legacy_player = host_type(&mut registry, "LegacyPlayer", 79);
    let legacy_inventory_type = host_type(&mut registry, "LegacyInventory", 80);
    host_field(
        &mut registry,
        cst_player,
        "CstPlayer",
        "inventory",
        cst_inventory,
        Some("CstInventory"),
    );
    host_field(
        &mut registry,
        cst_inventory_type,
        "CstInventory",
        "items",
        cst_items,
        Some("CstItems"),
    );
    host_field(
        &mut registry,
        legacy_player,
        "LegacyPlayer",
        "inventory",
        legacy_inventory,
        Some("LegacyInventory"),
    );
    host_field(
        &mut registry,
        legacy_inventory_type,
        "LegacyInventory",
        "items",
        legacy_items,
        Some("LegacyItems"),
    );

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(cst: CstPlayer, legacy: LegacyPlayer) {
    let cst_call = cst.inventory.items.remove();
    let legacy_call = legacy.inventory.items.remove();
}
"#,
    )
    .expect("semantic source should parse");
    let facts = cst_payload_compiler_facts_with_options(
        &semantic,
        CompilerOptions::default(),
        Some(registry.compile_view()),
    );
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let statements = payload.body.statement_payloads();
    let cst_call = statements[0]
        .let_initializer_expression_payload()
        .expect("CST remove call payload");
    let legacy_call = statements[1]
        .let_initializer_expression_payload()
        .expect("legacy remove call fallback");
    let mismatched_payload = body_payloads::CompilerExpressionPayload::syntax(
        source,
        cst_call
            .syntax_expression()
            .expect("CST expression")
            .clone(),
        legacy_call.fallback(),
    );
    let callee_payload = mismatched_payload
        .call_callee_payload()
        .expect("mismatched callee payload");
    let compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    assert_eq!(
        compiler.host_collection_method_target_root_name_for_test(
            callee_payload.fallback(),
            Some(&callee_payload),
            "remove",
        ),
        Some("cst".to_owned()),
        "collection host path root must come from the CST receiver payload"
    );
}

fn assert_cst_let_initializer_call_argument_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(call_argument_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_assignment_value_call_argument_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.assignment_value_expression_payload())
        .flat_map(call_argument_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_nested_call_argument_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .flat_map(|statement| statement.call_argument_payloads().unwrap_or_default())
        .map(|argument| argument.value_expression_payload())
        .flat_map(call_argument_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_return_value_call_argument_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.return_value_expression_payload())
        .flat_map(call_argument_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_call_argument_names(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&str],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .flat_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(|payload| payload.call_argument_payloads().unwrap_or_default())
        .filter_map(|argument| argument.syntax_name())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

fn assert_cst_let_initializer_call_callee_path_segments(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&[&str]],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .filter_map(|payload| payload.call_callee_payload())
        .filter_map(|callee| callee.syntax_path_segments())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_path_segments(expected));
}

fn call_argument_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
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

#[test]
fn semantic_function_call_callee_and_receiver_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn call_targets() {
    let callable = |value| value;
    let closure_result = ({
        let selected = callable;
        selected
    })({
        let value = 7;
        value
    });
    let receiver_result = ({
        let label = "ready";
        label
    }).len();
    return closure_result + receiver_result;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("call_targets")
        .expect("call_targets function");

    assert_cst_let_initializer_call_callee_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let selected = callable;"),
            (SyntaxStatementKind::Expr, "selected"),
        ]],
    );
    assert_cst_let_initializer_method_receiver_body_payloads(
        &payload.body,
        &[vec![
            (SyntaxStatementKind::Let, "let label = \"ready\";"),
            (SyntaxStatementKind::Expr, "label"),
        ]],
    );
    assert_cst_let_initializer_method_names(&payload.body, &["len"]);

    let program = compile_program_source(source, text)
        .expect("CST-backed call callees and method receivers should compile");
    let function = program
        .function("call_targets")
        .expect("call_targets bytecode");
    assert!(
        function.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::CallClosure { .. }
        )),
        "CST-backed non-path callee should lower as a closure call"
    );
}

#[test]
fn callback_expression_lambda_method_callee_has_cst_payload() {
    let source = SourceId::new(1);
    let text = r#"
fn callback_method() {
    option::some("quest").filter(|value| value.starts_with("Q"));
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("callback_method")
        .expect("callback_method function");

    let actual = payload
        .body
        .statement_payloads()
        .iter()
        .flat_map(|statement| statement.call_argument_payloads().unwrap_or_default())
        .map(|argument| argument.value_expression_payload())
        .filter_map(|lambda| {
            let body = lambda.fallback_lambda_body()?;
            lambda.lambda_body_payload(body)
        })
        .filter_map(|body| body.call_callee_payload())
        .filter_map(|callee| callee.syntax_field_name())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_strings(&["starts_with"]));
}

#[test]
fn missing_callback_lambda_body_payload_does_not_use_legacy_body() {
    let source = SourceId::new(1);
    let cst_text = r#"
fn main() {
    option::some("quest").filter(|value|);
}
"#;
    let legacy_text = r#"
fn main() {
    option::some("quest").filter(|value| value.starts_with("q"));
}
"#;
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, cst_text);
    let cst_arg = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST body")
        .statements()
        .next()
        .expect("CST statement")
        .as_expr()
        .expect("CST expr statement")
        .expression()
        .expect("CST expression")
        .as_call()
        .expect("CST call")
        .arguments()
        .into_iter()
        .next()
        .expect("CST callback argument");
    assert!(
        cst_arg
            .expression()
            .expect("CST lambda argument")
            .as_lambda()
            .expect("CST lambda")
            .body()
            .is_none(),
        "recovered CST callback lambda should not expose a body"
    );

    let semantic = parse_semantic_source(source, legacy_text).expect("legacy source should parse");
    let (payload, _, _) = semantic.function("main").expect("main function");
    let legacy_call = payload.body.statement_payloads()[0]
        .expression_payload()
        .expect("legacy expression payload");
    let ExprKind::Call { callee, args } = &legacy_call.fallback().kind else {
        panic!("expected callback method call");
    };
    let arg_payload = body_payloads::CompilerArgumentPayload::syntax(source, cst_arg, &args[0]);
    let (mut compiler, _) = cst_payload_compiler_for_function(&semantic, "main");

    let error = compiler
        .compile_call_expr_with_arg_payloads(
            legacy_call.fallback(),
            callee,
            args,
            legacy_call.call_callee_payload().as_ref(),
            Some(&[arg_payload]),
        )
        .expect_err("missing CST callback lambda body must not compile legacy body");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST lambda body")
    ));
}

#[test]
fn chained_callback_method_callees_have_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn callback_chain() {
    let option_chain = option::some("quest")
        .map(|value| value.to_upper())
        .filter(|value| value.starts_with("Q"));
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic
        .function("callback_chain")
        .expect("callback_chain function");
    let initializer = payload
        .body
        .statement_payloads()
        .into_iter()
        .find_map(|statement| statement.let_initializer_expression_payload())
        .expect("let initializer payload");
    assert_eq!(
        initializer.kind(),
        Some(SyntaxExpressionKind::Call),
        "initializer syntax: {:?}",
        initializer
            .syntax_expression()
            .map(|expression| expression.syntax().text().to_string())
    );
    let callee = initializer.call_callee_payload().expect("callee payload");
    assert_eq!(
        callee.kind(),
        Some(SyntaxExpressionKind::Field),
        "callee syntax: {:?}",
        callee
            .syntax_expression()
            .map(|expression| expression.syntax().text().to_string())
    );

    let actual = chained_call_callee_names(initializer);
    assert_eq!(
        actual,
        expected_strings(&["filter", "map", "starts_with", "to_upper"])
    );
}

fn assert_cst_let_initializer_call_callee_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(call_callee_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_let_initializer_method_receiver_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .flat_map(method_receiver_block_payloads)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_let_initializer_method_names(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[&str],
) {
    let actual = body
        .statement_payloads()
        .iter()
        .filter_map(|statement| statement.let_initializer_expression_payload())
        .filter_map(call_method_name)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_strings(expected));
}

fn call_method_name(payload: body_payloads::CompilerExpressionPayload<'_>) -> Option<String> {
    payload.call_callee_payload()?.syntax_field_name()
}

fn chained_call_callee_names(payload: body_payloads::CompilerExpressionPayload<'_>) -> Vec<String> {
    let mut names = Vec::new();
    collect_chained_call_callee_names(payload, &mut names);
    names.sort();
    names
}

fn collect_chained_call_callee_names(
    payload: body_payloads::CompilerExpressionPayload<'_>,
    names: &mut Vec<String>,
) {
    if let Some(callee) = payload.call_callee_payload() {
        if let Some(name) = callee.syntax_field_name() {
            names.push(name);
        }
        collect_chained_call_callee_names(callee, names);
    }
    for argument in payload.call_argument_payloads().unwrap_or_default() {
        let value = argument.value_expression_payload();
        collect_chained_call_callee_names(value.clone(), names);
        let lambda_body = value
            .fallback_lambda_body()
            .and_then(|body| value.lambda_body_payload(body));
        if let Some(lambda_body) = lambda_body {
            collect_chained_call_callee_names(lambda_body, names);
        }
    }
    if let Some(base) = payload.field_base_payload() {
        collect_chained_call_callee_names(base, names);
    }
}

fn expected_strings(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|name| (*name).to_owned()).collect()
}

fn expected_path_segments(expected: &[&[&str]]) -> Vec<Vec<String>> {
    expected
        .iter()
        .map(|path| path.iter().map(|segment| (*segment).to_owned()).collect())
        .collect()
}

fn call_callee_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    payload
        .call_callee_payload()
        .into_iter()
        .flat_map(nested_call_target_block_payloads)
        .collect()
}

fn method_receiver_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    payload
        .call_callee_payload()
        .and_then(|callee| callee.field_base_payload())
        .into_iter()
        .flat_map(nested_call_target_block_payloads)
        .collect()
}

fn nested_call_target_block_payloads(
    payload: body_payloads::CompilerExpressionPayload<'_>,
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    if let Some(body) = payload.block_body_payload() {
        return vec![cst_statement_texts(&body)];
    }
    if let Some(inner) = payload.paren_inner_payload() {
        return nested_call_target_block_payloads(inner);
    }
    Vec::new()
}
