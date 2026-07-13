use std::collections::BTreeMap;

use vela_host::adapter::ScriptStateAdapter;
use vela_host::object::ScriptHostObject;
use vela_host::path::HostRef;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::heap::ScriptHeap;
use vela_vm::owned_to_persistent_value;
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;

use super::VelaValue;

#[derive(Default)]
pub struct CallArgs<'a> {
    entries: Vec<CallArg<'a>>,
    fallback: Option<&'a mut (dyn ScriptStateAdapter + Send)>,
}

impl<'a> CallArgs<'a> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from_positional(args: impl IntoIterator<Item = OwnedValue>) -> Self {
        Self {
            entries: args.into_iter().map(CallArg::Positional).collect(),
            fallback: None,
        }
    }

    #[must_use]
    pub fn from_values(args: impl IntoIterator<Item = VelaValue>) -> Self {
        Self {
            entries: args.into_iter().map(CallArg::PositionalValue).collect(),
            fallback: None,
        }
    }

    pub fn push(&mut self, value: impl Into<OwnedValue>) -> &mut Self {
        self.entries.push(CallArg::Positional(value.into()));
        self
    }

    #[cfg(feature = "serde")]
    pub fn push_serde<T>(&mut self, value: &T) -> VmResult<&mut Self>
    where
        T: serde::Serialize + ?Sized,
    {
        self.entries
            .push(CallArg::Positional(vela_vm::serde::to_owned_value(value)?));
        Ok(self)
    }

    pub fn push_vela_value(&mut self, value: VelaValue) -> &mut Self {
        self.entries.push(CallArg::PositionalValue(value));
        self
    }

    pub fn push_value(
        &mut self,
        name: impl Into<String>,
        value: impl Into<OwnedValue>,
    ) -> &mut Self {
        self.entries.push(CallArg::Named {
            name: name.into(),
            value: value.into(),
        });
        self
    }

    #[cfg(feature = "serde")]
    pub fn push_serde_value<T>(&mut self, name: impl Into<String>, value: &T) -> VmResult<&mut Self>
    where
        T: serde::Serialize + ?Sized,
    {
        self.entries.push(CallArg::Named {
            name: name.into(),
            value: vela_vm::serde::to_owned_value(value)?,
        });
        Ok(self)
    }

    pub fn push_named_vela_value(
        &mut self,
        name: impl Into<String>,
        value: VelaValue,
    ) -> &mut Self {
        self.entries.push(CallArg::NamedValue {
            name: name.into(),
            value,
        });
        self
    }

    pub fn push_host_handle(
        &mut self,
        name: impl Into<String>,
        host_ref: vela_host::path::HostRef,
    ) -> &mut Self {
        self.push_value(name, OwnedValue::HostRef(host_ref))
    }

    pub fn push_host_ref<T>(&mut self, name: impl Into<String>, value: &'a T) -> &mut Self
    where
        T: ScriptHostObject + Sync + 'a,
    {
        self.entries.push(CallArg::NamedHost {
            name: name.into(),
            host_ref: None,
            type_id: value.host_type_id(),
            binding: HostArgBinding::Shared(value),
        });
        self
    }

    pub fn push_host_mut<T>(&mut self, name: impl Into<String>, value: &'a mut T) -> &mut Self
    where
        T: ScriptHostObject + Send + 'a,
    {
        self.entries.push(CallArg::NamedHost {
            name: name.into(),
            host_ref: None,
            type_id: value.host_type_id(),
            binding: HostArgBinding::Mutable(value),
        });
        self
    }

    #[must_use]
    pub fn with(mut self, value: impl Into<OwnedValue>) -> Self {
        self.push(value);
        self
    }

    #[cfg(feature = "serde")]
    pub fn with_serde<T>(mut self, value: &T) -> VmResult<Self>
    where
        T: serde::Serialize + ?Sized,
    {
        self.push_serde(value)?;
        Ok(self)
    }

    #[must_use]
    pub fn with_vela_value(mut self, value: VelaValue) -> Self {
        self.push_vela_value(value);
        self
    }

    #[must_use]
    pub fn with_value(mut self, name: impl Into<String>, value: impl Into<OwnedValue>) -> Self {
        self.push_value(name, value);
        self
    }

    #[cfg(feature = "serde")]
    pub fn with_serde_value<T>(mut self, name: impl Into<String>, value: &T) -> VmResult<Self>
    where
        T: serde::Serialize + ?Sized,
    {
        self.push_serde_value(name, value)?;
        Ok(self)
    }

    #[must_use]
    pub fn with_named_vela_value(mut self, name: impl Into<String>, value: VelaValue) -> Self {
        self.push_named_vela_value(name, value);
        self
    }

    #[must_use]
    pub fn with_host_handle(
        mut self,
        name: impl Into<String>,
        host_ref: vela_host::path::HostRef,
    ) -> Self {
        self.push_host_handle(name, host_ref);
        self
    }

    #[must_use]
    pub fn with_host_ref<T>(mut self, name: impl Into<String>, value: &'a T) -> Self
    where
        T: ScriptHostObject + Sync + 'a,
    {
        self.push_host_ref(name, value);
        self
    }

    #[must_use]
    pub fn with_host_mut<T>(mut self, name: impl Into<String>, value: &'a mut T) -> Self
    where
        T: ScriptHostObject + Send + 'a,
    {
        self.push_host_mut(name, value);
        self
    }

    #[must_use]
    pub fn with_fallback_adapter(
        mut self,
        adapter: &'a mut (dyn ScriptStateAdapter + Send),
    ) -> Self {
        self.fallback = Some(adapter);
        self
    }

    #[cfg(test)]
    pub(super) fn set_fallback_adapter(
        &mut self,
        adapter: &'a mut (dyn ScriptStateAdapter + Send),
    ) {
        self.fallback = Some(adapter);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn resolve_values(
        &self,
        entry: &str,
        params: &[String],
        param_defaults: &[bool],
        runtime_id: u64,
        heap: &mut ScriptHeap,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Vec<Value>> {
        match self.mode()? {
            CallArgsMode::Empty | CallArgsMode::Positional => self
                .entries
                .iter()
                .map(|arg| arg.runtime_value(runtime_id, heap, budget))
                .collect(),
            CallArgsMode::Named => {
                self.resolve_named_values(entry, params, param_defaults, runtime_id, heap, budget)
            }
        }
    }

    fn mode(&self) -> VmResult<CallArgsMode> {
        let mut has_positional = false;
        let mut has_named = false;
        for entry in &self.entries {
            match entry {
                CallArg::Positional(_) | CallArg::PositionalValue(_) => has_positional = true,
                CallArg::Named { .. } | CallArg::NamedValue { .. } | CallArg::NamedHost { .. } => {
                    has_named = true
                }
            }
        }
        match (has_positional, has_named) {
            (false, false) => Ok(CallArgsMode::Empty),
            (true, false) => Ok(CallArgsMode::Positional),
            (false, true) => Ok(CallArgsMode::Named),
            (true, true) => Err(call_args_type_error(
                "mixed positional and named call arguments",
            )),
        }
    }

    fn resolve_named_values(
        &self,
        entry: &str,
        params: &[String],
        param_defaults: &[bool],
        runtime_id: u64,
        heap: &mut ScriptHeap,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Vec<Value>> {
        let mut values = BTreeMap::new();
        for (index, arg) in self.entries.iter().enumerate() {
            let Some(name) = arg.name() else {
                continue;
            };
            if !params.iter().any(|param| param == name) {
                return Err(call_args_type_error("unknown named call argument"));
            }
            if values.insert(name.to_owned(), index).is_some() {
                return Err(call_args_type_error("duplicate named call argument"));
            }
        }

        let mut resolved = Vec::with_capacity(params.len());
        for (index, param) in params.iter().enumerate() {
            if let Some(arg_index) = values.remove(param) {
                resolved.push(self.entries[arg_index].runtime_value(runtime_id, heap, budget)?);
            } else if param_defaults.get(index).copied().unwrap_or(false) {
                resolved.push(Value::Missing);
            } else {
                return Err(VmError::new(VmErrorKind::ArityMismatch {
                    name: entry.to_owned(),
                    expected: params.len(),
                    actual: self.entries.len(),
                }));
            }
        }
        Ok(resolved)
    }

    pub(super) fn assign_direct_host_refs(&mut self, next_object_id: &mut u64) {
        for entry in &mut self.entries {
            if let CallArg::NamedHost {
                host_ref, type_id, ..
            } = entry
                && host_ref.is_none()
            {
                *host_ref = Some(HostRef::new(
                    *type_id,
                    vela_common::HostObjectId::new(*next_object_id),
                    1,
                ));
                *next_object_id = next_object_id.saturating_add(1);
            }
        }
    }

    pub(super) fn take_fallback(&mut self) -> Option<&'a mut (dyn ScriptStateAdapter + Send)> {
        self.fallback.take()
    }

    pub(super) fn direct_binding(&self, root: HostRef) -> Option<&HostArgBinding<'a>> {
        self.entries.iter().find_map(|entry| match entry {
            CallArg::NamedHost {
                host_ref: Some(host_ref),
                binding,
                ..
            } if *host_ref == root => Some(binding),
            _ => None,
        })
    }

    pub(super) fn direct_binding_mut(&mut self, root: HostRef) -> Option<&mut HostArgBinding<'a>> {
        self.entries.iter_mut().find_map(|entry| match entry {
            CallArg::NamedHost {
                host_ref: Some(host_ref),
                binding,
                ..
            } if *host_ref == root => Some(binding),
            _ => None,
        })
    }

    pub(super) fn direct_binding_by_type(
        &self,
        type_id: vela_common::HostTypeId,
    ) -> Option<&HostArgBinding<'a>> {
        self.entries.iter().find_map(|entry| match entry {
            CallArg::NamedHost {
                type_id: binding_type,
                binding,
                ..
            } if *binding_type == type_id => Some(binding),
            _ => None,
        })
    }
}

impl From<Vec<OwnedValue>> for CallArgs<'_> {
    fn from(value: Vec<OwnedValue>) -> Self {
        Self::from_positional(value)
    }
}

pub(super) enum CallArg<'a> {
    Positional(OwnedValue),
    PositionalValue(VelaValue),
    Named {
        name: String,
        value: OwnedValue,
    },
    NamedValue {
        name: String,
        value: VelaValue,
    },
    NamedHost {
        name: String,
        host_ref: Option<HostRef>,
        type_id: vela_common::HostTypeId,
        binding: HostArgBinding<'a>,
    },
}

impl CallArg<'_> {
    fn runtime_value(
        &self,
        runtime_id: u64,
        heap: &mut ScriptHeap,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        match self {
            Self::Positional(value) | Self::Named { value, .. } => {
                owned_to_persistent_value(value.clone(), heap, Some(budget))
            }
            Self::PositionalValue(value) | Self::NamedValue { value, .. } => {
                if value.runtime_id() == runtime_id {
                    Ok(value.value())
                } else {
                    Err(call_args_type_error("VelaValue belongs to another Runtime"))
                }
            }
            Self::NamedHost {
                host_ref: Some(host_ref),
                ..
            } => Ok(Value::HostRef(*host_ref)),
            Self::NamedHost { host_ref: None, .. } => Err(call_args_type_error(
                "direct host argument was not assigned an execution identity",
            )),
        }
    }

    fn name(&self) -> Option<&str> {
        match self {
            Self::Positional(_) | Self::PositionalValue(_) => None,
            Self::Named { name, .. }
            | Self::NamedValue { name, .. }
            | Self::NamedHost { name, .. } => Some(name),
        }
    }
}

pub(super) enum HostArgBinding<'a> {
    Shared(&'a (dyn ScriptHostObject + Sync)),
    Mutable(&'a mut (dyn ScriptHostObject + Send)),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallArgsMode {
    Empty,
    Positional,
    Named,
}

pub(crate) fn call_args_type_error(operation: &'static str) -> VmError {
    VmError::new(VmErrorKind::TypeMismatch { operation })
}
