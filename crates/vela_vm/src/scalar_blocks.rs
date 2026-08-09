//! Focused executor for verified compact scalar basic-block plans.

use vela_bytecode::{
    ChargedScalarTarget, DebugNameId, InstructionOffset, ScalarBlockPlan, ScalarBlockPlanId,
    ScalarConstant, ScalarExitKind, ScalarOpKind, ScalarSourcePointId,
};
use vela_common::Span;

use crate::budget::ExecutionBudget;
use crate::error::{VmError, VmErrorKind, VmResult};
use crate::frame::CallFrame;
use crate::value::Value;
use crate::{VmBytecodeProfiler, i64_ops};

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
    let ScalarBlockExecution {
        plan_id,
        plan,
        function,
        instruction,
        profiler,
    } = execution;
    for operation in &plan.operations {
        profile_subpoint::<PROFILE>(profiler, function, instruction, plan_id, operation.source);
        let source = source_point(plan, operation.source);
        charge::<CHARGE_BUDGET>(budget, operation.execution_units, source)?;
        execute_operation(frame, operation.kind)
            .map_err(|error| error.with_source_span_if_absent(source))?;
    }

    profile_subpoint::<PROFILE>(profiler, function, instruction, plan_id, plan.exit.source);
    let exit_source = source_point(plan, plan.exit.source);
    charge::<CHARGE_BUDGET>(budget, plan.exit.execution_units, exit_source)?;
    let target = match plan.exit.kind {
        ScalarExitKind::Fallthrough(target) | ScalarExitKind::Jump(target) => target,
        ScalarExitKind::BoolBranch {
            condition,
            passed,
            failed,
        } => {
            if frame
                .read_bool(condition, "scalar block branch")
                .map_err(|error| error.with_source_span_if_absent(exit_source))?
            {
                passed
            } else {
                failed
            }
        }
        ScalarExitKind::I64CompareBranch {
            op,
            lhs,
            rhs,
            passed,
            failed,
        } => {
            let lhs = frame
                .read_i64(lhs, "scalar block compare")
                .map_err(|error| error.with_source_span_if_absent(exit_source))?;
            let rhs = frame
                .read_i64(rhs, "scalar block compare")
                .map_err(|error| error.with_source_span_if_absent(exit_source))?;
            if i64_ops::compare(lhs, op, rhs) {
                passed
            } else {
                failed
            }
        }
    };
    charge_target::<CHARGE_BUDGET>(plan, budget, target)?;
    Ok(target.target)
}

fn execute_operation(frame: &mut CallFrame, operation: ScalarOpKind) -> VmResult<()> {
    match operation {
        ScalarOpKind::LoadScalar { dst, value } => frame.write(
            dst,
            match value {
                ScalarConstant::Bool(value) => Value::Bool(value),
                ScalarConstant::I64(value) => Value::I64(value),
            },
        ),
        ScalarOpKind::Move { dst, src } => frame.write(dst, frame.read(src)?),
        ScalarOpKind::BoolNot { dst, src } => {
            let value = frame.read_bool(src, "scalar bool not")?;
            frame.write_bool(dst, !value)
        }
        ScalarOpKind::I64Add { dst, lhs, rhs } => {
            let value = i64_ops::add_raw(frame.read_i64(lhs, "add")?, frame.read_i64(rhs, "add")?)?;
            frame.write_i64(dst, value)
        }
        ScalarOpKind::I64Sub { dst, lhs, rhs } => {
            let value = i64_ops::sub_raw(frame.read_i64(lhs, "sub")?, frame.read_i64(rhs, "sub")?)?;
            frame.write_i64(dst, value)
        }
        ScalarOpKind::I64Mul { dst, lhs, rhs } => {
            let value = i64_ops::mul_raw(frame.read_i64(lhs, "mul")?, frame.read_i64(rhs, "mul")?)?;
            frame.write_i64(dst, value)
        }
        ScalarOpKind::I64Rem { dst, lhs, rhs } => {
            let value = i64_ops::rem_raw(frame.read_i64(lhs, "rem")?, frame.read_i64(rhs, "rem")?)?;
            frame.write_i64(dst, value)
        }
        ScalarOpKind::I64AddImm { dst, lhs, imm } => {
            let value = i64_ops::add_raw(frame.read_i64(lhs, "add")?, imm)?;
            frame.write_i64(dst, value)
        }
        ScalarOpKind::I64SubImm { dst, lhs, imm } => {
            let value = i64_ops::sub_raw(frame.read_i64(lhs, "sub")?, imm)?;
            frame.write_i64(dst, value)
        }
        ScalarOpKind::I64MulImm { dst, lhs, imm } => {
            let value = i64_ops::mul_raw(frame.read_i64(lhs, "mul")?, imm)?;
            frame.write_i64(dst, value)
        }
        ScalarOpKind::I64RemImm { dst, lhs, imm } => {
            let value = i64_ops::rem_raw(frame.read_i64(lhs, "rem")?, imm)?;
            frame.write_i64(dst, value)
        }
        ScalarOpKind::I64Compare { dst, op, lhs, rhs } => {
            let lhs = frame.read_i64(lhs, "compare")?;
            let rhs = frame.read_i64(rhs, "compare")?;
            frame.write_bool(dst, i64_ops::compare(lhs, op, rhs))
        }
        ScalarOpKind::I64CompareImm { dst, op, lhs, imm } => {
            let lhs = frame.read_i64(lhs, "compare")?;
            frame.write_bool(dst, i64_ops::compare(lhs, op, imm))
        }
    }
}

fn charge_target<const CHARGE_BUDGET: bool>(
    plan: &ScalarBlockPlan,
    budget: &mut Option<&mut ExecutionBudget>,
    target: ChargedScalarTarget,
) -> VmResult<()> {
    let source = target
        .budget_source
        .and_then(|source| source_point(plan, source));
    charge::<CHARGE_BUDGET>(budget, target.execution_units, source)
}

#[inline(always)]
fn charge<const CHARGE_BUDGET: bool>(
    budget: &mut Option<&mut ExecutionBudget>,
    units: u32,
    source: Option<Span>,
) -> VmResult<()> {
    if CHARGE_BUDGET && units != 0 {
        budget
            .as_deref_mut()
            .expect("execution-unit budget mode requires a budget")
            .charge_execution_units(u64::from(units))
            .map_err(|error| error.with_source_span_if_absent(source))?;
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

fn source_point(plan: &ScalarBlockPlan, source: ScalarSourcePointId) -> Option<Span> {
    plan.source_points.get(source.index()).copied()
}

pub(crate) fn missing_plan_error(plan: ScalarBlockPlanId) -> VmError {
    VmError::new(VmErrorKind::InstructionOutOfBounds {
        offset: plan.index(),
    })
}
