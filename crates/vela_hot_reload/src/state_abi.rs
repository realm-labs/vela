use std::collections::BTreeMap;

use vela_bytecode::{ProgramImage, StateDescriptor, StateStorage, StateVisibility};
use vela_def::StateId;
use vela_mir::MirTypeContract;

use crate::error::{HotReloadError, HotReloadErrorKind, HotReloadResult};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StateChanges {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub initializer_changed: Vec<String>,
    pub visibility_changed: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateAbi {
    pub id: StateId,
    pub qualified_name: String,
    pub visibility: StateVisibility,
    pub storage: StateStorage,
    pub type_contract: MirTypeContract,
}

impl From<&StateDescriptor> for StateAbi {
    fn from(state: &StateDescriptor) -> Self {
        Self {
            id: state.id,
            qualified_name: state.qualified_name.clone(),
            visibility: state.visibility,
            storage: state.storage,
            type_contract: state.type_contract.clone(),
        }
    }
}

pub(crate) fn compare_state_abi(
    previous: &ProgramImage,
    next: &ProgramImage,
) -> HotReloadResult<StateChanges> {
    let old = states_by_id(previous.states());
    let new = states_by_id(next.states());
    let mut changes = StateChanges::default();

    for (id, old_state) in &old {
        let Some(new_state) = new.get(id) else {
            changes.removed.push(old_state.qualified_name.clone());
            continue;
        };
        if old_state.storage != new_state.storage {
            return Err(HotReloadError::new(
                HotReloadErrorKind::ChangedStateStorage {
                    state: new_state.qualified_name.clone(),
                    old: old_state.storage,
                    new: new_state.storage,
                    source_span: new_state.source_span.map(Box::new),
                },
            ));
        }
        if old_state.type_contract != new_state.type_contract {
            return Err(HotReloadError::new(HotReloadErrorKind::ChangedStateType {
                state: new_state.qualified_name.clone(),
                old: Box::new(old_state.type_contract.clone()),
                new: Box::new(new_state.type_contract.clone()),
                source_span: new_state.source_span.map(Box::new),
            }));
        }
        if old_state.visibility != new_state.visibility {
            changes
                .visibility_changed
                .push(new_state.qualified_name.clone());
        }
        if initializer_changed(previous, old_state, next, new_state) {
            changes
                .initializer_changed
                .push(new_state.qualified_name.clone());
        }
    }

    changes.added.extend(
        new.iter()
            .filter(|(id, _)| !old.contains_key(id))
            .map(|(_, state)| state.qualified_name.clone()),
    );
    changes.added.sort();
    changes.removed.sort();
    changes.initializer_changed.sort();
    changes.visibility_changed.sort();
    Ok(changes)
}

fn states_by_id(states: &[StateDescriptor]) -> BTreeMap<StateId, &StateDescriptor> {
    states.iter().map(|state| (state.id, state)).collect()
}

fn initializer_changed(
    previous: &ProgramImage,
    old_state: &StateDescriptor,
    next: &ProgramImage,
    new_state: &StateDescriptor,
) -> bool {
    match (old_state.initializer, new_state.initializer) {
        (Some(old), Some(new)) => match (previous.function_by_id(old), next.function_by_id(new)) {
            (Some(old), Some(new)) => !same_executable_body(old, new),
            (old, new) => old != new,
        },
        (old, new) => old != new,
    }
}

fn same_executable_body(
    old: &vela_bytecode::UnlinkedCodeObject,
    new: &vela_bytecode::UnlinkedCodeObject,
) -> bool {
    old.asyncness == new.asyncness
        && old.params == new.params
        && old.param_defaults == new.param_defaults
        && old.capture_count == new.capture_count
        && old.register_count == new.register_count
        && old.cache_sites == new.cache_sites
        && old.constants == new.constants
        && old.host_targets == new.host_targets
        && old.param_guards == new.param_guards
        && old.return_guard == new.return_guard
        && old.nested_functions.len() == new.nested_functions.len()
        && old
            .nested_functions
            .iter()
            .zip(&new.nested_functions)
            .all(|(old, new)| same_executable_body(old, new))
        && old.instructions.len() == new.instructions.len()
        && old
            .instructions
            .iter()
            .zip(&new.instructions)
            .all(|(old, new)| old.kind == new.kind && old.execution_units == new.execution_units)
}
