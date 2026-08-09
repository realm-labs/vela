use std::collections::BTreeSet;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::linked::InstructionKind;
use crate::{CacheSiteDesc, CacheSiteId, ExecutableGenerationId, LinkedProgram, ProgramImage};

mod task;

#[cfg(feature = "artifact-codec")]
pub(crate) use task::collect_compiled_task_targets;
pub use task::{
    ArtifactFeatureSet, ArtifactTaskContinuation, ArtifactTaskOperation, ArtifactTaskParameter,
    ArtifactTaskServiceRequirement, ArtifactTaskSignature, ArtifactTaskTarget,
};
use task::{collect_task_targets, verify_task_target_table};

static NEXT_EXECUTABLE_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Content checksum for one immutable linked artifact.
///
/// The process-local executable generation ID is deliberately excluded so
/// independently linked copies of the same deployment artifact compare equal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactChecksum([u8; 32]);

impl ArtifactChecksum {
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ArtifactChecksum {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// One immutable linker output for every generation-local executable layout.
#[derive(Debug, PartialEq)]
pub struct LinkedArtifact {
    program: Arc<LinkedProgram>,
    image: ProgramImage,
    cache_layout: Box<[CacheSiteDesc]>,
    profile_layout: ProfileLayout,
    mir_executables: Box<[MirExecutableLayout]>,
    verified_mir: Arc<vela_mir::OwnedVerifiedMirBundle>,
    binding_schema: Arc<crate::RustBindingSchema>,
    package_metadata: Option<crate::PackageArtifactMetadata>,
    required_features: ArtifactFeatureSet,
    task_targets: Box<[ArtifactTaskTarget]>,
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
    pub scalar_units: Box<[ProfileScalarUnitLayout]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileScalarUnitLayout {
    pub offset: crate::InstructionOffset,
    pub plan: crate::ScalarBlockPlanId,
    pub source_count: usize,
    pub has_range_loop: bool,
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
        Arc, ArtifactFeatureSet, ExecutableGenerationId, LinkedArtifact, LinkedProgram,
        NEXT_EXECUTABLE_GENERATION, Ordering, ProgramImage, profile_layout, test_mir_binding,
    };

    /// Declares one extern state on a fixture program and returns its slot.
    ///
    /// Fixtures that execute `LoadExternState` need a matching declaration,
    /// because `linked_artifact` verifies every program it wraps.
    pub fn push_extern_state(
        program: &mut LinkedProgram,
        qualified_name: impl Into<String>,
    ) -> vela_common::StateSlot {
        let slot = vela_common::StateSlot::new(program.states().len());
        let id = vela_def::StateId::new(u128::try_from(slot.get()).unwrap_or_default() + 1);
        program.push_state(crate::LinkedStateDescriptor {
            id,
            qualified_name: qualified_name.into(),
            visibility: crate::StateVisibility::Private,
            storage: crate::StateStorage::Extern,
            type_contract: vela_mir::MirTypeContract::Host(vela_mir::HostTypeTarget {
                semantic: vela_def::TypeId::new(1),
                runtime: vela_common::HostTypeId::new(1),
            }),
            initializer: None,
            source_span: None,
        });
        slot
    }

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
        let profile_layout = profile_layout(&program);
        let (verified_mir, mir_executables) = test_mir_binding(&program);
        let artifact = LinkedArtifact {
            program: Arc::new(program),
            image: ProgramImage::from_program(&crate::UnlinkedProgram::new()),
            cache_layout,
            profile_layout,
            mir_executables,
            verified_mir,
            binding_schema: Arc::new(crate::RustBindingSchema::empty()),
            package_metadata: None,
            required_features: ArtifactFeatureSet::empty(),
            task_targets: Box::new([]),
        };
        // Every `LinkedArtifact` the interpreter can execute must be verified,
        // including fixtures. The VM indexes registers without a bounds check
        // and relies on `verify_register_count` having already proven every
        // register operand in range; see `vela_vm::frame::registers`.
        artifact
            .program
            .verify()
            .expect("test fixture linked program must verify before it can be executed");
        Arc::new(artifact)
    }

    #[must_use]
    pub fn into_linked_program(artifact: Arc<LinkedArtifact>) -> LinkedProgram {
        let artifact = Arc::try_unwrap(artifact).expect("test artifact must have one owner");
        Arc::try_unwrap(artifact.program).expect("test linked program must have one owner")
    }
}

impl LinkedArtifact {
    /// Returns a deterministic checksum of linked code and sealed metadata.
    ///
    /// This is computed on demand because deployment staging is off the
    /// request path. Runtime dispatch never hashes an artifact.
    #[must_use]
    pub fn checksum(&self) -> ArtifactChecksum {
        let mut program = self.program.as_ref().clone();
        program.set_generation(ExecutableGenerationId::default());
        let mut hasher = blake3::Hasher::new();
        hash_debug(&mut hasher, &program);
        hash_debug(&mut hasher, &self.image);
        hash_debug(&mut hasher, &self.cache_layout);
        hash_debug(&mut hasher, &self.profile_layout);
        hash_debug(&mut hasher, &self.mir_executables);
        hash_debug(&mut hasher, &self.verified_mir);
        hash_debug(&mut hasher, &self.binding_schema);
        hash_debug(&mut hasher, &self.package_metadata);
        hash_debug(&mut hasher, &self.required_features);
        hash_debug(&mut hasher, &self.task_targets);
        ArtifactChecksum::new(*hasher.finalize().as_bytes())
    }

    pub(crate) fn finish_unbound(
        image: ProgramImage,
        mut program: LinkedProgram,
    ) -> Result<UnboundLinkedProgram, crate::verification::VerificationError> {
        program.set_generation(ExecutableGenerationId::new(
            NEXT_EXECUTABLE_GENERATION.fetch_add(1, Ordering::Relaxed),
        ));
        let cache_layout = image.cache_sites().to_vec().into_boxed_slice();
        let profile_layout = profile_layout(&program);
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

    #[must_use]
    pub fn binding_schema(&self) -> &Arc<crate::RustBindingSchema> {
        &self.binding_schema
    }

    #[must_use]
    pub const fn package_metadata(&self) -> Option<&crate::PackageArtifactMetadata> {
        self.package_metadata.as_ref()
    }

    #[must_use]
    pub const fn required_features(&self) -> ArtifactFeatureSet {
        self.required_features
    }

    #[must_use]
    pub fn task_targets(&self) -> &[ArtifactTaskTarget] {
        &self.task_targets
    }

    pub fn verify(&self) -> Result<(), crate::verification::VerificationError> {
        self.image.verify()?;
        self.program.verify()?;
        verify_cache_correspondence(self)
    }
}

fn hash_debug(hasher: &mut blake3::Hasher, value: &impl fmt::Debug) {
    use fmt::Write as _;

    struct HashWriter<'a>(&'a mut blake3::Hasher);

    impl fmt::Write for HashWriter<'_> {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.0.update(value.as_bytes());
            Ok(())
        }
    }

    write!(HashWriter(hasher), "{value:?}").expect("hash writer is infallible");
    hasher.update(&[0]);
}

impl UnboundLinkedProgram {
    #[cfg(feature = "artifact-codec")]
    pub(crate) fn bind_portable(
        self,
        binding_schema: Arc<crate::RustBindingSchema>,
        required_features: ArtifactFeatureSet,
        task_targets: Box<[ArtifactTaskTarget]>,
    ) -> Result<LinkedArtifact, crate::linker::LinkError> {
        let artifact = LinkedArtifact {
            program: self.program,
            image: self.image,
            cache_layout: self.cache_layout,
            profile_layout: self.profile_layout,
            mir_executables: Box::new([]),
            verified_mir: Arc::new(vela_mir::OwnedVerifiedMirBundle::default()),
            binding_schema,
            package_metadata: None,
            required_features,
            task_targets,
        };
        verify_task_target_table(&artifact)?;
        Ok(artifact)
    }

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
            binding_schema: Arc::new(crate::RustBindingSchema::empty()),
            package_metadata: None,
            required_features: ArtifactFeatureSet::empty(),
            task_targets: Box::new([]),
        }
    }

    pub(crate) fn bind_compiled_mir(
        self,
        bundle: Arc<vela_mir::OwnedVerifiedMirBundle>,
        binding_schema: Arc<crate::RustBindingSchema>,
        compiled_layouts: &[crate::compiler::CompiledMirExecutable],
        budget_layouts: &[crate::compiler::CompiledExecutableBudgetLayout],
        package_metadata: Option<crate::PackageArtifactMetadata>,
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
            let function = owner.program().function(expected.function).ok_or(
                crate::linker::LinkError::MissingMirFunction {
                    root: expected.root,
                    function: expected.function,
                },
            )?;
            verify_selected_source_mapping(index, code, function, &analyses.budget)?;
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
        let task_targets = collect_task_targets(&bundle, &mir_executables)?;
        let required_features = if task_targets.is_empty() {
            ArtifactFeatureSet::empty()
        } else {
            ArtifactFeatureSet::host_scoped_tasks()
        };
        let artifact = LinkedArtifact {
            program: self.program,
            image: self.image,
            cache_layout: self.cache_layout,
            profile_layout: self.profile_layout,
            mir_executables,
            verified_mir: bundle,
            binding_schema,
            package_metadata,
            required_features,
            task_targets,
        };
        verify_task_target_table(&artifact)?;
        Ok(artifact)
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

fn verify_selected_source_mapping(
    executable: usize,
    code: &crate::LinkedCodeObject,
    function: &vela_mir::MirFunction,
    budget: &vela_mir::MirBudgetSchedule,
) -> Result<(), crate::linker::LinkError> {
    for selected in &code.selected_units {
        let mismatch = || crate::linker::LinkError::MirSelectedPlanMismatch {
            executable,
            instruction: selected.instruction,
        };
        let statement_id = selected.mir_statement.ok_or_else(mismatch)?;
        let block_id = selected.mir_terminator.ok_or_else(mismatch)?;
        let block = function.block(block_id).ok_or_else(mismatch)?;
        if !block.statements().contains(&statement_id) {
            return Err(mismatch());
        }
        let statement = function.statement(statement_id).ok_or_else(mismatch)?;
        let terminator = block.terminator().ok_or_else(mismatch)?;
        if selected.source_points.as_ref()
            != [statement.origin.span, terminator.origin.span].as_slice()
        {
            return Err(mismatch());
        }
    }
    for (plan_index, plan) in code.scalar_blocks.iter().enumerate() {
        let plan_id = crate::ScalarBlockPlanId::new(plan_index);
        let instruction = code
            .instructions
            .iter()
            .position(|instruction| {
                matches!(instruction.kind, InstructionKind::RunScalarBlock { plan } if plan == plan_id)
            })
            .map(crate::InstructionOffset)
            .unwrap_or(crate::InstructionOffset(0));
        let mismatch = || crate::linker::LinkError::MirSelectedPlanMismatch {
            executable,
            instruction,
        };
        let block_id = plan.mir_terminator.ok_or_else(mismatch)?;
        let block = function.block(block_id).ok_or_else(mismatch)?;
        let terminator = block.terminator().ok_or_else(mismatch)?;
        if plan.mir_statements.as_ref() != block.statements()
            || plan.operations.len() != plan.mir_statements.len()
            || plan.source_points.get(plan.exit.source.index()).copied()
                != Some(terminator.origin.span)
            || plan.exit.execution_units
                != budget
                    .terminator_before(block_id)
                    .map_or(0, |point| point.units)
        {
            return Err(mismatch());
        }
        for ((statement_id, operation), operation_index) in plan
            .mir_statements
            .iter()
            .zip(plan.operations.iter())
            .zip(0_usize..)
        {
            let statement = function.statement(*statement_id).ok_or_else(mismatch)?;
            if plan.source_points.get(operation.source.index()).copied()
                != Some(statement.origin.span)
                || operation.execution_units
                    != budget
                        .statement_before(*statement_id)
                        .map_or(0, |point| point.units)
                || !scalar_budget_site_matches(
                    plan,
                    vela_mir::MirBudgetSite::StatementBefore(*statement_id),
                    crate::scalar_plan::ScalarBudgetLocation::Operation(operation_index),
                    budget.statement_before(*statement_id),
                )
            {
                return Err(mismatch());
            }
        }
        if !scalar_budget_site_matches(
            plan,
            vela_mir::MirBudgetSite::TerminatorBefore(block_id),
            crate::scalar_plan::ScalarBudgetLocation::Exit,
            budget.terminator_before(block_id),
        ) || !scalar_exit_budget_matches(plan, block_id, &terminator.kind, budget)
            || !scalar_range_loop_source_matches(plan, block_id, function, budget)
        {
            return Err(mismatch());
        }
    }
    Ok(())
}

fn scalar_range_loop_source_matches(
    plan: &crate::ScalarBlockPlan,
    latch: vela_mir::MirBlockId,
    function: &vela_mir::MirFunction,
    budget: &vela_mir::MirBudgetSchedule,
) -> bool {
    let (Some(range_loop), Some(header)) = (plan.range_loop, plan.mir_range_header) else {
        return plan.range_loop.is_none() && plan.mir_range_header.is_none();
    };
    let Some(terminator) = function
        .block(header)
        .and_then(vela_mir::MirBasicBlock::terminator)
    else {
        return false;
    };
    let vela_mir::MirTerminatorKind::RangeNext {
        mode: vela_mir::MirRangeStepMode::I64Proven,
        next,
        done,
        inclusive,
        ..
    } = &terminator.kind
    else {
        return false;
    };
    if *next != latch
        || range_loop.inclusive != *inclusive
        || plan
            .source_points
            .get(range_loop.header_source.index())
            .copied()
            != Some(terminator.origin.span)
        || range_loop.header_execution_units
            != budget
                .terminator_before(header)
                .map_or(0, |point| point.units)
        || !scalar_edge_budget_matches(plan, range_loop.next_edge, header, *next, budget)
    {
        return false;
    }
    let expected = budget.edge(header, *done);
    range_loop.done_target.execution_units == expected.map_or(0, |point| point.units)
        && match expected {
            Some(point) => range_loop.done_target.budget_source.is_some_and(|source| {
                plan.source_points.get(source.index()).copied() == Some(point.origin.span)
            }),
            None => range_loop.done_target.budget_source.is_none(),
        }
}

fn scalar_edge_budget_matches(
    plan: &crate::ScalarBlockPlan,
    edge: crate::ChargedScalarEdge,
    from: vela_mir::MirBlockId,
    to: vela_mir::MirBlockId,
    budget: &vela_mir::MirBudgetSchedule,
) -> bool {
    let expected = budget.edge(from, to);
    edge.execution_units == expected.map_or(0, |point| point.units)
        && match expected {
            Some(point) => edge.budget_source.is_some_and(|source| {
                plan.source_points.get(source.index()).copied() == Some(point.origin.span)
            }),
            None => edge.budget_source.is_none(),
        }
}

fn scalar_budget_site_matches(
    plan: &crate::ScalarBlockPlan,
    site: vela_mir::MirBudgetSite,
    location: crate::scalar_plan::ScalarBudgetLocation,
    expected: Option<vela_mir::MirBudgetPoint>,
) -> bool {
    let actual = plan
        .mir_budget_sites
        .iter()
        .filter(|candidate| candidate.site == site)
        .collect::<Vec<_>>();
    match expected {
        Some(point) => {
            actual.as_slice()
                == [&crate::scalar_plan::ScalarMirBudgetSite {
                    site,
                    point,
                    location,
                }]
        }
        None => actual.is_empty(),
    }
}

fn scalar_exit_budget_matches(
    plan: &crate::ScalarBlockPlan,
    from: vela_mir::MirBlockId,
    terminator: &vela_mir::MirTerminatorKind,
    budget: &vela_mir::MirBudgetSchedule,
) -> bool {
    let matches_target =
        |target: &crate::ChargedScalarTarget,
         to: vela_mir::MirBlockId,
         location: crate::scalar_plan::ScalarBudgetLocation| {
            let expected = budget.edge(from, to);
            target.execution_units == expected.map_or(0, |point| point.units)
                && match expected {
                    Some(point) => target.budget_source.is_some_and(|source| {
                        plan.source_points.get(source.index()).copied() == Some(point.origin.span)
                    }),
                    None => target.budget_source.is_none(),
                }
                && scalar_budget_site_matches(
                    plan,
                    vela_mir::MirBudgetSite::Edge { from, to },
                    location,
                    expected,
                )
        };
    match (&plan.exit.kind, terminator) {
        (crate::ScalarExitKind::Jump(target), vela_mir::MirTerminatorKind::Jump(to)) => {
            matches_target(
                target,
                *to,
                crate::scalar_plan::ScalarBudgetLocation::JumpEdge,
            )
        }
        (
            crate::ScalarExitKind::BoolBranch { passed, failed, .. },
            vela_mir::MirTerminatorKind::Branch {
                then_block,
                else_block,
                ..
            },
        ) => {
            matches_target(
                passed,
                *then_block,
                crate::scalar_plan::ScalarBudgetLocation::PassedEdge,
            ) && matches_target(
                failed,
                *else_block,
                crate::scalar_plan::ScalarBudgetLocation::FailedEdge,
            )
        }
        _ => false,
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
                    asyncness: vela_common::CallableAsyncness::Sync,
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
        if !instruction_implements_budget_boundary(code, instruction, *mapped) {
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
    for plan in &code.scalar_blocks {
        for scalar_site in &plan.mir_budget_sites {
            let Some(point) = expected.get(&scalar_site.site) else {
                return Err(crate::linker::LinkError::ExtraMirBudgetCharge {
                    executable,
                    offset: crate::InstructionOffset(0),
                    site: scalar_site.site,
                });
            };
            if !seen.insert(scalar_site.site) {
                return Err(crate::linker::LinkError::DuplicateMirBudgetCharge {
                    executable,
                    site: scalar_site.site,
                });
            }
            if scalar_site.point != *point {
                return Err(crate::linker::LinkError::MirBudgetChargeMismatch {
                    executable,
                    site: scalar_site.site,
                    expected: *point,
                    actual_class: scalar_site.point.class,
                    actual_units: scalar_site.point.units,
                });
            }
            actual_units = actual_units.saturating_add(u64::from(scalar_site.point.units));
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
    code: &crate::LinkedCodeObject,
    instruction: &crate::Instruction,
    mapped: crate::compiler::ExecutableBudgetSite,
) -> bool {
    if let crate::compiler::ExecutableBudgetBoundary::Scalar { plan, location } = mapped.boundary {
        return scalar_instruction_implements_budget_boundary(
            code,
            instruction,
            mapped,
            plan,
            location,
        );
    }
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
        crate::compiler::ExecutableBudgetBoundary::Scalar { .. } => false,
    }
}

fn scalar_instruction_implements_budget_boundary(
    code: &crate::LinkedCodeObject,
    instruction: &crate::Instruction,
    mapped: crate::compiler::ExecutableBudgetSite,
    plan_id: crate::ScalarBlockPlanId,
    location: crate::scalar_plan::ScalarBudgetLocation,
) -> bool {
    if !matches!(instruction.kind, InstructionKind::RunScalarBlock { plan } if plan == plan_id) {
        return false;
    }
    let Some(plan) = code.scalar_blocks.get(plan_id.index()) else {
        return false;
    };
    let mut sites = plan.mir_budget_sites.iter().filter(|site| {
        site.site == mapped.site
            && site.point.class == mapped.class
            && site.point.units == mapped.units
            && site.location == location
    });
    let Some(site) = sites.next() else {
        return false;
    };
    if sites.next().is_some() {
        return false;
    }
    let source_matches = |source: crate::ScalarSourcePointId| {
        plan.source_points.get(source.index()).copied() == Some(site.point.origin.span)
    };
    match location {
        crate::scalar_plan::ScalarBudgetLocation::Operation(index) => {
            plan.operations.get(index).is_some_and(|operation| {
                operation.execution_units == mapped.units && source_matches(operation.source)
            })
        }
        crate::scalar_plan::ScalarBudgetLocation::Exit => {
            plan.exit.execution_units == mapped.units && source_matches(plan.exit.source)
        }
        crate::scalar_plan::ScalarBudgetLocation::JumpEdge => match plan.exit.kind {
            crate::ScalarExitKind::Jump(target) => {
                scalar_target_implements_budget(plan, target, mapped.units, site.point.origin.span)
            }
            _ => false,
        },
        crate::scalar_plan::ScalarBudgetLocation::PassedEdge => match plan.exit.kind {
            crate::ScalarExitKind::BoolBranch { passed, .. }
            | crate::ScalarExitKind::I64CompareBranch { passed, .. } => {
                scalar_target_implements_budget(plan, passed, mapped.units, site.point.origin.span)
            }
            _ => false,
        },
        crate::scalar_plan::ScalarBudgetLocation::FailedEdge => match plan.exit.kind {
            crate::ScalarExitKind::BoolBranch { failed, .. }
            | crate::ScalarExitKind::I64CompareBranch { failed, .. } => {
                scalar_target_implements_budget(plan, failed, mapped.units, site.point.origin.span)
            }
            _ => false,
        },
    }
}

fn scalar_target_implements_budget(
    plan: &crate::ScalarBlockPlan,
    target: crate::ChargedScalarTarget,
    units: u32,
    span: vela_common::Span,
) -> bool {
    target.execution_units == units
        && target
            .budget_source
            .is_some_and(|source| plan.source_points.get(source.index()).copied() == Some(span))
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
                | InstructionKind::Task(_)
        ),
        Class::HostAccess => matches!(
            kind,
            InstructionKind::ReleaseBorrowLease { .. }
                | InstructionKind::TryReleaseBorrowLease { .. }
                | InstructionKind::HostRead { .. }
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

fn profile_layout(program: &LinkedProgram) -> ProfileLayout {
    ProfileLayout {
        functions: program
            .functions()
            .map(|(handle, code)| ProfileFunctionLayout {
                handle,
                debug_name: code.debug_name,
                instruction_count: code.instructions.len(),
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
    use vela_common::StateSlot;

    use crate::{
        CacheSiteKind, FunctionIndex, InstructionOffset, Linker, Register, UnlinkedCodeObject,
        UnlinkedInstruction, UnlinkedInstructionKind, UnlinkedProgram,
    };

    #[test]
    fn nested_local_cache_zero_sites_receive_distinct_generation_ids() {
        let first = cached_state_lambda("first", "main::first", StateSlot::new(0));
        let second = cached_state_lambda("second", "main::second", StateSlot::new(1));
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
        program.set_states([
            crate::StateDescriptor::test_extern(vela_def::StateId::new(1), "main::first"),
            crate::StateDescriptor::test_extern(vela_def::StateId::new(2), "main::second"),
        ]);
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

    fn cached_state_lambda(name: &str, state: &str, slot: StateSlot) -> UnlinkedCodeObject {
        let mut code = UnlinkedCodeObject::new(name, 1);
        let site = code.push_cache_site(CacheSiteKind::ExternStateRead, InstructionOffset(0));
        code.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::LoadExternState {
                dst: Register(0),
                state: state.to_owned(),
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
