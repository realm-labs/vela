use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::linked::InstructionKind;
use crate::{CacheSiteDesc, CacheSiteId, ExecutableGenerationId, LinkedProgram, ProgramImage};

static NEXT_EXECUTABLE_GENERATION: AtomicU64 = AtomicU64::new(1);

/// One immutable linker output for every generation-local executable layout.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkedArtifact {
    program: Arc<LinkedProgram>,
    image: ProgramImage,
    cache_layout: Box<[CacheSiteDesc]>,
    profile_layout: ProfileLayout,
    mir_executables: Box<[MirExecutableLayout]>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProfileLayout {
    functions: Box<[ProfileFunctionLayout]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileFunctionLayout {
    pub handle: crate::ScriptFunctionHandle,
    pub debug_name: crate::DebugNameId,
    pub instruction_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirExecutableLayout {
    pub root: vela_def::FunctionId,
    pub function: vela_mir::MirFunctionId,
    pub handle: crate::ScriptFunctionHandle,
}

impl LinkedArtifact {
    pub(crate) fn finish(
        image: ProgramImage,
        mut program: LinkedProgram,
    ) -> Result<Self, crate::verification::VerificationError> {
        program.set_generation(ExecutableGenerationId::new(
            NEXT_EXECUTABLE_GENERATION.fetch_add(1, Ordering::Relaxed),
        ));
        let cache_layout = image.cache_sites().to_vec().into_boxed_slice();
        let profile_layout = ProfileLayout {
            functions: program
                .functions()
                .map(|(handle, code)| ProfileFunctionLayout {
                    handle,
                    debug_name: code.debug_name,
                    instruction_count: code.instructions.len(),
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        };
        let artifact = Self {
            program: Arc::new(program),
            image,
            cache_layout,
            profile_layout,
            mir_executables: Box::new([]),
        };
        artifact.verify()?;
        Ok(artifact)
    }

    pub fn attach_verified_mir(
        mut self,
        bundle: &vela_mir::OwnedVerifiedMirBundle,
    ) -> Result<Self, crate::linker::LinkError> {
        let mut by_symbol = BTreeMap::new();
        for (root, owned) in bundle.roots() {
            for (function, body) in owned.program().functions() {
                by_symbol.insert(body.code_symbol().to_owned(), (root, function));
            }
        }
        let mut layouts = Vec::with_capacity(self.program.function_count());
        for (handle, code) in self.program.functions() {
            let name = self.program.debug_name(code.debug_name);
            let (root, function) = by_symbol.get(name).copied().ok_or_else(|| {
                crate::linker::LinkError::MissingMirExecutableMapping {
                    name: name.to_owned(),
                }
            })?;
            let owner = bundle
                .root(root)
                .expect("MIR symbol map retains its root owner");
            let analyses = owner
                .analyses(function)
                .expect("verified MIR function retains sealed analyses");
            let expected_units = analyses
                .budget
                .statement_points()
                .map(|(_, point)| u64::from(point.units))
                .sum::<u64>()
                + analyses
                    .budget
                    .terminator_points()
                    .map(|(_, point)| u64::from(point.units))
                    .sum::<u64>();
            let actual_units = code
                .instructions
                .iter()
                .map(|instruction| {
                    u64::from(instruction.execution_units)
                        + match instruction.kind {
                            InstructionKind::ChargeExecutionUnits { units } => u64::from(units),
                            _ => 0,
                        }
                })
                .sum::<u64>();
            if expected_units != actual_units {
                return Err(crate::linker::LinkError::MirBudgetScheduleMismatch {
                    name: name.to_owned(),
                    expected_units,
                    actual_units,
                });
            }
            layouts.push(MirExecutableLayout {
                root,
                function,
                handle,
            });
        }
        self.mir_executables = layouts.into_boxed_slice();
        Ok(self)
    }

    #[must_use]
    pub fn program(&self) -> &LinkedProgram {
        self.program.as_ref()
    }

    #[must_use]
    pub fn program_owner(&self) -> Arc<LinkedProgram> {
        Arc::clone(&self.program)
    }

    #[must_use]
    pub const fn image(&self) -> &ProgramImage {
        &self.image
    }

    #[must_use]
    pub fn cache_layout(&self) -> &[CacheSiteDesc] {
        &self.cache_layout
    }

    #[must_use]
    pub fn generation(&self) -> ExecutableGenerationId {
        self.program.generation()
    }

    #[must_use]
    pub const fn profile_layout(&self) -> &ProfileLayout {
        &self.profile_layout
    }

    #[must_use]
    pub fn mir_executable(
        &self,
        handle: crate::ScriptFunctionHandle,
    ) -> Option<&MirExecutableLayout> {
        self.mir_executables
            .iter()
            .find(|layout| layout.handle == handle)
    }

    pub fn mir_executables(&self) -> &[MirExecutableLayout] {
        &self.mir_executables
    }

    #[must_use]
    pub fn into_program(self) -> LinkedProgram {
        Arc::try_unwrap(self.program).unwrap_or_else(|program| (*program).clone())
    }

    pub fn verify(&self) -> Result<(), crate::verification::VerificationError> {
        self.image.verify()?;
        self.program.verify()?;
        verify_cache_correspondence(self)
    }
}

impl ProfileLayout {
    #[must_use]
    pub fn functions(&self) -> &[ProfileFunctionLayout] {
        &self.functions
    }
}

impl Deref for LinkedArtifact {
    type Target = LinkedProgram;

    fn deref(&self) -> &Self::Target {
        self.program()
    }
}

fn verify_cache_correspondence(
    artifact: &LinkedArtifact,
) -> Result<(), crate::verification::VerificationError> {
    for (handle, code) in artifact.program.functions() {
        let image = artifact
            .image
            .function(crate::FunctionIndex(handle.index()))
            .ok_or_else(|| crate::verification::VerificationError {
                function: "<artifact>".to_owned(),
                instruction: None,
                kind: crate::verification::VerificationErrorKind::FunctionIndexOutOfBounds {
                    function: crate::FunctionIndex(handle.index()),
                    function_count: artifact.image.function_count(),
                },
            })?;
        if code.cache_sites != image.cache_sites {
            return Err(crate::verification::VerificationError {
                function: image.name.clone(),
                instruction: None,
                kind: crate::verification::VerificationErrorKind::CacheSiteIdMismatch {
                    expected: image
                        .cache_sites
                        .sites()
                        .first()
                        .map_or(CacheSiteId::new(0), |site| site.id),
                    actual: code
                        .cache_sites
                        .sites()
                        .first()
                        .map_or(CacheSiteId::new(0), |site| site.id),
                },
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use vela_common::GlobalSlot;

    use crate::{
        CacheSiteKind, FunctionIndex, InstructionOffset, Linker, Register, UnlinkedCodeObject,
        UnlinkedInstruction, UnlinkedInstructionKind, UnlinkedProgram,
    };

    #[test]
    fn nested_local_cache_zero_sites_receive_distinct_generation_ids() {
        let first = cached_global_lambda("first", "main::first", GlobalSlot::new(0));
        let second = cached_global_lambda("second", "main::second", GlobalSlot::new(1));
        let mut main = UnlinkedCodeObject::new("main", 2);
        main.nested_functions = vec![first, second];
        main.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::MakeClosure {
                dst: Register(0),
                function: FunctionIndex(0),
                captures: Vec::new(),
            },
        ));
        main.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::MakeClosure {
                dst: Register(1),
                function: FunctionIndex(1),
                captures: Vec::new(),
            },
        ));
        main.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
            src: Register(1),
        }));

        let mut program = UnlinkedProgram::new();
        program.set_global_layout(["main::first".to_owned(), "main::second".to_owned()]);
        program.insert_function(main);
        let artifact = Linker::new()
            .link_program(&program)
            .expect("artifact should link");

        let first = artifact
            .function(crate::ScriptFunctionHandle::new(1))
            .expect("first lambda");
        let second = artifact
            .function(crate::ScriptFunctionHandle::new(2))
            .expect("second lambda");
        let first_site = first.cache_sites.sites()[0].id;
        let second_site = second.cache_sites.sites()[0].id;
        assert_ne!(first_site, second_site);
        assert_eq!(artifact.cache_layout().len(), 2);
        assert_eq!(artifact.profile_layout().functions().len(), 3);
    }

    fn cached_global_lambda(name: &str, global: &str, slot: GlobalSlot) -> UnlinkedCodeObject {
        let mut code = UnlinkedCodeObject::new(name, 1);
        let site = code.push_cache_site(CacheSiteKind::GlobalRead, InstructionOffset(0));
        code.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::LoadGlobal {
                dst: Register(0),
                global: global.to_owned(),
                slot: Some(slot),
                cache_site: Some(site),
            },
        ));
        code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
            src: Register(0),
        }));
        code
    }
}
