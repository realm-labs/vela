use super::*;

#[test]
fn typed_value_method_argument_without_child_payload_does_not_use_legacy_value() {
    with_cst_payload_compiler(
        r#"
fn main() {
    let values: Array<i64> = [];
    values.push({
        let value = 1;
        value
    });
}
"#,
        |compiler, payload| {
            let statements = paired_statement_payloads_for_body(
                payload.body.syntax_payload().source,
                &payload.body,
            );
            compiler
                .compile_statement_payload_for_test(&statements[0])
                .expect("typed array local should compile");
            let call = statements[1]
                .expression_payload()
                .expect("value method call payload");
            let ExprKind::Call { callee, args } = &call.fallback().kind else {
                panic!("expected value method call");
            };
            let callee_payload = call.call_callee_payload();

            let error = compiler
                .compile_call_expr_with_arg_payloads(
                    call.fallback(),
                    callee,
                    args,
                    callee_payload.as_ref(),
                    Some(&[]),
                )
                .expect_err("missing typed method argument payload must not use legacy value");

            assert!(matches!(
                error.kind,
                CompileErrorKind::UnsupportedSyntax("missing CST call argument value")
            ));
        },
    );
}

#[test]
fn typed_native_argument_without_child_payload_does_not_use_legacy_value() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    registry
        .register_function(vela_registry::FunctionDef::new(
            vela_def::DefPath::function("host", std::iter::empty::<&str>(), "native_take"),
            vela_registry::FunctionSignature::new(
                [vela_registry::ParamDef::new("value", Some("i64"))],
                None::<vela_registry::TypeHintDef>,
            ),
        ))
        .expect("native function should register");
    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main() {
    native_take({
        let value = 1;
        value
    });
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
    let mut compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");
    let call = paired_statement_payloads_for_body(source, &payload.body)[0]
        .expression_payload()
        .expect("native call payload");
    let ExprKind::Call { callee, args } = &call.fallback().kind else {
        panic!("expected native call");
    };
    let callee_payload = call.call_callee_payload();

    let error = compiler
        .compile_call_expr_with_arg_payloads(
            call.fallback(),
            callee,
            args,
            callee_payload.as_ref(),
            Some(&[]),
        )
        .expect_err("missing typed native argument payload must not use legacy value");

    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("missing CST call argument value")
    ));
}
