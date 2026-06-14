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
            actual: "String".to_owned(),
            debug_name: "amount".to_owned(),
        }
    );
}

#[test]
fn linked_specialization_guard_mismatch_falls_back_without_language_error() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let value_name = program.intern_debug_name("value");

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 1);
    let value = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(7)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    let guard = code.intern_type_guard(vela_bytecode::TypeGuard::new(
        vela_bytecode::TypeGuardPlan::Primitive(vela_common::PrimitiveTag::String),
        vela_bytecode::GuardContext::new(
            vela_bytecode::GuardKind::Specialization,
            vela_bytecode::GuardLocation::Local,
            value_name,
        ),
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::GuardType {
            src: Register(0),
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
        .expect("linked specialization guard fixture should verify");

    assert_eq!(
        Vm::new().run_linked_program(&program, "main", &[]),
        Ok(OwnedValue::i64(7))
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
            actual: "String".to_owned(),
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
            actual: "String".to_owned(),
            debug_name: "value".to_owned(),
        }
    );
}

#[test]
fn linked_parameter_guard_rejects_mixed_array_contents() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(values: Array<i64>) {
    let total = 0;
    for value in values {
        total += value;
    }
    return total;
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::array([
            OwnedValue::i64(1),
            OwnedValue::from("bad"),
        ])],
        &mut budget,
    )
    .expect_err("array element contract should fail before body executes");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "String".to_owned(),
            debug_name: "values".to_owned(),
        }
    );
}

#[test]
fn linked_parameter_guard_charges_budget_for_array_scan() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(values: Array<i64>) {
    return 1;
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::new(1, usize::MAX, usize::MAX);

    let error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::array([OwnedValue::i64(1), OwnedValue::i64(2)])],
        &mut budget,
    )
    .expect_err("array guard scan should consume instruction budget");

    assert_eq!(
        error.kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::Instructions,
            limit: 1,
        }
    );
    assert_eq!(budget.instructions_executed(), 1);
}

#[test]
fn linked_parameter_guard_rejects_mixed_map_values() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(values: Map<String, i64>) {
    return values.get("level").unwrap_or(0);
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::map([
            ("level", OwnedValue::i64(1)),
            ("bad", OwnedValue::from("high")),
        ])],
        &mut budget,
    )
    .expect_err("map value contract should fail before body executes");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "String".to_owned(),
            debug_name: "values".to_owned(),
        }
    );
}

#[test]
fn linked_local_guard_accepts_value_keyed_map_keys() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn accept(values) {
    let typed: Map<i64, String> = values;
    return typed.get_or(1, "missing");
}

fn main() {
    let values = {"seed": ""};
    values.clear();
    values.set(1, "one");
    return accept(values);
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let value = run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget)
        .expect("i64 map key contract should pass");

    assert_eq!(value, OwnedValue::String("one".to_owned()));
}

#[test]
fn linked_local_guard_rejects_mismatched_map_keys() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn accept(values) {
    let typed: Map<i64, String> = values;
    return typed.len();
}

fn main() {
    return accept({"seed": "bad"});
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let error = run_linked_test_program_with_budget(&Vm::new(), &program, "main", &[], &mut budget)
        .expect_err("map key contract should fail before body uses typed map");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "String".to_owned(),
            debug_name: "typed".to_owned(),
        }
    );
}

#[test]
fn linked_parameter_guard_rejects_mixed_set_values() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(values: Set<String>) {
    return values.len();
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::set([
            OwnedValue::from("ok"),
            OwnedValue::i64(1),
        ])],
        &mut budget,
    )
    .expect_err("set element contract should fail before body executes");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "String".to_owned(),
            actual: "i64".to_owned(),
            debug_name: "values".to_owned(),
        }
    );
}

#[test]
fn linked_parameter_guard_marks_parameterized_iterators_without_consuming() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(values: Iterator<i64>) {
    return values.next().unwrap_or(0);
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let value = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::iterator([OwnedValue::i64(1)])],
        &mut budget,
    )
    .expect("Iterator<i64> guard should validate lazily without consuming entry item");

    assert_eq!(value, OwnedValue::i64(1));
}

#[test]
fn linked_parameter_guard_rejects_parameterized_iterator_item_mismatch_when_yielded() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(values: Iterator<i64>) {
    return values.next().unwrap_or(0);
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::iterator([OwnedValue::String("bad".to_owned())])],
        &mut budget,
    )
    .expect_err("Iterator<i64> should validate yielded items lazily");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "String".to_owned(),
            debug_name: "values".to_owned(),
        }
    );
}

#[test]
fn linked_parameter_guard_does_not_consume_invalid_unread_iterator_items() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(values: Iterator<i64>) {
    return 1;
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let value = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::iterator([OwnedValue::String("bad".to_owned())])],
        &mut budget,
    )
    .expect("Iterator<i64> entry guard should not eagerly consume items");

    assert_eq!(value, OwnedValue::i64(1));
}

#[test]
fn linked_parameter_guard_accepts_erased_iterator_any_contract() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main(values: Iterator<Any>) {
    return values.next().unwrap_or(0);
}
"#,
    )
    .expect("program should compile");
    let mut budget = ExecutionBudget::unbounded();

    let value = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::iterator([OwnedValue::i64(7)])],
        &mut budget,
    )
    .expect("Iterator<Any> should remain an erased iterator contract");

    assert_eq!(value, OwnedValue::i64(7));
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
            expected: "String".to_owned(),
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
fn echo_text(value: String) {
    return value;
}

fn echo_bytes(value: Bytes) {
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
fn linked_parameter_guard_accepts_option_and_result_payload_contracts() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn maybe_amount(value: Option<i64>) {
    return value;
}

fn grant(value: Result<i64, String>) {
    return value;
}
"#,
    )
    .expect("program should compile");

    let some = OwnedValue::enum_variant("Option", "Some", [("0", OwnedValue::i64(42))]);
    let none = OwnedValue::enum_variant("Option", "None", Vec::<(&str, OwnedValue)>::new());
    let ok = OwnedValue::enum_variant("Result", "Ok", [("0", OwnedValue::i64(7))]);
    let err = OwnedValue::enum_variant(
        "Result",
        "Err",
        [("0", OwnedValue::String("blocked".to_owned()))],
    );

    let mut budget = ExecutionBudget::unbounded();
    assert_eq!(
        run_linked_test_program_with_budget(
            &Vm::new(),
            &program,
            "maybe_amount",
            &[some],
            &mut budget
        )
        .expect("Option::Some payload guard should pass"),
        OwnedValue::enum_variant("Option", "Some", [("0", OwnedValue::i64(42))])
    );
    assert_eq!(
        run_linked_test_program_with_budget(
            &Vm::new(),
            &program,
            "maybe_amount",
            &[none],
            &mut budget
        )
        .expect("Option::None should not check a payload"),
        OwnedValue::enum_variant("Option", "None", Vec::<(&str, OwnedValue)>::new())
    );
    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "grant", &[ok], &mut budget)
            .expect("Result::Ok payload guard should pass"),
        OwnedValue::enum_variant("Result", "Ok", [("0", OwnedValue::i64(7))])
    );
    assert_eq!(
        run_linked_test_program_with_budget(&Vm::new(), &program, "grant", &[err], &mut budget)
            .expect("Result::Err payload guard should pass"),
        OwnedValue::enum_variant(
            "Result",
            "Err",
            [("0", OwnedValue::String("blocked".to_owned()))],
        )
    );
}

#[test]
fn linked_parameter_guard_rejects_option_and_result_payload_mismatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn maybe_amount(value: Option<i64>) {
    return value;
}

fn grant(value: Result<i64, String>) {
    return value;
}
"#,
    )
    .expect("program should compile");

    let mut budget = ExecutionBudget::unbounded();
    let option_error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "maybe_amount",
        &[OwnedValue::enum_variant(
            "Option",
            "Some",
            [("0", OwnedValue::String("wrong".to_owned()))],
        )],
        &mut budget,
    )
    .expect_err("Option::Some payload mismatch should fail");
    assert_eq!(
        option_error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "i64".to_owned(),
            actual: "String".to_owned(),
            debug_name: "value".to_owned(),
        }
    );

    let result_error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "grant",
        &[OwnedValue::enum_variant(
            "Result",
            "Err",
            [("0", OwnedValue::i64(9))],
        )],
        &mut budget,
    )
    .expect_err("Result::Err payload mismatch should fail");
    assert_eq!(
        result_error.kind(),
        VmErrorKind::TypeContractViolation {
            expected: "String".to_owned(),
            actual: "i64".to_owned(),
            debug_name: "value".to_owned(),
        }
    );
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
            actual: "String".to_owned(),
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
