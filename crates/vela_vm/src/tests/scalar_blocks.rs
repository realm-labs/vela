use super::*;

use std::cell::RefCell;

use vela_bytecode::{
    ChargedScalarEdge, ChargedScalarTarget, Constant, I64CompareOp, ScalarBlockPlan,
    ScalarBlockPlanId, ScalarConstant, ScalarExit, ScalarExitKind, ScalarOp, ScalarOpKind,
    ScalarRangeLoop, ScalarSourcePointId,
    linked::{Instruction, InstructionKind},
};

fn span(index: u32) -> Span {
    Span::new(SourceId::new(901), index, index + 1)
}

fn source(index: usize) -> ScalarSourcePointId {
    ScalarSourcePointId::new(index)
}

fn target(offset: usize) -> ChargedScalarTarget {
    ChargedScalarTarget {
        target: InstructionOffset(offset),
        execution_units: 0,
        budget_source: None,
    }
}

fn scalar_range_artifact(
    selected: bool,
    start: i64,
    end: i64,
    inclusive: bool,
    initial_total: i64,
) -> Arc<LinkedArtifact> {
    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 6);
    for (register, value) in [(0, start), (1, end), (4, initial_total)] {
        let constant = code.push_constant(Constant::i64(value));
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(register),
            constant,
        }));
    }
    let not_done = code.push_constant(Constant::Bool(false));
    code.push_instruction(Instruction::new(InstructionKind::LoadConst {
        dst: Register(2),
        constant: not_done,
    }));
    let done = InstructionOffset(if selected { 6 } else { 8 });
    code.push_instruction(
        Instruction::new(InstructionKind::I64RangeNext {
            cursor: Register(0),
            end: Register(1),
            done: Register(2),
            inclusive,
            dst: Register(3),
            jump_if_done: done,
        })
        .with_span(span(13))
        .with_execution_units(1),
    );

    if selected {
        let mut plan = ScalarBlockPlan::new(
            Box::new([
                ScalarOp {
                    kind: ScalarOpKind::Move {
                        dst: Register(5),
                        src: Register(3),
                    },
                    source: source(0),
                    execution_units: 0,
                },
                ScalarOp {
                    kind: ScalarOpKind::I64Add {
                        dst: Register(4),
                        lhs: Register(4),
                        rhs: Register(5),
                    },
                    source: source(1),
                    execution_units: 1,
                },
            ]),
            ScalarExit {
                kind: ScalarExitKind::Jump(target(4)),
                source: source(2),
                execution_units: 1,
            },
            Box::new([span(10), span(11), span(12), span(13)]),
        );
        plan.range_loop = Some(ScalarRangeLoop {
            cursor: Register(0),
            end: Register(1),
            done: Register(2),
            inclusive,
            dst: Register(3),
            header_source: source(3),
            header_execution_units: 1,
            next_edge: ChargedScalarEdge {
                execution_units: 0,
                budget_source: None,
            },
            done_target: target(done.0),
        });
        code.scalar_blocks.push(plan);
        code.push_instruction(Instruction::new(InstructionKind::RunScalarBlock {
            plan: ScalarBlockPlanId::new(0),
        }));
    } else {
        code.push_instruction(
            Instruction::new(InstructionKind::Move {
                dst: Register(5),
                src: Register(3),
            })
            .with_span(span(10)),
        );
        code.push_instruction(
            Instruction::new(InstructionKind::I64Add {
                dst: Register(4),
                lhs: Register(4),
                rhs: Register(5),
            })
            .with_span(span(11))
            .with_execution_units(1),
        );
        code.push_instruction(
            Instruction::new(InstructionKind::Jump {
                target: InstructionOffset(4),
            })
            .with_span(span(12))
            .with_execution_units(1),
        );
    }
    debug_assert_eq!(code.instructions.len(), done.0);
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(4),
    }));
    code.verify().expect("scalar range fixture should verify");
    let main = program.push_function(code);
    program.set_entry_point(main_name, main);
    linked_test_owner(program)
}

fn malformed_range_cursor_artifact(selected: bool) -> Arc<LinkedArtifact> {
    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 5);
    for (register, constant) in [
        (0, Constant::i64(0)),
        (1, Constant::i64(2)),
        (2, Constant::Bool(false)),
        (4, Constant::i64(0)),
    ] {
        let constant = code.push_constant(constant);
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: Register(register),
            constant,
        }));
    }
    let done = InstructionOffset(if selected { 6 } else { 8 });
    code.push_instruction(
        Instruction::new(InstructionKind::I64RangeNext {
            cursor: Register(0),
            end: Register(1),
            done: Register(2),
            inclusive: false,
            dst: Register(3),
            jump_if_done: done,
        })
        .with_span(span(13)),
    );
    if selected {
        let mut plan = ScalarBlockPlan::new(
            Box::new([
                ScalarOp {
                    kind: ScalarOpKind::LoadScalar {
                        dst: Register(0),
                        value: ScalarConstant::Bool(false),
                    },
                    source: source(0),
                    execution_units: 0,
                },
                ScalarOp {
                    kind: ScalarOpKind::Move {
                        dst: Register(4),
                        src: Register(3),
                    },
                    source: source(1),
                    execution_units: 0,
                },
            ]),
            ScalarExit {
                kind: ScalarExitKind::Jump(target(4)),
                source: source(2),
                execution_units: 0,
            },
            Box::new([span(10), span(11), span(12), span(13)]),
        );
        plan.range_loop = Some(ScalarRangeLoop {
            cursor: Register(0),
            end: Register(1),
            done: Register(2),
            inclusive: false,
            dst: Register(3),
            header_source: source(3),
            header_execution_units: 0,
            next_edge: ChargedScalarEdge {
                execution_units: 0,
                budget_source: None,
            },
            done_target: target(done.0),
        });
        code.scalar_blocks.push(plan);
        code.push_instruction(Instruction::new(InstructionKind::RunScalarBlock {
            plan: ScalarBlockPlanId::new(0),
        }));
    } else {
        let false_constant = code.push_constant(Constant::Bool(false));
        code.push_instruction(
            Instruction::new(InstructionKind::LoadConst {
                dst: Register(0),
                constant: false_constant,
            })
            .with_span(span(10)),
        );
        code.push_instruction(
            Instruction::new(InstructionKind::Move {
                dst: Register(4),
                src: Register(3),
            })
            .with_span(span(11)),
        );
        code.push_instruction(
            Instruction::new(InstructionKind::Jump {
                target: InstructionOffset(4),
            })
            .with_span(span(12)),
        );
    }
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(4),
    }));
    code.verify()
        .expect("malformed range cursor fixture should verify structurally");
    let main = program.push_function(code);
    program.set_entry_point(main_name, main);
    linked_test_owner(program)
}

fn scalar_artifact(plan: ScalarBlockPlan, register_count: u16) -> Arc<LinkedArtifact> {
    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, register_count);
    code.scalar_blocks.push(plan);
    code.push_instruction(Instruction::new(InstructionKind::RunScalarBlock {
        plan: ScalarBlockPlanId::new(0),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(register_count - 1),
    }));
    code.verify().expect("scalar block code should verify");
    let main = program.push_function(code);
    program.set_entry_point(main_name, main);
    program
        .verify()
        .expect("scalar block program should verify");
    linked_test_owner(program)
}

fn arithmetic_plan() -> ScalarBlockPlan {
    ScalarBlockPlan::new(
        Box::new([
            ScalarOp {
                kind: ScalarOpKind::LoadScalar {
                    dst: Register(0),
                    value: ScalarConstant::I64(4),
                },
                source: source(0),
                execution_units: 1,
            },
            ScalarOp {
                kind: ScalarOpKind::I64AddImm {
                    dst: Register(1),
                    lhs: Register(0),
                    imm: 2,
                },
                source: source(1),
                execution_units: 1,
            },
            ScalarOp {
                kind: ScalarOpKind::I64MulImm {
                    dst: Register(2),
                    lhs: Register(1),
                    imm: 2,
                },
                source: source(2),
                execution_units: 1,
            },
        ]),
        ScalarExit {
            kind: ScalarExitKind::Jump(target(1)),
            source: source(3),
            execution_units: 1,
        },
        Box::new([span(0), span(1), span(2), span(3)]),
    )
}

fn ordinary_arithmetic_artifact(initial: i64) -> Arc<LinkedArtifact> {
    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 3);
    let initial = code.push_constant(Constant::i64(initial));
    code.push_instruction(
        Instruction::new(InstructionKind::LoadConst {
            dst: Register(0),
            constant: initial,
        })
        .with_span(span(0))
        .with_execution_units(1),
    );
    code.push_instruction(
        Instruction::new(InstructionKind::I64AddImm {
            dst: Register(1),
            lhs: Register(0),
            imm: 2,
        })
        .with_span(span(1))
        .with_execution_units(1),
    );
    code.push_instruction(
        Instruction::new(InstructionKind::I64MulImm {
            dst: Register(2),
            lhs: Register(1),
            imm: 2,
        })
        .with_span(span(2))
        .with_execution_units(1),
    );
    code.push_instruction(
        Instruction::new(InstructionKind::Jump {
            target: InstructionOffset(4),
        })
        .with_span(span(3))
        .with_execution_units(1),
    );
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));
    code.verify()
        .expect("ordinary scalar fixture should verify");
    let main = program.push_function(code);
    program.set_entry_point(main_name, main);
    linked_test_owner(program)
}

fn ordinary_overflow_artifact() -> Arc<LinkedArtifact> {
    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 3);
    let maximum = code.push_constant(Constant::i64(i64::MAX));
    let unreachable = code.push_constant(Constant::i64(99));
    code.push_instruction(
        Instruction::new(InstructionKind::LoadConst {
            dst: Register(0),
            constant: maximum,
        })
        .with_span(span(0)),
    );
    code.push_instruction(
        Instruction::new(InstructionKind::I64AddImm {
            dst: Register(1),
            lhs: Register(0),
            imm: 1,
        })
        .with_span(span(1)),
    );
    code.push_instruction(
        Instruction::new(InstructionKind::LoadConst {
            dst: Register(2),
            constant: unreachable,
        })
        .with_span(span(2)),
    );
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));
    code.verify()
        .expect("ordinary overflow fixture should verify");
    let main = program.push_function(code);
    program.set_entry_point(main_name, main);
    linked_test_owner(program)
}

fn ordinary_branch_artifact() -> Arc<LinkedArtifact> {
    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 4);
    for (register, value, source_index) in [(0, 7, 0), (1, 10, 1), (2, 20, 2)] {
        let constant = code.push_constant(Constant::i64(value));
        code.push_instruction(
            Instruction::new(InstructionKind::LoadConst {
                dst: Register(register),
                constant,
            })
            .with_span(span(source_index)),
        );
    }
    code.push_instruction(
        Instruction::new(InstructionKind::Less {
            dst: Register(3),
            lhs: Register(0),
            rhs: Register(1),
        })
        .with_span(span(3)),
    );
    code.push_instruction(Instruction::new(InstructionKind::JumpIfFalse {
        condition: Register(3),
        target: InstructionOffset(6),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));
    code.verify()
        .expect("ordinary branch fixture should verify");
    let main = program.push_function(code);
    program.set_entry_point(main_name, main);
    linked_test_owner(program)
}

fn selected_loop_control_artifact() -> Arc<LinkedArtifact> {
    let plan = ScalarBlockPlan::new(
        Box::new([
            ScalarOp {
                kind: ScalarOpKind::I64AddImm {
                    dst: Register(0),
                    lhs: Register(0),
                    imm: 1,
                },
                source: source(0),
                execution_units: 0,
            },
            ScalarOp {
                kind: ScalarOpKind::I64AddImm {
                    dst: Register(1),
                    lhs: Register(1),
                    imm: 10,
                },
                source: source(1),
                execution_units: 0,
            },
            ScalarOp {
                kind: ScalarOpKind::I64CompareImm {
                    dst: Register(2),
                    op: I64CompareOp::Less,
                    lhs: Register(0),
                    imm: 4,
                },
                source: source(2),
                execution_units: 0,
            },
        ]),
        ScalarExit {
            kind: ScalarExitKind::BoolBranch {
                condition: Register(2),
                passed: target(2),
                failed: target(3),
            },
            source: source(3),
            execution_units: 0,
        },
        (2..6).map(span).collect::<Vec<_>>().into_boxed_slice(),
    );
    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 3);
    for (register, source_index) in [(0, 0), (1, 1)] {
        let zero = code.push_constant(Constant::i64(0));
        code.push_instruction(
            Instruction::new(InstructionKind::LoadConst {
                dst: Register(register),
                constant: zero,
            })
            .with_span(span(source_index)),
        );
    }
    code.scalar_blocks.push(plan);
    code.push_instruction(Instruction::new(InstructionKind::RunScalarBlock {
        plan: ScalarBlockPlanId::new(0),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));
    code.verify().expect("selected loop fixture should verify");
    let main = program.push_function(code);
    program.set_entry_point(main_name, main);
    linked_test_owner(program)
}

fn ordinary_loop_control_artifact() -> Arc<LinkedArtifact> {
    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 3);
    let zero = code.push_constant(Constant::i64(0));
    for (register, source_index) in [(0, 0), (1, 1)] {
        code.push_instruction(
            Instruction::new(InstructionKind::LoadConst {
                dst: Register(register),
                constant: zero,
            })
            .with_span(span(source_index)),
        );
    }
    code.push_instruction(
        Instruction::new(InstructionKind::I64AddImm {
            dst: Register(0),
            lhs: Register(0),
            imm: 1,
        })
        .with_span(span(2)),
    );
    code.push_instruction(
        Instruction::new(InstructionKind::I64AddImm {
            dst: Register(1),
            lhs: Register(1),
            imm: 10,
        })
        .with_span(span(3)),
    );
    code.push_instruction(
        Instruction::new(InstructionKind::I64CmpImm {
            dst: Register(2),
            op: I64CompareOp::Less,
            lhs: Register(0),
            imm: 4,
        })
        .with_span(span(4)),
    );
    code.push_instruction(
        Instruction::new(InstructionKind::JumpIfFalse {
            condition: Register(2),
            target: InstructionOffset(7),
        })
        .with_span(span(5)),
    );
    code.push_instruction(Instruction::new(InstructionKind::Jump {
        target: InstructionOffset(2),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));
    code.verify().expect("ordinary loop fixture should verify");
    let main = program.push_function(code);
    program.set_entry_point(main_name, main);
    linked_test_owner(program)
}

#[test]
fn linked_scalar_block_executes_checked_operations_and_exact_budget_units() {
    let artifact = scalar_artifact(arithmetic_plan(), 3);
    let ordinary = ordinary_arithmetic_artifact(4);
    let mut budget = ExecutionBudget::new(4, usize::MAX, usize::MAX);
    let selected_result =
        Vm::new().run_linked_program_with_budget(&artifact, "main", &[], &mut budget);
    let mut ordinary_budget = ExecutionBudget::new(4, usize::MAX, usize::MAX);
    let ordinary_result =
        Vm::new().run_linked_program_with_budget(&ordinary, "main", &[], &mut ordinary_budget);
    assert_eq!(selected_result, ordinary_result);
    assert_eq!(selected_result, Ok(OwnedValue::i64(12)));
    assert_eq!(budget.execution_units_consumed(), 4);
    assert_eq!(ordinary_budget.execution_units_consumed(), 4);

    let mut exhausted = ExecutionBudget::new(3, usize::MAX, usize::MAX);
    let error = Vm::new()
        .run_linked_program_with_budget(&artifact, "main", &[], &mut exhausted)
        .expect_err("terminator charge should exhaust the budget");
    assert_eq!(
        error.kind(),
        VmErrorKind::BudgetExceeded {
            budget: ExecutionBudgetKind::ExecutionUnits,
            limit: 3,
        }
    );
    assert_eq!(error.source_span, Some(span(3)));
    assert_eq!(exhausted.execution_units_consumed(), 3);
    let mut ordinary_exhausted = ExecutionBudget::new(3, usize::MAX, usize::MAX);
    let ordinary_error = Vm::new()
        .run_linked_program_with_budget(&ordinary, "main", &[], &mut ordinary_exhausted)
        .expect_err("ordinary terminator charge should exhaust the budget");
    assert_eq!(ordinary_error.kind(), error.kind());
    assert_eq!(ordinary_error.source_span, error.source_span);
    assert_eq!(ordinary_exhausted.execution_units_consumed(), 3);
}

#[test]
fn linked_scalar_block_stops_at_the_first_trap_source() {
    let plan = ScalarBlockPlan::new(
        Box::new([
            ScalarOp {
                kind: ScalarOpKind::LoadScalar {
                    dst: Register(0),
                    value: ScalarConstant::I64(i64::MAX),
                },
                source: source(0),
                execution_units: 0,
            },
            ScalarOp {
                kind: ScalarOpKind::I64AddImm {
                    dst: Register(1),
                    lhs: Register(0),
                    imm: 1,
                },
                source: source(1),
                execution_units: 0,
            },
            ScalarOp {
                kind: ScalarOpKind::LoadScalar {
                    dst: Register(2),
                    value: ScalarConstant::I64(99),
                },
                source: source(2),
                execution_units: 0,
            },
        ]),
        ScalarExit {
            kind: ScalarExitKind::Jump(target(1)),
            source: source(3),
            execution_units: 0,
        },
        Box::new([span(0), span(1), span(2), span(3)]),
    );
    let error = Vm::new()
        .run_linked_program(&scalar_artifact(plan, 3), "main", &[])
        .expect_err("checked addition should trap");
    let ordinary_error = Vm::new()
        .run_linked_program(&ordinary_overflow_artifact(), "main", &[])
        .expect_err("ordinary checked addition should trap");
    assert_eq!(
        error.kind(),
        VmErrorKind::ArithmeticOverflow { operation: "add" }
    );
    assert_eq!(error.source_span, Some(span(1)));
    assert_eq!(ordinary_error.kind(), error.kind());
    assert_eq!(ordinary_error.source_span, error.source_span);
}

#[test]
fn linked_scalar_block_rejects_a_malformed_entry_value_before_unchecked_slot_reuse() {
    let plan = ScalarBlockPlan::new(
        Box::new([
            ScalarOp {
                kind: ScalarOpKind::I64AddImm {
                    dst: Register(0),
                    lhs: Register(0),
                    imm: 1,
                },
                source: source(0),
                execution_units: 0,
            },
            ScalarOp {
                kind: ScalarOpKind::I64AddImm {
                    dst: Register(0),
                    lhs: Register(0),
                    imm: 1,
                },
                source: source(1),
                execution_units: 0,
            },
            ScalarOp {
                kind: ScalarOpKind::I64AddImm {
                    dst: Register(0),
                    lhs: Register(0),
                    imm: 1,
                },
                source: source(2),
                execution_units: 0,
            },
        ]),
        ScalarExit {
            kind: ScalarExitKind::Jump(target(1)),
            source: source(3),
            execution_units: 0,
        },
        Box::new([span(0), span(1), span(2), span(3)]),
    );
    let error = Vm::new()
        .run_linked_program(&scalar_artifact(plan, 1), "main", &[])
        .expect_err("Unit in a proven i64 lane should remain a type error");
    assert_eq!(error.kind(), VmErrorKind::TypeMismatch { operation: "add" });
    assert_eq!(error.source_span, Some(span(0)));
}

#[test]
fn linked_scalar_block_fused_branch_selects_the_exact_exit() {
    let plan = ScalarBlockPlan::new(
        Box::new([
            ScalarOp {
                kind: ScalarOpKind::LoadScalar {
                    dst: Register(0),
                    value: ScalarConstant::I64(7),
                },
                source: source(0),
                execution_units: 0,
            },
            ScalarOp {
                kind: ScalarOpKind::LoadScalar {
                    dst: Register(1),
                    value: ScalarConstant::I64(10),
                },
                source: source(1),
                execution_units: 0,
            },
            ScalarOp {
                kind: ScalarOpKind::LoadScalar {
                    dst: Register(2),
                    value: ScalarConstant::I64(20),
                },
                source: source(2),
                execution_units: 0,
            },
        ]),
        ScalarExit {
            kind: ScalarExitKind::I64CompareBranch {
                op: I64CompareOp::Less,
                lhs: Register(0),
                rhs: Register(1),
                passed: target(1),
                failed: target(2),
            },
            source: source(3),
            execution_units: 0,
        },
        Box::new([span(0), span(1), span(2), span(3)]),
    );

    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 3);
    code.scalar_blocks.push(plan);
    code.push_instruction(Instruction::new(InstructionKind::RunScalarBlock {
        plan: ScalarBlockPlanId::new(0),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));
    code.verify().expect("branch plan should verify");
    let main = program.push_function(code);
    program.set_entry_point(main_name, main);
    let artifact = linked_test_owner(program);
    let selected = Vm::new().run_linked_program(&artifact, "main", &[]);
    let ordinary = Vm::new().run_linked_program(&ordinary_branch_artifact(), "main", &[]);
    assert_eq!(selected, ordinary);
    assert_eq!(selected, Ok(OwnedValue::i64(10)));
}

#[test]
fn linked_scalar_block_loop_matches_ordinary_continue_and_break_exits() {
    let selected = Vm::new().run_linked_program(&selected_loop_control_artifact(), "main", &[]);
    let ordinary = Vm::new().run_linked_program(&ordinary_loop_control_artifact(), "main", &[]);
    assert_eq!(selected, ordinary);
    assert_eq!(selected, Ok(OwnedValue::i64(40)));
}

#[test]
fn linked_scalar_range_loop_matches_empty_one_element_and_bound_modes() {
    for (start, end, inclusive, expected) in [
        (4, 4, false, 0),
        (3, 4, false, 3),
        (1, 4, false, 6),
        (1, 3, true, 6),
        (i64::MAX, i64::MAX, true, i64::MAX),
    ] {
        let selected = Vm::new().run_linked_program(
            &scalar_range_artifact(true, start, end, inclusive, 0),
            "main",
            &[],
        );
        let ordinary = Vm::new().run_linked_program(
            &scalar_range_artifact(false, start, end, inclusive, 0),
            "main",
            &[],
        );
        assert_eq!(selected, ordinary, "range {start}..{end}");
        assert_eq!(selected, Ok(OwnedValue::i64(expected)));
    }
}

#[test]
fn linked_scalar_range_loop_matches_exact_budget_failure_points() {
    let selected = scalar_range_artifact(true, 0, 4, false, 0);
    let ordinary = scalar_range_artifact(false, 0, 4, false, 0);
    for limit in 0..=13 {
        let mut selected_budget = ExecutionBudget::new(limit, usize::MAX, usize::MAX);
        let selected_result =
            Vm::new().run_linked_program_with_budget(&selected, "main", &[], &mut selected_budget);
        let mut ordinary_budget = ExecutionBudget::new(limit, usize::MAX, usize::MAX);
        let ordinary_result =
            Vm::new().run_linked_program_with_budget(&ordinary, "main", &[], &mut ordinary_budget);
        assert_eq!(selected_result, ordinary_result, "budget limit {limit}");
        assert_eq!(
            selected_budget.execution_units_consumed(),
            ordinary_budget.execution_units_consumed(),
            "budget limit {limit}"
        );
    }
    let mut exact = ExecutionBudget::new(13, usize::MAX, usize::MAX);
    assert_eq!(
        Vm::new().run_linked_program_with_budget(&selected, "main", &[], &mut exact),
        Ok(OwnedValue::i64(6))
    );
    assert_eq!(exact.execution_units_consumed(), 13);
}

#[test]
fn linked_scalar_range_loop_preserves_iteration_n_overflow() {
    let selected = Vm::new()
        .run_linked_program(
            &scalar_range_artifact(true, 0, 4, false, i64::MAX - 1),
            "main",
            &[],
        )
        .expect_err("third iteration should overflow");
    let ordinary = Vm::new()
        .run_linked_program(
            &scalar_range_artifact(false, 0, 4, false, i64::MAX - 1),
            "main",
            &[],
        )
        .expect_err("ordinary third iteration should overflow");
    assert_eq!(selected.kind(), ordinary.kind());
    assert_eq!(selected.source_span, Some(span(11)));
    assert_eq!(selected.source_span, ordinary.source_span);
}

#[test]
fn linked_scalar_range_loop_rejects_a_malformed_internal_header_value() {
    let selected = Vm::new()
        .run_linked_program(&malformed_range_cursor_artifact(true), "main", &[])
        .expect_err("internal range header should reject a bool cursor");
    let ordinary = Vm::new()
        .run_linked_program(&malformed_range_cursor_artifact(false), "main", &[])
        .expect_err("ordinary range header should reject a bool cursor");
    assert_eq!(
        selected.kind(),
        VmErrorKind::TypeMismatch { operation: "range" }
    );
    assert_eq!(selected.kind(), ordinary.kind());
    assert_eq!(selected.source_span, None);
    assert_eq!(selected.source_span, ordinary.source_span);
}

#[derive(Default)]
struct ScalarProfiler {
    subpoints: RefCell<Vec<(ScalarBlockPlanId, ScalarSourcePointId)>>,
    loop_events: RefCell<Vec<ScalarLoopProfileEvent>>,
}

impl VmBytecodeProfiler for ScalarProfiler {
    fn record_scalar_subpoint(
        &self,
        _function: vela_bytecode::DebugNameId,
        _offset: InstructionOffset,
        plan: ScalarBlockPlanId,
        source: ScalarSourcePointId,
    ) {
        self.subpoints.borrow_mut().push((plan, source));
    }

    fn record_scalar_loop_event(
        &self,
        _function: vela_bytecode::DebugNameId,
        _offset: InstructionOffset,
        _plan: ScalarBlockPlanId,
        event: ScalarLoopProfileEvent,
    ) {
        self.loop_events.borrow_mut().push(event);
    }
}

#[test]
fn profiled_scalar_block_records_every_logical_subpoint() {
    let artifact = scalar_artifact(arithmetic_plan(), 3);
    let main = artifact.entry_point_by_name("main").expect("main entry");
    let profiler = ScalarProfiler::default();
    let mut budget = ExecutionBudget::unbounded();
    let result = Vm::new()
        .execute_linked_call(
            crate::linked_execution::LinkedExecutionCall {
                owner: Arc::clone(&artifact),
                function: main,
                captures: &[],
                args: &[],
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: None,
                bytecode_profiler: Some(&profiler),
            },
            None,
            None,
            Some(&mut budget),
        )
        .expect("profiled scalar block should execute");
    assert_eq!(result, Value::I64(12));
    assert_eq!(
        profiler.subpoints.into_inner(),
        vec![
            (ScalarBlockPlanId::new(0), source(0)),
            (ScalarBlockPlanId::new(0), source(1)),
            (ScalarBlockPlanId::new(0), source(2)),
            (ScalarBlockPlanId::new(0), source(3)),
        ]
    );
}

#[test]
fn profiled_scalar_range_loop_records_entries_iterations_exits_and_backedges() {
    let artifact = scalar_range_artifact(true, 0, 3, false, 0);
    let main = artifact.entry_point_by_name("main").expect("main entry");
    let profiler = ScalarProfiler::default();
    let mut budget = ExecutionBudget::unbounded();
    let result = Vm::new()
        .execute_linked_call(
            crate::linked_execution::LinkedExecutionCall {
                owner: Arc::clone(&artifact),
                function: main,
                captures: &[],
                args: &[],
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: None,
                bytecode_profiler: Some(&profiler),
            },
            None,
            None,
            Some(&mut budget),
        )
        .expect("profiled scalar range loop should execute");
    assert_eq!(result, Value::I64(3));
    assert_eq!(
        profiler.loop_events.into_inner(),
        [
            ScalarLoopProfileEvent::Entry,
            ScalarLoopProfileEvent::Iteration,
            ScalarLoopProfileEvent::ChargedBackedge,
            ScalarLoopProfileEvent::Iteration,
            ScalarLoopProfileEvent::ChargedBackedge,
            ScalarLoopProfileEvent::Iteration,
            ScalarLoopProfileEvent::ChargedBackedge,
            ScalarLoopProfileEvent::Exit,
        ]
    );
}
