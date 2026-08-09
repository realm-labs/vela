use std::alloc::System;
use std::error::Error;
use std::hint::black_box;
use std::sync::Arc;

use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use vela_bytecode::linked::{Instruction, InstructionKind};
use vela_bytecode::{
    ChargedScalarTarget, Constant, I64CompareOp, InstructionOffset, LinkedArtifact,
    LinkedCodeObject, LinkedProgram, Register, ScalarBlockPlan, ScalarBlockPlanId, ScalarExit,
    ScalarExitKind, ScalarOp, ScalarOpKind, ScalarSourcePointId,
};
use vela_common::{SourceId, Span};
use vela_vm::Vm;
use vela_vm::owned_value::OwnedValue;

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const MANY_ENTRIES: i64 = 10_001;

fn main() -> Result<(), Box<dyn Error>> {
    let one = loop_artifact(1);
    let many = loop_artifact(MANY_ENTRIES);
    let one_vm = Vm::new();
    let many_vm = Vm::new();

    black_box(one_vm.run_linked_program(&one, "main", &[])?);
    black_box(many_vm.run_linked_program(&many, "main", &[])?);

    let one_region = Region::new(GLOBAL);
    let one_result = black_box(one_vm.run_linked_program(&one, "main", &[])?);
    let one_stats = one_region.change();

    let many_region = Region::new(GLOBAL);
    let many_result = black_box(many_vm.run_linked_program(&many, "main", &[])?);
    let many_stats = many_region.change();

    assert_eq!(one_result, OwnedValue::i64(1));
    assert_eq!(many_result, OwnedValue::i64(MANY_ENTRIES));
    assert_eq!(many_stats.allocations, one_stats.allocations);
    assert_eq!(many_stats.bytes_allocated, one_stats.bytes_allocated);
    assert_eq!(many_stats.bytes_deallocated, one_stats.bytes_deallocated);

    println!(
        "scalar_block_allocation_result one_entries=1 many_entries={MANY_ENTRIES} one_allocations={} many_allocations={} incremental_allocations={} one_allocated_bytes={} many_allocated_bytes={} incremental_allocated_bytes=0 checksum={MANY_ENTRIES}",
        one_stats.allocations,
        many_stats.allocations,
        many_stats.allocations.saturating_sub(one_stats.allocations),
        one_stats.bytes_allocated,
        many_stats.bytes_allocated,
    );
    Ok(())
}

fn loop_artifact(limit: i64) -> Arc<LinkedArtifact> {
    let source_points = (0..4)
        .map(|index| Span::new(SourceId::new(902), index, index + 1))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let plan = ScalarBlockPlan::new(
        Box::new([
            ScalarOp {
                kind: ScalarOpKind::I64AddImm {
                    dst: Register(0),
                    lhs: Register(0),
                    imm: 1,
                },
                source: ScalarSourcePointId::new(0),
                execution_units: 0,
            },
            ScalarOp {
                kind: ScalarOpKind::I64AddImm {
                    dst: Register(1),
                    lhs: Register(1),
                    imm: 1,
                },
                source: ScalarSourcePointId::new(1),
                execution_units: 0,
            },
            ScalarOp {
                kind: ScalarOpKind::I64CompareImm {
                    dst: Register(2),
                    op: I64CompareOp::Less,
                    lhs: Register(0),
                    imm: limit,
                },
                source: ScalarSourcePointId::new(2),
                execution_units: 0,
            },
        ]),
        ScalarExit {
            kind: ScalarExitKind::BoolBranch {
                condition: Register(2),
                passed: target(2),
                failed: target(3),
            },
            source: ScalarSourcePointId::new(3),
            execution_units: 0,
        },
        source_points,
    );

    let mut program = LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let mut code = LinkedCodeObject::new(main_name, 3);
    let zero = code.push_constant(Constant::i64(0));
    for register in [Register(0), Register(1)] {
        code.push_instruction(Instruction::new(InstructionKind::LoadConst {
            dst: register,
            constant: zero,
        }));
    }
    code.scalar_blocks.push(plan);
    code.push_instruction(Instruction::new(InstructionKind::RunScalarBlock {
        plan: ScalarBlockPlanId::new(0),
    }));
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(1),
    }));
    code.verify().expect("allocation fixture should verify");
    let main = program.push_function(code);
    program.set_entry_point(main_name, main);
    vela_bytecode::test_support::linked_artifact(program)
}

const fn target(offset: usize) -> ChargedScalarTarget {
    ChargedScalarTarget {
        target: InstructionOffset(offset),
        execution_units: 0,
        budget_source: None,
    }
}
