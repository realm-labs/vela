use std::collections::BTreeMap;

use vela_common::HostObjectId;
use vela_host::adapter::ScriptStateAdapter;
use vela_host::error::{HostError, HostErrorKind, HostResult};
use vela_host::object::ScriptHostObject;
use vela_host::path::{HostPath, HostRef};
use vela_host::value::HostValue;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::heap::ScriptHeap;
use vela_vm::owned_to_persistent_value;
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;

use super::VelaValue;

const DIRECT_HOST_OBJECT_ID_BASE: u64 = 1 << 63;

pub struct CallArgs<'a> {
    entries: Vec<CallArg<'a>>,
    next_direct_object_id: u64,
}

impl Default for CallArgs<'_> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            next_direct_object_id: DIRECT_HOST_OBJECT_ID_BASE,
        }
    }
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
            next_direct_object_id: DIRECT_HOST_OBJECT_ID_BASE,
        }
    }

    #[must_use]
    pub fn from_values(args: impl IntoIterator<Item = VelaValue>) -> Self {
        Self {
            entries: args.into_iter().map(CallArg::PositionalValue).collect(),
            next_direct_object_id: DIRECT_HOST_OBJECT_ID_BASE,
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
        T: ScriptHostObject + 'a,
    {
        let host_ref = self.next_direct_host_ref(value.host_type_id());
        self.entries.push(CallArg::NamedHost {
            name: name.into(),
            host_ref,
            binding: HostArgBinding::Shared(value),
        });
        self
    }

    pub fn push_host_mut<T>(&mut self, name: impl Into<String>, value: &'a mut T) -> &mut Self
    where
        T: ScriptHostObject + 'a,
    {
        let host_ref = self.next_direct_host_ref(value.host_type_id());
        self.entries.push(CallArg::NamedHost {
            name: name.into(),
            host_ref,
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
        T: ScriptHostObject + 'a,
    {
        self.push_host_ref(name, value);
        self
    }

    #[must_use]
    pub fn with_host_mut<T>(mut self, name: impl Into<String>, value: &'a mut T) -> Self
    where
        T: ScriptHostObject + 'a,
    {
        self.push_host_mut(name, value);
        self
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn resolve(
        &self,
        entry: &str,
        params: &[String],
        param_defaults: &[bool],
    ) -> VmResult<Vec<OwnedValue>> {
        match self.mode()? {
            CallArgsMode::Empty | CallArgsMode::Positional => {
                self.entries.iter().map(CallArg::owned_value).collect()
            }
            CallArgsMode::Named => self.resolve_named(entry, params, param_defaults),
        }
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

    fn resolve_named(
        &self,
        entry: &str,
        params: &[String],
        param_defaults: &[bool],
    ) -> VmResult<Vec<OwnedValue>> {
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
                resolved.push(self.entries[arg_index].owned_value()?);
            } else if param_defaults.get(index).copied().unwrap_or(false) {
                resolved.push(OwnedValue::Missing);
            } else {
                return Err(VmError {
                    kind: VmErrorKind::ArityMismatch {
                        name: entry.to_owned(),
                        expected: params.len(),
                        actual: self.entries.len(),
                    },
                    source_span: None,
                    call_stack: Default::default(),
                });
            }
        }
        Ok(resolved)
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
                return Err(VmError {
                    kind: VmErrorKind::ArityMismatch {
                        name: entry.to_owned(),
                        expected: params.len(),
                        actual: self.entries.len(),
                    },
                    source_span: None,
                    call_stack: Default::default(),
                });
            }
        }
        Ok(resolved)
    }

    fn next_direct_host_ref(&mut self, type_id: vela_common::HostTypeId) -> HostRef {
        let object_id = HostObjectId::new(self.next_direct_object_id);
        self.next_direct_object_id = self.next_direct_object_id.saturating_add(1);
        HostRef::new(type_id, object_id, 1)
    }
}

impl From<Vec<OwnedValue>> for CallArgs<'_> {
    fn from(value: Vec<OwnedValue>) -> Self {
        Self::from_positional(value)
    }
}

enum CallArg<'a> {
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
        host_ref: HostRef,
        binding: HostArgBinding<'a>,
    },
}

impl CallArg<'_> {
    fn owned_value(&self) -> VmResult<OwnedValue> {
        match self {
            Self::Positional(value) | Self::Named { value, .. } => Ok(value.clone()),
            Self::NamedHost { host_ref, .. } => Ok(OwnedValue::HostRef(*host_ref)),
            Self::PositionalValue(_) | Self::NamedValue { .. } => Err(call_args_type_error(
                "runtime VelaValue arguments require Runtime::call",
            )),
        }
    }

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
            Self::NamedHost { host_ref, .. } => Ok(Value::HostRef(*host_ref)),
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

enum HostArgBinding<'a> {
    Shared(&'a dyn ScriptHostObject),
    Mutable(&'a mut dyn ScriptHostObject),
}

pub(crate) struct CallArgsAdapter<'call, 'args> {
    args: &'call mut CallArgs<'args>,
    fallback: &'call mut dyn ScriptStateAdapter,
}

impl<'call, 'args> CallArgsAdapter<'call, 'args> {
    pub(crate) fn new(
        args: &'call mut CallArgs<'args>,
        fallback: &'call mut dyn ScriptStateAdapter,
    ) -> Self {
        Self { args, fallback }
    }

    fn direct_binding<'s>(&'s self, path: &HostPath) -> Option<&'s HostArgBinding<'args>> {
        for entry in &self.args.entries {
            if let CallArg::NamedHost {
                host_ref, binding, ..
            } = entry
                && *host_ref == path.root
            {
                return Some(binding);
            }
        }
        None
    }

    fn direct_binding_mut<'s>(
        &'s mut self,
        path: &HostPath,
    ) -> Option<&'s mut HostArgBinding<'args>> {
        for entry in &mut self.args.entries {
            if let CallArg::NamedHost {
                host_ref, binding, ..
            } = entry
                && *host_ref == path.root
            {
                return Some(binding);
            }
        }
        None
    }

    fn direct_access_error(path: &HostPath, action: &'static str) -> HostError {
        HostError {
            kind: HostErrorKind::PermissionDenied {
                path: path.clone(),
                action,
            },
            source_span: None,
        }
    }
}

impl ScriptStateAdapter for CallArgsAdapter<'_, '_> {
    fn read_path(&self, path: &HostPath) -> HostResult<HostValue> {
        match self.direct_binding(path) {
            Some(HostArgBinding::Shared(object)) => object.read_host_path(path),
            Some(HostArgBinding::Mutable(object)) => object.read_host_path(path),
            None => self.fallback.read_path(path),
        }
    }

    fn write_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()> {
        match self.direct_binding_mut(path) {
            Some(HostArgBinding::Shared(_)) => Err(Self::direct_access_error(path, "write")),
            Some(HostArgBinding::Mutable(object)) => object.write_host_path(path, value),
            None => self.fallback.write_path(path, value),
        }
    }

    fn remove_path(&mut self, path: &HostPath) -> HostResult<()> {
        match self.direct_binding_mut(path) {
            Some(HostArgBinding::Shared(_)) => Err(Self::direct_access_error(path, "write")),
            Some(HostArgBinding::Mutable(object)) => object.remove_host_path(path),
            None => self.fallback.remove_path(path),
        }
    }

    fn call_method(
        &mut self,
        path: &HostPath,
        method: vela_common::HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue> {
        match self.direct_binding_mut(path) {
            Some(HostArgBinding::Shared(_)) => Err(Self::direct_access_error(path, "call")),
            Some(HostArgBinding::Mutable(object)) => object.call_host_method(path, method, args),
            None => self.fallback.call_method(path, method, args),
        }
    }
}

pub(crate) struct EmptyStateAdapter;

impl ScriptStateAdapter for EmptyStateAdapter {
    fn read_path(&self, path: &HostPath) -> HostResult<HostValue> {
        Err(HostError {
            kind: HostErrorKind::MissingPath { path: path.clone() },
            source_span: None,
        })
    }

    fn write_path(&mut self, path: &HostPath, _value: HostValue) -> HostResult<()> {
        Err(HostError {
            kind: HostErrorKind::MissingPath { path: path.clone() },
            source_span: None,
        })
    }

    fn remove_path(&mut self, path: &HostPath) -> HostResult<()> {
        Err(HostError {
            kind: HostErrorKind::MissingPath { path: path.clone() },
            source_span: None,
        })
    }

    fn call_method(
        &mut self,
        _path: &HostPath,
        method: vela_common::HostMethodId,
        _args: &[HostValue],
    ) -> HostResult<HostValue> {
        Err(HostError {
            kind: HostErrorKind::UnsupportedMethod { method },
            source_span: None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallArgsMode {
    Empty,
    Positional,
    Named,
}

pub(crate) fn call_args_type_error(operation: &'static str) -> VmError {
    VmError {
        kind: VmErrorKind::TypeMismatch { operation },
        source_span: None,
        call_stack: Default::default(),
    }
}
