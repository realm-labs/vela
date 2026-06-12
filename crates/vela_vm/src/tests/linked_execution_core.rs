use super::*;
use crate::owned_value::OwnedValue;
use crate::value::Value as RuntimeValue;
use std::cell::{Cell, RefCell};

#[test]
fn runs_linked_program_basic_arithmetic_without_unlinked_code() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 5);
    let two = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));
    let three = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(3)));
    let four = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(4)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: two,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(1),
            constant: three,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(2),
            constant: four,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Mul {
            dst: Register(3),
            lhs: Register(1),
            rhs: Register(2),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Add {
            dst: Register(4),
            lhs: Register(0),
            rhs: Register(3),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(4) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);

    assert_eq!(
        Vm::new().run_linked_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(14)))
    );
}

#[test]
fn linked_execution_rejects_undersized_inline_cache_provider() {
    struct EmptyInlineCaches;

    impl VmInlineCaches for EmptyInlineCaches {
        fn len(&self) -> usize {
            0
        }
    }

    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 1);
    let cache_site = code.push_cache_site(CacheSiteKind::GlobalRead, InstructionOffset(0));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadGlobal {
            dst: Register(0),
            slot: vela_common::GlobalSlot::new(0),
            debug_name: main_name,
            cache_site: Some(cache_site),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(0) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    let code = program.function(function).expect("main function");

    let error = Vm::new()
        .execute_linked_call(
            crate::linked_execution::LinkedExecutionCall {
                code,
                program: &program,
                captures: &[],
                args: &[],
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: Some(&EmptyInlineCaches),
                bytecode_profiler: None,
            },
            None,
            None,
            None,
        )
        .expect_err("undersized inline caches should be rejected before dispatch");

    assert_eq!(
        error.kind(),
        VmErrorKind::InlineCacheLayoutMismatch {
            required: 1,
            actual: 0
        }
    );
}

#[test]
fn linked_native_dispatch_uses_id_not_debug_name_fallback() {
    let mut vm = Vm::new();
    vm.register_native("legacy_name", |_| {
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(99)))
    });

    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let native_name = program.intern_debug_name("legacy_name");
    let native_id = FunctionId::new(0x55);
    let native = program.push_native_function(vela_bytecode::LinkedNativeFunction::new(
        native_id,
        native_name,
    ));
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 1);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallNative {
            dst: Some(Register(0)),
            native,
            debug_name: native_name,
            cache_site: None,
            args: Vec::new(),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(0) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);

    let error = vm
        .run_linked_program(&program, "main", &[])
        .expect_err("linked native dispatch must not use debug name fallback");

    assert_eq!(
        error.kind(),
        VmErrorKind::UnknownNative {
            name: "legacy_name".to_owned()
        }
    );
}

#[test]
fn linked_program_calls_native_by_dense_handle() {
    let native_id = FunctionId::new(0x56);
    let mut vm = Vm::new();
    vm.register_native_with_id(native_id, |_| {
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    });

    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let native_name = program.intern_debug_name("debug_only_name");
    let native = program.push_native_function(vela_bytecode::LinkedNativeFunction::new(
        native_id,
        native_name,
    ));
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 1);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallNative {
            dst: Some(Register(0)),
            native,
            debug_name: native_name,
            cache_site: None,
            args: Vec::new(),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(0) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);

    assert_eq!(
        vm.run_linked_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
}

#[test]
fn linked_program_calls_value_method_by_dispatch_handle() {
    let method_id = vela_stdlib::std_method_id("String", "len").expect("String::len method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("debug_only_name");
    let method = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 2);
    let value = code.push_constant(Constant::String("gold".into()));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(1),
            receiver: Register(0),
            dispatch: method,
            debug_name: method_name,
            cache_site: None,
            args: Vec::new(),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(1) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);

    assert_eq!(
        Vm::new().run_linked_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(4)))
    );
}

#[test]
fn linked_value_method_dispatch_uses_id_not_debug_name_fallback() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("len");
    let method = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value {
            method_id: MethodId::new(0x55),
        },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 2);
    let value = code.push_constant(Constant::String("gold".into()));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(1),
            receiver: Register(0),
            dispatch: method,
            debug_name: method_name,
            cache_site: None,
            args: Vec::new(),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(1) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);

    let error = Vm::new()
        .run_linked_program(&program, "main", &[])
        .expect_err("linked value method dispatch must not use debug name fallback");

    assert_eq!(
        error.kind(),
        VmErrorKind::UnknownMethod {
            method: "len".to_owned()
        }
    );
}

#[test]
fn linked_program_calls_script_method_by_dispatch_handle() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("debug_only_method");
    let receiver_name = program.intern_debug_name("self");

    let mut main = vela_bytecode::LinkedCodeObject::new(main_name, 2);
    let method_function = vela_bytecode::ScriptFunctionHandle::new(1);
    let method = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Script {
            method_id: MethodId::new(0x66),
            function: method_function,
        },
    ));
    let value = main.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(41)));
    main.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    main.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(1),
            receiver: Register(0),
            dispatch: method,
            debug_name: method_name,
            cache_site: None,
            args: Vec::new(),
        },
    ));
    main.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(1) },
    ));

    let mut method_code =
        vela_bytecode::LinkedCodeObject::new(method_name, 1).with_params(vec![receiver_name]);
    method_code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(0) },
    ));

    let main = program.push_function(main);
    let method_handle = program.push_function(method_code);
    assert_eq!(method_handle, method_function);
    program.set_entry_point(main_name, main);

    assert_eq!(
        Vm::new().run_linked_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(41)))
    );
}

#[test]
fn linked_program_calls_host_method_by_dispatch_handle() {
    let host_ref = player_ref(3);
    let method_id = HostMethodId::new(8);
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let player_name = program.intern_debug_name("player");
    let method_name = program.intern_debug_name("debug_only_host_method");
    let method = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Host { method_id },
    ));

    let mut code =
        vela_bytecode::LinkedCodeObject::new(main_name, 3).with_params(vec![player_name]);
    let amount = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(20)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(1),
            constant: amount,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(2),
            receiver: Register(0),
            dispatch: method,
            debug_name: method_name,
            cache_site: None,
            args: vec![vela_bytecode::CallArgument::Register(Register(1))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(2) },
    ));
    let main = program.push_function(code);
    program.set_entry_point(main_name, main);

    let mut adapter = host_adapter(
        host_ref,
        HostValue::Scalar(vela_common::ScalarValue::I64(9)),
    );
    adapter.insert_method_return(
        method_id,
        HostValue::Scalar(vela_common::ScalarValue::I64(12)),
    );
    let mut access = HostAccess::new();
    let mut budget = ExecutionBudget::unbounded();
    let result = {
        let mut host = HostExecution {
            adapter: &mut adapter,
            access: &mut access,
            script_globals: None,
        };
        let code = program.function(main).expect("main linked code exists");
        Vm::new().execute_linked_call(
            crate::linked_execution::LinkedExecutionCall {
                code,
                program: &program,
                captures: &[],
                args: &[Value::HostRef(host_ref)],
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: None,
                bytecode_profiler: None,
            },
            Some(&mut host),
            None,
            Some(&mut budget),
        )
    };

    assert_eq!(result, Ok(Value::Scalar(vela_common::ScalarValue::I64(12))));
    assert_eq!(
        adapter.method_calls(),
        &[(
            HostPath::new(host_ref),
            method_id,
            vec![HostValue::Scalar(vela_common::ScalarValue::I64(20))]
        )]
    );
}

#[test]
fn linked_program_calls_script_function_by_dense_handle() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let helper_name = program.intern_debug_name("helper");

    let mut main = vela_bytecode::LinkedCodeObject::new(main_name, 2);
    let helper = vela_bytecode::ScriptFunctionHandle::new(1);
    main.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallFunction {
            dst: Register(0),
            function: helper,
            debug_name: helper_name,
            mode: vela_bytecode::ScriptCallMode::Checked,
            args: Vec::new(),
        },
    ));
    main.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(0) },
    ));

    let mut helper_code = vela_bytecode::LinkedCodeObject::new(helper_name, 1);
    let value = helper_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(11)));
    helper_code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    helper_code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(0) },
    ));

    let main = program.push_function(main);
    let helper_handle = program.push_function(helper_code);
    assert_eq!(helper_handle, helper);
    program.set_entry_point(main_name, main);

    assert_eq!(
        Vm::new().run_linked_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(11)))
    );
}

#[test]
fn linked_program_executes_closure_creation_and_call() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let closure_name = program.intern_debug_name("main::<lambda>");
    let param_name = program.intern_debug_name("amount");

    let mut main = vela_bytecode::LinkedCodeObject::new(main_name, 4);
    let captured = main.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(4)));
    let amount = main.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(5)));
    let closure = vela_bytecode::ScriptFunctionHandle::new(1);
    main.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: captured,
        },
    ));
    main.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeClosure {
            dst: Register(1),
            function: closure,
            captures: vec![Register(0)],
        },
    ));
    main.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(2),
            constant: amount,
        },
    ));
    main.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallClosure {
            dst: Register(3),
            callee: Register(1),
            args: vec![Register(2)],
        },
    ));
    main.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(3) },
    ));

    let mut closure_code = vela_bytecode::LinkedCodeObject::new(closure_name, 3)
        .with_capture_count(1)
        .with_params(vec![param_name]);
    closure_code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Add {
            dst: Register(2),
            lhs: Register(0),
            rhs: Register(1),
        },
    ));
    closure_code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(2) },
    ));

    let main = program.push_function(main);
    let closure_handle = program.push_function(closure_code);
    assert_eq!(closure_handle, closure);
    program.set_entry_point(main_name, main);

    assert_eq!(
        Vm::new().run_linked_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(9)))
    );
}

#[test]
fn linked_program_executes_array_and_index_ops() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 5);
    let two = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));
    let four = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(4)));
    let index = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(1)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: two,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(1),
            constant: four,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeArray {
            dst: Register(2),
            elements: vec![Register(0), Register(1)],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(3),
            constant: index,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::GetIndex {
            dst: Register(4),
            base: Register(2),
            index: Register(3),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(4) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);

    assert_eq!(
        Vm::new().run_linked_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(4)))
    );
}

#[test]
fn linked_program_executes_record_slot_reads_and_writes() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let reward_name = program.intern_debug_name("Reward");
    let count_name = program.intern_debug_name("count");
    let item_name = program.intern_debug_name("item_id");
    let reward_type = program.push_type(vela_bytecode::LinkedType::new(
        vela_def::TypeId::new(0x77),
        reward_name,
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 3);
    let initial = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));
    let updated = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(5)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: initial,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeRecord {
            dst: Register(1),
            ty: reward_type,
            fields: vec![
                (vela_bytecode::FieldSlot::new(1), item_name, Register(0)),
                (vela_bytecode::FieldSlot::new(0), count_name, Register(0)),
            ],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: updated,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::SetRecordSlot {
            record: Register(1),
            field: vela_bytecode::FieldSlot::new(0),
            debug_name: count_name,
            cache_site: None,
            src: Register(0),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::GetRecordSlot {
            dst: Register(2),
            record: Register(1),
            field: vela_bytecode::FieldSlot::new(0),
            debug_name: count_name,
            cache_site: None,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(2) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);

    assert_eq!(
        Vm::new().run_linked_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(5)))
    );
}

#[test]
fn linked_record_slot_reads_and_writes_populate_inline_caches() {
    let (program, write_site, read_site, reward_type_id) = linked_record_cache_program();
    let caches = RecordingRecordFieldCaches::new(2);

    assert_eq!(
        run_linked_record_cache_program(&program, &caches),
        Ok(Value::Scalar(vela_common::ScalarValue::I64(5)))
    );
    assert_eq!(caches.set_count(), 2);
    assert_eq!(
        caches.entry(write_site).map(|entry| entry.type_id),
        Some(reward_type_id)
    );
    assert_eq!(
        caches.entry(read_site).map(|entry| entry.type_id),
        Some(reward_type_id)
    );

    assert_eq!(
        run_linked_record_cache_program(&program, &caches),
        Ok(Value::Scalar(vela_common::ScalarValue::I64(5)))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_record_slot_inline_cache_miss_replaces_wrong_guards() {
    let (program, write_site, read_site, reward_type_id) = linked_record_cache_program();
    let caches = RecordingRecordFieldCaches::new(2);
    let wrong_entry = RecordFieldInlineCacheEntry {
        type_id: vela_def::TypeId::new(0xdead),
        shape_id: vela_common::ShapeId::new(0xbeef),
        field: vela_bytecode::FieldSlot::new(0),
    };
    caches.prime(write_site, wrong_entry);
    caches.prime(read_site, wrong_entry);

    assert_eq!(
        run_linked_record_cache_program(&program, &caches),
        Ok(Value::Scalar(vela_common::ScalarValue::I64(5)))
    );
    assert_eq!(caches.set_count(), 2);
    assert_eq!(
        caches.entry(write_site).map(|entry| entry.type_id),
        Some(reward_type_id)
    );
    assert_eq!(
        caches.entry(read_site).map(|entry| entry.type_id),
        Some(reward_type_id)
    );
}

#[test]
fn linked_record_slot_inline_cache_miss_replaces_wrong_slot() {
    let (program, write_site, read_site, _) = linked_record_cache_program();
    let caches = RecordingRecordFieldCaches::new(2);

    assert_eq!(
        run_linked_record_cache_program(&program, &caches),
        Ok(Value::Scalar(vela_common::ScalarValue::I64(5)))
    );
    let write_entry = caches
        .entry(write_site)
        .expect("write cache should be populated");
    let read_entry = caches
        .entry(read_site)
        .expect("read cache should be populated");
    caches.prime(
        write_site,
        RecordFieldInlineCacheEntry {
            field: vela_bytecode::FieldSlot::new(1),
            ..write_entry
        },
    );
    caches.prime(
        read_site,
        RecordFieldInlineCacheEntry {
            field: vela_bytecode::FieldSlot::new(1),
            ..read_entry
        },
    );

    assert_eq!(
        run_linked_record_cache_program(&program, &caches),
        Ok(Value::Scalar(vela_common::ScalarValue::I64(5)))
    );
    assert_eq!(caches.set_count(), 4);
    assert_eq!(
        caches.entry(write_site).map(|entry| entry.field),
        Some(vela_bytecode::FieldSlot::new(0))
    );
    assert_eq!(
        caches.entry(read_site).map(|entry| entry.field),
        Some(vela_bytecode::FieldSlot::new(0))
    );
}

fn linked_record_cache_program() -> (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    CacheSiteId,
    vela_def::TypeId,
) {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let reward_name = program.intern_debug_name("Reward");
    let count_name = program.intern_debug_name("count");
    let item_name = program.intern_debug_name("item_id");
    let reward_type_id = vela_def::TypeId::new(0x177);
    let reward_type =
        program.push_type(vela_bytecode::LinkedType::new(reward_type_id, reward_name));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 3);
    let initial = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));
    let updated = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(5)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: initial,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeRecord {
            dst: Register(1),
            ty: reward_type,
            fields: vec![
                (vela_bytecode::FieldSlot::new(1), item_name, Register(0)),
                (vela_bytecode::FieldSlot::new(0), count_name, Register(0)),
            ],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: updated,
        },
    ));
    let write_site = code.push_cache_site(CacheSiteKind::RecordFieldWrite, InstructionOffset(3));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::SetRecordSlot {
            record: Register(1),
            field: vela_bytecode::FieldSlot::new(0),
            debug_name: count_name,
            cache_site: Some(write_site),
            src: Register(0),
        },
    ));
    let read_site = code.push_cache_site(CacheSiteKind::RecordFieldRead, InstructionOffset(4));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::GetRecordSlot {
            dst: Register(2),
            record: Register(1),
            field: vela_bytecode::FieldSlot::new(0),
            debug_name: count_name,
            cache_site: Some(read_site),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(2) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, write_site, read_site, reward_type_id)
}

fn run_linked_record_cache_program(
    program: &vela_bytecode::LinkedProgram,
    caches: &RecordingRecordFieldCaches,
) -> VmResult<Value> {
    let code = program
        .functions()
        .find(|(_, code)| program.debug_name(code.debug_name) == "main")
        .map(|(_, code)| code)
        .expect("linked record cache fixture should have main");
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::unbounded();
    Vm::new().execute_linked_call(
        crate::linked_execution::LinkedExecutionCall {
            code,
            program,
            captures: &[],
            args: &[],
            check_param_guards: true,
            call_site: None,
            call_site_offset: None,
            inline_caches: Some(caches),
            bytecode_profiler: None,
        },
        None,
        Some(&mut heap_execution),
        Some(&mut budget),
    )
}

struct RecordingRecordFieldCaches {
    entries: RefCell<Vec<Option<RecordFieldInlineCacheEntry>>>,
    set_count: Cell<usize>,
}

impl RecordingRecordFieldCaches {
    fn new(len: usize) -> Self {
        Self {
            entries: RefCell::new(vec![None; len]),
            set_count: Cell::new(0),
        }
    }

    fn entry(&self, site: CacheSiteId) -> Option<RecordFieldInlineCacheEntry> {
        self.entries.borrow().get(site.index()).copied().flatten()
    }

    fn prime(&self, site: CacheSiteId, entry: RecordFieldInlineCacheEntry) {
        self.entries.borrow_mut()[site.index()] = Some(entry);
    }

    fn set_count(&self) -> usize {
        self.set_count.get()
    }
}

impl VmInlineCaches for RecordingRecordFieldCaches {
    fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    fn record_field(&self, site: CacheSiteId) -> Option<RecordFieldInlineCacheEntry> {
        self.entry(site)
    }

    fn set_record_field(&self, site: CacheSiteId, entry: RecordFieldInlineCacheEntry) {
        self.entries.borrow_mut()[site.index()] = Some(entry);
        self.set_count.set(self.set_count.get() + 1);
    }
}

#[test]
fn linked_record_construction_stores_type_and_shape_identity() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let reward_name = program.intern_debug_name("Reward");
    let count_name = program.intern_debug_name("count");
    let reward_type_id = vela_def::TypeId::new(0x177);
    let reward_type =
        program.push_type(vela_bytecode::LinkedType::new(reward_type_id, reward_name));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 2);
    let initial = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(3)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: initial,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeRecord {
            dst: Register(1),
            ty: reward_type,
            fields: vec![(vela_bytecode::FieldSlot::new(0), count_name, Register(0))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(1) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    program
        .verify()
        .expect("linked record identity fixture should verify");

    let code = program
        .function(function)
        .expect("linked function should exist");
    let mut heap = ScriptHeap::new();
    let mut heap = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::unbounded();
    let result = Vm::new()
        .execute_linked_call(
            crate::linked_execution::LinkedExecutionCall {
                code,
                program: &program,
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
        .expect("linked record construction should run");

    let RuntimeValue::HeapRef(record) = result else {
        panic!("expected record heap ref");
    };
    let Some(HeapValue::Record {
        type_name,
        identity: Some(identity),
        fields,
    }) = heap.heap.get(record)
    else {
        panic!("expected typed record heap value");
    };
    assert_eq!(type_name, "Reward");
    assert_eq!(identity.type_id, reward_type_id);
    assert_eq!(identity.shape_id, fields.shape_id());
    assert_eq!(fields.get_slot(0, "count"), Some(&RuntimeValue::i64(3)));
}

#[test]
fn linked_program_executes_enum_slot_reads_and_tag_checks() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let damage_name = program.intern_debug_name("Damage");
    let physical_name = program.intern_debug_name("Damage::Physical");
    let amount_name = program.intern_debug_name("amount");
    let damage_type = program.push_type(vela_bytecode::LinkedType::new(
        vela_def::TypeId::new(0x88),
        damage_name,
    ));
    let physical_variant = program.push_variant(vela_bytecode::LinkedVariant::new(
        vela_def::VariantId::new(0x89),
        damage_type,
        physical_name,
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 5);
    let amount = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(7)));
    let zero = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(0)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: amount,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeEnum {
            dst: Register(1),
            enum_ty: damage_type,
            variant: physical_variant,
            fields: vec![(vela_bytecode::FieldSlot::new(0), amount_name, Register(0))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::GetEnumSlot {
            dst: Register(2),
            value: Register(1),
            field: vela_bytecode::FieldSlot::new(0),
            debug_name: amount_name,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::EnumTagEqual {
            dst: Register(3),
            value: Register(1),
            enum_ty: damage_type,
            variant: physical_variant,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::JumpIfFalse {
            condition: Register(3),
            target: InstructionOffset(6),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(2) },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(4),
            constant: zero,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(4) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);

    assert_eq!(
        Vm::new().run_linked_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
}

#[test]
fn linked_enum_tag_checks_use_ids_not_debug_names() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let damage_name = program.intern_debug_name("Damage");
    let physical_name = program.intern_debug_name("Damage::Physical");
    let renamed_physical_name = program.intern_debug_name("Damage::RenamedPhysical");
    let amount_name = program.intern_debug_name("amount");
    let damage_type = program.push_type(vela_bytecode::LinkedType::new(
        vela_def::TypeId::new(0x98),
        damage_name,
    ));
    let physical_variant = program.push_variant(vela_bytecode::LinkedVariant::new(
        vela_def::VariantId::new(0x99),
        damage_type,
        physical_name,
    ));
    let renamed_physical_variant = program.push_variant(vela_bytecode::LinkedVariant::new(
        vela_def::VariantId::new(0x99),
        damage_type,
        renamed_physical_name,
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 5);
    let amount = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(7)));
    let zero = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(0)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: amount,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeEnum {
            dst: Register(1),
            enum_ty: damage_type,
            variant: physical_variant,
            fields: vec![(vela_bytecode::FieldSlot::new(0), amount_name, Register(0))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::EnumTagEqual {
            dst: Register(2),
            value: Register(1),
            enum_ty: damage_type,
            variant: renamed_physical_variant,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::JumpIfFalse {
            condition: Register(2),
            target: InstructionOffset(6),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::GetEnumSlot {
            dst: Register(3),
            value: Register(1),
            field: vela_bytecode::FieldSlot::new(0),
            debug_name: amount_name,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(3) },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(4),
            constant: zero,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(4) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);

    assert_eq!(
        Vm::new().run_linked_program(&program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(7)))
    );
}
