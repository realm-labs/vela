use std::sync::Arc;

use vela_bytecode::{ArtifactTaskServiceRequirement, LinkedArtifact};
use vela_common::Detachability;
use vela_mir::{MirEffect, MirTypeContract};
use vela_reflect::access::FunctionEffectSet;
use vela_reflect::modules::{DetachedTargetDesc, DetachedValueMode};
use vela_reflect::registry::TypeRegistry;

pub(super) fn registry_for_artifact(
    base: Arc<TypeRegistry>,
    artifact: &LinkedArtifact,
) -> Arc<TypeRegistry> {
    let mut registry = (*base).clone();
    for target in artifact.task_targets() {
        let reflected = DetachedTargetDesc {
            parameter_contracts: target
                .worker_signature
                .parameters
                .iter()
                .map(|parameter| {
                    parameter
                        .contract
                        .as_ref()
                        .map_or_else(|| "Any".to_owned(), task_contract_display)
                })
                .collect(),
            parameter_modes: target
                .worker_signature
                .parameter_detachability
                .iter()
                .map(|fact| reflect_detachability(*fact))
                .collect(),
            result_contract: target
                .worker_signature
                .return_contract
                .as_ref()
                .map_or_else(|| "Any".to_owned(), task_contract_display),
            result_mode: reflect_detachability(target.worker_signature.result_detachability),
            effects: reflect_task_effects(target.worker_signature.effects),
            requires_service_generation: matches!(
                target.service_requirement,
                ArtifactTaskServiceRequirement::InheritOriginatingGeneration
            ),
        };
        registry.register_detached_target(target.worker, reflected);
    }
    Arc::new(registry)
}

fn reflect_detachability(fact: Detachability) -> DetachedValueMode {
    match fact {
        Detachability::Detachable => DetachedValueMode::Detachable,
        Detachability::RuntimeChecked => DetachedValueMode::RuntimeChecked,
        Detachability::NonDetachable(_) => {
            unreachable!("linked task target rejects non-detachable contracts")
        }
    }
}

fn reflect_task_effects(effect: MirEffect) -> FunctionEffectSet {
    FunctionEffectSet {
        reads_host: effect.host_read || effect.host_write,
        writes_host: effect.host_write,
        emits_events: effect.emits_event,
        reads_time: effect.reads_time,
        uses_random: effect.uses_random,
        reads_io: effect.reads_io,
        writes_io: effect.writes_io,
        reads_reflection: effect.reflection_read,
        writes_reflection: effect.reflection_write,
        calls_reflection: effect.reflection_call,
        spawns_tasks: effect.task_spawn,
    }
}

fn task_contract_display(contract: &MirTypeContract) -> String {
    format!("{contract:?}")
}
