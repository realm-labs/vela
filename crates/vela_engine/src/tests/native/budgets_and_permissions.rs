use super::*;

use vela_bytecode::UnlinkedProgram;
use vela_common::Span;
use vela_vm::error::VmResult;

fn run_linked_program(
    engine: &Engine,
    program: &UnlinkedProgram,
    args: &[OwnedValue],
) -> VmResult<OwnedValue> {
    let linked = engine
        .link_test_program(program)
        .expect("engine native budget test program should link");
    engine
        .into_vm_for_program(program)
        .run_linked_program(&linked, "main", args)
}

#[test]
fn runtime_call_enforces_call_options_budget() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn main() {
    let total = 0;
    for value in 1..=100 {
        total += value;
    }
    return total;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let error = runtime
        .call_raw("main", &[], CallOptions::new(4, usize::MAX, usize::MAX))
        .expect_err("runtime call should exhaust execution-unit budget");

    assert_eq!(
        error,
        VmError::new(VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::ExecutionUnits,
            limit: 4
        })
        .with_source_span(Some(Span::new(SourceId::new(1), 49, 56)))
        .with_call_stack(Arc::from([vela_vm::error::VmStackFrame {
            function: "main".to_owned(),
            call_site: None,
            bytecode_offset: None,
        }]))
    );
}

#[test]
fn runtime_call_enforces_call_depth_budget() {
    let engine = Engine::builder().build().expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn recurse() {
    return recurse();
}

fn main() {
    return recurse();
}
"#,
        )
        .expect("program should compile");
    let mut runtime = Runtime::new(engine, program).expect("runtime should initialize");

    let error = runtime
        .call_raw("main", &[], CallOptions::new(u64::MAX, usize::MAX, 2))
        .expect_err("runtime call should exhaust call-depth budget");

    assert_eq!(
        error.kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::CallDepth,
            limit: 2
        }
    );
    assert_eq!(error.call_stack.len(), 3);
    assert_eq!(error.call_stack[0].function, "recurse");
    assert_eq!(
        error.call_stack[0].bytecode_offset,
        Some(vela_bytecode::InstructionOffset(0))
    );
    assert_eq!(error.call_stack[1].function, "recurse");
    assert_eq!(
        error.call_stack[1].bytecode_offset,
        Some(vela_bytecode::InstructionOffset(0))
    );
    assert_eq!(error.call_stack[2].function, "main");
    assert_eq!(error.call_stack[2].bytecode_offset, None);
}

#[test]
fn engine_allows_pure_native_calls_without_capabilities() {
    let engine = Engine::builder()
        .register_native_fn(
            NativeFunctionDesc::new("game::secret", NativeFunctionId::new(3))
                .returns(TypeHint::i64())
                .access(FunctionAccess::public()),
            |_| Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(99))),
        )
        .build()
        .expect("engine should build");
    let program = engine
        .compile_source_with_id(
            SourceId::new(1),
            r#"
fn main() {
    return game::secret();
}
"#,
        )
        .expect("program should compile");

    assert_eq!(
        run_linked_program(&engine, &program, &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(99)))
    );
}
