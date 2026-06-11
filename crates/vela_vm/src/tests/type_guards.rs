use super::*;

#[test]
fn linked_guard_type_accepts_matching_primitive_contract() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(value) {
    let amount: i64 = value;
    return amount;
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let value = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::i64(42)],
        &mut budget,
    )
    .expect("matching guard should pass");

    assert_eq!(value, OwnedValue::i64(42));
}

#[test]
fn linked_guard_type_rejects_dynamic_contract_mismatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(value) {
    let amount: i64 = value;
    return amount;
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::String("not an integer".to_owned())],
        &mut budget,
    )
    .expect_err("mismatched guard should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "string".to_owned(),
            debug_name: "amount".to_owned(),
        }
    );
}

#[test]
fn linked_parameter_guard_rejects_public_entry_mismatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(value: i64) {
    return value;
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::String("not an integer".to_owned())],
        &mut budget,
    )
    .expect_err("checked entry should reject mismatched argument");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "string".to_owned(),
            debug_name: "value".to_owned(),
        }
    );
}

#[test]
fn linked_parameter_guard_rejects_nested_script_call_mismatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn require_i64(value: i64) {
    return value;
}

fn main(value) {
    return require_i64(value);
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::String("not an integer".to_owned())],
        &mut budget,
    )
    .expect_err("script call checked entry should reject mismatched argument");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "string".to_owned(),
            debug_name: "value".to_owned(),
        }
    );
}

#[test]
fn linked_static_safe_script_call_uses_unchecked_entry() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn require_i64(value: i64) {
    return value;
}

fn main() {
    return require_i64(12);
}
"#,
    )
    .expect("program should compile");
    let mut linked = Linker::new()
        .link_program(&program)
        .expect("program should link");
    let helper_handle = linked
        .entry_point_by_name("require_i64")
        .expect("helper entry should exist");
    let (_, helper) = linked
        .functions_mut()
        .find(|(handle, _)| *handle == helper_handle)
        .expect("helper should exist");
    let guard = helper.param_guards[0].guard;
    helper.type_guards[guard.index()].plan =
        vela_bytecode::TypeGuardPlan::Primitive(vela_common::PrimitiveTag::String);

    let mut budget = ExecutionBudget::unbounded();
    let value = Vm::new()
        .run_linked_program_with_budget(&linked, "main", &[], &mut budget)
        .expect("unchecked static-safe call should skip the poisoned param guard");
    assert_eq!(value, OwnedValue::i64(12));

    let mut budget = ExecutionBudget::unbounded();
    let error = Vm::new()
        .run_linked_program_with_budget(&linked, "require_i64", &[OwnedValue::i64(12)], &mut budget)
        .expect_err("public entry should still execute the poisoned param guard");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "string".to_owned(),
            actual: "i64".to_owned(),
            debug_name: "value".to_owned(),
        }
    );
}

#[test]
fn linked_parameter_guard_accepts_string_and_bytes_primitive_tags() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn echo_text(value: string) {
    return value;
}

fn echo_bytes(value: bytes) {
    return value;
}
"#,
    )
    .expect("program should compile");

    let mut budget = ExecutionBudget::unbounded();
    let text = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "echo_text",
        &[OwnedValue::String("ok".to_owned())],
        &mut budget,
    )
    .expect("string primitive guard should pass");
    assert_eq!(text, OwnedValue::String("ok".to_owned()));

    let bytes = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "echo_bytes",
        &[OwnedValue::Bytes(vec![0, 1, 255])],
        &mut budget,
    )
    .expect("bytes primitive guard should pass");
    assert_eq!(bytes, OwnedValue::Bytes(vec![0, 1, 255]));
}

#[test]
fn linked_return_guard_rejects_dynamic_contract_mismatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(value) -> i64 {
    return value;
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::String("not an integer".to_owned())],
        &mut budget,
    )
    .expect_err("return guard should reject mismatched value");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "string".to_owned(),
            debug_name: "return".to_owned(),
        }
    );
}

#[test]
fn linked_guard_type_accepts_record_type_and_shape_handles() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let reward_name = program.intern_debug_name("Reward");
    let count_name = program.intern_debug_name("count");
    let reward_type = program.push_type(vela_bytecode::LinkedType::new(
        vela_def::TypeId::new(0x177),
        reward_name,
    ));
    let expected_shape =
        crate::script_object::ScriptFields::single("Reward", "count", Value::i64(0)).shape_id();

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 3);
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
    let type_guard = code.intern_type_guard(vela_bytecode::TypeGuard::new(
        vela_bytecode::TypeGuardPlan::Type(reward_type),
        vela_bytecode::GuardContext::new(
            vela_bytecode::GuardKind::Contract,
            vela_bytecode::GuardLocation::Local,
            reward_name,
        ),
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::GuardType {
            src: Register(1),
            guard: type_guard,
        },
    ));
    let shape_guard = code.intern_type_guard(vela_bytecode::TypeGuard::new(
        vela_bytecode::TypeGuardPlan::Shape {
            ty: reward_type,
            shape_id: expected_shape,
        },
        vela_bytecode::GuardContext::new(
            vela_bytecode::GuardKind::Contract,
            vela_bytecode::GuardLocation::Local,
            count_name,
        ),
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::GuardType {
            src: Register(1),
            guard: shape_guard,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(0) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    program
        .verify()
        .expect("linked record guard fixture should verify");

    assert_eq!(
        Vm::new().run_linked_program(&program, "main", &[]),
        Ok(OwnedValue::i64(3))
    );
}

#[test]
fn linked_guard_type_rejects_mismatched_record_type_handle() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let reward_name = program.intern_debug_name("Reward");
    let other_name = program.intern_debug_name("Other");
    let count_name = program.intern_debug_name("count");
    let reward_type = program.push_type(vela_bytecode::LinkedType::new(
        vela_def::TypeId::new(0x277),
        reward_name,
    ));
    let other_type = program.push_type(vela_bytecode::LinkedType::new(
        vela_def::TypeId::new(0x278),
        other_name,
    ));

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
    let guard = code.intern_type_guard(vela_bytecode::TypeGuard::new(
        vela_bytecode::TypeGuardPlan::Type(other_type),
        vela_bytecode::GuardContext::new(
            vela_bytecode::GuardKind::Contract,
            vela_bytecode::GuardLocation::Local,
            count_name,
        ),
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::GuardType {
            src: Register(1),
            guard,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(0) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    program
        .verify()
        .expect("linked record mismatch fixture should verify");

    let error = Vm::new()
        .run_linked_program(&program, "main", &[])
        .expect_err("mismatched record type guard should fail");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "Other".to_owned(),
            actual: "record".to_owned(),
            debug_name: "count".to_owned(),
        }
    );
}

#[test]
fn linked_guard_type_rejects_mismatched_record_shape_handle() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let reward_name = program.intern_debug_name("Reward");
    let count_name = program.intern_debug_name("count");
    let reward_type = program.push_type(vela_bytecode::LinkedType::new(
        vela_def::TypeId::new(0x377),
        reward_name,
    ));
    let wrong_shape =
        crate::script_object::ScriptFields::single("Reward", "other", Value::i64(0)).shape_id();

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
    let guard = code.intern_type_guard(vela_bytecode::TypeGuard::new(
        vela_bytecode::TypeGuardPlan::Shape {
            ty: reward_type,
            shape_id: wrong_shape,
        },
        vela_bytecode::GuardContext::new(
            vela_bytecode::GuardKind::Contract,
            vela_bytecode::GuardLocation::Local,
            count_name,
        ),
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::GuardType {
            src: Register(1),
            guard,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(0) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    program
        .verify()
        .expect("linked record shape mismatch fixture should verify");

    let error = Vm::new()
        .run_linked_program(&program, "main", &[])
        .expect_err("mismatched record shape guard should fail");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "Reward".to_owned(),
            actual: "record".to_owned(),
            debug_name: "count".to_owned(),
        }
    );
}

#[test]
fn linked_guard_type_accepts_and_rejects_enum_variant_handles() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let damage_name = program.intern_debug_name("Damage");
    let physical_name = program.intern_debug_name("Damage::Physical");
    let magic_name = program.intern_debug_name("Damage::Magic");
    let amount_name = program.intern_debug_name("amount");
    let damage_type = program.push_type(vela_bytecode::LinkedType::new(
        vela_def::TypeId::new(0x477),
        damage_name,
    ));
    let physical_variant = program.push_variant(vela_bytecode::LinkedVariant::new(
        vela_def::VariantId::new(0x478),
        damage_type,
        physical_name,
    ));
    let magic_variant = program.push_variant(vela_bytecode::LinkedVariant::new(
        vela_def::VariantId::new(0x479),
        damage_type,
        magic_name,
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 2);
    let initial = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(7)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: initial,
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
    let physical_guard = code.intern_type_guard(vela_bytecode::TypeGuard::new(
        vela_bytecode::TypeGuardPlan::Variant(physical_variant),
        vela_bytecode::GuardContext::new(
            vela_bytecode::GuardKind::Contract,
            vela_bytecode::GuardLocation::Local,
            physical_name,
        ),
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::GuardType {
            src: Register(1),
            guard: physical_guard,
        },
    ));
    let magic_guard = code.intern_type_guard(vela_bytecode::TypeGuard::new(
        vela_bytecode::TypeGuardPlan::Variant(magic_variant),
        vela_bytecode::GuardContext::new(
            vela_bytecode::GuardKind::Contract,
            vela_bytecode::GuardLocation::Local,
            magic_name,
        ),
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::GuardType {
            src: Register(1),
            guard: magic_guard,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(0) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    program
        .verify()
        .expect("linked enum variant guard fixture should verify");

    let error = Vm::new()
        .run_linked_program(&program, "main", &[])
        .expect_err("mismatched enum variant guard should fail");
    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "Damage::Magic".to_owned(),
            actual: "enum".to_owned(),
            debug_name: "Damage::Magic".to_owned(),
        }
    );
}

#[test]
fn static_contract_mismatch_remains_compile_error() {
    compile_program_source(
        SourceId::new(1),
        r#"
fn require_i64(value: i64) {
    return value;
}

fn main() {
    return require_i64("not an integer");
}
"#,
    )
    .expect_err("statically known mismatch should not reach runtime");
}
