use super::*;

fn context_fixture() -> (Vm, Arc<LinkedArtifact>) {
    let program = compile_standard_program_source_with_native_functions(
        SourceId::new(91),
        include_str!("../../tests/fixtures/dispatch/context_callback.vela"),
        &["bridge::value"],
    )
    .expect("compile synchronous callback with a context native");
    let mut vm = Vm::new().with_standard_natives();
    vm.register_context_host_native_with_id(
        function_id_for_native_name("bridge::value"),
        |args, _host, _budget| Ok(args[0].clone()),
    );
    let mut linker = Linker::new();
    vm.native_implementation_ids()
        .for_each(|id| linker.add_native_implementation(id));
    let artifact = linker
        .link_test_program(&program)
        .expect("link context native");
    (vm, artifact)
}

fn start_context_session(
    vm: &Vm,
    artifact: &Arc<LinkedArtifact>,
    heap: &mut HeapExecution<'_>,
    budget: &mut ExecutionBudget,
) -> LinkedExecutionSession {
    let mut session = vm
        .start_linked_execution(
            LinkedExecutionStart {
                artifact,
                function: artifact.entry_point_by_name("main").expect("main entry"),
                args: &[],
                roots: &[],
                inline_caches: None,
                bytecode_profiler: None,
            },
            None,
            heap,
            budget,
        )
        .expect("start synchronous session");
    session.enable_context_native_boundaries();
    session
}

#[test]
fn synchronous_context_callback_reentry_preserves_roots_owner_and_budget() {
    let (vm, artifact) = context_fixture();
    let mut heap = ScriptHeap::new();
    let mut heap = HeapExecution::new(&mut heap).with_safe_point_gc_budget(GcBudget::unlimited());
    let mut budget = ExecutionBudget::new(10_000, 1_000_000, 16);
    let mut session = start_context_session(&vm, &artifact, &mut heap, &mut budget);
    let LinkedDriveOutcome::ContextBoundary(call) = vm
        .drive_linked_execution(&mut session, None, &mut heap, &mut budget, None, None)
        .expect("drive to synchronous context invocation")
    else {
        panic!("context native must return control to its host");
    };
    assert_eq!(
        call.native_id(),
        function_id_for_native_name("bridge::value")
    );
    assert_eq!(call.args(), &[OwnedValue::i64(41)]);
    assert!(
        session
            .frames
            .iter()
            .all(|frame| Arc::ptr_eq(&frame.owner, &artifact))
    );
    let caller_depth = budget.current_call_depth();
    assert!(caller_depth > 1);
    let before_reentry = budget.execution_units_consumed();
    vm.push_linked_reentry(
        &mut session,
        LinkedExecutionReentry {
            artifact: &artifact,
            function: artifact
                .entry_point_by_name("reenter")
                .expect("reentry function"),
            args: &[],
            inline_caches: None,
            bytecode_profiler: None,
        },
        None,
        &mut heap,
        &mut budget,
    )
    .expect("same-owner synchronous reentry");
    assert_eq!(budget.current_call_depth(), caller_depth + 1);
    let LinkedDriveOutcome::ReentryComplete(result) = vm
        .drive_linked_execution(&mut session, None, &mut heap, &mut budget, None, None)
        .expect("complete synchronous reentry")
    else {
        panic!("reentry must return to its native caller");
    };
    assert_eq!(budget.current_call_depth(), caller_depth);
    assert!(budget.execution_units_consumed() > before_reentry);
    let (result, _root) = result.into_parts();
    heap.heap.set_gc_config(crate::heap::GcConfig {
        max_pause_micros: 1_000,
        heap_growth_factor: 1.0,
    });
    let garbage = heap.heap.allocate(HeapValue::String("garbage".into()));
    heap.collect_frame_at_safe_point(
        &session.frames.last().expect("caller frame").registers,
        Some(&mut budget),
    );
    assert!(
        !heap.heap.contains(garbage),
        "the safe point must actually collect"
    );
    assert_eq!(
        value_to_owned(&result, Some(&heap)),
        Ok(OwnedValue::Array(vec![
            OwnedValue::i64(7),
            OwnedValue::i64(8)
        ]))
    );
    vm.resume_linked_context_call(
        &mut session,
        Ok(call.args()[0].clone()),
        None,
        Some(&mut heap),
        Some(&mut budget),
    )
    .expect("complete native invocation inside collection callback");
    let LinkedDriveOutcome::Complete(result) = vm
        .drive_linked_execution(&mut session, None, &mut heap, &mut budget, None, None)
        .expect("complete callback and caller")
    else {
        panic!("root call must complete synchronously");
    };
    assert_eq!(result, Value::i64(42));
    assert_eq!(budget.current_call_depth(), 0);
    assert!(session.pending_native.is_empty());
}

#[test]
fn synchronous_context_error_retains_source_span_and_callback_stack() {
    let (vm, artifact) = context_fixture();
    let mut heap = ScriptHeap::new();
    let mut heap = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(10_000, 1_000_000, 16);
    let mut session = start_context_session(&vm, &artifact, &mut heap, &mut budget);
    assert!(matches!(
        vm.drive_linked_execution(&mut session, None, &mut heap, &mut budget, None, None),
        Ok(LinkedDriveOutcome::ContextBoundary(_))
    ));
    let error = vm
        .resume_linked_context_call(
            &mut session,
            Err(VmError::new(VmErrorKind::DivisionByZero)),
            None,
            Some(&mut heap),
            Some(&mut budget),
        )
        .expect_err("native failure propagates through its synchronous callback");
    assert_eq!(error.kind(), VmErrorKind::DivisionByZero);
    assert!(error.source_span.is_some());
    assert!(
        error
            .call_stack
            .iter()
            .any(|frame| frame.function.ends_with("main"))
    );
    assert!(error.call_stack.len() >= 3);
}

#[test]
fn synchronous_context_completion_requires_a_pending_invocation() {
    let (vm, artifact) = context_fixture();
    let mut heap = ScriptHeap::new();
    let mut heap = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(10_000, 1_000_000, 16);
    let mut session = start_context_session(&vm, &artifact, &mut heap, &mut budget);
    let error = vm
        .resume_linked_context_call(
            &mut session,
            Ok(OwnedValue::Unit),
            None,
            Some(&mut heap),
            Some(&mut budget),
        )
        .expect_err("cannot complete a native call that has not begun");
    assert_eq!(
        error.kind(),
        VmErrorKind::UnsupportedLinkedInstruction {
            opcode: "context resume without a pending invocation",
        }
    );
    assert_eq!(budget.current_call_depth(), 1);
}

#[test]
fn synchronous_collection_callback_keeps_its_creation_owner() {
    let old = compile_standard_program_source(
        SourceId::new(93),
        "fn helper() { return 41; } fn make() { return |value| helper() + value; }",
    )
    .expect("compile callback creation generation");
    let new = compile_standard_program_source(
        SourceId::new(94),
        "fn helper() { return 900; } fn main(callback) { return [1].map(callback)[0]; }",
    )
    .expect("compile callback caller generation");
    let vm = Vm::new().with_standard_natives();
    let mut linker = Linker::new();
    vm.native_implementation_ids()
        .for_each(|id| linker.add_native_implementation(id));
    let old = linker.link_test_program(&old).expect("link old owner");
    let new = linker.link_test_program(&new).expect("link new owner");
    let mut heap = ScriptHeap::new();
    let mut heap = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::new(10_000, 1_000_000, 16);
    let callback = vm
        .execute_linked_call(
            crate::linked_execution::LinkedExecutionCall {
                owner: Arc::clone(&old),
                function: old.entry_point_by_name("make").expect("callback factory"),
                captures: &[],
                args: &[],
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: None,
                bytecode_profiler: None,
            },
            None,
            Some(&mut heap),
            Some(&mut budget),
        )
        .expect("retain old callback");
    let result = vm
        .execute_linked_call(
            crate::linked_execution::LinkedExecutionCall {
                owner: new.clone(),
                function: new.entry_point_by_name("main").expect("new caller"),
                captures: &[],
                args: &[callback],
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: None,
                bytecode_profiler: None,
            },
            None,
            Some(&mut heap),
            Some(&mut budget),
        )
        .expect("invoke old callback from new code");
    assert_eq!(result, Value::i64(42));
    assert_eq!(budget.current_call_depth(), 0);
}

#[test]
fn synchronous_collection_callbacks_obey_call_depth_budget() {
    for callback in [
        "[value - 1].any(|next| dive(next))",
        "[value - 1].iter().map(|next| dive(next)).next().unwrap_or(false)",
    ] {
        let source = format!(
            "fn dive(value) {{ if value == 0 {{ return true; }} return {callback}; }}\n\
             fn main() {{ return dive(100); }}"
        );
        let program = compile_standard_program_source(SourceId::new(92), &source)
            .expect("compile synchronous callback recursion");
        let vm = Vm::new().with_standard_natives();
        let mut budget = ExecutionBudget::new(10_000, 1_000_000, 8);
        let error = run_linked_test_program_with_budget(&vm, &program, "main", &[], &mut budget)
            .expect_err("callback recursion must respect the shared call-depth budget");
        assert_eq!(
            error.kind(),
            VmErrorKind::BudgetExceeded {
                budget: ExecutionBudgetKind::CallDepth,
                limit: 8,
            }
        );
        assert!(error.source_span.is_some());
        assert_eq!(budget.current_call_depth(), 0);
    }
}
