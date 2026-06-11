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
fn compiler_emits_linked_parameter_and_return_guard_metadata() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn f(x: i64) -> i64 {
    return x;
}
fn main() {
    return f(12);
}
"#,
    )
    .expect("typed function should compile");
    let unlinked = program.function("f").expect("function should exist");

    assert_eq!(unlinked.param_guards.len(), 1);
    assert_eq!(unlinked.param_guards[0].parameter, 0);
    assert!(matches!(
        unlinked.param_guards[0].guard.plan,
        crate::UnlinkedTypeGuardPlan::Primitive(vela_common::PrimitiveTag::I64)
    ));
    assert!(matches!(
        unlinked.return_guard.as_ref().map(|guard| &guard.plan),
        Some(crate::UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::I64
        ))
    ));

    let linked = crate::Linker::new()
        .link_program(&program)
        .expect("program should link");
    linked.verify().expect("linked guards should verify");
    let function = linked
        .entry_point_by_name("f")
        .and_then(|handle| linked.function(handle))
        .expect("linked function should exist");

    assert_eq!(function.param_guards.len(), 1);
    let param_guard = function.param_guards[0].guard;
    assert!(matches!(
        function.type_guard(param_guard).map(|guard| &guard.plan),
        Some(crate::TypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::I64
        ))
    ));
    let return_guard = function.return_guard.expect("return guard should exist");
    assert!(matches!(
        function.type_guard(return_guard).map(|guard| &guard.plan),
        Some(crate::TypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::I64
        ))
    ));
}

#[test]
fn compiler_leaves_unhinted_functions_without_guard_metadata() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn f(x) {
    return x;
}
"#,
    )
    .expect("unhinted function should compile");
    let unlinked = program.function("f").expect("function should exist");

    assert!(unlinked.param_guards.is_empty());
    assert!(unlinked.return_guard.is_none());

    let linked = crate::Linker::new()
        .link_program(&program)
        .expect("program should link");
    let function = linked
        .entry_point_by_name("f")
        .and_then(|handle| linked.function(handle))
        .expect("linked function should exist");

    assert!(function.type_guards.is_empty());
    assert!(function.param_guards.is_empty());
    assert!(function.return_guard.is_none());
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
    assert!(
        !code.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GuardType { .. }
        ))
    );
}

#[test]
fn compiler_emits_local_guard_for_dynamic_typed_let() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(value) {
    let amount: i64 = value;
    return amount;
}
"#,
    )
    .expect("dynamic typed let should compile with runtime guard");
    let main = program.function("main").expect("main function");
    let guard = main
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::GuardType { guard, .. } => Some(guard),
            _ => None,
        })
        .expect("dynamic typed let should emit GuardType");

    assert!(matches!(
        guard.plan,
        crate::UnlinkedTypeGuardPlan::Primitive(vela_common::PrimitiveTag::I64)
    ));
    assert_eq!(guard.context.location, crate::GuardLocation::Local);
    assert_eq!(guard.context.debug_name, "amount");

    let linked = crate::Linker::new()
        .link_program(&program)
        .expect("program should link");
    linked.verify().expect("linked local guard should verify");
    let main = linked
        .entry_point_by_name("main")
        .and_then(|handle| linked.function(handle))
        .expect("linked main should exist");
    assert!(
        main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            crate::InstructionKind::GuardType { .. }
        ))
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
