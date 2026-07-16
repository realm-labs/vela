use super::*;
use crate::value::Value as RuntimeValue;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

#[test]
fn linked_context_native_pauses_and_resumes_the_same_frame() {
    let native_id = FunctionId::new(0x58);
    let mut vm = Vm::new();
    vm.register_context_host_native_with_id(native_id, |_args, _host, _budget| {
        Ok(OwnedValue::Unit)
    });

    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let native_name = program.intern_debug_name("context_add_one");
    let native = program.push_native_function(vela_bytecode::LinkedNativeFunction::new(
        native_id,
        native_name,
    ));
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 2);
    let value = code.push_constant(Constant::i64(41));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallNative {
            dst: Some(Register(1)),
            native,
            debug_name: native_name,
            cache_site: None,
            args: vec![Register(0)],
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
        .expect("context entry should prepare");
    session.enable_context_native_boundaries();

    let LinkedDriveOutcome::ContextBoundary(prepared) = vm
        .drive_linked_execution(&mut session, None, &mut heap, &mut budget, None, None)
        .expect("context native should pause at the runtime boundary")
    else {
        panic!("context native should suspend the linked frame");
    };
    assert_eq!(prepared.native_id(), native_id);
    assert_eq!(prepared.name(), "context_add_one");
    assert_eq!(prepared.args(), &[OwnedValue::i64(41)]);

    vm.resume_linked_context_call(
        &mut session,
        Ok(OwnedValue::i64(42)),
        Some(&mut heap),
        Some(&mut budget),
    )
    .expect("context result should resume the frame");
    let LinkedDriveOutcome::Complete(value) = vm
        .drive_linked_execution(&mut session, None, &mut heap, &mut budget, None, None)
        .expect("resumed context entry should complete")
    else {
        panic!("resumed context entry should not suspend again");
    };
    assert_eq!(value, RuntimeValue::i64(42));
}

#[test]
fn linked_async_native_arguments_and_results_are_owned_across_gc() {
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
    let value = code.push_constant(Constant::String("kept across await".into()));
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
    assert_eq!(
        prepared.args(),
        &[OwnedValue::String("kept across await".into())]
    );
    let stats = heap.heap.collect_full(&[]);
    assert_eq!(
        stats.swept, 1,
        "native arguments must not borrow heap values"
    );

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
    let RuntimeValue::HeapRef(value) = value else {
        panic!("owned async result should be materialized back into the script heap");
    };
    assert_eq!(
        heap.heap.get(value),
        Some(&HeapValue::String("kept across await".into()))
    );
}
