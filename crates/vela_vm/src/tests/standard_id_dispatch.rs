use super::*;
use crate::owned_value::OwnedValue;

#[test]
fn call_native_uses_resolved_id_before_name_fallback() {
    let native_id = vela_common::FunctionId::new(77);
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
        native: Some(vela_common::standard_ids::MATH_ABS_FUNCTION_ID),
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
        value_method_id: Some(vela_common::standard_ids::STRING_LEN_METHOD_ID),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(4)));
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
        value_method_id: Some(vela_common::standard_ids::RANGE_LEN_METHOD_ID),
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
        value_method_id: Some(vela_common::standard_ids::ARRAY_LEN_METHOD_ID),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(3),
    }));

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Int(2)));
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
        value_method_id: Some(vela_common::standard_ids::MAP_IS_EMPTY_METHOD_ID),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Bool(false)));
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
        native: Some(vela_common::standard_ids::SET_FROM_ARRAY_FUNCTION_ID),
        args: vec![Register(2)],
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(4),
        receiver: Register(3),
        method: "missing_len".into(),
        value_method_id: Some(vela_common::standard_ids::SET_LEN_METHOD_ID),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(4),
    }));

    assert_eq!(vm.run(&code), Ok(OwnedValue::Int(2)));
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
        value_method_id: Some(vela_common::standard_ids::ARRAY_CONTAINS_METHOD_ID),
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
        value_method_id: Some(vela_common::standard_ids::MAP_HAS_METHOD_ID),
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
        native: Some(vela_common::standard_ids::SET_FROM_ARRAY_FUNCTION_ID),
        args: vec![Register(2)],
    }));
    set_code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(4),
        receiver: Register(3),
        method: "missing_has".into(),
        value_method_id: Some(vela_common::standard_ids::SET_HAS_METHOD_ID),
        args: vec![vela_bytecode::CallArgument::Register(Register(1))],
    }));
    set_code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(4),
    }));
    let mut vm = Vm::new();
    vm.register_standard_natives();
    assert_eq!(vm.run(&set_code), Ok(OwnedValue::Bool(true)));

    assert_eq!(
        run_set_relation_by_id(
            vela_common::standard_ids::SET_IS_SUBSET_METHOD_ID,
            &[2],
            &[2, 4],
        ),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        run_set_relation_by_id(
            vela_common::standard_ids::SET_IS_SUPERSET_METHOD_ID,
            &[2, 4],
            &[2],
        ),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        run_set_relation_by_id(
            vela_common::standard_ids::SET_IS_DISJOINT_METHOD_ID,
            &[2],
            &[4],
        ),
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
        native: Some(vela_common::standard_ids::SET_FROM_ARRAY_FUNCTION_ID),
        args: vec![receiver_array],
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallNative {
        dst: Some(other_set),
        name: "missing::set_from_array".into(),
        native: Some(vela_common::standard_ids::SET_FROM_ARRAY_FUNCTION_ID),
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
        value_method_id: Some(vela_common::standard_ids::OPTION_IS_NONE_METHOD_ID),
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
        value_method_id: Some(vela_common::standard_ids::RESULT_IS_ERR_METHOD_ID),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));

    assert_eq!(Vm::new().run(&code), Ok(OwnedValue::Bool(true)));
}
