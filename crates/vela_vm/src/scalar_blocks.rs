//! Focused executor for verified compact scalar basic-block plans.

use vela_bytecode::{
    ChargedScalarEdge, ChargedScalarTarget, DebugNameId, InstructionOffset, Register,
    ScalarBlockPlan, ScalarBlockPlanId, ScalarConstant, ScalarExitKind, ScalarOpKind,
    ScalarRangeLoop, ScalarSourcePointId,
};
use vela_common::Span;

use crate::budget::ExecutionBudget;
use crate::error::{VmError, VmErrorKind, VmResult};
use crate::frame::CallFrame;
use crate::value::Value;
use crate::{ScalarLoopProfileEvent, VmBytecodeProfiler, i64_ops};

pub(crate) struct ScalarBlockExecution<'a> {
    pub(crate) plan_id: ScalarBlockPlanId,
    pub(crate) plan: &'a ScalarBlockPlan,
    pub(crate) function: DebugNameId,
    pub(crate) instruction: InstructionOffset,
    pub(crate) profiler: Option<&'a dyn VmBytecodeProfiler>,
}

pub(crate) fn execute_scalar_block<const CHARGE_BUDGET: bool, const PROFILE: bool>(
    execution: ScalarBlockExecution<'_>,
    frame: &mut CallFrame,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<InstructionOffset> {
    match execution.plan.range_loop {
        Some(range_loop) => execute_scalar_range_loop::<CHARGE_BUDGET, PROFILE>(
            execution, frame, budget, range_loop,
        ),
        None => execute_scalar_block_once::<CHARGE_BUDGET, PROFILE>(execution, frame, budget),
    }
}

fn execute_scalar_block_once<const CHARGE_BUDGET: bool, const PROFILE: bool>(
    execution: ScalarBlockExecution<'_>,
    frame: &mut CallFrame,
    budget: &mut Option<&mut ExecutionBudget>,
) -> VmResult<InstructionOffset> {
    let ScalarBlockExecution {
        plan_id,
        plan,
        function,
        instruction,
        profiler,
    } = execution;
    let registers = frame.scalar_registers_mut();
    for operation in &plan.operations {
        profile_subpoint::<PROFILE>(profiler, function, instruction, plan_id, operation.source);
        charge::<CHARGE_BUDGET>(plan, budget, operation.execution_units, operation.source)?;
        if let Err(error) = execute_operation(registers, operation.kind) {
            return Err(error.with_source_span_if_absent(source_point(plan, operation.source)));
        }
    }
    profile_subpoint::<PROFILE>(profiler, function, instruction, plan_id, plan.exit.source);
    charge::<CHARGE_BUDGET>(plan, budget, plan.exit.execution_units, plan.exit.source)?;
    let target = scalar_exit_target(registers, plan)?;
    charge_target::<CHARGE_BUDGET>(plan, budget, target)?;
    Ok(target.target)
}

#[inline(never)]
fn execute_scalar_range_loop<const CHARGE_BUDGET: bool, const PROFILE: bool>(
    execution: ScalarBlockExecution<'_>,
    frame: &mut CallFrame,
    budget: &mut Option<&mut ExecutionBudget>,
    range_loop: ScalarRangeLoop,
) -> VmResult<InstructionOffset> {
    let ScalarBlockExecution {
        plan_id,
        plan,
        function,
        instruction,
        profiler,
    } = execution;
    let registers = frame.scalar_registers_mut();
    profile_loop_event::<PROFILE>(
        profiler,
        function,
        instruction,
        plan_id,
        ScalarLoopProfileEvent::Entry,
    );
    loop {
        profile_loop_event::<PROFILE>(
            profiler,
            function,
            instruction,
            plan_id,
            ScalarLoopProfileEvent::Iteration,
        );
        for operation in &plan.operations {
            profile_subpoint::<PROFILE>(profiler, function, instruction, plan_id, operation.source);
            charge::<CHARGE_BUDGET>(plan, budget, operation.execution_units, operation.source)?;
            if let Err(error) = execute_operation(registers, operation.kind) {
                return Err(error.with_source_span_if_absent(source_point(plan, operation.source)));
            }
        }

        profile_subpoint::<PROFILE>(profiler, function, instruction, plan_id, plan.exit.source);
        charge::<CHARGE_BUDGET>(plan, budget, plan.exit.execution_units, plan.exit.source)?;
        let target =
            match plan.exit.kind {
                ScalarExitKind::Fallthrough(target) | ScalarExitKind::Jump(target) => target,
                ScalarExitKind::BoolBranch {
                    condition,
                    passed,
                    failed,
                } => {
                    let condition = match read_bool(registers, condition, "scalar block branch") {
                        Ok(condition) => condition,
                        Err(error) => {
                            return Err(error
                                .with_source_span_if_absent(source_point(plan, plan.exit.source)));
                        }
                    };
                    if condition { passed } else { failed }
                }
                ScalarExitKind::I64CompareBranch {
                    op,
                    lhs,
                    rhs,
                    passed,
                    failed,
                } => {
                    let lhs = match read_i64(registers, lhs, "scalar block compare") {
                        Ok(value) => value,
                        Err(error) => {
                            return Err(error
                                .with_source_span_if_absent(source_point(plan, plan.exit.source)));
                        }
                    };
                    let rhs = match read_i64(registers, rhs, "scalar block compare") {
                        Ok(value) => value,
                        Err(error) => {
                            return Err(error
                                .with_source_span_if_absent(source_point(plan, plan.exit.source)));
                        }
                    };
                    if i64_ops::compare(lhs, op, rhs) {
                        passed
                    } else {
                        failed
                    }
                }
            };
        profile_loop_event::<PROFILE>(
            profiler,
            function,
            instruction,
            plan_id,
            ScalarLoopProfileEvent::ChargedBackedge,
        );
        charge_target::<CHARGE_BUDGET>(plan, budget, target)?;
        debug_assert_eq!(target.target, range_header_target(plan));
        if execute_range_header::<CHARGE_BUDGET, PROFILE>(
            ScalarBlockExecution {
                plan_id,
                plan,
                function,
                instruction,
                profiler,
            },
            registers,
            budget,
            range_loop,
        )? {
            profile_loop_event::<PROFILE>(
                profiler,
                function,
                instruction,
                plan_id,
                ScalarLoopProfileEvent::Exit,
            );
            charge_target::<CHARGE_BUDGET>(plan, budget, range_loop.done_target)?;
            return Ok(range_loop.done_target.target);
        }
        charge_edge::<CHARGE_BUDGET>(plan, budget, range_loop.next_edge)?;
    }
}

#[inline(always)]
fn scalar_exit_target(
    registers: &[Value],
    plan: &ScalarBlockPlan,
) -> VmResult<ChargedScalarTarget> {
    match plan.exit.kind {
        ScalarExitKind::Fallthrough(target) | ScalarExitKind::Jump(target) => Ok(target),
        ScalarExitKind::BoolBranch {
            condition,
            passed,
            failed,
        } => read_bool(registers, condition, "scalar block branch")
            .map(|condition| if condition { passed } else { failed })
            .map_err(|error| {
                error.with_source_span_if_absent(source_point(plan, plan.exit.source))
            }),
        ScalarExitKind::I64CompareBranch {
            op,
            lhs,
            rhs,
            passed,
            failed,
        } => {
            let lhs = read_i64(registers, lhs, "scalar block compare").map_err(|error| {
                error.with_source_span_if_absent(source_point(plan, plan.exit.source))
            })?;
            let rhs = read_i64(registers, rhs, "scalar block compare").map_err(|error| {
                error.with_source_span_if_absent(source_point(plan, plan.exit.source))
            })?;
            Ok(if i64_ops::compare(lhs, op, rhs) {
                passed
            } else {
                failed
            })
        }
    }
}

fn execute_range_header<const CHARGE_BUDGET: bool, const PROFILE: bool>(
    execution: ScalarBlockExecution<'_>,
    registers: &mut [Value],
    budget: &mut Option<&mut ExecutionBudget>,
    range_loop: ScalarRangeLoop,
) -> VmResult<bool> {
    profile_subpoint::<PROFILE>(
        execution.profiler,
        execution.function,
        execution.instruction,
        execution.plan_id,
        range_loop.header_source,
    );
    charge::<CHARGE_BUDGET>(
        execution.plan,
        budget,
        range_loop.header_execution_units,
        range_loop.header_source,
    )?;
    if read_bool(registers, range_loop.done, "range")? {
        return Ok(true);
    }
    let current = read_i64(registers, range_loop.cursor, "range")?;
    let end = read_i64(registers, range_loop.end, "range")?;
    let has_next = if range_loop.inclusive {
        current <= end
    } else {
        current < end
    };
    if !has_next {
        write(registers, range_loop.done, Value::Bool(true))?;
        return Ok(true);
    }
    write(registers, range_loop.dst, Value::I64(current))?;
    if current == i64::MAX {
        write(registers, range_loop.done, Value::Bool(true))
    } else {
        write(registers, range_loop.cursor, Value::I64(current + 1))
    }?;
    Ok(false)
}

fn range_header_target(plan: &ScalarBlockPlan) -> InstructionOffset {
    match plan.exit.kind {
        ScalarExitKind::Jump(target) => target.target,
        _ => unreachable!("verified scalar range loop has an unconditional latch"),
    }
}

#[inline(always)]
fn execute_operation(registers: &mut [Value], operation: ScalarOpKind) -> VmResult<()> {
    match operation {
        ScalarOpKind::LoadScalar { dst, value } => write(
            registers,
            dst,
            match value {
                ScalarConstant::Bool(value) => Value::Bool(value),
                ScalarConstant::I64(value) => Value::I64(value),
            },
        ),
        ScalarOpKind::Move { dst, src } => {
            let value = read(registers, src);
            write(registers, dst, value)
        }
        ScalarOpKind::BoolNot { dst, src } => {
            let value = read_bool(registers, src, "scalar bool not")?;
            write(registers, dst, Value::Bool(!value))
        }
        ScalarOpKind::I64Add { dst, lhs, rhs } => {
            let value = i64_ops::add_raw(
                read_i64(registers, lhs, "add")?,
                read_i64(registers, rhs, "add")?,
            )?;
            write(registers, dst, Value::I64(value))
        }
        ScalarOpKind::I64Sub { dst, lhs, rhs } => {
            let value = i64_ops::sub_raw(
                read_i64(registers, lhs, "sub")?,
                read_i64(registers, rhs, "sub")?,
            )?;
            write(registers, dst, Value::I64(value))
        }
        ScalarOpKind::I64Mul { dst, lhs, rhs } => {
            let value = i64_ops::mul_raw(
                read_i64(registers, lhs, "mul")?,
                read_i64(registers, rhs, "mul")?,
            )?;
            write(registers, dst, Value::I64(value))
        }
        ScalarOpKind::I64Rem { dst, lhs, rhs } => {
            let value = i64_ops::rem_raw(
                read_i64(registers, lhs, "rem")?,
                read_i64(registers, rhs, "rem")?,
            )?;
            write(registers, dst, Value::I64(value))
        }
        ScalarOpKind::I64AddImm { dst, lhs, imm } => {
            let value = i64_ops::add_raw(read_i64(registers, lhs, "add")?, imm)?;
            write(registers, dst, Value::I64(value))
        }
        ScalarOpKind::I64SubImm { dst, lhs, imm } => {
            let value = i64_ops::sub_raw(read_i64(registers, lhs, "sub")?, imm)?;
            write(registers, dst, Value::I64(value))
        }
        ScalarOpKind::I64MulImm { dst, lhs, imm } => {
            let value = i64_ops::mul_raw(read_i64(registers, lhs, "mul")?, imm)?;
            write(registers, dst, Value::I64(value))
        }
        ScalarOpKind::I64RemImm { dst, lhs, imm } => {
            let value = i64_ops::rem_raw(read_i64(registers, lhs, "rem")?, imm)?;
            write(registers, dst, Value::I64(value))
        }
        ScalarOpKind::I64Compare { dst, op, lhs, rhs } => {
            let lhs = read_i64(registers, lhs, "compare")?;
            let rhs = read_i64(registers, rhs, "compare")?;
            write(registers, dst, Value::Bool(i64_ops::compare(lhs, op, rhs)))
        }
        ScalarOpKind::I64CompareImm { dst, op, lhs, imm } => {
            let lhs = read_i64(registers, lhs, "compare")?;
            write(registers, dst, Value::Bool(i64_ops::compare(lhs, op, imm)))
        }
    }
}

/// Scalar plan verification proves every register is below the owning code
/// object's register count, and a linked frame is created with exactly that
/// count. The slice never resizes while a block executes, so unchecked access
/// is confined to this verified-plan boundary.
#[inline(always)]
#[allow(unsafe_code)]
fn read(registers: &[Value], register: Register) -> Value {
    // SAFETY: The invariant above is re-established by unlinked, portable, and
    // linked verification before a plan can execute.
    unsafe { *registers.get_unchecked(usize::from(register.0)) }
}

#[inline(always)]
#[allow(unsafe_code)]
fn write(registers: &mut [Value], register: Register, value: Value) -> VmResult<()> {
    // SAFETY: See `read`; the same verified bound applies to destinations.
    unsafe {
        *registers.get_unchecked_mut(usize::from(register.0)) = value;
    }
    Ok(())
}

#[inline(always)]
fn read_i64(registers: &[Value], register: Register, operation: &'static str) -> VmResult<i64> {
    match read(registers, register) {
        Value::I64(value) => Ok(value),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

#[inline(always)]
fn read_bool(registers: &[Value], register: Register, operation: &'static str) -> VmResult<bool> {
    match read(registers, register) {
        Value::Bool(value) => Ok(value),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch { operation })),
    }
}

fn charge_target<const CHARGE_BUDGET: bool>(
    plan: &ScalarBlockPlan,
    budget: &mut Option<&mut ExecutionBudget>,
    target: ChargedScalarTarget,
) -> VmResult<()> {
    if CHARGE_BUDGET && target.execution_units != 0 {
        let result = budget
            .as_deref_mut()
            .expect("execution-unit budget mode requires a budget")
            .charge_execution_units(u64::from(target.execution_units));
        if let Err(error) = result {
            let source = target
                .budget_source
                .and_then(|source| source_point(plan, source));
            return Err(error.with_source_span_if_absent(source));
        }
    }
    Ok(())
}

fn charge_edge<const CHARGE_BUDGET: bool>(
    plan: &ScalarBlockPlan,
    budget: &mut Option<&mut ExecutionBudget>,
    edge: ChargedScalarEdge,
) -> VmResult<()> {
    if CHARGE_BUDGET && edge.execution_units != 0 {
        let result = budget
            .as_deref_mut()
            .expect("execution-unit budget mode requires a budget")
            .charge_execution_units(u64::from(edge.execution_units));
        if let Err(error) = result {
            let source = edge
                .budget_source
                .and_then(|source| source_point(plan, source));
            return Err(error.with_source_span_if_absent(source));
        }
    }
    Ok(())
}

#[inline(always)]
fn charge<const CHARGE_BUDGET: bool>(
    plan: &ScalarBlockPlan,
    budget: &mut Option<&mut ExecutionBudget>,
    units: u32,
    source: ScalarSourcePointId,
) -> VmResult<()> {
    if CHARGE_BUDGET && units != 0 {
        let result = budget
            .as_deref_mut()
            .expect("execution-unit budget mode requires a budget")
            .charge_execution_units(u64::from(units));
        if let Err(error) = result {
            return Err(error.with_source_span_if_absent(source_point(plan, source)));
        }
    }
    Ok(())
}

#[inline(always)]
fn profile_subpoint<const PROFILE: bool>(
    profiler: Option<&dyn VmBytecodeProfiler>,
    function: DebugNameId,
    instruction: InstructionOffset,
    plan: ScalarBlockPlanId,
    source: ScalarSourcePointId,
) {
    if PROFILE {
        profiler
            .expect("profile execution mode requires a profiler")
            .record_scalar_subpoint(function, instruction, plan, source);
    }
}

#[inline(always)]
fn profile_loop_event<const PROFILE: bool>(
    profiler: Option<&dyn VmBytecodeProfiler>,
    function: DebugNameId,
    instruction: InstructionOffset,
    plan: ScalarBlockPlanId,
    event: ScalarLoopProfileEvent,
) {
    if PROFILE {
        profiler
            .expect("profile execution mode requires a profiler")
            .record_scalar_loop_event(function, instruction, plan, event);
    }
}

fn source_point(plan: &ScalarBlockPlan, source: ScalarSourcePointId) -> Option<Span> {
    plan.source_points.get(source.index()).copied()
}

pub(crate) fn missing_plan_error(plan: ScalarBlockPlanId) -> VmError {
    VmError::new(VmErrorKind::InstructionOutOfBounds {
        offset: plan.index(),
    })
}
