use super::*;

use std::cell::RefCell;

use vela_bytecode::{
    ChargedScalarTarget, Constant, I64CompareOp, ScalarBlockPlan, ScalarBlockPlanId,
    ScalarConstant, ScalarExit, ScalarExitKind, ScalarOp, ScalarOpKind, ScalarSourcePointId,
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

#[derive(Default)]
struct ScalarProfiler {
    subpoints: RefCell<Vec<(ScalarBlockPlanId, ScalarSourcePointId)>>,
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
