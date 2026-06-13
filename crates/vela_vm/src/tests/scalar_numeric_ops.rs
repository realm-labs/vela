use super::*;
use vela_common::ScalarValue;

fn compile(source: &str) -> UnlinkedProgram {
    compile_program_source(SourceId::new(1), source).expect("program should compile")
}

fn call(program: &UnlinkedProgram, entry: &str, args: &[OwnedValue]) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_test_program_with_budget(&Vm::new(), program, entry, args, &mut budget)
}

#[test]
fn linked_normal_add_supports_every_scalar_tag() {
    let program = compile(
        r#"
fn add_i8(x: i8) { return x + 1i8; }
fn add_i16(x: i16) { return x + 1i16; }
fn add_i32(x: i32) { return x + 1i32; }
fn add_i64(x: i64) { return x + 1i64; }
fn add_u8(x: u8) { return x + 1u8; }
fn add_u16(x: u16) { return x + 1u16; }
fn add_u32(x: u32) { return x + 1u32; }
fn add_u64(x: u64) { return x + 1u64; }
fn add_f32(x: f32) { return x + 1.5f32; }
fn add_f64(x: f64) { return x + 1.5f64; }
"#,
    );

    for (entry, input, expected) in [
        (
            "add_i8",
            ScalarValue::I8(1),
            OwnedValue::Scalar(ScalarValue::I8(2)),
        ),
        (
            "add_i16",
            ScalarValue::I16(1),
            OwnedValue::Scalar(ScalarValue::I16(2)),
        ),
        (
            "add_i32",
            ScalarValue::I32(1),
            OwnedValue::Scalar(ScalarValue::I32(2)),
        ),
        (
            "add_i64",
            ScalarValue::I64(1),
            OwnedValue::Scalar(ScalarValue::I64(2)),
        ),
        (
            "add_u8",
            ScalarValue::U8(1),
            OwnedValue::Scalar(ScalarValue::U8(2)),
        ),
        (
            "add_u16",
            ScalarValue::U16(1),
            OwnedValue::Scalar(ScalarValue::U16(2)),
        ),
        (
            "add_u32",
            ScalarValue::U32(1),
            OwnedValue::Scalar(ScalarValue::U32(2)),
        ),
        (
            "add_u64",
            ScalarValue::U64(1),
            OwnedValue::Scalar(ScalarValue::U64(2)),
        ),
        (
            "add_f32",
            ScalarValue::F32(1.5),
            OwnedValue::Scalar(ScalarValue::F32(3.0)),
        ),
        (
            "add_f64",
            ScalarValue::F64(1.5),
            OwnedValue::Scalar(ScalarValue::F64(3.0)),
        ),
    ] {
        assert_eq!(
            call(&program, entry, &[OwnedValue::Scalar(input)]),
            Ok(expected)
        );
    }
}

#[test]
fn linked_normal_scalar_ops_cover_arithmetic_comparison_and_equality() {
    let program = compile(
        r#"
fn int_ops(x: i32) { return (((x - 2i32) * 3i32) / 2i32) % 5i32; }
fn uint_ops(x: u16) { return (((x - 2u16) * 3u16) / 2u16) % 5u16; }
fn float_ops(x: f32) { return ((x - 2.0f32) * 3.0f32) / 2.0f32; }
fn float_rem(x: f64) { return x % 2.0f64; }
fn less_u16(x: u16) { return x < 5u16; }
fn greater_equal_f32(x: f32) { return x >= 2.5f32; }
fn equal_i8(x) { return x == 1i8; }
"#,
    );

    assert_eq!(
        call(
            &program,
            "int_ops",
            &[OwnedValue::Scalar(ScalarValue::I32(6))]
        ),
        Ok(OwnedValue::Scalar(ScalarValue::I32(1)))
    );
    assert_eq!(
        call(
            &program,
            "uint_ops",
            &[OwnedValue::Scalar(ScalarValue::U16(6))]
        ),
        Ok(OwnedValue::Scalar(ScalarValue::U16(1)))
    );
    assert_eq!(
        call(
            &program,
            "float_ops",
            &[OwnedValue::Scalar(ScalarValue::F32(5.0))]
        ),
        Ok(OwnedValue::Scalar(ScalarValue::F32(4.5)))
    );
    assert_eq!(
        call(
            &program,
            "float_rem",
            &[OwnedValue::Scalar(ScalarValue::F64(5.5))]
        ),
        Ok(OwnedValue::Scalar(ScalarValue::F64(1.5)))
    );
    assert_eq!(
        call(
            &program,
            "less_u16",
            &[OwnedValue::Scalar(ScalarValue::U16(3))]
        ),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        call(
            &program,
            "greater_equal_f32",
            &[OwnedValue::Scalar(ScalarValue::F32(2.5))]
        ),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        call(
            &program,
            "equal_i8",
            &[OwnedValue::Scalar(ScalarValue::I8(1))]
        ),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        call(
            &program,
            "equal_i8",
            &[OwnedValue::Scalar(ScalarValue::I64(1))]
        ),
        Ok(OwnedValue::Bool(false))
    );
}

#[test]
fn linked_normal_scalar_negation_accepts_signed_and_float_only() {
    let program = compile(
        r#"
fn neg_i16(x: i16) { return -x; }
fn neg_f32(x: f32) { return -x; }
fn neg_u8(x: u8) { return -x; }
"#,
    );

    assert_eq!(
        call(
            &program,
            "neg_i16",
            &[OwnedValue::Scalar(ScalarValue::I16(3))]
        ),
        Ok(OwnedValue::Scalar(ScalarValue::I16(-3)))
    );
    assert_eq!(
        call(
            &program,
            "neg_f32",
            &[OwnedValue::Scalar(ScalarValue::F32(3.5))]
        ),
        Ok(OwnedValue::Scalar(ScalarValue::F32(-3.5)))
    );

    let error = call(
        &program,
        "neg_u8",
        &[OwnedValue::Scalar(ScalarValue::U8(3))],
    )
    .expect_err("unsigned scalar negation should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::TypeMismatch {
            operation: "negate"
        }
    );
}

#[test]
fn linked_normal_scalar_ops_reject_mixed_tags() {
    let program = compile(
        r#"
fn add_mixed(x) { return x + 1i8; }
"#,
    );

    let error = call(
        &program,
        "add_mixed",
        &[OwnedValue::Scalar(ScalarValue::I64(1))],
    )
    .expect_err("mixed scalar tags should fail");

    assert_eq!(error.kind(), VmErrorKind::TypeMismatch { operation: "add" });
}

#[test]
fn linked_normal_scalar_ops_check_overflow_and_division_by_zero() {
    let program = compile(
        r#"
fn inc_i8(x: i8) { return x + 1i8; }
fn dec_u8(x: u8) { return x - 1u8; }
fn div_zero_u32(x: u32) { return x / 0u32; }
fn neg_min_i8(x: i8) { return -x; }
"#,
    );

    let overflow = call(
        &program,
        "inc_i8",
        &[OwnedValue::Scalar(ScalarValue::I8(i8::MAX))],
    )
    .expect_err("signed overflow should fail");
    assert_eq!(
        overflow.kind(),
        VmErrorKind::ArithmeticOverflow { operation: "add" }
    );

    let underflow = call(
        &program,
        "dec_u8",
        &[OwnedValue::Scalar(ScalarValue::U8(0))],
    )
    .expect_err("unsigned underflow should fail");
    assert_eq!(
        underflow.kind(),
        VmErrorKind::ArithmeticOverflow { operation: "sub" }
    );

    let division_by_zero = call(
        &program,
        "div_zero_u32",
        &[OwnedValue::Scalar(ScalarValue::U32(4))],
    )
    .expect_err("division by zero should fail");
    assert_eq!(division_by_zero.kind(), VmErrorKind::DivisionByZero);

    let negate_overflow = call(
        &program,
        "neg_min_i8",
        &[OwnedValue::Scalar(ScalarValue::I8(i8::MIN))],
    )
    .expect_err("signed min negation should fail");
    assert_eq!(
        negate_overflow.kind(),
        VmErrorKind::ArithmeticOverflow {
            operation: "negate"
        }
    );
}

#[test]
fn linked_deferred_literal_arithmetic_reports_runtime_overflow() {
    let program = compile(
        r#"
fn add_ten(x) { return x + 10; }
"#,
    );

    let error = call(
        &program,
        "add_ten",
        &[OwnedValue::Scalar(ScalarValue::U8(250))],
    )
    .expect_err("deferred literal arithmetic overflow should fail");

    assert_eq!(
        error.kind(),
        VmErrorKind::ArithmeticOverflow {
            operation: "binary_int_literal"
        }
    );
}

#[test]
fn linked_i64_typed_arithmetic_instructions_execute() {
    let mut code = linked_i64_code(7);
    let ten = code.push_constant(Constant::Scalar(ScalarValue::I64(10)));
    let five = code.push_constant(Constant::Scalar(ScalarValue::I64(5)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: ten,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(1),
            constant: five,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::I64Add {
            dst: Register(2),
            lhs: Register(0),
            rhs: Register(1),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::I64MulImm {
            dst: Register(3),
            lhs: Register(2),
            imm: 3,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::I64SubImm {
            dst: Register(4),
            lhs: Register(3),
            imm: 5,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::I64RemImm {
            dst: Register(5),
            lhs: Register(4),
            imm: 17,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::I64AddImm {
            dst: Register(6),
            lhs: Register(5),
            imm: 1,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(6) },
    ));

    assert_eq!(
        run_linked_i64_code(code),
        Ok(OwnedValue::Scalar(ScalarValue::I64(7)))
    );
}

#[test]
fn linked_i64_typed_immediate_comparisons_execute() {
    let mut code = linked_i64_code(3);
    let ten = code.push_constant(Constant::Scalar(ScalarValue::I64(10)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: ten,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::I64CmpImm {
            dst: Register(1),
            op: vela_bytecode::I64CompareOp::Equal,
            lhs: Register(0),
            imm: 10,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::I64CmpImm {
            dst: Register(2),
            op: vela_bytecode::I64CompareOp::Greater,
            lhs: Register(0),
            imm: 9,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(2) },
    ));

    assert_eq!(run_linked_i64_code(code), Ok(OwnedValue::Bool(true)));
}

#[test]
fn linked_i64_typed_arithmetic_preserves_error_semantics_and_spans() {
    let span = Span::new(SourceId::new(9), 4, 12);
    let mut overflow_code = linked_i64_code(2);
    let max = overflow_code.push_constant(Constant::Scalar(ScalarValue::I64(i64::MAX)));
    overflow_code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: max,
        },
    ));
    overflow_code.push_instruction(
        vela_bytecode::linked::Instruction::new(
            vela_bytecode::linked::InstructionKind::I64AddImm {
                dst: Register(1),
                lhs: Register(0),
                imm: 1,
            },
        )
        .with_span(span),
    );
    overflow_code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(1) },
    ));

    let overflow = run_linked_i64_code(overflow_code).expect_err("overflow should fail");
    assert_eq!(
        overflow.kind(),
        VmErrorKind::ArithmeticOverflow { operation: "add" }
    );
    assert_eq!(overflow.source_span, Some(span));

    let mut rem_code = linked_i64_code(3);
    let four = rem_code.push_constant(Constant::Scalar(ScalarValue::I64(4)));
    let zero = rem_code.push_constant(Constant::Scalar(ScalarValue::I64(0)));
    rem_code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: four,
        },
    ));
    rem_code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(1),
            constant: zero,
        },
    ));
    rem_code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::I64Rem {
            dst: Register(2),
            lhs: Register(0),
            rhs: Register(1),
        },
    ));
    rem_code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(2) },
    ));

    let division_by_zero = run_linked_i64_code(rem_code).expect_err("rem by zero should fail");
    assert_eq!(division_by_zero.kind(), VmErrorKind::DivisionByZero);
}

fn linked_i64_code(register_count: u16) -> vela_bytecode::LinkedCodeObject {
    vela_bytecode::LinkedCodeObject::new(vela_registry::DebugNameId::new(0), register_count)
}

fn run_linked_i64_code(code: vela_bytecode::LinkedCodeObject) -> VmResult<OwnedValue> {
    code.verify().expect("linked i64 code should verify");
    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = code;
    code.debug_name = main_name;
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    program.verify().expect("linked i64 program should verify");
    Vm::new().run_linked_program(&program, "main", &[])
}
