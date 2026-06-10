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

fn std_method_id(owner: &str, name: &str) -> vela_common::HostMethodId {
    let Some(id) = vela_stdlib::std_method_id(owner, name) else {
        panic!("missing standard method identity for {owner}::{name}");
    };
    vela_common::HostMethodId::new(id.get())
}

#[test]
fn call_native_uses_resolved_id_before_name_fallback() {
    let native_id = vela_def::FunctionId::new(77);
    let mut vm = Vm::new();
    vm.register_native("diagnostic_name", |_| Ok(OwnedValue::Int(1)));
    vm.register_native_with_id(native_id, "resolved_name", |_| Ok(OwnedValue::Int(2)));

    let mut code = CodeObject::new("native_id", 1);
    code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: Some(Register(0)),
        name: "diagnostic_name".into(),
        native: Some(native_id),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(0),
    }));

    assert_eq!(vm.run(&code), Ok(OwnedValue::Int(2)));
}

#[test]
fn call_native_uses_resolved_host_id_before_name_fallback() {
    let native_id = FunctionId::new(78);
    let mut vm = Vm::new();
    vm.register_native("diagnostic_name", |_| Ok(OwnedValue::Int(1)));
    vm.register_host_native_with_id(native_id, "resolved_host", |_, _| Ok(OwnedValue::Int(3)));

    let mut code = CodeObject::new("host_native_id", 1);
    code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: Some(Register(0)),
        name: "diagnostic_name".into(),
        native: Some(native_id),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(0),
    }));

    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(vm.run_with_host(&code, &mut host), Ok(OwnedValue::Int(3)));
}

#[test]
fn call_native_uses_standard_native_id_before_name_fallback() {
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let mut code = CodeObject::new("standard_native_id", 2);
    let value = code.push_constant(Constant::Int(-4));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: value,
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: Some(Register(1)),
        name: "missing::abs".into(),
        native: Some(std_function_id(StdFunctionImplementation::MathAbs)),
        args: vec![Register(0)],
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));

    assert_eq!(vm.run(&code), Ok(OwnedValue::Int(4)));
}

#[test]
fn call_method_uses_standard_value_method_id_before_name_fallback() {
    let mut code = CodeObject::new("standard_value_method_id", 2);
    let value = code.push_constant(Constant::String("gold".into()));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: value,
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(1),
        receiver: Register(0),
        method: "missing_len".into(),
        value_method_id: Some(std_method_id("String", "len")),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(4)));
}

fn option_some(value: OwnedValue) -> OwnedValue {
    OwnedValue::enum_variant("Option", "Some", [("0", value)])
}

#[test]
fn call_method_uses_standard_range_method_id_before_name_fallback() {
    let mut code = CodeObject::new("standard_range_method_id", 4);
    let start = code.push_constant(Constant::Int(2));
    let end = code.push_constant(Constant::Int(5));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: start,
    }));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: end,
    }));
    code.push_instruction(Instruction::new(InstructionKind::MakeRange {
        dst: Register(2),
        start: Register(0),
        end: Register(1),
        inclusive: false,
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(3),
        receiver: Register(2),
        method: "missing_len".into(),
        value_method_id: Some(std_method_id("Range", "len")),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(3),
    }));

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(3)));
}

#[test]
fn call_method_uses_standard_array_method_id_before_name_fallback() {
    let mut code = CodeObject::new("standard_array_method_id", 4);
    let first = code.push_constant(Constant::Int(2));
    let second = code.push_constant(Constant::Int(4));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: first,
    }));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: second,
    }));
    code.push_instruction(Instruction::new(InstructionKind::MakeArray {
        dst: Register(2),
        elements: vec![Register(0), Register(1)],
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(3),
        receiver: Register(2),
        method: "missing_len".into(),
        value_method_id: Some(std_method_id("Array", "len")),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(3),
    }));

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(2)));
}

#[test]
fn call_method_uses_standard_array_lookup_ids_before_name_fallback() {
    assert_eq!(
        run_array_lookup_with_args_by_id(std_method_id("Array", "first"), &[]),
        Ok(option_some(OwnedValue::String("gold".to_owned())))
    );
    assert_eq!(
        run_array_lookup_with_args_by_id(std_method_id("Array", "last"), &[]),
        Ok(option_some(OwnedValue::String("xp".to_owned())))
    );
    assert_eq!(
        run_array_lookup_with_args_by_id(
            std_method_id("Array", "index_of"),
            &[Constant::String("xp".to_owned())],
        ),
        Ok(option_some(OwnedValue::Int(1)))
    );
}

fn run_array_lookup_with_args_by_id(
    method_id: vela_common::HostMethodId,
    args: &[Constant],
) -> VmResult<OwnedValue> {
    let arg_start = 3u16;
    let result = Register(arg_start + args.len() as u16);
    let mut code = CodeObject::new("standard_array_lookup_method_id", result.0 + 1);
    let first = code.push_constant(Constant::String("gold".to_owned()));
    let second = code.push_constant(Constant::String("xp".to_owned()));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: first,
    }));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: second,
    }));
    code.push_instruction(Instruction::new(InstructionKind::MakeArray {
        dst: Register(2),
        elements: vec![Register(0), Register(1)],
    }));
    for (index, arg) in args.iter().enumerate() {
        let register = Register(arg_start + index as u16);
        let constant = code.push_constant(arg.clone());
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: register,
            constant,
        }));
    }
    code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: result,
        receiver: Register(2),
        method: "missing_array_lookup".into(),
        value_method_id: Some(method_id),
        args: (0..args.len())
            .map(|index| vela_bytecode::CallArgument::Register(Register(arg_start + index as u16)))
            .collect(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return { src: result }));

    Vm::new().run(&code)
}

#[test]
fn call_method_uses_standard_array_transform_ids_before_name_fallback() {
    assert_eq!(
        run_array_transform_with_args_by_id(
            std_method_id("Array", "join"),
            &["gold", "xp", "bonus"],
            &[Constant::String(":".to_owned())],
        ),
        Ok(OwnedValue::String("gold:xp:bonus".to_owned()))
    );
    assert_eq!(
        run_array_transform_with_args_by_id(
            std_method_id("Array", "distinct"),
            &["gold", "xp", "gold"],
            &[],
        ),
        Ok(OwnedValue::array(["gold", "xp"]))
    );
    assert_eq!(
        run_array_transform_with_args_by_id(
            std_method_id("Array", "reverse"),
            &["gold", "xp", "bonus"],
            &[],
        ),
        Ok(OwnedValue::array(["bonus", "xp", "gold"]))
    );
    assert_eq!(
        run_array_transform_with_args_by_id(
            std_method_id("Array", "slice"),
            &["gold", "xp", "bonus"],
            &[Constant::Int(1), Constant::Int(3)],
        ),
        Ok(OwnedValue::array(["xp", "bonus"]))
    );
}

fn run_array_transform_with_args_by_id(
    method_id: vela_common::HostMethodId,
    receiver: &[&str],
    args: &[Constant],
) -> VmResult<OwnedValue> {
    let receiver_register = Register(receiver.len() as u16);
    let arg_start = receiver_register.0 + 1;
    let result = Register(arg_start + args.len() as u16);
    let mut code = CodeObject::new("standard_array_transform_method_id", result.0 + 1);

    let mut elements = Vec::with_capacity(receiver.len());
    for (index, value) in receiver.iter().enumerate() {
        let register = Register(index as u16);
        let constant = code.push_constant(Constant::String((*value).to_owned()));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: register,
            constant,
        }));
        elements.push(register);
    }
    code.push_instruction(Instruction::new(InstructionKind::MakeArray {
        dst: receiver_register,
        elements,
    }));
    for (index, arg) in args.iter().enumerate() {
        let register = Register(arg_start + index as u16);
        let constant = code.push_constant(arg.clone());
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: register,
            constant,
        }));
    }
    code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: result,
        receiver: receiver_register,
        method: "missing_array_transform".into(),
        value_method_id: Some(method_id),
        args: (0..args.len())
            .map(|index| vela_bytecode::CallArgument::Register(Register(arg_start + index as u16)))
            .collect(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return { src: result }));

    Vm::new().run(&code)
}

#[test]
fn call_method_uses_standard_array_mutator_ids_before_name_fallback() {
    let mut push_code = CodeObject::new("standard_array_push_method_id", 5);
    let first = push_code.push_constant(Constant::Int(2));
    let second = push_code.push_constant(Constant::Int(4));
    push_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: first,
    }));
    push_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: second,
    }));
    push_code.push_instruction(Instruction::new(InstructionKind::MakeArray {
        dst: Register(2),
        elements: vec![Register(0)],
    }));
    push_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(3),
        receiver: Register(2),
        method: "missing_push".into(),
        value_method_id: Some(std_method_id("Array", "push")),
        args: vec![vela_bytecode::CallArgument::Register(Register(1))],
    }));
    push_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(4),
        receiver: Register(2),
        method: "missing_len".into(),
        value_method_id: Some(std_method_id("Array", "len")),
        args: Vec::new(),
    }));
    push_code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(4),
    }));
    assert_eq!(Vm::new().run(&push_code), Ok(OwnedValue::Int(2)));

    let mut pop_code = CodeObject::new("standard_array_pop_method_id", 5);
    let first = pop_code.push_constant(Constant::Int(2));
    let second = pop_code.push_constant(Constant::Int(4));
    pop_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: first,
    }));
    pop_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: second,
    }));
    pop_code.push_instruction(Instruction::new(InstructionKind::MakeArray {
        dst: Register(2),
        elements: vec![Register(0), Register(1)],
    }));
    pop_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(3),
        receiver: Register(2),
        method: "missing_pop".into(),
        value_method_id: Some(std_method_id("Array", "pop")),
        args: Vec::new(),
    }));
    pop_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(4),
        receiver: Register(2),
        method: "missing_len".into(),
        value_method_id: Some(std_method_id("Array", "len")),
        args: Vec::new(),
    }));
    pop_code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(4),
    }));
    assert_eq!(Vm::new().run(&pop_code), Ok(OwnedValue::Int(1)));

    let mut clear_code = CodeObject::new("standard_array_clear_method_id", 5);
    let first = clear_code.push_constant(Constant::Int(2));
    let second = clear_code.push_constant(Constant::Int(4));
    clear_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: first,
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: second,
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::MakeArray {
        dst: Register(2),
        elements: vec![Register(0), Register(1)],
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(3),
        receiver: Register(2),
        method: "missing_clear".into(),
        value_method_id: Some(std_method_id("Array", "clear")),
        args: Vec::new(),
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(4),
        receiver: Register(2),
        method: "missing_len".into(),
        value_method_id: Some(std_method_id("Array", "len")),
        args: Vec::new(),
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(4),
    }));
    assert_eq!(Vm::new().run(&clear_code), Ok(OwnedValue::Int(0)));
}

#[test]
fn call_method_uses_standard_map_method_id_before_name_fallback() {
    let mut code = CodeObject::new("standard_map_method_id", 3);
    let value = code.push_constant(Constant::Int(6));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: value,
    }));
    code.push_instruction(Instruction::new(InstructionKind::MakeMap {
        dst: Register(1),
        entries: vec![("xp".into(), Register(0))],
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(2),
        receiver: Register(1),
        method: "missing_is_empty".into(),
        value_method_id: Some(std_method_id("Map", "is_empty")),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Bool(false)));
}

#[test]
fn call_method_uses_standard_map_mutator_ids_before_name_fallback() {
    let mut set_code = CodeObject::new("standard_map_set_method_id", 6);
    let key = set_code.push_constant(Constant::String("xp".into()));
    let value = set_code.push_constant(Constant::Int(6));
    set_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: key,
    }));
    set_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: value,
    }));
    set_code.push_instruction(Instruction::new(InstructionKind::MakeMap {
        dst: Register(2),
        entries: Vec::new(),
    }));
    set_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(3),
        receiver: Register(2),
        method: "missing_set".into(),
        value_method_id: Some(std_method_id("Map", "set")),
        args: vec![
            vela_bytecode::CallArgument::Register(Register(0)),
            vela_bytecode::CallArgument::Register(Register(1)),
        ],
    }));
    set_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(4),
        receiver: Register(2),
        method: "missing_has".into(),
        value_method_id: Some(std_method_id("Map", "has")),
        args: vec![vela_bytecode::CallArgument::Register(Register(0))],
    }));
    set_code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(4),
    }));
    assert_eq!(Vm::new().run(&set_code), Ok(OwnedValue::Bool(true)));

    let mut remove_code = CodeObject::new("standard_map_remove_method_id", 5);
    let key = remove_code.push_constant(Constant::String("xp".into()));
    let value = remove_code.push_constant(Constant::Int(6));
    remove_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: key,
    }));
    remove_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: value,
    }));
    remove_code.push_instruction(Instruction::new(InstructionKind::MakeMap {
        dst: Register(2),
        entries: vec![("xp".into(), Register(1))],
    }));
    remove_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(3),
        receiver: Register(2),
        method: "missing_remove".into(),
        value_method_id: Some(std_method_id("Map", "remove")),
        args: vec![vela_bytecode::CallArgument::Register(Register(0))],
    }));
    remove_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(4),
        receiver: Register(2),
        method: "missing_has".into(),
        value_method_id: Some(std_method_id("Map", "has")),
        args: vec![vela_bytecode::CallArgument::Register(Register(0))],
    }));
    remove_code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(4),
    }));
    assert_eq!(Vm::new().run(&remove_code), Ok(OwnedValue::Bool(false)));

    let mut clear_code = CodeObject::new("standard_map_clear_method_id", 5);
    let key = clear_code.push_constant(Constant::String("xp".into()));
    let value = clear_code.push_constant(Constant::Int(6));
    clear_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: key,
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: value,
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::MakeMap {
        dst: Register(2),
        entries: vec![("xp".into(), Register(1))],
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(3),
        receiver: Register(2),
        method: "missing_clear".into(),
        value_method_id: Some(std_method_id("Map", "clear")),
        args: Vec::new(),
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(4),
        receiver: Register(2),
        method: "missing_len".into(),
        value_method_id: Some(std_method_id("Map", "len")),
        args: Vec::new(),
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(4),
    }));
    assert_eq!(Vm::new().run(&clear_code), Ok(OwnedValue::Int(0)));
}

#[test]
fn call_method_uses_standard_set_method_id_before_name_fallback() {
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let mut code = CodeObject::new("standard_set_method_id", 5);
    let first = code.push_constant(Constant::Int(2));
    let second = code.push_constant(Constant::Int(4));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: first,
    }));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: second,
    }));
    code.push_instruction(Instruction::new(InstructionKind::MakeArray {
        dst: Register(2),
        elements: vec![Register(0), Register(1)],
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: Some(Register(3)),
        name: "missing::set_from_array".into(),
        native: Some(std_function_id(StdFunctionImplementation::SetFromArray)),
        args: vec![Register(2)],
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(4),
        receiver: Register(3),
        method: "missing_len".into(),
        value_method_id: Some(std_method_id("Set", "len")),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(4),
    }));

    assert_eq!(vm.run(&code), Ok(OwnedValue::Int(2)));
}

#[test]
fn call_method_uses_standard_set_mutator_ids_before_name_fallback() {
    let mut add_code = CodeObject::new("standard_set_add_method_id", 6);
    let first = add_code.push_constant(Constant::Int(2));
    let second = add_code.push_constant(Constant::Int(4));
    add_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: first,
    }));
    add_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: second,
    }));
    add_code.push_instruction(Instruction::new(InstructionKind::MakeArray {
        dst: Register(2),
        elements: vec![Register(0)],
    }));
    add_code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: Some(Register(3)),
        name: "missing::set_from_array".into(),
        native: Some(std_function_id(StdFunctionImplementation::SetFromArray)),
        args: vec![Register(2)],
    }));
    add_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(4),
        receiver: Register(3),
        method: "missing_add".into(),
        value_method_id: Some(std_method_id("Set", "add")),
        args: vec![vela_bytecode::CallArgument::Register(Register(1))],
    }));
    add_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(5),
        receiver: Register(3),
        method: "missing_has".into(),
        value_method_id: Some(std_method_id("Set", "has")),
        args: vec![vela_bytecode::CallArgument::Register(Register(1))],
    }));
    add_code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(5),
    }));
    let mut vm = Vm::new();
    vm.register_standard_natives();
    assert_eq!(vm.run(&add_code), Ok(OwnedValue::Bool(true)));

    let mut remove_code = CodeObject::new("standard_set_remove_method_id", 6);
    let first = remove_code.push_constant(Constant::Int(2));
    let second = remove_code.push_constant(Constant::Int(4));
    remove_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: first,
    }));
    remove_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: second,
    }));
    remove_code.push_instruction(Instruction::new(InstructionKind::MakeArray {
        dst: Register(2),
        elements: vec![Register(0), Register(1)],
    }));
    remove_code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: Some(Register(3)),
        name: "missing::set_from_array".into(),
        native: Some(std_function_id(StdFunctionImplementation::SetFromArray)),
        args: vec![Register(2)],
    }));
    remove_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(4),
        receiver: Register(3),
        method: "missing_remove".into(),
        value_method_id: Some(std_method_id("Set", "remove")),
        args: vec![vela_bytecode::CallArgument::Register(Register(1))],
    }));
    remove_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(5),
        receiver: Register(3),
        method: "missing_has".into(),
        value_method_id: Some(std_method_id("Set", "has")),
        args: vec![vela_bytecode::CallArgument::Register(Register(1))],
    }));
    remove_code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(5),
    }));
    let mut vm = Vm::new();
    vm.register_standard_natives();
    assert_eq!(vm.run(&remove_code), Ok(OwnedValue::Bool(false)));

    let mut clear_code = CodeObject::new("standard_set_clear_method_id", 6);
    let first = clear_code.push_constant(Constant::Int(2));
    let second = clear_code.push_constant(Constant::Int(4));
    clear_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: first,
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: second,
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::MakeArray {
        dst: Register(2),
        elements: vec![Register(0), Register(1)],
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: Some(Register(3)),
        name: "missing::set_from_array".into(),
        native: Some(std_function_id(StdFunctionImplementation::SetFromArray)),
        args: vec![Register(2)],
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(4),
        receiver: Register(3),
        method: "missing_clear".into(),
        value_method_id: Some(std_method_id("Set", "clear")),
        args: Vec::new(),
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(5),
        receiver: Register(3),
        method: "missing_len".into(),
        value_method_id: Some(std_method_id("Set", "len")),
        args: Vec::new(),
    }));
    clear_code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(5),
    }));
    let mut vm = Vm::new();
    vm.register_standard_natives();
    assert_eq!(vm.run(&clear_code), Ok(OwnedValue::Int(0)));
}

#[test]
fn call_method_uses_standard_collection_predicate_ids_before_name_fallback() {
    let mut array_code = CodeObject::new("standard_array_contains_method_id", 4);
    let first = array_code.push_constant(Constant::Int(2));
    let second = array_code.push_constant(Constant::Int(4));
    array_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: first,
    }));
    array_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: second,
    }));
    array_code.push_instruction(Instruction::new(InstructionKind::MakeArray {
        dst: Register(2),
        elements: vec![Register(0), Register(1)],
    }));
    array_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(3),
        receiver: Register(2),
        method: "missing_contains".into(),
        value_method_id: Some(std_method_id("Array", "contains")),
        args: vec![vela_bytecode::CallArgument::Register(Register(1))],
    }));
    array_code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(3),
    }));
    assert_eq!(Vm::new().run(&array_code), Ok(OwnedValue::Bool(true)));

    let mut map_code = CodeObject::new("standard_map_has_method_id", 4);
    let value = map_code.push_constant(Constant::Int(6));
    let key = map_code.push_constant(Constant::String("xp".into()));
    map_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: value,
    }));
    map_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: key,
    }));
    map_code.push_instruction(Instruction::new(InstructionKind::MakeMap {
        dst: Register(2),
        entries: vec![("xp".into(), Register(0))],
    }));
    map_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(3),
        receiver: Register(2),
        method: "missing_has".into(),
        value_method_id: Some(std_method_id("Map", "has")),
        args: vec![vela_bytecode::CallArgument::Register(Register(1))],
    }));
    map_code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(3),
    }));
    assert_eq!(Vm::new().run(&map_code), Ok(OwnedValue::Bool(true)));

    let mut set_code = CodeObject::new("standard_set_has_method_id", 5);
    let first = set_code.push_constant(Constant::Int(2));
    let second = set_code.push_constant(Constant::Int(4));
    set_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: first,
    }));
    set_code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: second,
    }));
    set_code.push_instruction(Instruction::new(InstructionKind::MakeArray {
        dst: Register(2),
        elements: vec![Register(0), Register(1)],
    }));
    set_code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: Some(Register(3)),
        name: "missing::set_from_array".into(),
        native: Some(std_function_id(StdFunctionImplementation::SetFromArray)),
        args: vec![Register(2)],
    }));
    set_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(4),
        receiver: Register(3),
        method: "missing_has".into(),
        value_method_id: Some(std_method_id("Set", "has")),
        args: vec![vela_bytecode::CallArgument::Register(Register(1))],
    }));
    set_code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(4),
    }));
    let mut vm = Vm::new();
    vm.register_standard_natives();
    assert_eq!(vm.run(&set_code), Ok(OwnedValue::Bool(true)));

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
    method_id: vela_common::HostMethodId,
    receiver_values: &[i64],
    other_values: &[i64],
) -> VmResult<OwnedValue> {
    let receiver_array = Register(receiver_values.len() as u16);
    let other_start = receiver_values.len() + 1;
    let other_array = Register((other_start + other_values.len()) as u16);
    let receiver_set = Register(other_array.0 + 1);
    let other_set = Register(receiver_set.0 + 1);
    let result = Register(other_set.0 + 1);

    let mut code = CodeObject::new("standard_set_relation_method_id", result.0 + 1);
    for (index, value) in receiver_values.iter().enumerate() {
        let constant = code.push_constant(Constant::Int(*value));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(index as u16),
            constant,
        }));
    }
    code.push_instruction(Instruction::new(InstructionKind::MakeArray {
        dst: receiver_array,
        elements: (0..receiver_values.len())
            .map(|index| Register(index as u16))
            .collect(),
    }));
    for (offset, value) in other_values.iter().enumerate() {
        let register = Register((other_start + offset) as u16);
        let constant = code.push_constant(Constant::Int(*value));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: register,
            constant,
        }));
    }
    code.push_instruction(Instruction::new(InstructionKind::MakeArray {
        dst: other_array,
        elements: (other_start..other_start + other_values.len())
            .map(|index| Register(index as u16))
            .collect(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: Some(receiver_set),
        name: "missing::set_from_array".into(),
        native: Some(std_function_id(StdFunctionImplementation::SetFromArray)),
        args: vec![receiver_array],
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: Some(other_set),
        name: "missing::set_from_array".into(),
        native: Some(std_function_id(StdFunctionImplementation::SetFromArray)),
        args: vec![other_array],
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: result,
        receiver: receiver_set,
        method: "missing_set_relation".into(),
        value_method_id: Some(method_id),
        args: vec![vela_bytecode::CallArgument::Register(other_set)],
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return { src: result }));

    let mut vm = Vm::new();
    vm.register_standard_natives();
    vm.run(&code)
}

#[test]
fn call_method_uses_standard_option_method_id_before_name_fallback() {
    let mut code = CodeObject::new("standard_option_method_id", 2);
    code.push_instruction(Instruction::new(InstructionKind::MakeEnum {
        dst: Register(0),
        enum_name: "Option".into(),
        variant: "None".into(),
        fields: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(1),
        receiver: Register(0),
        method: "missing_is_none".into(),
        value_method_id: Some(std_method_id("Option", "is_none")),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Bool(true)));
}

#[test]
fn call_method_uses_standard_result_method_id_before_name_fallback() {
    let mut code = CodeObject::new("standard_result_method_id", 2);
    code.push_instruction(Instruction::new(InstructionKind::MakeEnum {
        dst: Register(0),
        enum_name: "Result".into(),
        variant: "Err".into(),
        fields: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(1),
        receiver: Register(0),
        method: "missing_is_err".into(),
        value_method_id: Some(std_method_id("Result", "is_err")),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Bool(true)));
}
