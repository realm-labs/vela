use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use super::{IntoStateValue, RuntimeImageStorage, RuntimeImpl};

impl<I> RuntimeImpl<I>
where
    I: RuntimeImageStorage,
{
    pub fn set_state(
        &mut self,
        name: impl Into<String>,
        value: impl IntoStateValue,
    ) -> VmResult<()> {
        value.set_state(self, name.into())
    }

    pub fn state(&mut self, name: &str) -> VmResult<Option<OwnedValue>> {
        let Some(state) = self.vm_state_id(name) else {
            return Ok(None);
        };
        self.state.vm_states.value(state)
    }

    pub fn update_state(
        &mut self,
        name: &str,
        update: impl FnOnce(&mut OwnedValue),
    ) -> VmResult<()> {
        let state = self.vm_state_id(name).ok_or_else(|| {
            VmError::new(VmErrorKind::MissingVmState {
                name: name.to_owned(),
            })
        })?;
        let mut value = self.state.vm_states.value(state)?.ok_or_else(|| {
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
        let Some(state) = self.vm_state_id(name) else {
            return Ok(None);
        };
        self.state.vm_states.value_as(state)
    }

    pub(super) fn set_owned_state(&mut self, name: String, value: OwnedValue) -> VmResult<()> {
        let state = self
            .image
            .state_by_name(&name)
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

    fn vm_state_id(&self, name: &str) -> Option<vela_def::StateId> {
        self.image.state_by_name(name).map(|state| state.id)
    }
}
