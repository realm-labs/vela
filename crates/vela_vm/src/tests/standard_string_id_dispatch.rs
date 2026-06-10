use super::*;
use crate::owned_value::OwnedValue;

fn std_method_id(owner: &str, name: &str) -> vela_common::HostMethodId {
    let Some(id) = vela_stdlib::std_method_id(owner, name) else {
        panic!("missing standard method identity for {owner}::{name}");
    };
    vela_common::HostMethodId::new(id.get())
}

#[test]
fn call_method_uses_standard_string_predicate_ids_before_name_fallback() {
    assert_eq!(
        run_string_predicate_by_id(std_method_id("String", "contains"), ":"),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        run_string_predicate_by_id(std_method_id("String", "starts_with"), "reward"),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        run_string_predicate_by_id(std_method_id("String", "ends_with"), "gold"),
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
        run_string_transform_by_id(std_method_id("String", "to_upper"), "Reward"),
        Ok(OwnedValue::String("REWARD".to_owned()))
    );
    assert_eq!(
        run_string_transform_by_id(std_method_id("String", "to_lower"), "Reward"),
        Ok(OwnedValue::String("reward".to_owned()))
    );
    assert_eq!(
        run_string_transform_by_id(std_method_id("String", "trim"), " Reward "),
        Ok(OwnedValue::String("Reward".to_owned()))
    );
    assert_eq!(
        run_string_transform_by_id(std_method_id("String", "trim_start"), " Reward "),
        Ok(OwnedValue::String("Reward ".to_owned()))
    );
    assert_eq!(
        run_string_transform_by_id(std_method_id("String", "trim_end"), " Reward "),
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
            std_method_id("String", "replace"),
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
            std_method_id("String", "repeat"),
            "xp",
            &[Constant::Int(3)],
        ),
        Ok(OwnedValue::String("xpxpxp".to_owned()))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            std_method_id("String", "slice"),
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
            std_method_id("String", "find"),
            "reward:gold",
            &[Constant::String(":".to_owned())],
        ),
        Ok(option_some(OwnedValue::Int(6)))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            std_method_id("String", "strip_prefix"),
            "reward:gold",
            &[Constant::String("reward:".to_owned())],
        ),
        Ok(option_some(OwnedValue::String("gold".to_owned())))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            std_method_id("String", "strip_suffix"),
            "reward:gold",
            &[Constant::String(":gold".to_owned())],
        ),
        Ok(option_some(OwnedValue::String("reward".to_owned())))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            std_method_id("String", "char_at"),
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
            std_method_id("String", "split"),
            "reward:gold",
            &[Constant::String(":".to_owned())],
        ),
        Ok(OwnedValue::array(["reward", "gold"]))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            std_method_id("String", "split_once"),
            "reward:gold",
            &[Constant::String(":".to_owned())],
        ),
        Ok(option_some(OwnedValue::array(["reward", "gold"])))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            std_method_id("String", "split_lines"),
            "reward\ngold",
            &[],
        ),
        Ok(OwnedValue::array(["reward", "gold"]))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(
            std_method_id("String", "split_whitespace"),
            "reward gold",
            &[],
        ),
        Ok(OwnedValue::array(["reward", "gold"]))
    );
}

#[test]
fn call_method_uses_standard_string_parse_ids_before_name_fallback() {
    assert_eq!(
        run_string_transform_with_args_by_id(std_method_id("String", "parse_int"), "42", &[],),
        Ok(option_some(OwnedValue::Int(42)))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(std_method_id("String", "parse_float"), "1.5", &[],),
        Ok(option_some(OwnedValue::Float(1.5)))
    );
    assert_eq!(
        run_string_transform_with_args_by_id(std_method_id("String", "parse_bool"), "true", &[],),
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
