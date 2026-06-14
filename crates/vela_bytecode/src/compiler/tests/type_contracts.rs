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
        UnlinkedInstructionKind::CallFunction { args, mode, .. }
            if *mode == crate::ScriptCallMode::Unchecked
                && args.len() == 1
                && matches!(args[0], CallArgument::Register(_))
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
        label.message.contains("expected `i64`") && label.message.contains("found `String`")
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
    .map(|program| {
        let main = program.function("main").expect("main function");
        assert!(main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::CallFunction {
                mode: crate::ScriptCallMode::Checked,
                ..
            }
        )));
    })
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
fn compiler_contextualizes_typed_return_literals_without_guard_instruction() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() -> i8 {
    return 12;
}
"#,
    )
    .expect("return literal should be contextualized by return type");
    let function = program.function("main").expect("main function");

    assert!(
        function
            .constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I8(12)))
    );
    assert!(
        !function.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GuardType { .. }
        ))
    );
}

#[test]
fn compiler_rejects_static_return_contract_mismatches() {
    for source in [
        r#"
fn main() -> i64 {
    return "x";
}
"#,
        r#"
fn main() -> i64 {
    return;
}
"#,
        r#"
fn main() -> i64 {
    return 1.0;
}
"#,
    ] {
        let error = compile_program_source(SourceId::new(1), source)
            .expect_err("static return mismatch should fail before bytecode emission");
        assert_eq!(
            semantic_diagnostic_codes(error),
            ["compiler::type_contract_mismatch"]
        );
    }
}

#[test]
fn compiler_keeps_dynamic_return_contracts_for_runtime_guards() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(value) -> i64 {
    return value;
}
"#,
    )
    .expect("dynamic return should compile for runtime return guard");
    let function = program.function("main").expect("main function");

    assert!(matches!(
        function.return_guard.as_ref().map(|guard| &guard.plan),
        Some(crate::UnlinkedTypeGuardPlan::Primitive(
            vela_common::PrimitiveTag::I64
        ))
    ));
    assert!(
        !function.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GuardType { .. }
        ))
    );
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
fn compiler_emits_field_guard_for_dynamic_typed_record_write() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    amount: i64,
}
fn make_reward() {
    return Reward { amount: 1 };
}
fn main(value) {
    let reward: Reward = make_reward();
    reward.amount = value;
    return reward.amount;
}
"#,
    )
    .expect("dynamic typed record write should compile with runtime guard");
    let main = program.function("main").expect("main function");
    let guard_index = main
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction.kind, UnlinkedInstructionKind::GuardType { .. })
        })
        .expect("dynamic typed record write should emit GuardType");
    let set_index = main
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction.kind,
                UnlinkedInstructionKind::SetRecordSlot {
                    ref field,
                    slot: 0,
                    ..
                } if field == "amount"
            )
        })
        .expect("typed record write should use a slot write");

    assert!(guard_index < set_index);
    let UnlinkedInstructionKind::GuardType {
        src: guard_src,
        guard,
    } = &main.instructions[guard_index].kind
    else {
        panic!("expected GuardType");
    };
    let UnlinkedInstructionKind::SetRecordSlot { src: set_src, .. } =
        &main.instructions[set_index].kind
    else {
        panic!("expected SetRecordSlot");
    };

    assert_eq!(guard_src, set_src);
    assert_eq!(guard.context.location, crate::GuardLocation::Field);
    assert_eq!(guard.context.debug_name, "amount");
    assert!(matches!(
        guard.plan,
        crate::UnlinkedTypeGuardPlan::Primitive(vela_common::PrimitiveTag::I64)
    ));

    let linked = crate::Linker::new()
        .link_program(&program)
        .expect("program should link");
    linked.verify().expect("linked field guard should verify");
}

#[test]
fn compiler_emits_field_guard_for_dynamic_nested_typed_record_write() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Stats {
    level: i64,
}
struct Player {
    stats: Stats,
}
fn make_player() {
    return Player { stats: Stats { level: 1 } };
}
fn main(value) {
    let player: Player = make_player();
    player.stats.level = value;
    return player.stats.level;
}
"#,
    )
    .expect("dynamic nested typed record write should compile with runtime guard");
    let main = program.function("main").expect("main function");
    let guard_index = main
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction.kind, UnlinkedInstructionKind::GuardType { .. })
        })
        .expect("dynamic nested typed record write should emit GuardType");
    let set_index = main
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction.kind,
                UnlinkedInstructionKind::SetRecordSlot {
                    ref field,
                    slot: 0,
                    ..
                } if field == "level"
            )
        })
        .expect("nested typed record write should use a leaf slot write");

    assert!(guard_index < set_index);
    let UnlinkedInstructionKind::GuardType {
        src: guard_src,
        guard,
    } = &main.instructions[guard_index].kind
    else {
        panic!("expected GuardType");
    };
    let UnlinkedInstructionKind::SetRecordSlot { src: set_src, .. } =
        &main.instructions[set_index].kind
    else {
        panic!("expected SetRecordSlot");
    };

    assert_eq!(guard_src, set_src);
    assert_eq!(guard.context.location, crate::GuardLocation::Field);
    assert_eq!(guard.context.debug_name, "level");
    assert!(matches!(
        guard.plan,
        crate::UnlinkedTypeGuardPlan::Primitive(vela_common::PrimitiveTag::I64)
    ));

    let linked = crate::Linker::new()
        .link_program(&program)
        .expect("program should link");
    linked
        .verify()
        .expect("linked nested field guard should verify");
}

#[test]
fn compiler_contextualizes_typed_record_field_literals_without_guard() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    amount: u8,
}
fn make_reward() {
    return Reward { amount: 1 };
}
fn main() {
    let reward: Reward = make_reward();
    reward.amount = 12;
    return reward.amount;
}
"#,
    )
    .expect("typed record write literal should compile without runtime guard");
    let main = program.function("main").expect("main function");

    assert!(
        !main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GuardType { .. }
        ))
    );
    assert!(
        main.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::U8(12)))
    );
}

#[test]
fn compiler_contextualizes_nested_typed_record_field_literals_without_guard() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Stats {
    level: u8,
}
struct Player {
    stats: Stats,
}
fn make_player() {
    return Player { stats: Stats { level: 1 } };
}
fn main() {
    let player: Player = make_player();
    player.stats.level = 12;
    return player.stats.level;
}
"#,
    )
    .expect("nested typed record write literal should compile without runtime guard");
    let main = program.function("main").expect("main function");

    assert!(
        !main.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GuardType { .. }
        ))
    );
    assert!(
        main.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::U8(12)))
    );
}

#[test]
fn compiler_rejects_static_typed_record_field_contract_mismatches() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    amount: i64,
}
fn make_reward() {
    return Reward { amount: 1 };
}
fn main() {
    let reward: Reward = make_reward();
    reward.amount = "x";
    return reward.amount;
}
"#,
    )
    .expect_err("static typed record write mismatch should fail before bytecode emission");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::type_contract_mismatch"]
    );
}

#[test]
fn compiler_rejects_static_nested_typed_record_field_contract_mismatches() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
struct Stats {
    level: i64,
}
struct Player {
    stats: Stats,
}
fn make_player() {
    return Player { stats: Stats { level: 1 } };
}
fn main() {
    let player: Player = make_player();
    player.stats.level = "x";
    return player.stats.level;
}
"#,
    )
    .expect_err("static nested typed record write mismatch should fail before bytecode emission");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::type_contract_mismatch"]
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
        assert_error_span_text(source, error.span, "300");
        let CompileErrorKind::InvalidIntLiteral { literal, error } = error.kind else {
            panic!("expected invalid integer literal");
        };
        assert_eq!(literal, "300");
        assert!(error.contains("out of range"), "{error}");
    }
}

#[test]
fn compiler_rejects_out_of_range_contextual_float_literals() {
    for source in [
        r#"
fn f(x: f32) {
    return x;
}
fn main() {
    return f(1.0e100);
}
"#,
        r#"
fn main() {
    let amount: f32 = 1.0e100;
    return amount;
}
"#,
        r#"
fn main() {
    let amount: f64 = 1.0e10000;
    return amount;
}
"#,
    ] {
        let error = compile_program_source(SourceId::new(1), source)
            .expect_err("out-of-range contextual float should fail");
        assert_error_span_prefix(source, error.span, "1.0e");
        let CompileErrorKind::InvalidFloatLiteral { literal, error } = error.kind else {
            panic!("expected invalid float literal");
        };
        assert!(literal.starts_with("1.0e"), "{literal}");
        assert!(error.contains("out of range"), "{error}");
    }
}

#[test]
fn compiler_defers_inline_unsuffixed_int_literals_in_dynamic_binary_ops() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn inc(x) {
    return x + 1;
}
fn from_one(x) {
    return 1 - x;
}
"#,
    )
    .expect("dynamic inline int literals should compile as deferred binary ops");

    let inc = program.function("inc").expect("inc function");
    assert!(inc.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::BinaryIntLiteral {
                op: crate::BinaryLiteralOp::Add,
                ref literal,
                side: crate::BinaryLiteralSide::Right,
                ..
            } if literal == "1"
        )
    }));
    assert!(
        !inc.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let from_one = program.function("from_one").expect("from_one function");
    assert!(from_one.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::BinaryIntLiteral {
                op: crate::BinaryLiteralOp::Sub,
                ref literal,
                side: crate::BinaryLiteralSide::Left,
                ..
            } if literal == "1"
        )
    }));

    let linked = crate::Linker::new()
        .link_program(&program)
        .expect("program should link");
    linked
        .verify()
        .expect("linked deferred int ops should verify");
    let linked_inc = linked
        .entry_point_by_name("inc")
        .and_then(|handle| linked.function(handle))
        .expect("linked inc should exist");
    assert!(linked_inc.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            crate::linked::InstructionKind::BinaryIntLiteral {
                op: crate::BinaryLiteralOp::Add,
                ref literal,
                side: crate::BinaryLiteralSide::Right,
                ..
            } if literal == "1"
        )
    }));
}

#[test]
fn compiler_defers_inline_unsuffixed_float_literals_in_dynamic_binary_ops() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn scale(x) {
    return x * 1.5;
}
"#,
    )
    .expect("dynamic inline float literals should compile as deferred binary ops");
    let scale = program.function("scale").expect("scale function");

    assert!(scale.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            UnlinkedInstructionKind::BinaryFloatLiteral {
                op: crate::BinaryLiteralOp::Mul,
                ref literal,
                side: crate::BinaryLiteralSide::Right,
                ..
            } if literal == "1.5"
        )
    }));
    assert!(
        !scale
            .constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::F64(1.5)))
    );

    let linked = crate::Linker::new()
        .link_program(&program)
        .expect("program should link");
    linked
        .verify()
        .expect("linked deferred float ops should verify");
}

#[test]
fn compiler_contextualizes_inline_literals_for_known_numeric_operands() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn inc_i8(x: i8) {
    return x + 1;
}
"#,
        "inc_i8",
    )
    .expect("typed numeric operand should contextualize inline literal");

    assert!(!code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::BinaryIntLiteral { .. }
            | UnlinkedInstructionKind::BinaryFloatLiteral { .. }
    )));
    assert!(
        code.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I8(1)))
    );
    assert!(
        code.instructions
            .iter()
            .any(|instruction| { matches!(instruction.kind, UnlinkedInstructionKind::Add { .. }) })
    );
}

#[test]
fn compiler_keeps_bound_unsuffixed_literals_concrete_in_binary_ops() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn inc_strict(x) {
    let one = 1;
    return x + one;
}
"#,
        "inc_strict",
    )
    .expect("bound literal should compile as concrete constant");

    assert!(!code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::BinaryIntLiteral { .. }
            | UnlinkedInstructionKind::BinaryFloatLiteral { .. }
    )));
    assert!(
        code.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I64(1)))
    );
    assert!(
        code.instructions
            .iter()
            .any(|instruction| { matches!(instruction.kind, UnlinkedInstructionKind::Add { .. }) })
    );
}

fn assert_error_span_text(source: &str, span: Option<vela_common::Span>, expected: &str) {
    let span = span.expect("compile error should carry a source span");
    assert_eq!(span.source, SourceId::new(1));
    let actual = &source[span.start as usize..span.end as usize];
    assert_eq!(actual, expected);
}

fn assert_error_span_prefix(source: &str, span: Option<vela_common::Span>, expected: &str) {
    let span = span.expect("compile error should carry a source span");
    assert_eq!(span.source, SourceId::new(1));
    let actual = &source[span.start as usize..span.end as usize];
    assert!(
        actual.starts_with(expected),
        "expected span text starting with {expected:?}, got {actual:?}"
    );
}
