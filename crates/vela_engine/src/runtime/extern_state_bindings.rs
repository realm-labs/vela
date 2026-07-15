use std::collections::BTreeMap;

use vela_common::HostObjectId;
use vela_def::StateId;
use vela_host::adapter::ExternStateBinding;
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::object::ScriptHostObject;
use vela_host::path::HostRef;

const EXTERN_STATE_HOST_OBJECT_ID_BASE: u64 = 1 << 62;

pub struct RuntimeExternStateBindings {
    bindings: BTreeMap<StateId, ExternStateObject>,
    state_ids_by_name: BTreeMap<String, StateId>,
    expected_types_by_id: BTreeMap<StateId, Option<vela_common::HostTypeId>>,
    next_host_object_id: u64,
}

impl Default for RuntimeExternStateBindings {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeExternStateBindings {
    #[must_use]
    pub fn new() -> Self {
        Self {
            bindings: BTreeMap::new(),
            state_ids_by_name: BTreeMap::new(),
            expected_types_by_id: BTreeMap::new(),
            next_host_object_id: EXTERN_STATE_HOST_OBJECT_ID_BASE,
        }
    }

    #[must_use]
    pub fn with_state_layout(states: &[vela_bytecode::StateDescriptor]) -> Self {
        let mut store = Self::new();
        store.set_state_layout(states);
        store
    }

    pub fn set_state_layout(&mut self, states: &[vela_bytecode::StateDescriptor]) {
        self.state_ids_by_name = states
            .iter()
            .filter(|state| state.storage == vela_bytecode::StateStorage::Extern)
            .map(|state| (state.qualified_name.clone(), state.id))
            .collect();
        self.expected_types_by_id = states
            .iter()
            .filter(|state| state.storage == vela_bytecode::StateStorage::Extern)
            .map(|state| {
                let expected = match state.type_contract {
                    vela_mir::MirTypeContract::Host(target) => Some(target.runtime),
                    _ => None,
                };
                (state.id, expected)
            })
            .collect();
    }

    pub fn bind_host<T>(&mut self, name: impl Into<String>, value: T) -> HostResult<HostRef>
    where
        T: ScriptHostObject + Send + 'static,
    {
        let name = name.into();
        let state = self
            .state_ids_by_name
            .get(&name)
            .copied()
            .ok_or_else(|| HostError {
                kind: HostErrorKind::MissingExternState { name: name.clone() },
                source_span: None,
            })?;
        let actual_type = value.host_type_id();
        if let Some(expected) = self.expected_types_by_id.get(&state).copied().flatten()
            && expected != actual_type
        {
            return Err(HostError {
                kind: HostErrorKind::TypeMismatch {
                    expected,
                    actual: actual_type,
                },
                source_span: None,
            });
        }
        let host_ref = HostRef::new(actual_type, HostObjectId::new(self.next_host_object_id), 1);
        self.next_host_object_id = self.next_host_object_id.saturating_add(1);
        self.bindings.insert(
            state,
            ExternStateObject {
                host_ref,
                object: Box::new(value),
            },
        );
        Ok(host_ref)
    }

    pub(super) fn missing_bindings(
        &self,
        states: &[vela_bytecode::StateDescriptor],
    ) -> Vec<(String, Option<vela_common::Span>)> {
        states
            .iter()
            .filter(|state| state.storage == vela_bytecode::StateStorage::Extern)
            .filter(|state| !self.bindings.contains_key(&state.id))
            .map(|state| (state.qualified_name.clone(), state.source_span))
            .collect()
    }

    #[must_use]
    pub fn host_ref(&self, name: &str) -> Option<HostRef> {
        self.state_ids_by_name
            .get(name)
            .and_then(|state| self.bindings.get(state))
            .map(|binding| binding.host_ref)
    }

    pub(super) fn binding(&self, root: HostRef) -> Option<&ExternStateObject> {
        self.bindings
            .values()
            .find(|binding| binding.host_ref == root)
    }

    pub(super) fn binding_mut(&mut self, root: HostRef) -> Option<&mut ExternStateObject> {
        self.bindings
            .values_mut()
            .find(|binding| binding.host_ref == root)
    }

    pub(super) fn binding_by_type(
        &self,
        type_id: vela_common::HostTypeId,
    ) -> Option<&ExternStateObject> {
        self.bindings
            .values()
            .find(|binding| binding.host_ref.type_id == type_id)
    }

    pub(super) fn host_ref_for_binding(
        &self,
        state: ExternStateBinding<'_>,
    ) -> HostResult<HostRef> {
        self.bindings
            .get(&state.id)
            .map(|binding| binding.host_ref)
            .ok_or_else(|| HostError {
                kind: HostErrorKind::MissingExternState {
                    name: state.name.to_owned(),
                },
                source_span: None,
            })
    }
}

pub(super) struct ExternStateObject {
    pub(super) host_ref: HostRef,
    pub(super) object: Box<dyn ScriptHostObject + Send>,
}
