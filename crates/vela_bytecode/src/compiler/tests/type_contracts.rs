use super::*;

fn with_static_type_compiler(
    source: &str,
    inspect: impl for<'ast> FnOnce(&mut Compiler<'ast, 'static>, &'ast FunctionItem),
) {
    let semantic =
        parse_semantic_source(SourceId::new(1), source).expect("semantic source should parse");
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
        script_field_slots,
        schema_defaults,
        type_symbols,
        global_symbols,
        global_slots,
        global_type_symbols,
        const_values,
        options: CompilerOptions::default(),
        registry: None,
    };
    let (function, signature, bindings) = semantic.function("main").expect("main function");
    let mut compiler = Compiler::new(function.name.clone(), function, signature, bindings, facts)
        .expect("compiler should initialize");
    inspect(&mut compiler, function);
}

fn let_initializer(function: &FunctionItem, index: usize) -> &Expr {
    let statement = function
        .body
        .statements
        .get(index)
        .expect("statement should exist");
    let vela_syntax::ast::StmtKind::Let {
        value: Some(value), ..
    } = &statement.kind
    else {
        panic!("expected let initializer");
    };
    value
}

fn return_value(function: &FunctionItem) -> &Expr {
    let statement = function
        .body
        .statements
        .last()
        .expect("return statement should exist");
    let vela_syntax::ast::StmtKind::Return(Some(value)) = &statement.kind else {
        panic!("expected return value");
    };
    value
}

fn return_call_arg(function: &FunctionItem, index: usize) -> &Expr {
    let value = return_value(function);
    let vela_syntax::ast::ExprKind::Call { args, .. } = &value.kind else {
        panic!("expected return call expression");
    };
    &args.get(index).expect("call argument should exist").value
}

#[test]
fn compiler_classifies_literals_without_defaulting_unsuffixed_numbers() {
    with_static_type_compiler(
        r#"
fn main() {
    let integer = 12;
    let suffixed_integer = 12i8;
    let float = 12.0;
    let suffixed_float = 12.0f32;
    let text = "reward";
    let data = b"reward";
    return null;
}
"#,
        |compiler, function| {
            assert_eq!(
                compiler.static_type_for_expr(let_initializer(function, 0)),
                value_types::StaticExprType::UnsuffixedIntegerLiteral
            );
            assert_eq!(
                compiler.static_type_for_expr(let_initializer(function, 1)),
                value_types::StaticExprType::Exact(RuntimeTypeFact::primitive(
                    vela_common::PrimitiveTag::I8
                ))
            );
            assert_eq!(
                compiler.static_type_for_expr(let_initializer(function, 2)),
                value_types::StaticExprType::UnsuffixedFloatLiteral
            );
            assert_eq!(
                compiler.static_type_for_expr(let_initializer(function, 3)),
                value_types::StaticExprType::Exact(RuntimeTypeFact::primitive(
                    vela_common::PrimitiveTag::F32
                ))
            );
            assert_eq!(
                compiler.static_type_for_expr(let_initializer(function, 4)),
                value_types::StaticExprType::Exact(RuntimeTypeFact::primitive(
                    vela_common::PrimitiveTag::String
                ))
            );
            assert_eq!(
                compiler.static_type_for_expr(let_initializer(function, 5)),
                value_types::StaticExprType::Exact(RuntimeTypeFact::primitive(
                    vela_common::PrimitiveTag::Bytes
                ))
            );
        },
    );
}

#[test]
fn compiler_classifies_dynamic_hinted_and_local_value_facts() {
    with_static_type_compiler(
        r#"
fn main(dynamic, exact: i64) {
    let erased = dynamic;
    let copied = exact;
    let hinted: u32 = 1;
    let use_hinted = hinted;
    return copied;
}
"#,
        |compiler, function| {
            assert_eq!(
                compiler.static_type_for_expr(let_initializer(function, 0)),
                value_types::StaticExprType::Dynamic
            );
            assert_eq!(
                compiler.static_type_for_expr(let_initializer(function, 1)),
                value_types::StaticExprType::Exact(RuntimeTypeFact::primitive(
                    vela_common::PrimitiveTag::I64
                ))
            );

            for statement in function.body.statements.iter().take(3) {
                compiler
                    .compile_statement(statement)
                    .expect("let statement should compile");
            }

            assert_eq!(
                compiler.static_type_for_expr(let_initializer(function, 3)),
                value_types::StaticExprType::Exact(RuntimeTypeFact::primitive(
                    vela_common::PrimitiveTag::U32
                ))
            );
            assert_eq!(
                compiler.static_type_for_expr(return_value(function)),
                value_types::StaticExprType::Exact(RuntimeTypeFact::primitive(
                    vela_common::PrimitiveTag::I64
                ))
            );
        },
    );
}

#[test]
fn compiler_contextualizes_unsuffixed_script_call_arguments() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn f(x: u8) {
    return x;
}
fn main() {
    return f(12);
}
"#,
    )
    .expect("unsuffixed integer literal should be contextualized by parameter type");
    let main = program.function("main").expect("main function");

    assert!(
        main.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::U8(12)))
    );
    assert!(main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::CallFunction { args, .. }
            if args.len() == 1 && matches!(args[0], CallArgument::Register(_))
    )));
}

#[test]
fn compiler_rejects_static_script_call_contract_mismatches() {
    for source in [
        r#"
fn f(x: i64) {}
fn main() {
    f(12i8);
}
"#,
        r#"
fn f(x: i64) {}
fn main() {
    f(12.0);
}
"#,
        r#"
fn f(x: i64) {}
fn main() {
    f("12");
}
"#,
    ] {
        let error = compile_program_source(SourceId::new(1), source)
            .expect_err("static type contract mismatch should fail before bytecode emission");
        assert_eq!(
            semantic_diagnostic_codes(error),
            ["compiler::type_contract_mismatch"]
        );
    }
}

#[test]
fn compiler_reports_type_contract_mismatch_details() {
    let error = compile_function_source(
        SourceId::new(1),
        r##"
fn main() {
    let amount: i64 = "12";
    return amount;
}
"##,
        "main",
    )
    .expect_err("typed let static mismatch should fail before bytecode emission");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics");
    };
    assert_eq!(diagnostics.len(), 1);
    let diagnostic = &diagnostics[0];

    assert_eq!(
        diagnostic.code.as_deref(),
        Some("compiler::type_contract_mismatch")
    );
    assert!(
        diagnostic
            .message
            .contains("type contract mismatch for let binding `amount`"),
        "{diagnostic:?}"
    );
    assert!(diagnostic.span.is_some(), "{diagnostic:?}");
    assert!(diagnostic.labels.iter().any(|label| {
        label.message.contains("expected `i64`") && label.message.contains("found `string`")
    }));
}

#[test]
fn compiler_marks_dynamic_script_call_contracts_for_runtime_guards() {
    with_static_type_compiler(
        r#"
fn f(x: i64) {
    return x;
}
fn main(value) {
    return f(value);
}
"#,
        |compiler, function| {
            let expected = RuntimeTypeFact::primitive(vela_common::PrimitiveTag::I64);
            assert_eq!(
                compiler
                    .expected_type_for_expr(
                        return_call_arg(function, 0),
                        expected.clone(),
                        value_types::TypeContractContext::FunctionParameter {
                            name: "x".to_owned()
                        },
                    )
                    .expect("dynamic value should be accepted with a runtime guard"),
                value_types::ExpectedTypeOutcome::RequiresRuntimeGuard(expected)
            );
        },
    );

    compile_program_source(
        SourceId::new(1),
        r#"
fn f(x: i64) {
    return x;
}
fn main(value) {
    return f(value);
}
"#,
    )
    .expect("dynamic argument should compile for runtime contract guard");
}

#[test]
fn compiler_contextualizes_typed_let_literals() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let amount: u8 = 12;
    return amount;
}
"#,
        "main",
    )
    .expect("typed let should contextualize unsuffixed literal");

    assert!(
        code.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::U8(12)))
    );
}

#[test]
fn compiler_rejects_static_typed_let_contract_mismatches() {
    let error = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let amount: i64 = "12";
    return amount;
}
"#,
        "main",
    )
    .expect_err("typed let static mismatch should fail before bytecode emission");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::type_contract_mismatch"]
    );
}

#[test]
fn compiler_rejects_out_of_range_contextual_integer_literals() {
    for source in [
        r#"
fn f(x: u8) {
    return x;
}
fn main() {
    return f(300);
}
"#,
        r#"
fn main() {
    let amount: u8 = 300;
    return amount;
}
"#,
    ] {
        let error = compile_program_source(SourceId::new(1), source)
            .expect_err("out-of-range contextual literal should fail");
        let CompileErrorKind::InvalidIntLiteral { literal, error } = error.kind else {
            panic!("expected invalid integer literal");
        };
        assert_eq!(literal, "300");
        assert!(error.contains("out of range"), "{error}");
    }
}
