use super::*;
use crate::owned_value::OwnedValue;

#[test]
fn call_method_uses_standard_string_predicate_ids_before_name_fallback() {
    assert_eq!(
        run_string_predicate_by_id(vela_common::standard_ids::STRING_CONTAINS_METHOD_ID, ":"),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        run_string_predicate_by_id(
            vela_common::standard_ids::STRING_STARTS_WITH_METHOD_ID,
            "reward"
        ),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        run_string_predicate_by_id(
            vela_common::standard_ids::STRING_ENDS_WITH_METHOD_ID,
            "gold"
        ),
        Ok(OwnedValue::Bool(true))
    );
}

fn run_string_predicate_by_id(
    method_id: vela_common::HostMethodId,
    argument: &str,
) -> VmResult<OwnedValue> {
    let mut code = CodeObject::new("standard_string_predicate_method_id", 3);
    let receiver = code.push_constant(Constant::String("reward:gold".into()));
    let argument = code.push_constant(Constant::String(argument.into()));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: receiver,
    }));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(1),
        constant: argument,
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(2),
        receiver: Register(0),
        method: "missing_string_predicate".into(),
        value_method_id: Some(method_id),
        args: vec![vela_bytecode::CallArgument::Register(Register(1))],
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));

    Vm::new().run(&code)
}

#[test]
fn call_method_uses_standard_string_transform_ids_before_name_fallback() {
    assert_eq!(
        run_string_transform_by_id(
            vela_common::standard_ids::STRING_TO_UPPER_METHOD_ID,
            "Reward"
        ),
        Ok(OwnedValue::String("REWARD".to_owned()))
    );
    assert_eq!(
        run_string_transform_by_id(
            vela_common::standard_ids::STRING_TO_LOWER_METHOD_ID,
            "Reward"
        ),
        Ok(OwnedValue::String("reward".to_owned()))
    );
    assert_eq!(
        run_string_transform_by_id(vela_common::standard_ids::STRING_TRIM_METHOD_ID, " Reward "),
        Ok(OwnedValue::String("Reward".to_owned()))
    );
    assert_eq!(
        run_string_transform_by_id(
            vela_common::standard_ids::STRING_TRIM_START_METHOD_ID,
            " Reward "
        ),
        Ok(OwnedValue::String("Reward ".to_owned()))
    );
    assert_eq!(
        run_string_transform_by_id(
            vela_common::standard_ids::STRING_TRIM_END_METHOD_ID,
            " Reward "
        ),
        Ok(OwnedValue::String(" Reward".to_owned()))
    );
}

fn run_string_transform_by_id(
    method_id: vela_common::HostMethodId,
    receiver: &str,
) -> VmResult<OwnedValue> {
    let mut code = CodeObject::new("standard_string_transform_method_id", 2);
    let receiver = code.push_constant(Constant::String(receiver.into()));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: receiver,
    }));
    code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: Register(1),
        receiver: Register(0),
        method: "missing_string_transform".into(),
        value_method_id: Some(method_id),
        args: Vec::new(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));

    Vm::new().run(&code)
}

#[test]
fn call_method_uses_standard_string_argument_transform_ids_before_name_fallback() {
    assert_eq!(
        run_string_transform_with_args_by_id(
            vela_common::standard_ids::STRING_REPLACE_METHOD_ID,
            "gold.gold",
            &[
                Constant::String("gold".to_owned()),
                Constant::String("xp".to_owned())
            ],
        ),
        Ok(OwnedValue::String("xp.xp".to_owned()))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            vela_common::standard_ids::STRING_REPEAT_METHOD_ID,
            "xp",
            &[Constant::Int(3)],
        ),
        Ok(OwnedValue::String("xpxpxp".to_owned()))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            vela_common::standard_ids::STRING_SLICE_METHOD_ID,
            "reward",
            &[Constant::Int(1), Constant::Int(5)],
        ),
        Ok(OwnedValue::String("ewar".to_owned()))
    );
}

#[test]
fn call_method_uses_standard_string_option_ids_before_name_fallback() {
    assert_eq!(
        run_string_transform_with_args_by_id(
            vela_common::standard_ids::STRING_FIND_METHOD_ID,
            "reward:gold",
            &[Constant::String(":".to_owned())],
        ),
        Ok(option_some(OwnedValue::Int(6)))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            vela_common::standard_ids::STRING_STRIP_PREFIX_METHOD_ID,
            "reward:gold",
            &[Constant::String("reward:".to_owned())],
        ),
        Ok(option_some(OwnedValue::String("gold".to_owned())))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            vela_common::standard_ids::STRING_STRIP_SUFFIX_METHOD_ID,
            "reward:gold",
            &[Constant::String(":gold".to_owned())],
        ),
        Ok(option_some(OwnedValue::String("reward".to_owned())))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            vela_common::standard_ids::STRING_CHAR_AT_METHOD_ID,
            "reward:gold",
            &[Constant::Int(6)],
        ),
        Ok(option_some(OwnedValue::String(":".to_owned())))
    );
}

#[test]
fn call_method_uses_standard_string_split_ids_before_name_fallback() {
    assert_eq!(
        run_string_transform_with_args_by_id(
            vela_common::standard_ids::STRING_SPLIT_METHOD_ID,
            "reward:gold",
            &[Constant::String(":".to_owned())],
        ),
        Ok(OwnedValue::array(["reward", "gold"]))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            vela_common::standard_ids::STRING_SPLIT_ONCE_METHOD_ID,
            "reward:gold",
            &[Constant::String(":".to_owned())],
        ),
        Ok(option_some(OwnedValue::array(["reward", "gold"])))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            vela_common::standard_ids::STRING_SPLIT_LINES_METHOD_ID,
            "reward\ngold",
            &[],
        ),
        Ok(OwnedValue::array(["reward", "gold"]))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            vela_common::standard_ids::STRING_SPLIT_WHITESPACE_METHOD_ID,
            "reward gold",
            &[],
        ),
        Ok(OwnedValue::array(["reward", "gold"]))
    );
}

#[test]
fn call_method_uses_standard_string_parse_ids_before_name_fallback() {
    assert_eq!(
        run_string_transform_with_args_by_id(
            vela_common::standard_ids::STRING_PARSE_INT_METHOD_ID,
            "42",
            &[],
        ),
        Ok(option_some(OwnedValue::Int(42)))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            vela_common::standard_ids::STRING_PARSE_FLOAT_METHOD_ID,
            "1.5",
            &[],
        ),
        Ok(option_some(OwnedValue::Float(1.5)))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            vela_common::standard_ids::STRING_PARSE_BOOL_METHOD_ID,
            "true",
            &[],
        ),
        Ok(option_some(OwnedValue::Bool(true)))
    );
}

fn option_some(value: OwnedValue) -> OwnedValue {
    OwnedValue::enum_variant("Option", "Some", [("0", value)])
}

fn run_string_transform_with_args_by_id(
    method_id: vela_common::HostMethodId,
    receiver: &str,
    args: &[Constant],
) -> VmResult<OwnedValue> {
    let result = Register((args.len() + 1) as u16);
    let mut code = CodeObject::new("standard_string_arg_transform_method_id", result.0 + 1);
    let receiver = code.push_constant(Constant::String(receiver.into()));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(0),
        constant: receiver,
    }));
    for (index, arg) in args.iter().enumerate() {
        let register = Register((index + 1) as u16);
        let constant = code.push_constant(arg.clone());
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: register,
            constant,
        }));
    }
    code.push_instruction(Instruction::new(InstructionKind::CallMethod {
        dst: result,
        receiver: Register(0),
        method: "missing_string_arg_transform".into(),
        value_method_id: Some(method_id),
        args: (1..=args.len())
            .map(|index| vela_bytecode::CallArgument::Register(Register(index as u16)))
            .collect(),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return { src: result }));

    Vm::new().run(&code)
}
