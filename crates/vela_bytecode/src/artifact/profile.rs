use crate::LinkedProgram;
use crate::linked::InstructionKind;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProfileLayout {
    functions: Box<[ProfileFunctionLayout]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileFunctionLayout {
    pub handle: crate::ScriptFunctionHandle,
    pub debug_name: crate::DebugNameId,
    pub instruction_count: usize,
    pub selected_units: Box<[ProfileSelectedUnitLayout]>,
    pub scalar_units: Box<[ProfileScalarUnitLayout]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileSelectedUnitLayout {
    pub offset: crate::InstructionOffset,
    pub kind: crate::SelectedPhysicalUnitKind,
    pub covered_operations: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileScalarUnitLayout {
    pub offset: crate::InstructionOffset,
    pub plan: crate::ScalarBlockPlanId,
    pub source_count: usize,
    pub operation_sources: Box<[crate::ScalarSourcePointId]>,
    pub has_range_loop: bool,
}

impl ProfileLayout {
    #[must_use]
    pub fn functions(&self) -> &[ProfileFunctionLayout] {
        &self.functions
    }
}

pub(super) fn profile_layout(program: &LinkedProgram) -> ProfileLayout {
    ProfileLayout {
        functions: program
            .functions()
            .map(|(handle, code)| ProfileFunctionLayout {
                handle,
                debug_name: code.debug_name,
                instruction_count: code.instructions.len(),
                selected_units: code
                    .selected_units
                    .iter()
                    .map(|unit| ProfileSelectedUnitLayout {
                        offset: unit.instruction(),
                        kind: unit.kind(),
                        covered_operations: unit.covered_operations(),
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                scalar_units: code
                    .instructions
                    .iter()
                    .enumerate()
                    .filter_map(|(offset, instruction)| {
                        let InstructionKind::RunScalarBlock { plan } = instruction.kind else {
                            return None;
                        };
                        let scalar = &code.scalar_blocks[plan.index()];
                        Some(ProfileScalarUnitLayout {
                            offset: crate::InstructionOffset(offset),
                            plan,
                            source_count: scalar.source_points.len(),
                            operation_sources: scalar
                                .operations
                                .iter()
                                .map(|operation| operation.source)
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                            has_range_loop: scalar.range_loop.is_some(),
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    }
}
