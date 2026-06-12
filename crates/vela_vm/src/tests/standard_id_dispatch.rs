use super::*;
use crate::owned_value::OwnedValue;
use vela_stdlib_runtime::{StdFunctionImplementation, stdlib_function_runtime_bindings};

fn std_function_id(implementation: StdFunctionImplementation) -> vela_def::FunctionId {
    for binding in stdlib_function_runtime_bindings() {
        if binding.implementation == implementation {
            return binding.id;
        }
    }
    panic!("missing standard function runtime binding for {implementation:?}");
}

pub(super) fn std_method_id(owner: &str, name: &str) -> vela_def::MethodId {
    let Some(id) = vela_stdlib::std_method_id(owner, name) else {
        panic!("missing standard method identity for {owner}::{name}");
    };
    id
}

pub(super) fn run_linked_standard_id_code(
    vm: &Vm,
    code: UnlinkedCodeObject,
) -> VmResult<OwnedValue> {
    let entry = code.name.clone();
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    let mut linker = Linker::new();
    vm.native_ids
        .keys()
        .chain(vm.host_native_ids.keys())
        .copied()
        .for_each(|id| linker.add_native_implementation(id));
    let linked = linker
        .link_program(&program)
        .expect("standard id dispatch test program should link");
    vm.run_linked_program(&linked, &entry, &[])
}

fn run_linked_standard_id_code_with_host(
    vm: &Vm,
    code: UnlinkedCodeObject,
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let entry = code.name.clone();
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    let mut linker = Linker::new();
    vm.native_ids
        .keys()
        .chain(vm.host_native_ids.keys())
        .copied()
        .for_each(|id| linker.add_native_implementation(id));
    let linked = linker
        .link_program(&program)
        .expect("standard host id dispatch test program should link");
    let mut budget = ExecutionBudget::unbounded();
    vm.run_linked_program_with_host_budget_and_caches(&linked, &entry, &[], host, &mut budget, None)
}

#[test]
fn call_native_uses_resolved_id_even_when_debug_name_differs() {
    let native_id = vela_def::FunctionId::new(77);
    let mut vm = Vm::new();
    vm.register_native("diagnostic_name", |_| {
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    });
    vm.register_native_with_id(native_id, |_| {
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    });

    let mut code = UnlinkedCodeObject::new("native_id", 1);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: Some(Register(0)),
            name: "diagnostic_name".into(),
            native: native_id,
            cache_site: None,
            args: Vec::new(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(0),
    }));
    assert_eq!(
        run_linked_standard_id_code(&vm, code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn call_native_uses_resolved_host_id_even_when_debug_name_differs() {
    let native_id = FunctionId::new(78);
    let mut vm = Vm::new();
    vm.register_native("diagnostic_name", |_| {
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    });
    vm.register_host_native_with_id(native_id, |_, _| {
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(3)))
    });

    let mut code = UnlinkedCodeObject::new("host_native_id", 1);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: Some(Register(0)),
            name: "diagnostic_name".into(),
            native: native_id,
            cache_site: None,
            args: Vec::new(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(0),
    }));

    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };
    assert_eq!(
        run_linked_standard_id_code_with_host(&vm, code, &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(3)))
    );
}

#[test]
fn call_native_uses_standard_native_id_even_when_debug_name_differs() {
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let mut code = UnlinkedCodeObject::new("standard_native_id", 2);
    let value = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(-4)));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: Some(Register(1)),
            name: "missing::abs".into(),
            native: std_function_id(StdFunctionImplementation::MathAbs),
            cache_site: None,
            args: vec![Register(0)],
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));
    assert_eq!(
        run_linked_standard_id_code(&vm, code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(4)))
    );
}

#[test]
fn call_method_uses_standard_callback_method_id_before_debug_name() {
    let mut program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let mapped = [1, 2, 3].map(|value| value + 1);
    return mapped[0] + mapped[2];
}
"#,
    )
    .expect("standard callback method source should compile");
    replace_call_method_debug_name(
        &mut program,
        "main",
        std_method_id("Array", "map"),
        "missing_array_map",
    );

    let mut budget = ExecutionBudget::unbounded();
    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(6)))
    );
}

fn replace_call_method_debug_name(
    program: &mut UnlinkedProgram,
    function: &str,
    expected_method: MethodId,
    replacement: &str,
) {
    let code = program
        .function_mut(function)
        .expect("test function should exist");
    for instruction in &mut code.instructions {
        if let UnlinkedInstructionKind::CallMethodId {
            method, method_id, ..
        } = &mut instruction.kind
            && *method_id == expected_method
        {
            *method = replacement.to_owned();
            return;
        }
    }
    panic!("test method call should exist");
}

#[test]
fn call_method_uses_standard_value_method_id_before_name_fallback() {
    let mut code = UnlinkedCodeObject::new("standard_value_method_id", 2);
    let value = code.push_constant(Constant::String("gold".into()));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(1),
            receiver: Register(0),
            method: "missing_len".into(),
            method_id: std_method_id("String", "len"),
            args: Vec::new(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));

    assert_eq!(
        run_linked_standard_id_code(&Vm::new(), code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(4)))
    );
}

#[test]
fn call_method_uses_standard_range_method_id_before_name_fallback() {
    let mut code = UnlinkedCodeObject::new("standard_range_method_id", 4);
    let start = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));
    let end = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(5)));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: start,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: end,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeRange {
            dst: Register(2),
            start: Register(0),
            end: Register(1),
            inclusive: false,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(3),
            receiver: Register(2),
            method: "missing_len".into(),
            method_id: std_method_id("Range", "len"),
            args: Vec::new(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(3),
    }));

    assert_eq!(
        run_linked_standard_id_code(&Vm::new(), code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(3)))
    );
}

#[test]
fn call_method_uses_standard_map_method_id_before_name_fallback() {
    let mut code = UnlinkedCodeObject::new("standard_map_method_id", 3);
    let value = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(6)));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::MakeMap {
        dst: Register(1),
        entries: vec![("xp".into(), Register(0))],
    }));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(2),
            receiver: Register(1),
            method: "missing_is_empty".into(),
            method_id: std_method_id("Map", "is_empty"),
            args: Vec::new(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(2),
    }));

    assert_eq!(
        run_linked_standard_id_code(&Vm::new(), code),
        Ok(OwnedValue::Bool(false))
    );

    let mut get_or_code = UnlinkedCodeObject::new("standard_map_get_or_method_id", 5);
    let value = get_or_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(6)));
    let key = get_or_code.push_constant(Constant::String("xp".into()));
    let default = get_or_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(0)));
    get_or_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    get_or_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: key,
        },
    ));
    get_or_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(2),
            constant: default,
        },
    ));
    get_or_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::MakeMap {
        dst: Register(3),
        entries: vec![("xp".into(), Register(0))],
    }));
    get_or_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(4),
            receiver: Register(3),
            method: "missing_get_or".into(),
            method_id: std_method_id("Map", "get_or"),
            args: vec![
                vela_bytecode::CallArgument::Register(Register(1)),
                vela_bytecode::CallArgument::Register(Register(2)),
            ],
        },
    ));
    get_or_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(4),
    }));

    assert_eq!(
        run_linked_standard_id_code(&Vm::new(), get_or_code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(6)))
    );
}

#[test]
fn call_method_uses_standard_map_mutator_ids_before_name_fallback() {
    let mut set_code = UnlinkedCodeObject::new("standard_map_set_method_id", 6);
    let key = set_code.push_constant(Constant::String("xp".into()));
    let value = set_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(6)));
    set_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: key,
        },
    ));
    set_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: value,
        },
    ));
    set_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::MakeMap {
        dst: Register(2),
        entries: Vec::new(),
    }));
    set_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(3),
            receiver: Register(2),
            method: "missing_set".into(),
            method_id: std_method_id("Map", "set"),
            args: vec![
                vela_bytecode::CallArgument::Register(Register(0)),
                vela_bytecode::CallArgument::Register(Register(1)),
            ],
        },
    ));
    set_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(4),
            receiver: Register(2),
            method: "missing_has".into(),
            method_id: std_method_id("Map", "has"),
            args: vec![vela_bytecode::CallArgument::Register(Register(0))],
        },
    ));
    set_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(4),
    }));
    assert_eq!(
        run_linked_standard_id_code(&Vm::new(), set_code),
        Ok(OwnedValue::Bool(true))
    );

    let mut remove_code = UnlinkedCodeObject::new("standard_map_remove_method_id", 5);
    let key = remove_code.push_constant(Constant::String("xp".into()));
    let value = remove_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(6)));
    remove_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: key,
        },
    ));
    remove_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: value,
        },
    ));
    remove_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::MakeMap {
        dst: Register(2),
        entries: vec![("xp".into(), Register(1))],
    }));
    remove_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(3),
            receiver: Register(2),
            method: "missing_remove".into(),
            method_id: std_method_id("Map", "remove"),
            args: vec![vela_bytecode::CallArgument::Register(Register(0))],
        },
    ));
    remove_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(4),
            receiver: Register(2),
            method: "missing_has".into(),
            method_id: std_method_id("Map", "has"),
            args: vec![vela_bytecode::CallArgument::Register(Register(0))],
        },
    ));
    remove_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(4),
    }));
    assert_eq!(
        run_linked_standard_id_code(&Vm::new(), remove_code),
        Ok(OwnedValue::Bool(false))
    );

    let mut clear_code = UnlinkedCodeObject::new("standard_map_clear_method_id", 5);
    let key = clear_code.push_constant(Constant::String("xp".into()));
    let value = clear_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(6)));
    clear_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: key,
        },
    ));
    clear_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: value,
        },
    ));
    clear_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::MakeMap {
        dst: Register(2),
        entries: vec![("xp".into(), Register(1))],
    }));
    clear_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(3),
            receiver: Register(2),
            method: "missing_clear".into(),
            method_id: std_method_id("Map", "clear"),
            args: Vec::new(),
        },
    ));
    clear_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(4),
            receiver: Register(2),
            method: "missing_len".into(),
            method_id: std_method_id("Map", "len"),
            args: Vec::new(),
        },
    ));
    clear_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(4),
    }));
    assert_eq!(
        run_linked_standard_id_code(&Vm::new(), clear_code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(0)))
    );
}

#[test]
fn call_method_uses_standard_set_method_id_before_name_fallback() {
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let mut code = UnlinkedCodeObject::new("standard_set_method_id", 5);
    let first = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));
    let second = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(4)));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: first,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: second,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeArray {
            dst: Register(2),
            elements: vec![Register(0), Register(1)],
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: Some(Register(3)),
            name: "missing::set_from_array".into(),
            native: std_function_id(StdFunctionImplementation::SetFromArray),
            cache_site: None,
            args: vec![Register(2)],
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(4),
            receiver: Register(3),
            method: "missing_len".into(),
            method_id: std_method_id("Set", "len"),
            args: Vec::new(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(4),
    }));

    assert_eq!(
        run_linked_standard_id_code(&vm, code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn call_method_uses_standard_set_mutator_ids_before_name_fallback() {
    let mut add_code = UnlinkedCodeObject::new("standard_set_add_method_id", 6);
    let first = add_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));
    let second = add_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(4)));
    add_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: first,
        },
    ));
    add_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: second,
        },
    ));
    add_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeArray {
            dst: Register(2),
            elements: vec![Register(0)],
        },
    ));
    add_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: Some(Register(3)),
            name: "missing::set_from_array".into(),
            native: std_function_id(StdFunctionImplementation::SetFromArray),
            cache_site: None,
            args: vec![Register(2)],
        },
    ));
    add_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(4),
            receiver: Register(3),
            method: "missing_add".into(),
            method_id: std_method_id("Set", "add"),
            args: vec![vela_bytecode::CallArgument::Register(Register(1))],
        },
    ));
    add_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(5),
            receiver: Register(3),
            method: "missing_has".into(),
            method_id: std_method_id("Set", "has"),
            args: vec![vela_bytecode::CallArgument::Register(Register(1))],
        },
    ));
    add_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(5),
    }));
    let mut vm = Vm::new();
    vm.register_standard_natives();
    assert_eq!(
        run_linked_standard_id_code(&vm, add_code),
        Ok(OwnedValue::Bool(true))
    );

    let mut remove_code = UnlinkedCodeObject::new("standard_set_remove_method_id", 6);
    let first = remove_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));
    let second = remove_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(4)));
    remove_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: first,
        },
    ));
    remove_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: second,
        },
    ));
    remove_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeArray {
            dst: Register(2),
            elements: vec![Register(0), Register(1)],
        },
    ));
    remove_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: Some(Register(3)),
            name: "missing::set_from_array".into(),
            native: std_function_id(StdFunctionImplementation::SetFromArray),
            cache_site: None,
            args: vec![Register(2)],
        },
    ));
    remove_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(4),
            receiver: Register(3),
            method: "missing_remove".into(),
            method_id: std_method_id("Set", "remove"),
            args: vec![vela_bytecode::CallArgument::Register(Register(1))],
        },
    ));
    remove_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(5),
            receiver: Register(3),
            method: "missing_has".into(),
            method_id: std_method_id("Set", "has"),
            args: vec![vela_bytecode::CallArgument::Register(Register(1))],
        },
    ));
    remove_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(5),
    }));
    let mut vm = Vm::new();
    vm.register_standard_natives();
    assert_eq!(
        run_linked_standard_id_code(&vm, remove_code),
        Ok(OwnedValue::Bool(false))
    );

    let mut clear_code = UnlinkedCodeObject::new("standard_set_clear_method_id", 6);
    let first = clear_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));
    let second = clear_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(4)));
    clear_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: first,
        },
    ));
    clear_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: second,
        },
    ));
    clear_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeArray {
            dst: Register(2),
            elements: vec![Register(0), Register(1)],
        },
    ));
    clear_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: Some(Register(3)),
            name: "missing::set_from_array".into(),
            native: std_function_id(StdFunctionImplementation::SetFromArray),
            cache_site: None,
            args: vec![Register(2)],
        },
    ));
    clear_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(4),
            receiver: Register(3),
            method: "missing_clear".into(),
            method_id: std_method_id("Set", "clear"),
            args: Vec::new(),
        },
    ));
    clear_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(5),
            receiver: Register(3),
            method: "missing_len".into(),
            method_id: std_method_id("Set", "len"),
            args: Vec::new(),
        },
    ));
    clear_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(5),
    }));
    let mut vm = Vm::new();
    vm.register_standard_natives();
    assert_eq!(
        run_linked_standard_id_code(&vm, clear_code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(0)))
    );
}

#[test]
fn call_method_uses_standard_collection_predicate_ids_before_name_fallback() {
    let mut array_code = UnlinkedCodeObject::new("standard_array_contains_method_id", 4);
    let first = array_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));
    let second = array_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(4)));
    array_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: first,
        },
    ));
    array_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: second,
        },
    ));
    array_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeArray {
            dst: Register(2),
            elements: vec![Register(0), Register(1)],
        },
    ));
    array_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(3),
            receiver: Register(2),
            method: "missing_contains".into(),
            method_id: std_method_id("Array", "contains"),
            args: vec![vela_bytecode::CallArgument::Register(Register(1))],
        },
    ));
    array_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(3),
    }));
    assert_eq!(
        run_linked_standard_id_code(&Vm::new(), array_code),
        Ok(OwnedValue::Bool(true))
    );

    let mut map_code = UnlinkedCodeObject::new("standard_map_has_method_id", 4);
    let value = map_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(6)));
    let key = map_code.push_constant(Constant::String("xp".into()));
    map_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    map_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: key,
        },
    ));
    map_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::MakeMap {
        dst: Register(2),
        entries: vec![("xp".into(), Register(0))],
    }));
    map_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(3),
            receiver: Register(2),
            method: "missing_has".into(),
            method_id: std_method_id("Map", "has"),
            args: vec![vela_bytecode::CallArgument::Register(Register(1))],
        },
    ));
    map_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(3),
    }));
    assert_eq!(
        run_linked_standard_id_code(&Vm::new(), map_code),
        Ok(OwnedValue::Bool(true))
    );

    let mut set_code = UnlinkedCodeObject::new("standard_set_has_method_id", 5);
    let first = set_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));
    let second = set_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(4)));
    set_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: first,
        },
    ));
    set_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: second,
        },
    ));
    set_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeArray {
            dst: Register(2),
            elements: vec![Register(0), Register(1)],
        },
    ));
    set_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: Some(Register(3)),
            name: "missing::set_from_array".into(),
            native: std_function_id(StdFunctionImplementation::SetFromArray),
            cache_site: None,
            args: vec![Register(2)],
        },
    ));
    set_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(4),
            receiver: Register(3),
            method: "missing_has".into(),
            method_id: std_method_id("Set", "has"),
            args: vec![vela_bytecode::CallArgument::Register(Register(1))],
        },
    ));
    set_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(4),
    }));
    let mut vm = Vm::new();
    vm.register_standard_natives();
    assert_eq!(
        run_linked_standard_id_code(&vm, set_code),
        Ok(OwnedValue::Bool(true))
    );

    assert_eq!(
        run_set_relation_by_id(std_method_id("Set", "is_subset"), &[2], &[2, 4],),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        run_set_relation_by_id(std_method_id("Set", "is_superset"), &[2, 4], &[2],),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        run_set_relation_by_id(std_method_id("Set", "is_disjoint"), &[2], &[4],),
        Ok(OwnedValue::Bool(true))
    );
}

fn run_set_relation_by_id(
    method_id: vela_def::MethodId,
    receiver_values: &[i64],
    other_values: &[i64],
) -> VmResult<OwnedValue> {
    let receiver_array = Register(receiver_values.len() as u16);
    let other_start = receiver_values.len() + 1;
    let other_array = Register((other_start + other_values.len()) as u16);
    let receiver_set = Register(other_array.0 + 1);
    let other_set = Register(receiver_set.0 + 1);
    let result = Register(other_set.0 + 1);

    let mut code = UnlinkedCodeObject::new("standard_set_relation_method_id", result.0 + 1);
    for (index, value) in receiver_values.iter().enumerate() {
        let constant = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(*value)));
        code.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::LoadConst {
                dst: Register(index as u16),
                constant,
            },
        ));
    }
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeArray {
            dst: receiver_array,
            elements: (0..receiver_values.len())
                .map(|index| Register(index as u16))
                .collect(),
        },
    ));
    for (offset, value) in other_values.iter().enumerate() {
        let register = Register((other_start + offset) as u16);
        let constant = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(*value)));
        code.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::LoadConst {
                dst: register,
                constant,
            },
        ));
    }
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeArray {
            dst: other_array,
            elements: (other_start..other_start + other_values.len())
                .map(|index| Register(index as u16))
                .collect(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: Some(receiver_set),
            name: "missing::set_from_array".into(),
            native: std_function_id(StdFunctionImplementation::SetFromArray),
            cache_site: None,
            args: vec![receiver_array],
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: Some(other_set),
            name: "missing::set_from_array".into(),
            native: std_function_id(StdFunctionImplementation::SetFromArray),
            cache_site: None,
            args: vec![other_array],
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: result,
            receiver: receiver_set,
            method: "missing_set_relation".into(),
            method_id,
            args: vec![vela_bytecode::CallArgument::Register(other_set)],
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: result,
    }));

    let mut vm = Vm::new();
    vm.register_standard_natives();
    run_linked_standard_id_code(&vm, code)
}

#[test]
fn call_method_uses_standard_option_method_id_before_name_fallback() {
    let mut code = UnlinkedCodeObject::new("standard_option_method_id", 2);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeEnum {
            dst: Register(0),
            enum_name: "Option".into(),
            variant: "None".into(),
            fields: Vec::new(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(1),
            receiver: Register(0),
            method: "missing_is_none".into(),
            method_id: std_method_id("Option", "is_none"),
            args: Vec::new(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));

    assert_eq!(
        run_linked_standard_id_code(&Vm::new(), code),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn call_method_uses_standard_result_method_id_before_name_fallback() {
    let mut code = UnlinkedCodeObject::new("standard_result_method_id", 2);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeEnum {
            dst: Register(0),
            enum_name: "Result".into(),
            variant: "Err".into(),
            fields: Vec::new(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(1),
            receiver: Register(0),
            method: "missing_is_err".into(),
            method_id: std_method_id("Result", "is_err"),
            args: Vec::new(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));

    assert_eq!(
        run_linked_standard_id_code(&Vm::new(), code),
        Ok(OwnedValue::Bool(true))
    );
}
