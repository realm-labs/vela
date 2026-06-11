use super::standard_id_dispatch::{run_linked_standard_id_code, std_method_id};
use super::*;
use crate::owned_value::OwnedValue;

fn option_some(value: OwnedValue) -> OwnedValue {
    OwnedValue::enum_variant("Option", "Some", [("0", value)])
}

#[test]
fn call_method_uses_standard_array_method_id_before_name_fallback() {
    let mut code = UnlinkedCodeObject::new("standard_array_method_id", 4);
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
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(3),
            receiver: Register(2),
            method: "missing_len".into(),
            method_id: std_method_id("Array", "len"),
            args: Vec::new(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(3),
    }));

    assert_eq!(
        run_linked_standard_id_code(&Vm::new(), code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
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
        Ok(option_some(OwnedValue::Scalar(
            vela_common::ScalarValue::I64(1)
        )))
    );
}

fn run_array_lookup_with_args_by_id(
    method_id: vela_def::MethodId,
    args: &[Constant],
) -> VmResult<OwnedValue> {
    let arg_start = 3u16;
    let result = Register(arg_start + args.len() as u16);
    let mut code = UnlinkedCodeObject::new("standard_array_lookup_method_id", result.0 + 1);
    let first = code.push_constant(Constant::String("gold".to_owned()));
    let second = code.push_constant(Constant::String("xp".to_owned()));
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
    for (index, arg) in args.iter().enumerate() {
        let register = Register(arg_start + index as u16);
        let constant = code.push_constant(arg.clone());
        code.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::LoadConst {
                dst: register,
                constant,
            },
        ));
    }
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: result,
            receiver: Register(2),
            method: "missing_array_lookup".into(),
            method_id,
            args: (0..args.len())
                .map(|index| {
                    vela_bytecode::CallArgument::Register(Register(arg_start + index as u16))
                })
                .collect(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: result,
    }));

    run_linked_standard_id_code(&Vm::new(), code)
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
            &[
                Constant::Scalar(vela_common::ScalarValue::I64(1)),
                Constant::Scalar(vela_common::ScalarValue::I64(3)),
            ],
        ),
        Ok(OwnedValue::array(["xp", "bonus"]))
    );
    assert_eq!(
        run_array_transform_with_args_by_id(
            std_method_id("Array", "sort"),
            &["xp", "bonus", "gold"],
            &[],
        ),
        Ok(OwnedValue::array(["bonus", "gold", "xp"]))
    );
    assert_eq!(
        run_array_transform_with_args_by_id(
            std_method_id("Array", "min"),
            &["xp", "bonus", "gold"],
            &[],
        ),
        Ok(option_some(OwnedValue::String("bonus".to_owned())))
    );
    assert_eq!(
        run_array_transform_with_args_by_id(
            std_method_id("Array", "max"),
            &["xp", "bonus", "gold"],
            &[],
        ),
        Ok(option_some(OwnedValue::String("xp".to_owned())))
    );
}

fn run_array_transform_with_args_by_id(
    method_id: vela_def::MethodId,
    receiver: &[&str],
    args: &[Constant],
) -> VmResult<OwnedValue> {
    let receiver_register = Register(receiver.len() as u16);
    let arg_start = receiver_register.0 + 1;
    let result = Register(arg_start + args.len() as u16);
    let mut code = UnlinkedCodeObject::new("standard_array_transform_method_id", result.0 + 1);

    let mut elements = Vec::with_capacity(receiver.len());
    for (index, value) in receiver.iter().enumerate() {
        let register = Register(index as u16);
        let constant = code.push_constant(Constant::String((*value).to_owned()));
        code.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::LoadConst {
                dst: register,
                constant,
            },
        ));
        elements.push(register);
    }
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeArray {
            dst: receiver_register,
            elements,
        },
    ));
    for (index, arg) in args.iter().enumerate() {
        let register = Register(arg_start + index as u16);
        let constant = code.push_constant(arg.clone());
        code.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::LoadConst {
                dst: register,
                constant,
            },
        ));
    }
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: result,
            receiver: receiver_register,
            method: "missing_array_transform".into(),
            method_id,
            args: (0..args.len())
                .map(|index| {
                    vela_bytecode::CallArgument::Register(Register(arg_start + index as u16))
                })
                .collect(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: result,
    }));

    run_linked_standard_id_code(&Vm::new(), code)
}

#[test]
fn call_method_uses_standard_array_mutator_ids_before_name_fallback() {
    let mut push_code = UnlinkedCodeObject::new("standard_array_push_method_id", 5);
    let first = push_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));
    let second = push_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(4)));
    push_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: first,
        },
    ));
    push_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: second,
        },
    ));
    push_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeArray {
            dst: Register(2),
            elements: vec![Register(0)],
        },
    ));
    push_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(3),
            receiver: Register(2),
            method: "missing_push".into(),
            method_id: std_method_id("Array", "push"),
            args: vec![vela_bytecode::CallArgument::Register(Register(1))],
        },
    ));
    push_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(4),
            receiver: Register(2),
            method: "missing_len".into(),
            method_id: std_method_id("Array", "len"),
            args: Vec::new(),
        },
    ));
    push_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(4),
    }));
    assert_eq!(
        run_linked_standard_id_code(&Vm::new(), push_code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );

    let mut pop_code = UnlinkedCodeObject::new("standard_array_pop_method_id", 5);
    let first = pop_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));
    let second = pop_code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(4)));
    pop_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(0),
            constant: first,
        },
    ));
    pop_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: Register(1),
            constant: second,
        },
    ));
    pop_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeArray {
            dst: Register(2),
            elements: vec![Register(0), Register(1)],
        },
    ));
    pop_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(3),
            receiver: Register(2),
            method: "missing_pop".into(),
            method_id: std_method_id("Array", "pop"),
            args: Vec::new(),
        },
    ));
    pop_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(4),
            receiver: Register(2),
            method: "missing_len".into(),
            method_id: std_method_id("Array", "len"),
            args: Vec::new(),
        },
    ));
    pop_code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(4),
    }));
    assert_eq!(
        run_linked_standard_id_code(&Vm::new(), pop_code),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))
    );

    let mut clear_code = UnlinkedCodeObject::new("standard_array_clear_method_id", 5);
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
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(3),
            receiver: Register(2),
            method: "missing_clear".into(),
            method_id: std_method_id("Array", "clear"),
            args: Vec::new(),
        },
    ));
    clear_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(4),
            receiver: Register(2),
            method: "missing_len".into(),
            method_id: std_method_id("Array", "len"),
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

    assert_eq!(
        run_array_mutator_returns_receiver_by_id(
            std_method_id("Array", "insert"),
            &["gold", "bonus"],
            &[ArrayCallArg::I64(1), ArrayCallArg::String("xp"),],
        ),
        Ok(OwnedValue::array(["gold", "xp", "bonus"]))
    );
    assert_eq!(
        run_array_mutator_returns_receiver_by_id(
            std_method_id("Array", "extend"),
            &["gold"],
            &[ArrayCallArg::Array(&["xp", "bonus"])],
        ),
        Ok(OwnedValue::array(["gold", "xp", "bonus"]))
    );
    assert_eq!(
        run_array_mutator_returns_receiver_by_id(
            std_method_id("Array", "remove_at"),
            &["gold", "xp", "bonus"],
            &[ArrayCallArg::I64(1)],
        ),
        Ok(OwnedValue::array(["gold", "bonus"]))
    );
}

enum ArrayCallArg<'a> {
    String(&'a str),
    I64(i64),
    Array(&'a [&'a str]),
}

fn run_array_mutator_returns_receiver_by_id(
    method_id: vela_def::MethodId,
    receiver: &[&str],
    args: &[ArrayCallArg<'_>],
) -> VmResult<OwnedValue> {
    let mut code = UnlinkedCodeObject::new("standard_array_mutator_method_id", 32);
    let mut next_register = 0u16;
    let mut receiver_elements = Vec::with_capacity(receiver.len());
    for value in receiver {
        receiver_elements.push(load_string(&mut code, &mut next_register, value));
    }
    let receiver_register = Register(next_register);
    next_register += 1;
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeArray {
            dst: receiver_register,
            elements: receiver_elements,
        },
    ));

    let mut arg_registers = Vec::with_capacity(args.len());
    for arg in args {
        arg_registers.push(load_array_call_arg(&mut code, &mut next_register, arg));
    }
    let result = Register(next_register);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: result,
            receiver: receiver_register,
            method: "missing_array_mutator".into(),
            method_id,
            args: arg_registers
                .into_iter()
                .map(vela_bytecode::CallArgument::Register)
                .collect(),
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: receiver_register,
    }));

    run_linked_standard_id_code(&Vm::new(), code)
}

fn load_array_call_arg(
    code: &mut UnlinkedCodeObject,
    next_register: &mut u16,
    arg: &ArrayCallArg<'_>,
) -> Register {
    match arg {
        ArrayCallArg::String(value) => load_string(code, next_register, value),
        ArrayCallArg::I64(value) => load_i64(code, next_register, *value),
        ArrayCallArg::Array(values) => {
            let mut elements = Vec::with_capacity(values.len());
            for value in *values {
                elements.push(load_string(code, next_register, value));
            }
            let register = Register(*next_register);
            *next_register += 1;
            code.push_instruction(UnlinkedInstruction::new(
                UnlinkedInstructionKind::MakeArray {
                    dst: register,
                    elements,
                },
            ));
            register
        }
    }
}

fn load_string(code: &mut UnlinkedCodeObject, next_register: &mut u16, value: &str) -> Register {
    let register = Register(*next_register);
    *next_register += 1;
    let constant = code.push_constant(Constant::String(value.to_owned()));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: register,
            constant,
        },
    ));
    register
}

fn load_i64(code: &mut UnlinkedCodeObject, next_register: &mut u16, value: i64) -> Register {
    let register = Register(*next_register);
    *next_register += 1;
    let constant = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(value)));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadConst {
            dst: register,
            constant,
        },
    ));
    register
}
