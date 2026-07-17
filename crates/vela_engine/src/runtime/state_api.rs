use vela_host::error::HostResult;
use vela_host::object::ScriptHostObject;
use vela_host::path::HostRef;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use super::{IntoStateValue, RuntimeImageStorage, RuntimeImpl};

impl<I> RuntimeImpl<I>
where
    I: RuntimeImageStorage,
{
    pub fn replace_extern_state<T>(
        &mut self,
        name: impl Into<String>,
        value: T,
    ) -> HostResult<HostRef>
    where
        T: ScriptHostObject + Send + 'static,
    {
        self.state.extern_states.bind_host(name, value)
    }

    /// Stages a host object for an `extern state` declaration in a pending
    /// hot-reload generation. The binding is validated and published only if
    /// that generation is accepted at a Runtime safe point.
    pub fn stage_extern_state<T>(
        &mut self,
        name: impl Into<String>,
        value: T,
    ) -> HostResult<HostRef>
    where
        T: ScriptHostObject + Send + 'static,
    {
        self.state.extern_states.stage_host(name, value)
    }

    #[must_use]
    pub fn extern_state_ref(&self, name: &str) -> Option<HostRef> {
        self.state.extern_states.host_ref(name)
    }

    pub fn set_state(
        &mut self,
        name: impl Into<String>,
        value: impl IntoStateValue,
    ) -> VmResult<()> {
        value.set_state(self, name.into())
    }

    pub fn state(&mut self, name: &str) -> VmResult<Option<OwnedValue>> {
        self.state.vm_states.value(name)
    }

    pub fn update_state(
        &mut self,
        name: &str,
        update: impl FnOnce(&mut OwnedValue),
    ) -> VmResult<()> {
        let mut value = self.state.vm_states.value(name)?.ok_or_else(|| {
            VmError::new(VmErrorKind::MissingVmState {
                name: name.to_owned(),
            })
        })?;
        update(&mut value);
        self.set_owned_state(name.to_owned(), value)
    }

    #[cfg(feature = "serde")]
    pub fn state_as<T>(&self, name: &str) -> VmResult<Option<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        self.state.vm_states.value_as(name)
    }

    pub(super) fn set_owned_state(&mut self, name: String, value: OwnedValue) -> VmResult<()> {
        let state = self
            .image
            .linked_program()
            .states()
            .iter()
            .find(|state| {
                state.qualified_name == name && state.storage == vela_bytecode::StateStorage::Vm
            })
            .ok_or_else(|| VmError::new(VmErrorKind::MissingVmState { name: name.clone() }))?;
        let value = vela_vm::canonicalize_owned_value_contract(
            value,
            &state.type_contract,
            self.image.linked_program(),
            &mut self.state.vm_states.heap,
            None,
            &name,
        )?;
        self.state.vm_states.insert_prepared(state.id, value);
        self.state.vm_states.collect();
        Ok(())
    }
}
