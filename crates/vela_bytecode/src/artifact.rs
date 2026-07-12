use std::collections::BTreeSet;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::linked::InstructionKind;
use crate::{CacheSiteDesc, CacheSiteId, ExecutableGenerationId, LinkedProgram, ProgramImage};

static NEXT_EXECUTABLE_GENERATION: AtomicU64 = AtomicU64::new(1);

/// One immutable linker output for every generation-local executable layout.
#[derive(Debug, PartialEq)]
pub struct LinkedArtifact {
    program: Arc<LinkedProgram>,
    image: ProgramImage,
    cache_layout: Box<[CacheSiteDesc]>,
    profile_layout: ProfileLayout,
    mir_executables: Box<[MirExecutableLayout]>,
    verified_mir: Arc<vela_mir::OwnedVerifiedMirBundle>,
}

/// Private staged linker output. It cannot cross the production runtime boundary.
pub(crate) struct UnboundLinkedProgram {
    program: Arc<LinkedProgram>,
    image: ProgramImage,
    cache_layout: Box<[CacheSiteDesc]>,
    profile_layout: ProfileLayout,
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

#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod test_support {
    use super::{
        Arc, ExecutableGenerationId, LinkedArtifact, LinkedProgram, NEXT_EXECUTABLE_GENERATION,
        Ordering, ProfileFunctionLayout, ProfileLayout, ProgramImage, test_mir_binding,
    };

    #[must_use]
    pub fn linked_artifact(mut program: LinkedProgram) -> Arc<LinkedArtifact> {
        program.set_generation(ExecutableGenerationId::new(
            NEXT_EXECUTABLE_GENERATION.fetch_add(1, Ordering::Relaxed),
        ));
        let cache_layout = program
            .functions()
            .flat_map(|(_, code)| code.cache_sites.sites().iter().cloned())
            .collect::<Vec<_>>()
            .into_boxed_slice();
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
        let (verified_mir, mir_executables) = test_mir_binding(&program);
        Arc::new(LinkedArtifact {
            program: Arc::new(program),
            image: ProgramImage::from_program(&crate::UnlinkedProgram::new()),
            cache_layout,
            profile_layout,
            mir_executables,
            verified_mir,
        })
    }

    #[must_use]
    pub fn into_linked_program(artifact: Arc<LinkedArtifact>) -> LinkedProgram {
        let artifact = Arc::try_unwrap(artifact).expect("test artifact must have one owner");
        Arc::try_unwrap(artifact.program).expect("test linked program must have one owner")
    }
}

impl LinkedArtifact {
    pub(crate) fn finish_unbound(
        image: ProgramImage,
        mut program: LinkedProgram,
    ) -> Result<UnboundLinkedProgram, crate::verification::VerificationError> {
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
        let artifact = UnboundLinkedProgram {
            program: Arc::new(program),
            image,
            cache_layout,
            profile_layout,
        };
        artifact.verify()?;
        Ok(artifact)
    }

    #[must_use]
    pub fn verified_mir(&self) -> &Arc<vela_mir::OwnedVerifiedMirBundle> {
        &self.verified_mir
    }

    pub fn verify(&self) -> Result<(), crate::verification::VerificationError> {
        self.image.verify()?;
        self.program.verify()?;
        verify_cache_correspondence(self)
    }
}

impl UnboundLinkedProgram {
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn into_test_artifact(self) -> LinkedArtifact {
        let (verified_mir, mir_executables) = test_mir_binding(&self.program);
        LinkedArtifact {
            program: self.program,
            image: self.image,
            cache_layout: self.cache_layout,
            profile_layout: self.profile_layout,
            mir_executables,
            verified_mir,
        }
    }

    pub(crate) fn bind_compiled_mir(
        self,
        bundle: Arc<vela_mir::OwnedVerifiedMirBundle>,
        compiled_layouts: &[crate::compiler::CompiledMirExecutable],
        budget_layouts: &[crate::compiler::CompiledExecutableBudgetLayout],
    ) -> Result<LinkedArtifact, crate::linker::LinkError> {
        if compiled_layouts.len() != self.program.function_count() {
            return Err(crate::linker::LinkError::MirExecutableCountMismatch {
                expected: compiled_layouts.len(),
                actual: self.program.function_count(),
            });
        }
        if budget_layouts.len() != compiled_layouts.len() {
            return Err(crate::linker::LinkError::MirExecutableCountMismatch {
                expected: compiled_layouts.len(),
                actual: budget_layouts.len(),
            });
        }
        for (index, expected) in compiled_layouts.iter().enumerate() {
            let actual = self
                .image
                .function(crate::FunctionIndex(index))
                .and_then(|code| code.compiled_mir);
            if actual != Some(*expected) {
                return Err(crate::linker::LinkError::MirExecutableIdentityMismatch {
                    index: crate::FunctionIndex(index),
                    expected_root: expected.root,
                    expected_function: expected.function,
                    actual_root: actual.map(|identity| identity.root),
                    actual_function: actual.map(|identity| identity.function),
                });
            }
            let owner =
                bundle
                    .root(expected.root)
                    .ok_or(crate::linker::LinkError::MissingMirRoot {
                        root: expected.root,
                    })?;
            let analyses = owner.analyses(expected.function).ok_or(
                crate::linker::LinkError::MissingMirFunction {
                    root: expected.root,
                    function: expected.function,
                },
            )?;
            let code = self
                .program
                .function(crate::ScriptFunctionHandle::new(index))
                .ok_or(crate::linker::LinkError::MirExecutableCountMismatch {
                    expected: compiled_layouts.len(),
                    actual: self.program.function_count(),
                })?;
            verify_budget_mapping(index, code, &analyses.budget, &budget_layouts[index])?;
        }
        let mir_executables = compiled_layouts
            .iter()
            .enumerate()
            .map(|(index, layout)| MirExecutableLayout {
                root: layout.root,
                function: layout.function,
                handle: crate::ScriptFunctionHandle::new(index),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(LinkedArtifact {
            program: self.program,
            image: self.image,
            cache_layout: self.cache_layout,
            profile_layout: self.profile_layout,
            mir_executables,
            verified_mir: bundle,
        })
    }

    fn program(&self) -> &LinkedProgram {
        self.program.as_ref()
    }

    const fn image(&self) -> &ProgramImage {
        &self.image
    }

    fn verify(&self) -> Result<(), crate::verification::VerificationError> {
        self.image.verify()?;
        self.program.verify()?;
        verify_unbound_cache_correspondence(self)
    }
}

#[cfg(any(test, feature = "test-support"))]
fn test_mir_binding(
    program: &LinkedProgram,
) -> (
    Arc<vela_mir::OwnedVerifiedMirBundle>,
    Box<[MirExecutableLayout]>,
) {
    let mut owners = Vec::new();
    let mut layouts = Vec::new();
    for (handle, _) in program.functions() {
        let root = vela_def::script_function_id(
            vela_package::PackageId::anonymous().as_str(),
            &format!("__low_level::{}", handle.index()),
        );
        let body = vela_hir::ids::HirBodyId::new(handle.index() as u32);
        let origin = vela_mir::MirSourceOrigin::body(
            body,
            vela_common::Span::new(vela_common::SourceId::new(0), 0, 0),
        );
        let mut function = vela_mir::MirFunction::new(
            body,
            vela_mir::MirFunctionOwner::Function(root),
            format!("__low_level::{}", handle.index()),
            None,
            origin,
        );
        function
            .set_terminator(
                function.entry_block(),
                vela_mir::MirTerminator::new(
                    origin,
                    vela_mir::MirTerminatorKind::Return(None),
                    vela_mir::MirEffect::PURE,
                    None,
                ),
            )
            .expect("low-level test MIR terminates");
        let mut targets = vela_mir::MirTargetTable::default();
        assert!(
            targets.insert_test_function(vela_mir::CompileFunctionDescriptor {
                id: root,
                class: vela_mir::CompileFunctionClass::Script,
                canonical_symbol: format!("__low_level::{}", handle.index()),
                debug_name: format!("__low_level::{}", handle.index()),
                signature: vela_mir::CompileSignature {
                    parameters: Vec::new(),
                    positional: vela_mir::CompilePositionalPolicy::ExactOrTrailingDefaults,
                    return_contract: None,
                    effect: vela_mir::MirEffect::PURE,
                },
                access: vela_mir::CompileFunctionAccess::script(false),
            })
        );
        let mut mir = vela_mir::MirProgram::new(targets);
        let function = mir
            .add_function(function)
            .expect("low-level test MIR has one function");
        owners.push(vela_mir::verify_owned_mir(mir).expect("low-level test MIR verifies"));
        layouts.push(MirExecutableLayout {
            root,
            function,
            handle,
        });
    }
    (
        Arc::new(vela_mir::OwnedVerifiedMirBundle::new(owners)),
        layouts.into_boxed_slice(),
    )
}

impl LinkedArtifact {
    #[must_use]
    pub fn program(&self) -> &LinkedProgram {
        self.program.as_ref()
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
}

fn verify_budget_mapping(
    executable: usize,
    code: &crate::LinkedCodeObject,
    schedule: &vela_mir::MirBudgetSchedule,
    layout: &crate::compiler::CompiledExecutableBudgetLayout,
) -> Result<(), crate::linker::LinkError> {
    let expected = schedule
        .points()
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut seen_origins = BTreeSet::new();
    let mut expected_units = 0_u64;
    let mut actual_units = 0_u64;
    let sealed = layout
        .sites
        .iter()
        .map(|site| (site.site, *site))
        .collect::<std::collections::BTreeMap<_, _>>();
    if sealed.len() != layout.sites.len() {
        return Err(crate::linker::LinkError::DuplicateMirBudgetLayoutSite { executable });
    }
    for (site, point) in &expected {
        let Some(mapped) = sealed.get(site) else {
            return Err(crate::linker::LinkError::MissingMirBudgetLayoutSite {
                executable,
                site: *site,
            });
        };
        if mapped.class != point.class || mapped.units != point.units {
            return Err(crate::linker::LinkError::MirBudgetLayoutMismatch {
                executable,
                site: *site,
                offset: mapped.offset,
            });
        }
        let Some(instruction) = code.instructions.get(mapped.offset.0) else {
            return Err(crate::linker::LinkError::MirBudgetLayoutMismatch {
                executable,
                site: *site,
                offset: mapped.offset,
            });
        };
        if !instruction_implements_budget_boundary(instruction, *mapped) {
            return Err(crate::linker::LinkError::MirBudgetLayoutMismatch {
                executable,
                site: *site,
                offset: mapped.offset,
            });
        }
    }
    if sealed.keys().any(|site| !expected.contains_key(site)) {
        return Err(crate::linker::LinkError::ExtraMirBudgetLayoutSite { executable });
    }
    for (offset, instruction) in code.instructions.iter().enumerate() {
        let first_at_origin = instruction
            .mir_origin
            .is_none_or(|origin| seen_origins.insert(origin));
        let encoded_units = u64::from(instruction.execution_units)
            + match instruction.kind {
                InstructionKind::ChargeExecutionUnits { units } => u64::from(units),
                _ => 0,
            };
        let charged_units = instruction
            .mir_budget_charges
            .iter()
            .map(|charge| u64::from(charge.units))
            .sum::<u64>();
        actual_units = actual_units.saturating_add(encoded_units);
        if encoded_units != charged_units {
            return Err(crate::linker::LinkError::MirBudgetEncodingMismatch {
                executable,
                offset: crate::InstructionOffset(offset),
                encoded_units,
                mapped_units: charged_units,
            });
        }
        for charge in &instruction.mir_budget_charges {
            let Some(point) = expected.get(&charge.site) else {
                return Err(crate::linker::LinkError::ExtraMirBudgetCharge {
                    executable,
                    offset: crate::InstructionOffset(offset),
                    site: charge.site,
                });
            };
            if !seen.insert(charge.site) {
                return Err(crate::linker::LinkError::DuplicateMirBudgetCharge {
                    executable,
                    site: charge.site,
                });
            }
            if charge.class != point.class || charge.units != point.units {
                return Err(crate::linker::LinkError::MirBudgetChargeMismatch {
                    executable,
                    site: charge.site,
                    expected: *point,
                    actual_class: charge.class,
                    actual_units: charge.units,
                });
            }
            if instruction.mir_origin != Some(charge.site) || !first_at_origin {
                return Err(crate::linker::LinkError::MirBudgetPlacementMismatch {
                    executable,
                    offset: crate::InstructionOffset(offset),
                    site: charge.site,
                    origin: instruction.mir_origin,
                });
            }
            if matches!(charge.site, vela_mir::MirBudgetSite::Edge { .. })
                && !matches!(instruction.kind, InstructionKind::ChargeExecutionUnits { units } if units == charge.units)
            {
                return Err(crate::linker::LinkError::MirBudgetPlacementMismatch {
                    executable,
                    offset: crate::InstructionOffset(offset),
                    site: charge.site,
                    origin: instruction.mir_origin,
                });
            }
        }
    }
    for (site, point) in expected {
        expected_units = expected_units.saturating_add(u64::from(point.units));
        if !seen.contains(&site) {
            return Err(crate::linker::LinkError::MissingMirBudgetCharge { executable, site });
        }
    }
    if expected_units != actual_units {
        return Err(crate::linker::LinkError::MirBudgetTotalMismatch {
            executable,
            expected_units,
            actual_units,
        });
    }
    Ok(())
}

fn instruction_implements_budget_boundary(
    instruction: &crate::Instruction,
    mapped: crate::compiler::ExecutableBudgetSite,
) -> bool {
    if instruction.mir_origin != Some(mapped.site)
        || instruction.execution_units
            + match instruction.kind {
                InstructionKind::ChargeExecutionUnits { units } => units,
                _ => 0,
            }
            != mapped.units
        || instruction.mir_budget_charges.iter().all(|charge| {
            charge.site != mapped.site
                || charge.class != mapped.class
                || charge.units != mapped.units
        })
    {
        return false;
    }
    match mapped.boundary {
        crate::compiler::ExecutableBudgetBoundary::EdgeStub => matches!(
            instruction.kind,
            InstructionKind::ChargeExecutionUnits { units } if units == mapped.units
        ),
        crate::compiler::ExecutableBudgetBoundary::Operation => {
            !matches!(
                instruction.kind,
                InstructionKind::ChargeExecutionUnits { .. }
                    | InstructionKind::Jump { .. }
                    | InstructionKind::Return { .. }
            ) && instruction_matches_budget_class(&instruction.kind, mapped.class)
        }
    }
}

fn instruction_matches_budget_class(
    kind: &InstructionKind,
    class: vela_mir::MirBudgetClass,
) -> bool {
    use vela_mir::MirBudgetClass as Class;

    match class {
        Class::LoopBackedge => false,
        Class::IteratorStep => matches!(
            kind,
            InstructionKind::IterNext { .. }
                | InstructionKind::RangeNext { .. }
                | InstructionKind::I64RangeNext { .. }
        ),
        Class::Call => matches!(
            kind,
            InstructionKind::CallNative { .. }
                | InstructionKind::CallFunction { .. }
                | InstructionKind::CallClosure { .. }
                | InstructionKind::CallMethod { .. }
                | InstructionKind::CallDynamicMethod { .. }
        ),
        Class::HostAccess => matches!(
            kind,
            InstructionKind::HostRead { .. }
                | InstructionKind::HostWrite { .. }
                | InstructionKind::HostMutate { .. }
                | InstructionKind::HostRemove { .. }
                | InstructionKind::HostCall { .. }
        ),
        Class::Reflection => matches!(kind, InstructionKind::CallNative { .. }),
        Class::Allocation => matches!(
            kind,
            InstructionKind::LoadConst { .. }
                | InstructionKind::MakeClosure { .. }
                | InstructionKind::MakeArray { .. }
                | InstructionKind::MakeTuple { .. }
                | InstructionKind::MakeSetFromArray { .. }
                | InstructionKind::FormatString { .. }
                | InstructionKind::MakeMap { .. }
                | InstructionKind::MakeRecord { .. }
                | InstructionKind::MakeEnum { .. }
                | InstructionKind::IterInit { .. }
        ),
        Class::DynamicWork => matches!(
            kind,
            InstructionKind::Not { .. }
                | InstructionKind::Negate { .. }
                | InstructionKind::Add { .. }
                | InstructionKind::Sub { .. }
                | InstructionKind::Mul { .. }
                | InstructionKind::Div { .. }
                | InstructionKind::Rem { .. }
                | InstructionKind::Equal { .. }
                | InstructionKind::NotEqual { .. }
                | InstructionKind::IdentityEqual { .. }
                | InstructionKind::IdentityNotEqual { .. }
                | InstructionKind::Less { .. }
                | InstructionKind::LessEqual { .. }
                | InstructionKind::Greater { .. }
                | InstructionKind::GreaterEqual { .. }
                | InstructionKind::BinaryIntLiteral { .. }
                | InstructionKind::BinaryFloatLiteral { .. }
                | InstructionKind::GuardType { .. }
                | InstructionKind::GuardTupleArity { .. }
                | InstructionKind::GetIndex { .. }
                | InstructionKind::GetStringKeyIndex { .. }
                | InstructionKind::SetIndex { .. }
                | InstructionKind::SetStringKeyIndex { .. }
        ),
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

fn verify_unbound_cache_correspondence(
    artifact: &UnboundLinkedProgram,
) -> Result<(), crate::verification::VerificationError> {
    for (handle, code) in artifact.program().functions() {
        let image = artifact
            .image()
            .function(crate::FunctionIndex(handle.index()))
            .ok_or_else(|| crate::verification::VerificationError {
                function: "<artifact>".to_owned(),
                instruction: None,
                kind: crate::verification::VerificationErrorKind::FunctionIndexOutOfBounds {
                    function: crate::FunctionIndex(handle.index()),
                    function_count: artifact.image().function_count(),
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
            .link_test_program(&program)
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
