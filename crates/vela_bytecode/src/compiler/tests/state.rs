use super::*;

#[test]
fn compiler_lowers_vm_state_reads_and_assignments_to_vm_state_opcodes() {
    let program = compile_test_program(
        SourceId::new(5),
        r#"
state counter: i64 = 1;

fn update() {
    counter = 3;
    counter += 2;
    return counter;
}
"#,
    )
    .expect("VM state program should compile");
    let update = program.function("update").expect("update bytecode");
    let slot = program
        .state_slot("main::counter")
        .expect("counter should have a state slot");
    let state = program.state(slot).expect("counter descriptor");
    let initializer = state.initializer.expect("compiled state initializer");
    assert!(program.function_by_id(initializer).is_some());

    assert!(update.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::LoadState {
            state,
            slot: Some(actual),
            ..
        } if state == "main::counter" && *actual == slot
    )));
    assert!(update.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::StoreState {
            state,
            slot: Some(actual),
            ..
        } if state == "main::counter" && *actual == slot
    )));
    assert!(!update.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::LoadExternState { .. }
    )));
}

#[test]
fn compiler_allows_state_initializers_to_call_proven_pure_script_functions() {
    let program = compile_test_program(
        SourceId::new(51),
        r#"
fn initial_counter() -> i64 { return 7; }
state counter: i64 = initial_counter();
fn read() { return counter; }
"#,
    )
    .expect("pure script-call initializer should compile");
    let state = program.states().first().expect("state descriptor");
    let initializer = state.initializer.expect("initializer identity");

    assert!(program.function_by_id(initializer).is_some());
}

#[test]
fn compiler_rejects_transitive_state_reads_from_state_initializers() {
    let error = compile_test_program(
        SourceId::new(52),
        r#"
state source: i64 = 1;
fn read_source() -> i64 { return source; }
state target: i64 = read_source();
"#,
    )
    .expect_err("state reads through a script call must be rejected");

    assert!(matches!(
        error.kind,
        CompileErrorKind::InvalidStateInitializer { state, reason }
            if state == "main::target" && reason.contains("state access")
    ));
    assert!(error.span.is_some());
}

#[test]
fn compiler_rejects_extern_state_reads_from_state_initializers() {
    let error = compile_test_program(
        SourceId::new(53),
        r#"
extern state source: i64;
state target: i64 = source;
"#,
    )
    .expect_err("extern state reads must be rejected");

    assert!(matches!(
        error.kind,
        CompileErrorKind::InvalidStateInitializer { state, reason }
            if state == "main::target" && reason.contains("state access")
    ));
    assert!(error.span.is_some());
}

#[test]
fn compiler_rejects_standard_library_calls_from_state_initializers() {
    let error = compile_test_program(SourceId::new(54), "state target: i64 = math::max(1, 2);")
        .expect_err("standard-library calls must be rejected");

    assert!(matches!(
        error.kind,
        CompileErrorKind::InvalidStateInitializer { state, reason }
            if state == "main::target" && reason.contains("standard-library")
    ));
    assert!(error.span.is_some());
}

#[test]
fn compiler_rejects_async_work_from_state_initializers() {
    let error = compile_test_program(
        SourceId::new(55),
        r#"
async fn pending() -> i64 { return 1; }
state target: i64 = pending().await;
"#,
    )
    .expect_err("async state initializer work must be rejected");

    assert!(matches!(
        &error.kind,
        CompileErrorKind::InvalidHirGraph(diagnostics)
            if diagnostics.iter().any(|diagnostic| {
                diagnostic.code.as_deref() == Some("syntax::await_outside_async")
            })
    ));
}
