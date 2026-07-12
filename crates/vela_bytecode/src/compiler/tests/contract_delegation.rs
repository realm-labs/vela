use super::*;

#[test]
fn compiler_delegates_typed_let_contract_outcomes() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
fn contextual() {
    let amount: u8 = 12;
    return amount;
}

fn guarded(value) {
    let amount: i64 = value;
    return amount;
}
"#,
    )
    .expect("typed locals should preserve contextual and guarded outcomes");
    let contextual = program.function("contextual").expect("contextual function");
    let guarded = program.function("guarded").expect("guarded function");

    assert!(
        contextual
            .constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::U8(12)))
    );
    assert!(
        !contextual.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GuardType { .. }
        ))
    );
    let guard = guarded
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::GuardType { guard, .. } => Some(guard),
            _ => None,
        })
        .expect("dynamic typed local should emit a contract guard");
    assert_eq!(guard.context.location, crate::GuardLocation::Local);
    assert_eq!(guard.context.debug_name, "amount");

    let error = compile_test_program(
        SourceId::new(2),
        r#"fn main() { let amount: i64 = "x"; return amount; }"#,
    )
    .expect_err("static typed local mismatch should fail");
    let diagnostic = only_contract_diagnostic(error);
    assert_eq!(
        diagnostic.message,
        "type contract mismatch for typed local `amount`"
    );
    assert_eq!(
        diagnostic.labels[0].message,
        "expected `i64`, found `String`"
    );
}

#[test]
fn compiler_delegates_parameter_default_contracts() {
    let program = compile_test_program(
        SourceId::new(3),
        "fn grant(amount: u8 = 12) { return amount; }",
    )
    .expect("parameter default literal should use its owning parameter contract");
    let grant = program.function("grant").expect("grant function");
    assert!(
        grant
            .constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::U8(12)))
    );

    let error = compile_test_program(
        SourceId::new(4),
        r#"fn grant(amount: i64 = "x") { return amount; }"#,
    )
    .expect_err("static parameter default mismatch should fail");
    let diagnostic = only_contract_diagnostic(error);
    assert_eq!(
        diagnostic.message,
        "type contract mismatch for parameter `amount`"
    );
    assert_eq!(
        diagnostic.labels[0].message,
        "expected `i64`, found `String`"
    );
}

#[test]
fn compiler_delegates_script_argument_guard_selection() {
    let program = compile_test_program(
        SourceId::new(5),
        r#"
fn grant(amount: i64) { return amount; }
fn proven() { return grant(1i64); }
fn guarded(value) { return grant(value); }
"#,
    )
    .expect("script arguments should preserve checked versus unchecked selection");
    let call_mode = |name: &str| {
        program
            .function(name)
            .and_then(|function| {
                function
                    .instructions
                    .iter()
                    .find_map(|instruction| match instruction.kind {
                        UnlinkedInstructionKind::CallFunction { mode, .. } => Some(mode),
                        _ => None,
                    })
            })
            .expect("script call")
    };

    assert_eq!(call_mode("proven"), crate::ScriptCallMode::Unchecked);
    assert_eq!(call_mode("guarded"), crate::ScriptCallMode::Checked);
}

#[test]
fn compiler_delegates_erased_and_parameterized_container_contracts() {
    let program = compile_test_program(
        SourceId::new(6),
        r#"
fn erase(values: Array<i64>) {
    let result: Array = values;
    return result;
}

fn prove_at_runtime(values: Array) {
    let result: Array<i64> = values;
    return result;
}
"#,
    )
    .expect("erased and parameterized containers should preserve guard policy");
    let erase = program.function("erase").expect("erase function");
    let prove_at_runtime = program
        .function("prove_at_runtime")
        .expect("prove_at_runtime function");

    assert!(
        !erase.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GuardType { .. }
        ))
    );
    assert!(
        prove_at_runtime
            .instructions
            .iter()
            .any(|instruction| matches!(
                instruction.kind,
                UnlinkedInstructionKind::GuardType { .. }
            ))
    );

    let error = compile_test_program(
        SourceId::new(7),
        r#"
fn mismatch(values: Array<i64>) {
    let result: Array<String> = values;
    return result;
}
"#,
    )
    .expect_err("incompatible parameterized containers should fail statically");
    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::type_contract_mismatch"]
    );
}

#[test]
fn compiler_preserves_directional_function_and_closure_contracts() {
    compile_test_program(
        SourceId::new(8),
        r#"
fn accepts(value: Closure) {
    let callable: Function = value;
    return callable;
}
"#,
    )
    .expect("a concrete closure satisfies the erased Function contract");

    let error = compile_test_program(
        SourceId::new(9),
        r#"
fn rejects(value: Function) {
    let callback: Closure = value;
    return callback;
}
"#,
    )
    .expect_err("an erased Function does not prove the concrete Closure contract");
    let diagnostic = only_contract_diagnostic(error);
    assert_eq!(
        diagnostic.labels[0].message,
        "expected `Closure`, found `Function`"
    );
}

fn only_contract_diagnostic(error: TestCompileError) -> vela_common::Diagnostic {
    let mut diagnostics = error.into_semantic_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    let diagnostic = diagnostics.remove(0);
    assert_eq!(
        diagnostic.code.as_deref(),
        Some("compiler::type_contract_mismatch")
    );
    diagnostic
}
