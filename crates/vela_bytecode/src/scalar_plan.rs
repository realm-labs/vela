//! Compact, portable plans for verifier-selected scalar basic blocks.

use std::collections::BTreeSet;

use vela_common::Span;

use crate::{I64CompareOp, InstructionOffset, Register};

pub(crate) const MAX_SCALAR_BLOCKS_PER_CODE: usize = 4_096;
pub(crate) const MAX_SCALAR_OPERATIONS_PER_BLOCK: usize = 1_024;
pub(crate) const MAX_SCALAR_OPERATIONS_PER_CODE: usize = 65_536;
pub(crate) const MAX_SCALAR_SOURCE_POINTS_PER_BLOCK: usize = 2_048;

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ScalarBlockPlanId(u32);

impl ScalarBlockPlanId {
    #[must_use]
    pub fn new(index: usize) -> Self {
        Self(u32::try_from(index).expect("scalar block plan index exceeds u32::MAX"))
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarConstant {
    Bool(bool),
    I64(i64),
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarOpKind {
    LoadScalar {
        dst: Register,
        value: ScalarConstant,
    },
    Move {
        dst: Register,
        src: Register,
    },
    BoolNot {
        dst: Register,
        src: Register,
    },
    I64Add {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    I64Sub {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    I64Mul {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    I64Rem {
        dst: Register,
        lhs: Register,
        rhs: Register,
    },
    I64AddImm {
        dst: Register,
        lhs: Register,
        imm: i64,
    },
    I64SubImm {
        dst: Register,
        lhs: Register,
        imm: i64,
    },
    I64MulImm {
        dst: Register,
        lhs: Register,
        imm: i64,
    },
    I64RemImm {
        dst: Register,
        lhs: Register,
        imm: i64,
    },
    I64Compare {
        dst: Register,
        op: I64CompareOp,
        lhs: Register,
        rhs: Register,
    },
    I64CompareImm {
        dst: Register,
        op: I64CompareOp,
        lhs: Register,
        imm: i64,
    },
}

impl ScalarOpKind {
    fn registers(self) -> [Option<Register>; 3] {
        match self {
            Self::LoadScalar { dst, .. } => [Some(dst), None, None],
            Self::Move { dst, src } | Self::BoolNot { dst, src } => [Some(dst), Some(src), None],
            Self::I64Add { dst, lhs, rhs }
            | Self::I64Sub { dst, lhs, rhs }
            | Self::I64Mul { dst, lhs, rhs }
            | Self::I64Rem { dst, lhs, rhs }
            | Self::I64Compare { dst, lhs, rhs, .. } => [Some(dst), Some(lhs), Some(rhs)],
            Self::I64AddImm { dst, lhs, .. }
            | Self::I64SubImm { dst, lhs, .. }
            | Self::I64MulImm { dst, lhs, .. }
            | Self::I64RemImm { dst, lhs, .. }
            | Self::I64CompareImm { dst, lhs, .. } => [Some(dst), Some(lhs), None],
        }
    }
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarOp {
    pub kind: ScalarOpKind,
    pub source: ScalarSourcePointId,
    pub execution_units: u32,
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ScalarSourcePointId(u16);

impl ScalarSourcePointId {
    #[must_use]
    pub fn new(index: usize) -> Self {
        Self(u16::try_from(index).expect("scalar source point index exceeds u16::MAX"))
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChargedScalarTarget {
    pub target: InstructionOffset,
    pub execution_units: u32,
    pub budget_source: Option<ScalarSourcePointId>,
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarExitKind {
    Fallthrough(ChargedScalarTarget),
    Jump(ChargedScalarTarget),
    BoolBranch {
        condition: Register,
        passed: ChargedScalarTarget,
        failed: ChargedScalarTarget,
    },
    I64CompareBranch {
        op: I64CompareOp,
        lhs: Register,
        rhs: Register,
        passed: ChargedScalarTarget,
        failed: ChargedScalarTarget,
    },
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarExit {
    pub kind: ScalarExitKind,
    pub source: ScalarSourcePointId,
    pub execution_units: u32,
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarBlockPlan {
    pub operations: Box<[ScalarOp]>,
    pub exit: ScalarExit,
    pub source_points: Box<[Span]>,
    #[cfg_attr(feature = "artifact-codec", serde(skip))]
    pub(crate) mir_statements: Box<[vela_mir::MirStatementId]>,
    #[cfg_attr(feature = "artifact-codec", serde(skip))]
    pub(crate) mir_terminator: Option<vela_mir::MirBlockId>,
    #[cfg_attr(feature = "artifact-codec", serde(skip))]
    pub(crate) mir_budget_sites: Box<[ScalarMirBudgetSite]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScalarBudgetLocation {
    Operation(usize),
    Exit,
    JumpEdge,
    PassedEdge,
    FailedEdge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ScalarMirBudgetSite {
    pub(crate) site: vela_mir::MirBudgetSite,
    pub(crate) point: vela_mir::MirBudgetPoint,
    pub(crate) location: ScalarBudgetLocation,
}

impl ScalarBlockPlan {
    #[must_use]
    pub fn new(operations: Box<[ScalarOp]>, exit: ScalarExit, source_points: Box<[Span]>) -> Self {
        Self {
            operations,
            exit,
            source_points,
            mir_statements: Box::new([]),
            mir_terminator: None,
            mir_budget_sites: Box::new([]),
        }
    }

    #[must_use]
    pub(crate) fn with_mir_coverage(
        mut self,
        statements: Box<[vela_mir::MirStatementId]>,
        terminator: vela_mir::MirBlockId,
    ) -> Self {
        self.mir_statements = statements;
        self.mir_terminator = Some(terminator);
        self
    }

    #[must_use]
    pub(crate) fn with_mir_budget_sites(mut self, sites: Box<[ScalarMirBudgetSite]>) -> Self {
        self.mir_budget_sites = sites;
        self
    }
}

pub(crate) fn verify_scalar_block_plans(
    plans: &[ScalarBlockPlan],
    register_count: u16,
    instruction_count: usize,
) -> Result<(), &'static str> {
    if plans.len() > MAX_SCALAR_BLOCKS_PER_CODE {
        return Err("scalar block plan count exceeds the format limit");
    }
    let mut total_operations = 0_usize;
    for plan in plans {
        if plan.operations.len() > MAX_SCALAR_OPERATIONS_PER_BLOCK {
            return Err("scalar block operation count exceeds the format limit");
        }
        total_operations = total_operations
            .checked_add(plan.operations.len())
            .ok_or("scalar block operation count overflows")?;
        if total_operations > MAX_SCALAR_OPERATIONS_PER_CODE {
            return Err("scalar block operation total exceeds the format limit");
        }
        if plan.source_points.is_empty()
            || plan.source_points.len() > MAX_SCALAR_SOURCE_POINTS_PER_BLOCK
        {
            return Err("scalar block source-point count is invalid");
        }
        let fused_exit = !matches!(plan.exit.kind, ScalarExitKind::Fallthrough(_));
        if plan.operations.len() < if fused_exit { 2 } else { 3 } {
            return Err("scalar block does not meet the minimum profitable size");
        }

        let mut used_sources = BTreeSet::new();
        let mut total_units = plan.exit.execution_units;
        for operation in &plan.operations {
            verify_source(operation.source, plan.source_points.len())?;
            used_sources.insert(operation.source);
            total_units = total_units
                .checked_add(operation.execution_units)
                .ok_or("scalar block execution-unit coverage overflows")?;
            for register in operation.kind.registers().into_iter().flatten() {
                verify_register(register, register_count)?;
            }
        }
        verify_source(plan.exit.source, plan.source_points.len())?;
        used_sources.insert(plan.exit.source);
        verify_exit(
            plan.exit.kind,
            register_count,
            instruction_count,
            plan.source_points.len(),
            &mut used_sources,
            &mut total_units,
        )?;
        if used_sources.len() != plan.source_points.len() {
            return Err("scalar block contains an unreferenced source point");
        }
    }
    Ok(())
}

pub(crate) fn verify_unlinked_scalar_block_references(
    code: &crate::UnlinkedCodeObject,
) -> Result<(), &'static str> {
    let references = code.instructions.iter().filter_map(|instruction| {
        if let crate::UnlinkedInstructionKind::RunScalarBlock { plan } = instruction.kind {
            Some((plan, instruction.execution_units))
        } else {
            None
        }
    });
    verify_scalar_block_references(code.scalar_blocks.len(), references)
}

pub(crate) fn verify_linked_scalar_block_references(
    code: &crate::LinkedCodeObject,
) -> Result<(), &'static str> {
    let references = code.instructions.iter().filter_map(|instruction| {
        if let crate::linked::InstructionKind::RunScalarBlock { plan } = instruction.kind {
            Some((plan, instruction.execution_units))
        } else {
            None
        }
    });
    verify_scalar_block_references(code.scalar_blocks.len(), references)
}

fn verify_scalar_block_references(
    plan_count: usize,
    references: impl Iterator<Item = (ScalarBlockPlanId, u32)>,
) -> Result<(), &'static str> {
    let mut referenced = BTreeSet::new();
    for (plan, instruction_units) in references {
        if plan.index() >= plan_count {
            return Err("scalar block plan handle is out of bounds");
        }
        if instruction_units != 0 {
            return Err("scalar block entry carries duplicate instruction budget units");
        }
        if !referenced.insert(plan) {
            return Err("scalar block plan is referenced by more than one instruction");
        }
    }
    if referenced.len() != plan_count {
        return Err("scalar block plan table contains an unreferenced plan");
    }
    Ok(())
}

fn verify_exit(
    exit: ScalarExitKind,
    register_count: u16,
    instruction_count: usize,
    source_count: usize,
    used_sources: &mut BTreeSet<ScalarSourcePointId>,
    total_units: &mut u32,
) -> Result<(), &'static str> {
    let mut verify_target = |target: ChargedScalarTarget| {
        if target.target.0 > instruction_count {
            return Err("scalar block exit target is out of bounds");
        }
        match (target.execution_units, target.budget_source) {
            (0, None) => {}
            (0, Some(_)) => return Err("uncharged scalar target carries a budget source"),
            (_, None) => return Err("charged scalar target is missing a budget source"),
            (_, Some(source)) => {
                verify_source(source, source_count)?;
                used_sources.insert(source);
            }
        }
        *total_units = total_units
            .checked_add(target.execution_units)
            .ok_or("scalar block execution-unit coverage overflows")?;
        Ok(())
    };
    match exit {
        ScalarExitKind::Fallthrough(target) | ScalarExitKind::Jump(target) => verify_target(target),
        ScalarExitKind::BoolBranch {
            condition,
            passed,
            failed,
        } => {
            verify_register(condition, register_count)?;
            verify_target(passed)?;
            verify_target(failed)
        }
        ScalarExitKind::I64CompareBranch {
            lhs,
            rhs,
            passed,
            failed,
            ..
        } => {
            verify_register(lhs, register_count)?;
            verify_register(rhs, register_count)?;
            verify_target(passed)?;
            verify_target(failed)
        }
    }
}

fn verify_source(source: ScalarSourcePointId, source_count: usize) -> Result<(), &'static str> {
    if source.index() >= source_count {
        return Err("scalar block source point is out of bounds");
    }
    Ok(())
}

fn verify_register(register: Register, register_count: u16) -> Result<(), &'static str> {
    if register.0 >= register_count {
        return Err("scalar block register is out of bounds");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_common::SourceId;

    fn span(start: u32, end: u32) -> Span {
        Span::new(SourceId::new(0), start, end)
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

    fn valid_plan() -> ScalarBlockPlan {
        ScalarBlockPlan {
            operations: Box::new([
                ScalarOp {
                    kind: ScalarOpKind::LoadScalar {
                        dst: Register(0),
                        value: ScalarConstant::I64(1),
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
                    execution_units: 0,
                },
            ]),
            exit: ScalarExit {
                kind: ScalarExitKind::Jump(target(3)),
                source: source(2),
                execution_units: 1,
            },
            source_points: Box::new([span(0, 1), span(1, 2), span(2, 3)]),
            mir_statements: Box::new([]),
            mir_terminator: None,
            mir_budget_sites: Box::new([]),
        }
    }

    #[test]
    fn compact_operation_layout_stays_bounded() {
        assert!(size_of::<ScalarOpKind>() <= 24);
        assert!(size_of::<ScalarOp>() <= 32);
        assert!(size_of::<ScalarBlockPlanId>() <= 4);
    }

    #[test]
    fn scalar_plan_verifier_accepts_bounded_fused_exit() {
        verify_scalar_block_plans(&[valid_plan()], 2, 4).expect("valid scalar plan");
    }

    #[test]
    fn scalar_plan_verifier_rejects_invalid_register_source_and_target() {
        let mut register = valid_plan();
        register.operations[0].kind = ScalarOpKind::Move {
            dst: Register(2),
            src: Register(0),
        };
        assert_eq!(
            verify_scalar_block_plans(&[register], 2, 4),
            Err("scalar block register is out of bounds")
        );

        let mut source_point = valid_plan();
        source_point.operations[0].source = source(3);
        assert_eq!(
            verify_scalar_block_plans(&[source_point], 2, 4),
            Err("scalar block source point is out of bounds")
        );

        let mut exit = valid_plan();
        exit.exit.kind = ScalarExitKind::Jump(target(5));
        assert_eq!(
            verify_scalar_block_plans(&[exit], 2, 4),
            Err("scalar block exit target is out of bounds")
        );
    }

    #[test]
    fn scalar_plan_verifier_rejects_incomplete_budget_and_source_coverage() {
        let mut budget = valid_plan();
        budget.exit.kind = ScalarExitKind::Jump(ChargedScalarTarget {
            target: InstructionOffset(3),
            execution_units: 1,
            budget_source: None,
        });
        assert_eq!(
            verify_scalar_block_plans(&[budget], 2, 4),
            Err("charged scalar target is missing a budget source")
        );

        let mut source_point = valid_plan();
        source_point.source_points = Box::new([span(0, 1), span(1, 2), span(2, 3), span(3, 4)]);
        assert_eq!(
            verify_scalar_block_plans(&[source_point], 2, 4),
            Err("scalar block contains an unreferenced source point")
        );
    }

    #[test]
    fn scalar_plan_reference_verifier_rejects_missing_duplicate_and_orphan_handles() {
        let mut code = crate::UnlinkedCodeObject::new("main", 2);
        code.scalar_blocks.push(valid_plan());
        assert_eq!(
            verify_unlinked_scalar_block_references(&code),
            Err("scalar block plan table contains an unreferenced plan")
        );

        code.push_instruction(crate::UnlinkedInstruction::new(
            crate::UnlinkedInstructionKind::RunScalarBlock {
                plan: ScalarBlockPlanId::new(1),
            },
        ));
        assert_eq!(
            verify_unlinked_scalar_block_references(&code),
            Err("scalar block plan handle is out of bounds")
        );

        code.instructions[0].kind = crate::UnlinkedInstructionKind::RunScalarBlock {
            plan: ScalarBlockPlanId::new(0),
        };
        code.push_instruction(crate::UnlinkedInstruction::new(
            crate::UnlinkedInstructionKind::RunScalarBlock {
                plan: ScalarBlockPlanId::new(0),
            },
        ));
        assert_eq!(
            verify_unlinked_scalar_block_references(&code),
            Err("scalar block plan is referenced by more than one instruction")
        );
    }
}
