use super::*;

use std::cell::RefCell;

use vela_bytecode::{
    ChargedScalarTarget, I64CompareOp, ScalarBlockPlan, ScalarBlockPlanId, ScalarConstant,
    ScalarExit, ScalarExitKind, ScalarOp, ScalarOpKind, ScalarSourcePointId,
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
    ScalarBlockPlan {
        operations: Box::new([
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
        exit: ScalarExit {
            kind: ScalarExitKind::Jump(target(1)),
            source: source(3),
            execution_units: 1,
        },
        source_points: Box::new([span(0), span(1), span(2), span(3)]),
    }
}

#[test]
fn linked_scalar_block_executes_checked_operations_and_exact_budget_units() {
    let artifact = scalar_artifact(arithmetic_plan(), 3);
    let mut budget = ExecutionBudget::new(4, usize::MAX, usize::MAX);
    assert_eq!(
        Vm::new().run_linked_program_with_budget(&artifact, "main", &[], &mut budget),
        Ok(OwnedValue::i64(12))
    );
    assert_eq!(budget.execution_units_consumed(), 4);

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
}

#[test]
fn linked_scalar_block_stops_at_the_first_trap_source() {
    let plan = ScalarBlockPlan {
        operations: Box::new([
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
        exit: ScalarExit {
            kind: ScalarExitKind::Jump(target(1)),
            source: source(3),
            execution_units: 0,
        },
        source_points: Box::new([span(0), span(1), span(2), span(3)]),
    };
    let error = Vm::new()
        .run_linked_program(&scalar_artifact(plan, 3), "main", &[])
        .expect_err("checked addition should trap");
    assert_eq!(
        error.kind(),
        VmErrorKind::ArithmeticOverflow { operation: "add" }
    );
    assert_eq!(error.source_span, Some(span(1)));
}

#[test]
fn linked_scalar_block_fused_branch_selects_the_exact_exit() {
    let plan = ScalarBlockPlan {
        operations: Box::new([
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
        exit: ScalarExit {
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
        source_points: Box::new([span(0), span(1), span(2), span(3)]),
    };

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
    assert_eq!(
        Vm::new().run_linked_program(&artifact, "main", &[]),
        Ok(OwnedValue::i64(10))
    );
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
