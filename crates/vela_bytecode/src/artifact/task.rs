//! Sealed linked and portable metadata for host-scoped task targets.

use std::collections::{BTreeMap, BTreeSet};

use super::{LinkedArtifact, MirExecutableLayout};

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ArtifactFeatureSet {
    bits: u64,
}

impl ArtifactFeatureSet {
    pub const HOST_SCOPED_TASKS: u64 = 1 << 0;
    pub const SUPPORTED: Self = Self {
        bits: Self::HOST_SCOPED_TASKS,
    };

    #[must_use]
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    #[must_use]
    pub const fn host_scoped_tasks() -> Self {
        Self {
            bits: Self::HOST_SCOPED_TASKS,
        }
    }

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.bits
    }

    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    #[must_use]
    pub const fn contains(self, required: Self) -> bool {
        self.bits & required.bits == required.bits
    }

    #[must_use]
    pub const fn has_unknown(self) -> bool {
        self.bits & !Self::SUPPORTED.bits != 0
    }
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactTaskOperation {
    SpawnScoped,
    SpawnScopedThen,
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactTaskServiceRequirement {
    /// Use an ordinary capsule when admitted outside Services and retain the
    /// exact originating Service generation when one is present.
    InheritOriginatingGeneration,
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactTaskParameter {
    pub contract: Option<vela_mir::MirTypeContract>,
    pub has_default: bool,
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactTaskSignature {
    pub asyncness: vela_common::CallableAsyncness,
    pub parameters: Box<[ArtifactTaskParameter]>,
    pub parameter_detachability: Box<[vela_common::Detachability]>,
    pub return_contract: Option<vela_mir::MirTypeContract>,
    pub result_detachability: vela_common::Detachability,
    pub effects: vela_mir::MirEffect,
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactTaskContinuation {
    pub function: vela_def::FunctionId,
    pub target: u32,
    pub debug_name: String,
    pub outcome_contract: vela_mir::MirTypeContract,
    pub resume_parameters: Box<[ArtifactTaskParameter]>,
    pub effects: vela_mir::MirEffect,
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactTaskTarget {
    pub operation: ArtifactTaskOperation,
    pub caller: vela_def::FunctionId,
    pub caller_target: u32,
    pub worker: vela_def::FunctionId,
    pub worker_target: u32,
    pub worker_debug_name: String,
    pub worker_signature: ArtifactTaskSignature,
    pub continuation: Option<ArtifactTaskContinuation>,
    pub service_requirement: ArtifactTaskServiceRequirement,
}

pub(super) fn collect_task_targets(
    bundle: &vela_mir::OwnedVerifiedMirBundle,
    layouts: &[MirExecutableLayout],
) -> Result<Box<[ArtifactTaskTarget]>, crate::linker::LinkError> {
    fn parameters(values: &[vela_mir::CompileParameter]) -> Box<[ArtifactTaskParameter]> {
        values
            .iter()
            .map(|parameter| ArtifactTaskParameter {
                contract: parameter.contract.clone(),
                has_default: !matches!(
                    parameter.default,
                    vela_mir::CompileParameterDefault::Required
                ),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    fn handle(
        layouts: &[MirExecutableLayout],
        function: vela_def::FunctionId,
        debug_name: &str,
    ) -> Result<u32, crate::linker::LinkError> {
        layouts
            .iter()
            .find(|layout| layout.root == function)
            .map(|layout| layout.handle.get())
            .ok_or_else(|| crate::linker::LinkError::MissingTaskTarget {
                name: debug_name.to_owned(),
                id: function,
            })
    }

    let effects = sealed_transitive_effects(bundle, layouts)?;
    let mut targets = Vec::new();
    for layout in layouts {
        let owner = bundle
            .root(layout.root)
            .ok_or(crate::linker::LinkError::MissingMirRoot { root: layout.root })?;
        let function = owner.program().function(layout.function).ok_or(
            crate::linker::LinkError::MissingMirFunction {
                root: layout.root,
                function: layout.function,
            },
        )?;
        for (_, statement) in function.statements() {
            let vela_mir::MirStatementKind::Task(task) = &statement.kind else {
                continue;
            };
            let worker_target = handle(layouts, task.worker, &task.worker_debug_name)?;
            let continuation = task
                .continuation
                .as_ref()
                .map(|continuation| {
                    let target = handle(layouts, continuation.function, &continuation.debug_name)?;
                    Ok(ArtifactTaskContinuation {
                        function: continuation.function,
                        target,
                        debug_name: continuation.debug_name.clone(),
                        outcome_contract: continuation.outcome_contract.clone(),
                        resume_parameters: parameters(&continuation.resume_parameters),
                        effects: effects
                            .get(&continuation.function)
                            .copied()
                            .unwrap_or(continuation.signature.effect),
                    })
                })
                .transpose()?;
            targets.push(ArtifactTaskTarget {
                operation: if continuation.is_some() {
                    ArtifactTaskOperation::SpawnScopedThen
                } else {
                    ArtifactTaskOperation::SpawnScoped
                },
                caller: layout.root,
                caller_target: layout.handle.get(),
                worker: task.worker,
                worker_target,
                worker_debug_name: task.worker_debug_name.clone(),
                worker_signature: ArtifactTaskSignature {
                    asyncness: task.worker_signature.asyncness,
                    parameters: parameters(&task.worker_signature.parameters),
                    parameter_detachability: task
                        .detachability
                        .parameters
                        .clone()
                        .into_boxed_slice(),
                    return_contract: task.worker_signature.return_contract.clone(),
                    result_detachability: task.detachability.result,
                    effects: effects
                        .get(&task.worker)
                        .copied()
                        .unwrap_or(task.worker_signature.effect),
                },
                continuation,
                service_requirement: ArtifactTaskServiceRequirement::InheritOriginatingGeneration,
            });
        }
    }
    Ok(targets.into_boxed_slice())
}

fn sealed_transitive_effects(
    bundle: &vela_mir::OwnedVerifiedMirBundle,
    layouts: &[MirExecutableLayout],
) -> Result<BTreeMap<vela_def::FunctionId, vela_mir::MirEffect>, crate::linker::LinkError> {
    let mut effects = BTreeMap::new();
    let mut callees = BTreeMap::<_, BTreeSet<_>>::new();
    for layout in layouts {
        let owner = bundle
            .root(layout.root)
            .ok_or(crate::linker::LinkError::MissingMirRoot { root: layout.root })?;
        let function = owner.program().function(layout.function).ok_or(
            crate::linker::LinkError::MissingMirFunction {
                root: layout.root,
                function: layout.function,
            },
        )?;
        let mut direct = vela_mir::MirEffect::PURE;
        let mut targets = BTreeSet::new();
        for (_, statement) in function.statements() {
            direct = direct.union(statement.effect);
            collect_statement_callees(&statement.kind, &mut targets);
        }
        for (_, block) in function.blocks() {
            if let Some(terminator) = block.terminator() {
                direct = direct.union(terminator.effect);
                if let vela_mir::MirTerminatorKind::AwaitCall { operation, .. } = &terminator.kind
                    && let vela_mir::MirAwaitOperation::Call(call) = operation.as_ref()
                {
                    collect_call_callee(call, &mut targets);
                }
            }
        }
        effects.insert(layout.root, direct);
        callees.insert(layout.root, targets);
    }

    loop {
        let mut changed = false;
        for (function, targets) in &callees {
            let mut effect = effects[function];
            for target in targets {
                if let Some(callee) = effects.get(target) {
                    effect = effect.union(*callee);
                }
            }
            if effect != effects[function] {
                effects.insert(*function, effect);
                changed = true;
            }
        }
        if !changed {
            return Ok(effects);
        }
    }
}

fn collect_statement_callees(
    statement: &vela_mir::MirStatementKind,
    targets: &mut BTreeSet<vela_def::FunctionId>,
) {
    match statement {
        vela_mir::MirStatementKind::Call(call) => collect_call_callee(call, targets),
        vela_mir::MirStatementKind::Task(task) => {
            targets.insert(task.worker);
            if let Some(continuation) = &task.continuation {
                targets.insert(continuation.function);
            }
        }
        _ => {}
    }
}

fn collect_call_callee(call: &vela_mir::MirCall, targets: &mut BTreeSet<vela_def::FunctionId>) {
    match call {
        vela_mir::MirCall::ScriptFunction { function, .. } => {
            targets.insert(*function);
        }
        vela_mir::MirCall::ScriptMethod { target, .. } => {
            targets.insert(target.function);
        }
        _ => {}
    }
}

#[cfg(feature = "artifact-codec")]
pub(crate) fn collect_compiled_task_targets(
    bundle: &vela_mir::OwnedVerifiedMirBundle,
    layouts: &[crate::compiler::CompiledMirExecutable],
) -> Result<Box<[ArtifactTaskTarget]>, crate::linker::LinkError> {
    let layouts = layouts
        .iter()
        .enumerate()
        .map(|(index, layout)| MirExecutableLayout {
            root: layout.root,
            function: layout.function,
            handle: crate::ScriptFunctionHandle::new(index),
        })
        .collect::<Vec<_>>();
    collect_task_targets(bundle, &layouts)
}

pub(super) fn verify_task_target_table(
    artifact: &LinkedArtifact,
) -> Result<(), crate::linker::LinkError> {
    if artifact.required_features.has_unknown() {
        return Err(crate::linker::LinkError::InvalidTaskMetadata(
            "artifact requires unknown feature bits".to_owned(),
        ));
    }
    let task_feature = artifact
        .required_features
        .contains(ArtifactFeatureSet::host_scoped_tasks());
    if task_feature != !artifact.task_targets.is_empty() {
        return Err(crate::linker::LinkError::InvalidTaskMetadata(
            "host-scoped task feature bit disagrees with the target table".to_owned(),
        ));
    }
    for target in &artifact.task_targets {
        let caller = crate::ScriptFunctionHandle::new(target.caller_target as usize);
        let worker = crate::ScriptFunctionHandle::new(target.worker_target as usize);
        if artifact.program.entry_point_by_id(target.caller) != Some(caller) {
            return Err(crate::linker::LinkError::InvalidTaskMetadata(format!(
                "task caller {:?} does not match target slot {}",
                target.caller, target.caller_target
            )));
        }
        if artifact.program.entry_point_by_id(target.worker) != Some(worker) {
            return Err(crate::linker::LinkError::InvalidTaskMetadata(format!(
                "task worker {:?} does not match target slot {}",
                target.worker, target.worker_target
            )));
        }
        let Some(worker_code) = artifact.program.function(worker) else {
            return Err(crate::linker::LinkError::InvalidTaskMetadata(
                "task worker slot is out of bounds".to_owned(),
            ));
        };
        if worker_code.asyncness != vela_common::CallableAsyncness::Async
            || target.worker_signature.asyncness != vela_common::CallableAsyncness::Async
            || worker_code.params.len() != target.worker_signature.parameters.len()
            || target.worker_signature.parameters.len()
                != target.worker_signature.parameter_detachability.len()
            || target
                .worker_signature
                .parameter_detachability
                .iter()
                .any(|fact| fact.rejection().is_some())
            || target
                .worker_signature
                .result_detachability
                .rejection()
                .is_some()
        {
            return Err(crate::linker::LinkError::InvalidTaskMetadata(format!(
                "task worker `{}` has an invalid sealed ABI",
                target.worker_debug_name
            )));
        }
        let expects_continuation =
            matches!(target.operation, ArtifactTaskOperation::SpawnScopedThen);
        if expects_continuation != target.continuation.is_some() {
            return Err(crate::linker::LinkError::InvalidTaskMetadata(
                "task operation disagrees with its continuation target".to_owned(),
            ));
        }
        if let Some(continuation) = &target.continuation {
            let handle = crate::ScriptFunctionHandle::new(continuation.target as usize);
            let code = artifact.program.function(handle).ok_or_else(|| {
                crate::linker::LinkError::InvalidTaskMetadata(
                    "task continuation slot is out of bounds".to_owned(),
                )
            })?;
            if artifact.program.entry_point_by_id(continuation.function) != Some(handle)
                || code.asyncness != vela_common::CallableAsyncness::Sync
                || code.params.len() != continuation.resume_parameters.len() + 1
            {
                return Err(crate::linker::LinkError::InvalidTaskMetadata(format!(
                    "task continuation `{}` has an invalid sealed ABI",
                    continuation.debug_name
                )));
            }
        }
    }
    Ok(())
}
