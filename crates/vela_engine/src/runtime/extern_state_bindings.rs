use std::collections::{BTreeMap, BTreeSet};

use vela_common::{HostObjectId, HostTypeId};
use vela_def::StateId;
use vela_host::adapter::ExternStateBinding;
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::object::ScriptHostObject;
use vela_host::path::{HostRef, HostSlotRef};
use vela_host::slot::HostSlotTable;

use super::image::RuntimeImage;

const EXTERN_STATE_HOST_OBJECT_ID_BASE: u64 = 1 << 62;

pub(super) fn extern_state_schema(
    image: &RuntimeImage,
    name: &str,
) -> HostResult<(StateId, vela_common::HostTypeId)> {
    let state = image
        .state_by_name(name)
        .filter(|state| state.storage == vela_bytecode::StateStorage::Extern)
        .ok_or_else(|| HostError {
            kind: HostErrorKind::MissingExternState {
                name: name.to_owned(),
            },
            source_span: None,
        })?;
    let vela_mir::MirTypeContract::Host(expected) = state.type_contract else {
        unreachable!("verified extern state descriptor must carry a host contract");
    };
    Ok((state.id, expected.runtime))
}

pub struct RuntimeExternStateBindings {
    bindings: BTreeMap<StateId, HostSlotRef>,
    pending: BTreeMap<String, HostSlotRef>,
    objects: HostSlotTable<ExternStateObject>,
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
            pending: BTreeMap::new(),
            objects: HostSlotTable::new(),
        }
    }

    pub fn bind_host<T>(
        &mut self,
        state: StateId,
        expected: vela_common::HostTypeId,
        value: T,
    ) -> HostResult<HostRef>
    where
        T: ScriptHostObject + Send + 'static,
    {
        let actual_type = value.host_type_id();
        if expected != actual_type {
            return Err(HostError {
                kind: HostErrorKind::TypeMismatch {
                    expected,
                    actual: actual_type,
                },
                source_span: None,
            });
        }
        if let Some(previous) = self.bindings.remove(&state) {
            let _ = self.objects.remove(previous);
        }
        let handle = self.objects.insert(ExternStateObject {
            type_id: actual_type,
            state: Some(state),
            object: Box::new(value),
        });
        self.bindings.insert(state, handle);
        Ok(Self::root_for(handle, actual_type))
    }

    pub fn stage_host<T>(&mut self, name: impl Into<String>, value: T) -> HostResult<HostRef>
    where
        T: ScriptHostObject + Send + 'static,
    {
        let name = name.into();
        let actual_type = value.host_type_id();
        if let Some(previous) = self.pending.remove(&name) {
            let _ = self.objects.remove(previous);
        }
        let handle = self.objects.insert(ExternStateObject {
            type_id: actual_type,
            state: None,
            object: Box::new(value),
        });
        self.pending.insert(name, handle);
        Ok(Self::root_for(handle, actual_type))
    }

    pub(super) fn validate_layout(
        &self,
        states: &[vela_bytecode::StateDescriptor],
    ) -> Result<(), (String, HostError)> {
        for state in states
            .iter()
            .filter(|state| state.storage == vela_bytecode::StateStorage::Extern)
        {
            let handle = self
                .bindings
                .get(&state.id)
                .or_else(|| self.pending.get(&state.qualified_name))
                .ok_or_else(|| {
                    (
                        state.qualified_name.clone(),
                        HostError {
                            kind: HostErrorKind::MissingExternState {
                                name: state.qualified_name.clone(),
                            },
                            source_span: state.source_span,
                        },
                    )
                })?;
            let binding = self
                .objects
                .get(*handle)
                .expect("extern state indexes reference live dense slots");
            let vela_mir::MirTypeContract::Host(expected) = state.type_contract else {
                unreachable!("verified extern state descriptor must carry a host contract");
            };
            if expected.runtime != binding.type_id {
                return Err((
                    state.qualified_name.clone(),
                    HostError {
                        kind: HostErrorKind::TypeMismatch {
                            expected: expected.runtime,
                            actual: binding.type_id,
                        },
                        source_span: state.source_span,
                    },
                ));
            }
        }
        Ok(())
    }

    pub(super) fn commit_layout(&mut self, states: &[vela_bytecode::StateDescriptor]) {
        for state in states
            .iter()
            .filter(|state| state.storage == vela_bytecode::StateStorage::Extern)
        {
            if !self.bindings.contains_key(&state.id)
                && let Some(handle) = self.pending.remove(&state.qualified_name)
            {
                let binding = self
                    .objects
                    .get_mut(handle)
                    .expect("pending extern state index references a live dense slot");
                binding.state = Some(state.id);
                self.bindings.insert(state.id, handle);
            }
        }
    }

    pub(super) fn retain_state_ids(&mut self, retained: &BTreeSet<StateId>) {
        let mut removed = Vec::new();
        self.bindings.retain(|state, handle| {
            let keep = retained.contains(state);
            if !keep {
                removed.push(*handle);
            }
            keep
        });
        for handle in removed {
            let _ = self.objects.remove(handle);
        }
    }

    #[cfg(test)]
    pub(super) fn contains_state_id(&self, state: StateId) -> bool {
        self.bindings.contains_key(&state)
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
    pub fn host_ref(&self, state: StateId) -> Option<HostRef> {
        let handle = *self.bindings.get(&state)?;
        let binding = self.objects.get(handle)?;
        Some(Self::root_for(handle, binding.type_id))
    }

    pub(super) fn binding(&self, root: HostRef) -> Option<&ExternStateObject> {
        let binding = self.entry(root)?;
        binding.state.is_some().then_some(binding)
    }

    pub(super) fn binding_mut(&mut self, root: HostRef) -> Option<&mut ExternStateObject> {
        let handle = self.handle(root)?;
        let binding = self.objects.get_mut(handle)?;
        binding.state.is_some().then_some(binding)
    }

    pub(super) fn binding_by_type(&self, type_id: HostTypeId) -> Option<&ExternStateObject> {
        self.objects
            .iter()
            .map(|(_, binding)| binding)
            .find(|binding| binding.state.is_some() && binding.type_id == type_id)
    }

    pub(super) fn host_ref_for_binding(
        &self,
        state: ExternStateBinding<'_>,
    ) -> HostResult<HostRef> {
        self.bindings
            .get(&state.id)
            .and_then(|handle| {
                self.objects
                    .get(*handle)
                    .map(|binding| Self::root_for(*handle, binding.type_id))
            })
            .ok_or_else(|| HostError {
                kind: HostErrorKind::MissingExternState {
                    name: state.name.to_owned(),
                },
                source_span: None,
            })
    }

    fn entry(&self, root: HostRef) -> Option<&ExternStateObject> {
        self.objects.get(self.handle(root)?)
    }

    fn handle(&self, root: HostRef) -> Option<HostSlotRef> {
        let slot = root
            .object_id
            .get()
            .checked_sub(EXTERN_STATE_HOST_OBJECT_ID_BASE)
            .and_then(|slot| u32::try_from(slot).ok())?;
        let handle = HostSlotRef::new(slot, root.generation);
        let binding = self.objects.get(handle)?;
        (binding.type_id == root.type_id).then_some(handle)
    }

    fn root_for(handle: HostSlotRef, type_id: HostTypeId) -> HostRef {
        HostRef::new(
            type_id,
            HostObjectId::new(EXTERN_STATE_HOST_OBJECT_ID_BASE + u64::from(handle.slot())),
            handle.generation(),
        )
    }
}

pub(super) struct ExternStateObject {
    type_id: HostTypeId,
    state: Option<StateId>,
    pub(super) object: Box<dyn ScriptHostObject + Send>,
}

#[cfg(test)]
mod tests {
    use vela_host::target::HostTargetInstance;
    use vela_host::value::HostValue;

    use super::*;

    struct DenseExternObject(HostTypeId);

    impl ScriptHostObject for DenseExternObject {
        fn host_type_id(&self) -> HostTypeId {
            self.0
        }

        fn read_resolved_host(
            &self,
            _access: vela_host::resolved::ResolvedHostAccess,
            _target: HostTargetInstance<'_>,
        ) -> HostResult<HostValue> {
            Ok(HostValue::Unit)
        }
    }

    #[test]
    fn extern_objects_use_dense_generation_checked_identity() {
        let state = StateId::new(1);
        let type_id = HostTypeId::new(95);
        let mut bindings = RuntimeExternStateBindings::new();
        let first = bindings
            .bind_host(state, type_id, DenseExternObject(type_id))
            .expect("matching extern host should bind");

        assert_eq!(first.object_id.get(), EXTERN_STATE_HOST_OBJECT_ID_BASE);
        assert_eq!(first.generation, 1);
        assert!(!bindings.objects.spilled());
        assert!(bindings.binding(first).is_some());
        assert!(
            bindings
                .binding(HostRef::new(
                    HostTypeId::new(96),
                    first.object_id,
                    first.generation,
                ))
                .is_none()
        );
        assert!(
            bindings
                .binding(HostRef::new(
                    first.type_id,
                    first.object_id,
                    first.generation + 1,
                ))
                .is_none()
        );

        let replacement = bindings
            .bind_host(state, type_id, DenseExternObject(type_id))
            .expect("replacement extern host should bind");
        assert_eq!(replacement.object_id, first.object_id);
        assert_ne!(replacement.generation, first.generation);
        assert!(bindings.binding(first).is_none());
        assert!(bindings.binding(replacement).is_some());

        bindings.retain_state_ids(&BTreeSet::new());
        assert!(bindings.binding(replacement).is_none());
    }

    #[test]
    fn staged_extern_roots_remain_inactive_until_commit() {
        let type_id = HostTypeId::new(97);
        let mut bindings = RuntimeExternStateBindings::new();
        let staged = bindings
            .stage_host("main::value", DenseExternObject(type_id))
            .expect("extern host should stage");

        assert!(bindings.entry(staged).is_some());
        assert!(bindings.binding(staged).is_none());
        assert!(bindings.binding_by_type(type_id).is_none());
    }
}
