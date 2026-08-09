//! Portable physical coverage for verifier-selected interpreter units.

use vela_common::Span;

use crate::{
    InstructionOffset, LinkedCodeObject, UnlinkedCodeObject, UnlinkedInstructionKind,
    linked::InstructionKind,
};

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedPhysicalUnitKind {
    I64CmpImmJumpIfFalse,
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedPhysicalUnit {
    pub(crate) instruction: InstructionOffset,
    pub(crate) kind: SelectedPhysicalUnitKind,
    pub(crate) covered_operations: u16,
    pub(crate) source_points: Box<[Span]>,
    pub(crate) exits: Box<[InstructionOffset]>,
    pub(crate) budget_units: Box<[u32]>,
    #[cfg_attr(feature = "artifact-codec", serde(skip))]
    pub(crate) mir_statement: Option<vela_mir::MirStatementId>,
    #[cfg_attr(feature = "artifact-codec", serde(skip))]
    pub(crate) mir_terminator: Option<vela_mir::MirBlockId>,
}

impl SelectedPhysicalUnit {
    #[must_use]
    pub(crate) fn i64_cmp_imm_jump_if_false(
        instruction: InstructionOffset,
        source_points: [Span; 2],
        exits: [InstructionOffset; 2],
        budget_units: [u32; 2],
        mir_statement: vela_mir::MirStatementId,
        mir_terminator: vela_mir::MirBlockId,
    ) -> Self {
        Self {
            instruction,
            kind: SelectedPhysicalUnitKind::I64CmpImmJumpIfFalse,
            covered_operations: 2,
            source_points: Box::new(source_points),
            exits: Box::new(exits),
            budget_units: Box::new(budget_units),
            mir_statement: Some(mir_statement),
            mir_terminator: Some(mir_terminator),
        }
    }

    #[must_use]
    pub const fn instruction(&self) -> InstructionOffset {
        self.instruction
    }

    #[must_use]
    pub const fn kind(&self) -> SelectedPhysicalUnitKind {
        self.kind
    }

    #[must_use]
    pub const fn source_points(&self) -> &[Span] {
        &self.source_points
    }

    #[must_use]
    pub const fn exits(&self) -> &[InstructionOffset] {
        &self.exits
    }

    #[must_use]
    pub const fn budget_units(&self) -> &[u32] {
        &self.budget_units
    }
}

pub(crate) fn verify_selected_physical_units(
    code: &UnlinkedCodeObject,
) -> Result<(), &'static str> {
    let mut previous = None;
    for selected in &code.selected_units {
        let index = selected.instruction.0;
        if previous.is_some_and(|previous| previous >= index) {
            return Err("selected-unit instruction offsets are not strictly increasing");
        }
        previous = Some(index);
        let instruction = code
            .instructions
            .get(index)
            .ok_or("selected-unit instruction offset is out of bounds")?;
        if selected.covered_operations != 2
            || selected.source_points.len() != 2
            || selected.exits.len() != 2
            || selected.budget_units.len() != 2
        {
            return Err("selected-unit physical coverage shape is invalid");
        }
        let false_target = match (&selected.kind, &instruction.kind) {
            (
                SelectedPhysicalUnitKind::I64CmpImmJumpIfFalse,
                UnlinkedInstructionKind::I64CmpImmJumpIfFalse { target, .. },
            ) => *target,
            _ => return Err("selected-unit kind does not match its instruction"),
        };
        if instruction.span != Some(selected.source_points[0]) {
            return Err("selected-unit trapping source point disagrees with its instruction");
        }
        if selected.exits[0] != InstructionOffset(index + 1)
            || selected.exits[1] != false_target
            || selected
                .exits
                .iter()
                .any(|target| target.0 > code.instructions.len())
        {
            return Err("selected-unit physical exit coverage is invalid");
        }
        let expected_units = selected
            .budget_units
            .iter()
            .try_fold(0_u32, |total, units| total.checked_add(*units))
            .ok_or("selected-unit budget coverage overflows")?;
        if instruction.execution_units != expected_units {
            return Err("selected-unit budget coverage disagrees with its instruction");
        }
    }

    for (index, instruction) in code.instructions.iter().enumerate() {
        if matches!(
            instruction.kind,
            UnlinkedInstructionKind::I64CmpImmJumpIfFalse { .. }
        ) && !code
            .selected_units
            .iter()
            .any(|selected| selected.instruction.0 == index)
        {
            return Err("selected instruction is missing physical coverage");
        }
    }
    Ok(())
}

pub(crate) fn verify_linked_selected_physical_units(
    code: &LinkedCodeObject,
) -> Result<(), &'static str> {
    let mut previous = None;
    for selected in &code.selected_units {
        let index = selected.instruction.0;
        if previous.is_some_and(|previous| previous >= index) {
            return Err("selected-unit instruction offsets are not strictly increasing");
        }
        previous = Some(index);
        let instruction = code
            .instructions
            .get(index)
            .ok_or("selected-unit instruction offset is out of bounds")?;
        if selected.covered_operations != 2
            || selected.source_points.len() != 2
            || selected.exits.len() != 2
            || selected.budget_units.len() != 2
        {
            return Err("selected-unit physical coverage shape is invalid");
        }
        let false_target = match (&selected.kind, &instruction.kind) {
            (
                SelectedPhysicalUnitKind::I64CmpImmJumpIfFalse,
                InstructionKind::I64CmpImmJumpIfFalse { target, .. },
            ) => *target,
            _ => return Err("selected-unit kind does not match its instruction"),
        };
        if instruction.span != Some(selected.source_points[0])
            || selected.exits[0] != InstructionOffset(index + 1)
            || selected.exits[1] != false_target
            || selected
                .exits
                .iter()
                .any(|target| target.0 > code.instructions.len())
        {
            return Err("selected-unit physical source or exit coverage is invalid");
        }
        let expected_units = selected
            .budget_units
            .iter()
            .try_fold(0_u32, |total, units| total.checked_add(*units))
            .ok_or("selected-unit budget coverage overflows")?;
        if instruction.execution_units != expected_units {
            return Err("selected-unit budget coverage disagrees with its instruction");
        }
    }
    for (index, instruction) in code.instructions.iter().enumerate() {
        if matches!(
            instruction.kind,
            InstructionKind::I64CmpImmJumpIfFalse { .. }
        ) && !code
            .selected_units
            .iter()
            .any(|selected| selected.instruction.0 == index)
        {
            return Err("selected instruction is missing physical coverage");
        }
    }
    Ok(())
}
