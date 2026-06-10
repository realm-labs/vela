use super::*;
use crate::owned_value::OwnedValue;
use crate::value::Value as RuntimeValue;

#[test]
fn runs_basic_arithmetic() {
    let mut code = UnlinkedCodeObject::new("calc", 5);
    let two = code.push_constant(Constant::Int(2));
    let three = code.push_constant(Constant::Int(3));
    let four = code.push_constant(Constant::Int(4));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: two,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: three,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(2),
            constant: four,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Mul {
        dst: Register(3),
        lhs: Register(1),
        rhs: Register(2),
    }));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Add {
        dst: Register(4),
        lhs: Register(0),
        rhs: Register(3),
    }));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(4),
    }));

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Int(14)));
}

#[test]
fn linker_rejects_script_function_id_debug_name_mismatch() {
    let mut helper = UnlinkedCodeObject::new("helper", 1);
    let value = helper.push_constant(Constant::Int(7));
    helper.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    helper.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(0),
    }));

    let mut main = UnlinkedCodeObject::new("main", 1);
    main.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallFunction {
            dst: Register(0),
            target: FunctionId::new(0xDEAD),
            name: "helper".to_owned(),
            args: Vec::new(),
        },
    ));
    main.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(0),
    }));

    let mut program = UnlinkedProgram::new();
    program.insert_function(helper);
    program.insert_function(main);

    let error = Linker::new()
        .link_program(&program)
        .expect_err("matching debug name must not rescue wrong FunctionId");

    assert!(matches!(
        error,
        vela_bytecode::LinkError::MissingScriptFunction { name, id }
            if name == "helper" && id == FunctionId::new(0xDEAD)
    ));
}

#[test]
fn runs_linked_program_basic_arithmetic_without_unlinked_code() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 5);
    let two = code.push_constant(Constant::Int(2));
    let three = code.push_constant(Constant::Int(3));
    let four = code.push_constant(Constant::Int(4));
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
        Ok(OwnedValue::Int(14))
    );
}

#[test]
fn linked_native_dispatch_uses_id_not_debug_name_fallback() {
    let mut vm = Vm::new();
    vm.register_native("legacy_name", |_| Ok(OwnedValue::Int(99)));

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
    vm.register_native_with_id(native_id, "actual_name", |_| Ok(OwnedValue::Int(7)));

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
        Ok(OwnedValue::Int(7))
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
        Ok(OwnedValue::Int(4))
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
    let value = main.push_constant(Constant::Int(41));
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
        Ok(OwnedValue::Int(41))
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
    let amount = code.push_constant(Constant::Int(20));
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
            args: vec![vela_bytecode::CallArgument::Register(Register(1))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(2) },
    ));
    let main = program.push_function(code);
    program.set_entry_point(main_name, main);

    let mut adapter = host_adapter(host_ref, HostValue::Int(9));
    adapter.insert_method_return(method_id, HostValue::Int(12));
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
                call_site: None,
                call_site_offset: None,
                inline_caches: None,
            },
            Some(&mut host),
            None,
            Some(&mut budget),
        )
    };

    assert_eq!(result, Ok(Value::Int(12)));
    assert_eq!(
        adapter.method_calls(),
        &[(HostPath::new(host_ref), method_id, vec![HostValue::Int(20)])]
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
            args: Vec::new(),
        },
    ));
    main.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(0) },
    ));

    let mut helper_code = vela_bytecode::LinkedCodeObject::new(helper_name, 1);
    let value = helper_code.push_constant(Constant::Int(11));
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
        Ok(OwnedValue::Int(11))
    );
}

#[test]
fn linked_program_executes_closure_creation_and_call() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let closure_name = program.intern_debug_name("main::<lambda>");
    let param_name = program.intern_debug_name("amount");

    let mut main = vela_bytecode::LinkedCodeObject::new(main_name, 4);
    let captured = main.push_constant(Constant::Int(4));
    let amount = main.push_constant(Constant::Int(5));
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
        Ok(OwnedValue::Int(9))
    );
}

#[test]
fn linked_program_executes_array_and_index_ops() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 5);
    let two = code.push_constant(Constant::Int(2));
    let four = code.push_constant(Constant::Int(4));
    let index = code.push_constant(Constant::Int(1));
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
        Ok(OwnedValue::Int(4))
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
    let initial = code.push_constant(Constant::Int(2));
    let updated = code.push_constant(Constant::Int(5));
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
            src: Register(0),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::GetRecordSlot {
            dst: Register(2),
            record: Register(1),
            field: vela_bytecode::FieldSlot::new(0),
            debug_name: count_name,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(2) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);

    assert_eq!(
        Vm::new().run_linked_program(&program, "main", &[]),
        Ok(OwnedValue::Int(5))
    );
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
    let amount = code.push_constant(Constant::Int(7));
    let zero = code.push_constant(Constant::Int(0));
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
        Ok(OwnedValue::Int(7))
    );
}

#[test]
fn branches_on_false_conditions() {
    let mut code = UnlinkedCodeObject::new("branch", 3);
    let false_id = code.push_constant(Constant::Bool(false));
    let one = code.push_constant(Constant::Int(1));
    let two = code.push_constant(Constant::Int(2));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: false_id,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::JumpIfFalse {
            condition: Register(0),
            target: InstructionOffset(4),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: one,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Jump {
        target: InstructionOffset(5),
    }));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: two,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Int(2)));
}

#[test]
fn linked_program_execution_charges_instruction_budget() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 1);
    let value = code.push_constant(Constant::Int(1));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(0) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    let mut budget = ExecutionBudget::new(1, usize::MAX, usize::MAX);

    let error = Vm::new()
        .run_linked_program_with_budget(&program, "main", &[], &mut budget)
        .expect_err("second instruction should exceed the budget");

    assert_eq!(
        error.kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::Instructions,
            limit: 1,
        }
    );
}

#[test]
fn calls_registered_native_functions() {
    let mut vm = Vm::new();
    let native_id = function_id_for_native_name("log");
    vm.register_native("log", |args| {
        assert_eq!(args, [OwnedValue::String("level up".into())]);
        Ok(OwnedValue::Null)
    });

    let mut code = UnlinkedCodeObject::new("native", 2);
    code.push_constant(Constant::String("level up".into()));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: ConstantId(0),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: Some(Register(1)),
            name: "log".into(),
            native: native_id,
            args: vec![Register(0)],
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));

    assert_eq!(
        run_linked_test_code_with_linker(
            &vm,
            code,
            Linker::new().with_native_implementation(native_id)
        ),
        Ok(OwnedValue::Null)
    );
}

#[test]
fn instruction_budget_stops_dispatch_before_next_instruction() {
    let mut code = UnlinkedCodeObject::new("budgeted", 2);
    let one = code.push_constant(Constant::Int(1));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: one,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Move {
        dst: Register(1),
        src: Register(0),
    }));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));
    let mut budget = ExecutionBudget::new(2, usize::MAX, usize::MAX);

    let error = run_linked_test_code_with_budget(code, &mut budget)
        .expect_err("third instruction exceeds budget");

    assert_eq!(
        error.kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::Instructions,
            limit: 2,
        }
    );
    assert_eq!(budget.instructions_executed(), 2);
    assert_eq!(budget.current_call_depth(), 0);
}

#[test]
fn call_depth_budget_stops_recursive_scripts() {
    let program = compile_program_source(
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
    .expect("compile recursive source");
    let linked = link_test_program(&program);
    let mut budget = ExecutionBudget::new(100, usize::MAX, 2);

    let error = Vm::new()
        .run_linked_program_with_budget(&linked, "main", &[], &mut budget)
        .expect_err("recursive call exceeds call depth");

    assert_eq!(
        error.kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::CallDepth,
            limit: 2,
        }
    );
    assert_eq!(budget.current_call_depth(), 0);
}

#[test]
fn call_frame_registers_expose_heap_roots_for_gc() {
    let mut heap = ScriptHeap::new();
    let rooted = heap.allocate(HeapValue::String("rooted".into()));
    let garbage = heap.allocate(HeapValue::String("garbage".into()));
    let mut frame = CallFrame::new(2);
    frame
        .write(Register(0), RuntimeValue::HeapRef(rooted))
        .expect("write heap root");

    let roots = frame.heap_roots();
    let root_slots = frame.heap_root_slots();
    let stats = heap.collect_full(&roots);

    assert_eq!(roots, vec![rooted]);
    assert_eq!(root_slots.len(), 1);
    assert_eq!(root_slots[0].register, Register(0));
    assert_eq!(root_slots[0].reference, rooted);
    assert_eq!(stats.marked, 1);
    assert_eq!(stats.swept, 1);
    assert!(heap.contains(rooted));
    assert!(!heap.contains(garbage));
}

#[test]
fn nested_values_expose_heap_roots_for_gc() {
    let mut heap = ScriptHeap::new();
    let rooted = heap.allocate(HeapValue::String("nested".into()));
    let garbage = heap.allocate(HeapValue::String("garbage".into()));
    let mut fields = BTreeMap::new();
    fields.insert("item".into(), RuntimeValue::HeapRef(rooted));
    let record = heap.allocate(HeapValue::Record {
        type_name: "Reward".into(),
        fields: ScriptFields::from_pairs("Reward", fields),
    });
    let mut frame = CallFrame::new(1);
    frame
        .write(Register(0), RuntimeValue::HeapRef(record))
        .expect("write nested root");

    let stats = heap.collect_full(&frame.heap_roots());

    assert_eq!(stats.marked, 2);
    assert_eq!(stats.swept, 1);
    assert!(heap.contains(rooted));
    assert!(!heap.contains(garbage));
}

#[test]
fn record_slot_bytecode_reads_and_writes_by_slot() {
    let mut code = UnlinkedCodeObject::new("slot_record", 3);
    let count = code.push_constant(Constant::Int(2));
    let updated = code.push_constant(Constant::Int(5));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: count,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeRecord {
            dst: Register(1),
            type_name: "Reward".into(),
            fields: vec![
                ("item_id".into(), Register(0)),
                ("count".into(), Register(0)),
            ],
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: updated,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::SetRecordSlot {
            record: Register(1),
            field: "count".into(),
            slot: 0,
            src: Register(0),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::GetRecordSlot {
            dst: Register(2),
            record: Register(1),
            field: "count".into(),
            slot: 0,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(2),
    }));

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Int(5)));
}

#[test]
fn enum_slot_bytecode_reads_by_slot() {
    let mut code = UnlinkedCodeObject::new("slot_enum", 3);
    let amount = code.push_constant(Constant::Int(7));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: amount,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeEnum {
            dst: Register(1),
            enum_name: "Damage".into(),
            variant: "Physical".into(),
            fields: vec![("amount".into(), Register(0))],
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::GetEnumSlot {
            dst: Register(2),
            value: Register(1),
            field: "amount".into(),
            slot: 0,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(2),
    }));

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Int(7)));
}

#[test]
fn runs_compiled_arithmetic_source() {
    let code = compile_function_source(
        SourceId::new(1),
        "fn main() { let base = 2; return base + 3 * 4; }",
        "main",
    )
    .expect("compile arithmetic source");

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Int(14)));
}

#[test]
fn runs_compiled_radix_ints_and_exponent_floats() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let base = 0x10 + 0b10;
    let scaled = 3.5e+1 / 2.5;
    if base == 18 && scaled == 14.0 {
        return scaled;
    }
    return 0.0;
}
"#,
        "main",
    )
    .expect("compile numeric literal source");

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Float(14.0)));
}

#[test]
fn runs_compiled_large_int_comparisons_without_float_rounding() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let low = 9007199254740992;
    let high = 9007199254740993;
    if low < high && high > low && low <= high && high >= low {
        return 1;
    }
    return 0;
}
"#,
        "main",
    )
    .expect("compile large int comparison source");

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Int(1)));
}

#[test]
fn runs_compiled_scalar_equality_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    if "tick" == "tick"
        && "tick" != "tock"
        && true == true
        && false != true
        && 7 == 7
        && 7 != 8
        && 7 != 7.0
        && null == null
        && null != false
    {
        return 1;
    }
    return 0;
}
"#,
        "main",
    )
    .expect("compile scalar equality source");

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Int(1)));
}

#[test]
fn runs_compiled_shebang_source() {
    let code = compile_function_source(
        SourceId::new(1),
        "#!/usr/bin/env vela\nfn main() { return 7; }\n",
        "main",
    )
    .expect("compile shebang source");

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Int(7)));
}

#[test]
fn runs_compiled_unicode_string_escapes() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"fn main() { return "\u{41}\u{7a}"; }"#,
        "main",
    )
    .expect("compile unicode escaped string source");

    assert_eq!(
        run_linked_test_code(code),
        Ok(OwnedValue::String("Az".into()))
    );
}

#[test]
fn runs_compiled_unary_operator_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    if !false {
        return -5;
    }
    return 0;
}
"#,
        "main",
    )
    .expect("compile unary operator source");

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Int(-5)));
}

#[test]
fn runs_compiled_logical_short_circuit_source() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn and_case() {
    return false && fail();
}

fn or_case() {
    return true || fail();
}

fn truthy_case() {
    return true && 5 && ("reward" || fail());
}

fn fail() {
    return false;
}
"#,
    )
    .expect("compile logical short-circuit source");
    let linked = link_test_program(&program);

    assert_eq!(
        Vm::new().run_linked_program(&linked, "and_case", &[]),
        Ok(OwnedValue::Bool(false))
    );
    assert_eq!(
        Vm::new().run_linked_program(&linked, "or_case", &[]),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        Vm::new().run_linked_program(&linked, "truthy_case", &[]),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn runs_long_compiled_logical_chains_without_recursive_lowering() {
    let and_chain = std::iter::repeat_n("true", 160)
        .collect::<Vec<_>>()
        .join(" && ");
    let or_chain = std::iter::once("false")
        .chain(std::iter::repeat_n("false", 158))
        .chain(std::iter::once("true"))
        .collect::<Vec<_>>()
        .join(" || ");
    let source = format!(
        r#"
fn and_case() {{
    return {and_chain};
}}

fn or_case() {{
    return {or_chain};
}}
"#
    );
    let program =
        compile_program_source(SourceId::new(1), &source).expect("compile long logical chains");
    let linked = link_test_program(&program);

    assert_eq!(
        Vm::new().run_linked_program(&linked, "and_case", &[]),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        Vm::new().run_linked_program(&linked, "or_case", &[]),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn runs_compiled_local_assignment_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = 1;
    value += 4;
    value *= 3;
    value -= 5;
    value /= 2;
    value %= 5;
    let copy = (value = value + 10);
    return value + copy;
}
"#,
        "main",
    )
    .expect("compile local assignment source");

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Int(20)));
}

#[test]
fn runs_compiled_index_read_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let values = [2, 4, 8];
    let rewards = { "xp": 6 };
    return values[1] + rewards["xp"];
}
"#,
        "main",
    )
    .expect("compile index read source");

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Int(10)));
}

#[test]
fn managed_heap_execution_reads_heap_index_values() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn array_case() {
    let names = ["gold", "xp"];
    return names[1];
}

fn map_case() {
    let rewards = { "gold": 7 };
    return rewards["gold"];
}
"#,
    )
    .expect("compile heap index source");
    let linked = link_test_program(&program);
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        Vm::new()
            .run_linked_program_with_budget(&linked, "array_case", &[], &mut budget)
            .expect("run heap array index"),
        OwnedValue::String("xp".into())
    );
    assert_eq!(
        Vm::new()
            .run_linked_program_with_budget(&linked, "map_case", &[], &mut budget)
            .expect("run heap map index"),
        OwnedValue::Int(7)
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn runs_compiled_index_write_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let values = [2, 4, 8];
    let rewards = { "xp": 6 };
    values[1] = 10;
    values[2] += 5;
    rewards["xp"] += values[1];
    rewards["gold"] = 3;
    let copy = (values[0] = rewards["gold"]);
    return values[0] + values[1] + values[2] + rewards["xp"] + copy;
}
"#,
        "main",
    )
    .expect("compile index write source");

    assert_eq!(run_linked_test_code(code), Ok(OwnedValue::Int(45)));
}

#[test]
fn runs_compiled_record_field_write_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let reward = Reward { item_id: "gold", count: 2 };
    reward.count += 3;
    reward.item_id = "xp";
    return reward.count + reward.item_id.len();
}
"#,
        "main",
    )
    .expect("compile record field write source");

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(7)));
}

#[test]
fn runs_compiled_nested_record_field_write_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let player = Player {
        stats: Stats {
            level: 2,
            exp: 5,
        },
    };
    player.stats.level += 3;
    player.stats.exp = player.stats.level + 1;
    return player.stats.level + player.stats.exp;
}
"#,
        "main",
    )
    .expect("compile nested record field write source");

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(11)));
}

#[test]
fn runs_compiled_indexed_record_field_write_source() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let players = [
        Player { level: 2, exp: 5 },
        Player { level: 7, exp: 1 },
    ];
    players[0].level += 3;
    players[1].exp = players[0].level + 4;
    return players[0].level + players[1].exp;
}
"#,
        "main",
    )
    .expect("compile indexed record field write source");

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(14)));
}

#[test]
fn managed_heap_execution_writes_heap_index_values() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn array_case() {
    let names = ["gold", "xp"];
    names[0] = "silver";
    return names[0];
}

fn map_case() {
    let rewards = { "gold": 7 };
    rewards["gold"] += 5;
    rewards["xp"] = 3;
    return rewards["gold"] + rewards["xp"];
}
"#,
    )
    .expect("compile heap index write source");
    let linked = link_test_program(&program);
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        Vm::new()
            .run_linked_program_with_budget(&linked, "array_case", &[], &mut budget)
            .expect("run heap array index write"),
        OwnedValue::String("silver".into())
    );
    assert_eq!(
        Vm::new()
            .run_linked_program_with_budget(&linked, "map_case", &[], &mut budget)
            .expect("run heap map index write"),
        OwnedValue::Int(15)
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}

#[test]
fn managed_heap_execution_writes_heap_record_fields() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let reward = Reward { item_id: "gold", count: 2 };
    reward.count += 5;
    reward.item_id = "xp";
    return reward.count + reward.item_id.len();
}
"#,
    )
    .expect("compile heap record field writes");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        Vm::new().run_program_with_managed_heap_and_budget(&program, "main", &[], &mut budget),
        Ok(OwnedValue::Int(9))
    );
    assert_eq!(budget.memory_bytes_allocated(), 0);
}
