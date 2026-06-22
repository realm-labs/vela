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
fn mismatched_call_payloads_do_not_pair_arguments_by_index() {
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
            assert!(
                args[0].syntax_argument().is_none(),
                "mismatched spans must not receive index-based CST arguments"
            );
            assert!(args[0].syntax_name().is_none());
            assert!(
                args[0]
                    .value_expression_payload()
                    .syntax_expression()
                    .is_none()
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
                None,
                "mismatched argument payloads must not expose legacy argument names"
            );
        },
    );
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
                    CompileErrorKind::UnsupportedSyntax("callable expression")
                ),
                "expected unsupported callable expression, got {error:?}"
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
                    CompileErrorKind::UnsupportedSyntax("callable expression")
                ),
                "expected unsupported callable expression, got {error:?}"
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

            compiler
                .compile_expr_with_payload(mismatched_payload.fallback(), Some(&mismatched_payload))
                .expect("mismatched method fallback should compile as a callable expression");

            assert!(
                compiler
                    .code
                    .instructions
                    .iter()
                    .all(|instruction| !matches!(
                        &instruction.kind,
                        UnlinkedInstructionKind::CallDynamicMethod { method, .. }
                            | UnlinkedInstructionKind::CallMethodId { method, .. }
                            if method == "len"
                    )),
                "mismatched non-field CST callee must not use the legacy method name"
            );
            assert!(
                compiler
                    .code
                    .instructions
                    .iter()
                    .any(|instruction| matches!(
                        &instruction.kind,
                        UnlinkedInstructionKind::CallClosure { .. }
                    )),
                "mismatched non-field CST callee should fall through to callable expression lowering"
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

    compiler
        .compile_expr_with_payload(mismatched_payload.fallback(), Some(&mismatched_payload))
        .expect("mismatched host push fallback should compile as a callable expression");

    assert!(
        compiler
            .code
            .instructions
            .iter()
            .all(|instruction| !matches!(
                &instruction.kind,
                UnlinkedInstructionKind::HostMutate {
                    op: vela_host::resolved::HostMutationOp::Push,
                    ..
                }
            )),
        "mismatched non-field CST callee must not use the legacy host push name"
    );
    assert!(
        compiler
            .code
            .instructions
            .iter()
            .any(|instruction| matches!(
                &instruction.kind,
                UnlinkedInstructionKind::CallClosure { .. }
            )),
        "mismatched non-field CST callee should fall through to callable expression lowering"
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

    compile_program_source(source, text)
        .expect("CST-backed call callees and method receivers should compile");
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
        .filter_map(|lambda| lambda.lambda_body_payload())
        .filter_map(|body| body.call_callee_payload())
        .filter_map(|callee| callee.syntax_field_name())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_strings(&["starts_with"]));
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
        if let Some(lambda_body) = value.lambda_body_payload() {
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
