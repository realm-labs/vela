use super::*;
use crate::value::Value as RuntimeValue;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

#[test]
fn linked_execution_prepares_and_resumes_async_native_calls() {
    let native_id = FunctionId::new(0x57);
    let mut vm = Vm::new();
    vm.register_async_native_with_id(native_id, |args| {
        Box::pin(async move { Ok(args.first().cloned().unwrap_or(OwnedValue::Unit)) })
    });

    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let native_name = program.intern_debug_name("async_identity");
    let native = program.push_native_function(
        vela_bytecode::LinkedNativeFunction::new(native_id, native_name)
            .with_asyncness(vela_common::CallableAsyncness::Async),
    );
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 2)
        .with_asyncness(vela_common::CallableAsyncness::Async);
    let value = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(42)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::AwaitCall {
            operation: Box::new(vela_bytecode::linked::InstructionKind::CallNative {
                dst: Some(Register(1)),
                native,
                debug_name: native_name,
                cache_site: None,
                args: vec![Register(0)],
            }),
            resume: InstructionOffset(2),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(1) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    let artifact = linked_test_owner(program);

    let mut heap = ScriptHeap::new();
    let mut heap = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::unbounded();
    let mut session = vm
        .start_linked_execution(
            LinkedExecutionStart {
                artifact: &artifact,
                function,
                args: &[],
                roots: &[],
                inline_caches: None,
                bytecode_profiler: None,
            },
            &mut heap,
            &mut budget,
        )
        .expect("async entry should prepare");

    let LinkedDriveOutcome::AsyncBoundary(prepared) = vm
        .drive_linked_execution(&mut session, None, &mut heap, &mut budget, None, None)
        .expect("await should reach an async boundary")
    else {
        panic!("awaited async native should suspend");
    };
    let mut future = prepared.invoke();
    let mut context = Context::from_waker(Waker::noop());
    let Poll::Ready(result) = Pin::new(&mut future).poll(&mut context) else {
        panic!("identity future should be ready");
    };
    vm.resume_linked_async_call(&mut session, result, Some(&mut heap), Some(&mut budget))
        .expect("ready value should resume the frame");

    let LinkedDriveOutcome::Complete(value) = vm
        .drive_linked_execution(&mut session, None, &mut heap, &mut budget, None, None)
        .expect("resumed entry should complete")
    else {
        panic!("resumed entry should not suspend again");
    };
    assert_eq!(value, RuntimeValue::i64(42));
}
