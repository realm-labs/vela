use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use vela_bytecode::LinkedProgram;
use vela_host::adapter::ScriptStateAdapter;
use vela_host::error::HostResult;
use vela_host::lease::{
    ErasedHostLease, ExclusiveScopedHost, HostLeaseKind, MutableHostLeaseSlot, ScopedHostLeaseSlot,
    SharedScopedHost, host_lease_unsupported, host_object_busy,
};
use vela_host::object::{ScriptHostFieldAccess, ScriptHostObject};
use vela_host::path::HostRef;
use vela_vm::budget::ExecutionBudget;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::heap::ScriptHeap;
use vela_vm::owned_to_linked_persistent_value;
use vela_vm::owned_value::OwnedValue;
use vela_vm::value::Value;

use super::VelaValue;

#[derive(Default)]
pub struct CallArgs<'a> {
    entries: Vec<CallArg<'a>>,
    fallback: Option<&'a mut (dyn ScriptStateAdapter + Send)>,
}

pub(super) struct CallArgRuntime<'program, 'heap, 'budget> {
    runtime_id: u64,
    program: &'program LinkedProgram,
    heap: &'heap mut ScriptHeap,
    budget: &'budget mut ExecutionBudget,
}

impl<'program, 'heap, 'budget> CallArgRuntime<'program, 'heap, 'budget> {
    pub(super) fn new(
        runtime_id: u64,
        program: &'program LinkedProgram,
        heap: &'heap mut ScriptHeap,
        budget: &'budget mut ExecutionBudget,
    ) -> Self {
        Self {
            runtime_id,
            program,
            heap,
            budget,
        }
    }
}

/// Captures the call-scoped HostRef assigned to one direct host argument.
///
/// Generated adapters use this token to prove that a returned HostRef is the
/// exact direct Rust origin before returning that already-live Rust borrow.
#[doc(hidden)]
#[derive(Clone, Default)]
pub struct DirectHostIdentity(Arc<parking_lot::Mutex<Option<HostRef>>>);

impl DirectHostIdentity {
    #[must_use]
    pub fn host_ref(&self) -> Option<HostRef> {
        *self.0.lock()
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
            identity: None,
            type_id: value.host_type_id(),
            binding: HostArgBinding::Shared {
                object: value,
                leases: Arc::new(AtomicUsize::new(0)),
            },
        });
        self
    }

    /// Adds a shared HostRef-backed standard collection view using the exact
    /// concrete binding identity generated for `T`.
    #[doc(hidden)]
    pub fn push_collection_ref<T>(&mut self, name: impl Into<String>, value: &'a T) -> &mut Self
    where
        T: ScriptHostObject + crate::standard::StandardTypeBinding + Sync + 'a,
    {
        self.entries.push(CallArg::NamedHost {
            name: name.into(),
            host_ref: None,
            identity: None,
            type_id: crate::standard::standard_collection_host_type_id::<T>(),
            binding: HostArgBinding::Shared {
                object: value,
                leases: Arc::new(AtomicUsize::new(0)),
            },
        });
        self
    }

    /// Adds a shared, fixed-length Rust slice without copying its elements.
    #[doc(hidden)]
    pub fn push_slice_ref<T>(&mut self, name: impl Into<String>, value: &'a [T]) -> &mut Self
    where
        T: ScriptHostFieldAccess + crate::type_registration::RustValueType + Send + Sync + 'static,
    {
        let type_id = crate::standard::standard_slice_host_type_id::<T>();
        self.entries.push(CallArg::NamedHost {
            name: name.into(),
            host_ref: None,
            identity: None,
            type_id,
            binding: HostArgBinding::Scoped {
                object: Arc::new(parking_lot::RwLock::new(Box::new(
                    SharedScopedHost::with_type_id(value, type_id),
                ))),
                mutable: false,
            },
        });
        self
    }

    #[doc(hidden)]
    pub fn push_positional_host_ref<T>(&mut self, value: &'a T) -> &mut Self
    where
        T: ScriptHostObject + Sync + 'a,
    {
        self.entries.push(CallArg::PositionalHost {
            host_ref: None,
            identity: None,
            type_id: value.host_type_id(),
            binding: HostArgBinding::Shared {
                object: value,
                leases: Arc::new(AtomicUsize::new(0)),
            },
        });
        self
    }

    #[doc(hidden)]
    pub fn push_positional_collection_ref<T>(&mut self, value: &'a T) -> &mut Self
    where
        T: ScriptHostObject + crate::standard::StandardTypeBinding + Sync + 'a,
    {
        self.entries.push(CallArg::PositionalHost {
            host_ref: None,
            identity: None,
            type_id: crate::standard::standard_collection_host_type_id::<T>(),
            binding: HostArgBinding::Shared {
                object: value,
                leases: Arc::new(AtomicUsize::new(0)),
            },
        });
        self
    }

    #[doc(hidden)]
    pub fn push_positional_slice_ref<T>(&mut self, value: &'a [T]) -> &mut Self
    where
        T: ScriptHostFieldAccess + crate::type_registration::RustValueType + Send + Sync + 'static,
    {
        let type_id = crate::standard::standard_slice_host_type_id::<T>();
        self.entries.push(CallArg::PositionalHost {
            host_ref: None,
            identity: None,
            type_id,
            binding: HostArgBinding::Scoped {
                object: Arc::new(parking_lot::RwLock::new(Box::new(
                    SharedScopedHost::with_type_id(value, type_id),
                ))),
                mutable: false,
            },
        });
        self
    }

    #[doc(hidden)]
    pub fn push_tracked_positional_host_ref<T>(&mut self, value: &'a T) -> DirectHostIdentity
    where
        T: ScriptHostObject + Sync + 'a,
    {
        let identity = DirectHostIdentity::default();
        self.entries.push(CallArg::PositionalHost {
            host_ref: None,
            identity: Some(identity.clone()),
            type_id: value.host_type_id(),
            binding: HostArgBinding::Shared {
                object: value,
                leases: Arc::new(AtomicUsize::new(0)),
            },
        });
        identity
    }

    /// Adds writable call-scoped host state.
    ///
    /// Mutable-origin bindings require `Sync` as well as `Send` so async
    /// `&self` methods can acquire genuine coexisting shared leases. Types
    /// without that capability fail at the embedding boundary.
    pub fn push_host_mut<T>(&mut self, name: impl Into<String>, value: &'a mut T) -> &mut Self
    where
        T: ScriptHostObject + Send + Sync + 'a,
    {
        self.entries.push(CallArg::NamedHost {
            name: name.into(),
            host_ref: None,
            identity: None,
            type_id: value.host_type_id(),
            binding: HostArgBinding::Mutable {
                object: Arc::new(parking_lot::RwLock::new(value)),
            },
        });
        self
    }

    /// Adds an exclusive HostRef-backed standard collection view using the
    /// exact concrete binding identity generated for `T`.
    #[doc(hidden)]
    pub fn push_collection_mut<T>(&mut self, name: impl Into<String>, value: &'a mut T) -> &mut Self
    where
        T: ScriptHostObject + crate::standard::StandardTypeBinding + Send + Sync + 'a,
    {
        self.entries.push(CallArg::NamedHost {
            name: name.into(),
            host_ref: None,
            identity: None,
            type_id: crate::standard::standard_collection_host_type_id::<T>(),
            binding: HostArgBinding::Mutable {
                object: Arc::new(parking_lot::RwLock::new(value)),
            },
        });
        self
    }

    /// Adds an exclusive, fixed-length Rust slice without copying its elements.
    #[doc(hidden)]
    pub fn push_slice_mut<T>(&mut self, name: impl Into<String>, value: &'a mut [T]) -> &mut Self
    where
        T: ScriptHostFieldAccess + crate::type_registration::RustValueType + Send + Sync + 'static,
    {
        let type_id = crate::standard::standard_slice_host_type_id::<T>();
        self.entries.push(CallArg::NamedHost {
            name: name.into(),
            host_ref: None,
            identity: None,
            type_id,
            binding: HostArgBinding::Scoped {
                object: Arc::new(parking_lot::RwLock::new(Box::new(
                    ExclusiveScopedHost::with_type_id(value, type_id),
                ))),
                mutable: true,
            },
        });
        self
    }

    #[doc(hidden)]
    pub fn push_positional_host_mut<T>(&mut self, value: &'a mut T) -> &mut Self
    where
        T: ScriptHostObject + Send + Sync + 'a,
    {
        self.entries.push(CallArg::PositionalHost {
            host_ref: None,
            identity: None,
            type_id: value.host_type_id(),
            binding: HostArgBinding::Mutable {
                object: Arc::new(parking_lot::RwLock::new(value)),
            },
        });
        self
    }

    #[doc(hidden)]
    pub fn push_positional_collection_mut<T>(&mut self, value: &'a mut T) -> &mut Self
    where
        T: ScriptHostObject + crate::standard::StandardTypeBinding + Send + Sync + 'a,
    {
        self.entries.push(CallArg::PositionalHost {
            host_ref: None,
            identity: None,
            type_id: crate::standard::standard_collection_host_type_id::<T>(),
            binding: HostArgBinding::Mutable {
                object: Arc::new(parking_lot::RwLock::new(value)),
            },
        });
        self
    }

    #[doc(hidden)]
    pub fn push_positional_slice_mut<T>(&mut self, value: &'a mut [T]) -> &mut Self
    where
        T: ScriptHostFieldAccess + crate::type_registration::RustValueType + Send + Sync + 'static,
    {
        let type_id = crate::standard::standard_slice_host_type_id::<T>();
        self.entries.push(CallArg::PositionalHost {
            host_ref: None,
            identity: None,
            type_id,
            binding: HostArgBinding::Scoped {
                object: Arc::new(parking_lot::RwLock::new(Box::new(
                    ExclusiveScopedHost::with_type_id(value, type_id),
                ))),
                mutable: true,
            },
        });
        self
    }

    #[doc(hidden)]
    pub fn push_tracked_positional_host_mut<T>(&mut self, value: &'a mut T) -> DirectHostIdentity
    where
        T: ScriptHostObject + Send + Sync + 'a,
    {
        let identity = DirectHostIdentity::default();
        self.entries.push(CallArg::PositionalHost {
            host_ref: None,
            identity: Some(identity.clone()),
            type_id: value.host_type_id(),
            binding: HostArgBinding::Mutable {
                object: Arc::new(parking_lot::RwLock::new(value)),
            },
        });
        identity
    }

    pub(crate) fn push_reborrowed_host_ref<T>(
        &mut self,
        name: impl Into<String>,
        host_ref: HostRef,
        value: &'a T,
    ) -> &mut Self
    where
        T: ScriptHostObject + Sync + 'a,
    {
        self.entries.push(CallArg::NamedHost {
            name: name.into(),
            host_ref: Some(host_ref),
            identity: None,
            type_id: value.host_type_id(),
            binding: HostArgBinding::Shared {
                object: value,
                leases: Arc::new(AtomicUsize::new(0)),
            },
        });
        self
    }

    pub(crate) fn push_reborrowed_host_mut<T>(
        &mut self,
        name: impl Into<String>,
        host_ref: HostRef,
        value: &'a mut T,
    ) -> &mut Self
    where
        T: ScriptHostObject + Send + Sync + 'a,
    {
        self.entries.push(CallArg::NamedHost {
            name: name.into(),
            host_ref: Some(host_ref),
            identity: None,
            type_id: value.host_type_id(),
            binding: HostArgBinding::Mutable {
                object: Arc::new(parking_lot::RwLock::new(value)),
            },
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

    /// Adds writable call-scoped host state with shared/exclusive async lease
    /// support. The bound deliberately rejects non-`Sync` mutable origins.
    #[must_use]
    pub fn with_host_mut<T>(mut self, name: impl Into<String>, value: &'a mut T) -> Self
    where
        T: ScriptHostObject + Send + Sync + 'a,
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

    pub(super) fn resolve_values(
        &self,
        entry: &str,
        params: &[String],
        param_defaults: &[bool],
        runtime: &mut CallArgRuntime<'_, '_, '_>,
    ) -> VmResult<Vec<Value>> {
        match self.mode()? {
            CallArgsMode::Empty | CallArgsMode::Positional => self
                .entries
                .iter()
                .map(|arg| arg.runtime_value(runtime))
                .collect(),
            CallArgsMode::Named => {
                self.resolve_named_values(entry, params, param_defaults, runtime)
            }
        }
    }

    fn mode(&self) -> VmResult<CallArgsMode> {
        let mut has_positional = false;
        let mut has_named = false;
        for entry in &self.entries {
            match entry {
                CallArg::Positional(_)
                | CallArg::PositionalValue(_)
                | CallArg::PositionalHost { .. } => has_positional = true,
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
        runtime: &mut CallArgRuntime<'_, '_, '_>,
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
                resolved.push(self.entries[arg_index].runtime_value(runtime)?);
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
                host_ref,
                identity,
                type_id,
                ..
            }
            | CallArg::PositionalHost {
                host_ref,
                identity,
                type_id,
                ..
            } = entry
                && host_ref.is_none()
            {
                let assigned =
                    HostRef::new(*type_id, vela_common::HostObjectId::new(*next_object_id), 1);
                *host_ref = Some(assigned);
                if let Some(identity) = identity {
                    *identity.0.lock() = Some(assigned);
                }
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
            }
            | CallArg::PositionalHost {
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
            }
            | CallArg::PositionalHost {
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
    ) -> Option<(HostRef, &HostArgBinding<'a>)> {
        self.entries.iter().find_map(|entry| match entry {
            CallArg::NamedHost {
                host_ref: Some(host_ref),
                type_id: binding_type,
                binding,
                ..
            }
            | CallArg::PositionalHost {
                host_ref: Some(host_ref),
                type_id: binding_type,
                binding,
                ..
            } if *binding_type == type_id => Some((*host_ref, binding)),
            _ => None,
        })
    }

    pub(super) fn take_host_lease(
        &mut self,
        root: HostRef,
        kind: HostLeaseKind,
    ) -> HostResult<ErasedHostLease<'a>> {
        let Some(binding) = self.direct_binding_mut(root) else {
            return Err(host_lease_unsupported(root));
        };
        match (binding, kind) {
            (HostArgBinding::Shared { object, leases }, HostLeaseKind::Shared) => {
                if object.lease_any().is_none() {
                    return Err(host_lease_unsupported(root));
                }
                leases.fetch_add(1, Ordering::AcqRel);
                Ok(ErasedHostLease::SharedBorrowed {
                    object: *object,
                    leases: Arc::clone(leases),
                })
            }
            (HostArgBinding::Shared { .. }, HostLeaseKind::Exclusive) => {
                Err(host_object_busy(root))
            }
            (HostArgBinding::Mutable { object }, HostLeaseKind::Shared) => {
                let Some(leased) = object.try_read_arc() else {
                    return Err(host_object_busy(root));
                };
                if leased.lease_any().is_none() {
                    return Err(host_lease_unsupported(root));
                }
                Ok(ErasedHostLease::SharedMutable { object: leased })
            }
            (HostArgBinding::Mutable { object }, HostLeaseKind::Exclusive) => {
                let Some(mut leased) = object.try_write_arc() else {
                    return Err(host_object_busy(root));
                };
                if leased.lease_any().is_none() || leased.lease_any_mut().is_none() {
                    return Err(host_lease_unsupported(root));
                }
                Ok(ErasedHostLease::Exclusive { object: leased })
            }
            (HostArgBinding::Scoped { object, .. }, HostLeaseKind::Shared) => {
                let Some(leased) = object.try_read_arc() else {
                    return Err(host_object_busy(root));
                };
                Ok(ErasedHostLease::ScopedShared { object: leased })
            }
            (
                HostArgBinding::Scoped {
                    object,
                    mutable: true,
                },
                HostLeaseKind::Exclusive,
            ) => {
                let Some(leased) = object.try_write_arc() else {
                    return Err(host_object_busy(root));
                };
                Ok(ErasedHostLease::ScopedExclusive { object: leased })
            }
            (HostArgBinding::Scoped { mutable: false, .. }, HostLeaseKind::Exclusive) => {
                Err(host_object_busy(root))
            }
        }
    }

    pub(super) fn take_host_leases(
        &mut self,
        requests: &[(HostRef, HostLeaseKind)],
    ) -> HostResult<Vec<ErasedHostLease<'a>>> {
        let mut leases = Vec::with_capacity(requests.len());
        for (root, kind) in requests {
            match self.take_host_lease(*root, *kind) {
                Ok(lease) => leases.push(lease),
                Err(error) => {
                    drop(leases);
                    return Err(error);
                }
            }
        }
        Ok(leases)
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
        identity: Option<DirectHostIdentity>,
        type_id: vela_common::HostTypeId,
        binding: HostArgBinding<'a>,
    },
    PositionalHost {
        host_ref: Option<HostRef>,
        identity: Option<DirectHostIdentity>,
        type_id: vela_common::HostTypeId,
        binding: HostArgBinding<'a>,
    },
}

impl CallArg<'_> {
    fn runtime_value(&self, runtime: &mut CallArgRuntime<'_, '_, '_>) -> VmResult<Value> {
        match self {
            Self::Positional(value) | Self::Named { value, .. } => {
                owned_to_linked_persistent_value(
                    value.clone(),
                    runtime.program,
                    runtime.heap,
                    Some(runtime.budget),
                )
            }
            Self::PositionalValue(value) | Self::NamedValue { value, .. } => {
                if value.runtime_id() == runtime.runtime_id {
                    Ok(value.value())
                } else {
                    Err(call_args_type_error("VelaValue belongs to another Runtime"))
                }
            }
            Self::NamedHost {
                host_ref: Some(host_ref),
                ..
            }
            | Self::PositionalHost {
                host_ref: Some(host_ref),
                ..
            } => Ok(Value::HostRef(*host_ref)),
            Self::NamedHost { host_ref: None, .. }
            | Self::PositionalHost { host_ref: None, .. } => Err(call_args_type_error(
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
            Self::PositionalHost { .. } => None,
        }
    }
}

pub(super) enum HostArgBinding<'a> {
    Shared {
        object: &'a (dyn ScriptHostObject + Sync),
        leases: Arc<AtomicUsize>,
    },
    Mutable {
        object: MutableHostLeaseSlot<'a>,
    },
    Scoped {
        object: ScopedHostLeaseSlot<'a>,
        mutable: bool,
    },
}

impl HostArgBinding<'_> {
    pub(super) const fn receiver_access(&self) -> HostLeaseKind {
        match self {
            Self::Shared { .. } | Self::Scoped { mutable: false, .. } => HostLeaseKind::Shared,
            Self::Mutable { .. } | Self::Scoped { mutable: true, .. } => HostLeaseKind::Exclusive,
        }
    }

    pub(super) fn inspect<T>(
        &self,
        root: HostRef,
        inspect: impl FnOnce(&dyn ScriptHostObject) -> HostResult<T>,
    ) -> HostResult<T> {
        match self {
            Self::Shared { object, .. } => inspect(*object),
            Self::Mutable { object } => object
                .try_read()
                .ok_or_else(|| host_object_busy(root))
                .and_then(|object| inspect(&**object)),
            Self::Scoped { object, .. } => object
                .try_read()
                .ok_or_else(|| host_object_busy(root))
                .and_then(|object| inspect(&**object)),
        }
    }

    pub(super) fn mutate<T>(
        &mut self,
        root: HostRef,
        mutate: impl FnOnce(&mut dyn ScriptHostObject) -> HostResult<T>,
    ) -> HostResult<T> {
        match self {
            Self::Shared { .. } => Err(host_object_busy(root)),
            Self::Mutable { object } => object
                .try_write()
                .ok_or_else(|| host_object_busy(root))
                .and_then(|mut object| mutate(&mut **object)),
            Self::Scoped { object, .. } => object
                .try_write()
                .ok_or_else(|| host_object_busy(root))
                .and_then(|mut object| mutate(&mut **object)),
        }
    }
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

#[cfg(test)]
mod lease_tests {
    use std::any::Any;

    use vela_common::HostTypeId;
    use vela_host::lease::HostLeaseKind;
    use vela_host::object::ScriptHostObject;
    use vela_host::target::HostTargetInstance;
    use vela_host::value::HostValue;

    use super::{CallArg, CallArgs};

    struct LeaseHost;

    impl ScriptHostObject for LeaseHost {
        fn host_type_id(&self) -> HostTypeId {
            HostTypeId::new(0x1EA5E)
        }

        fn lease_any(&self) -> Option<&dyn Any> {
            Some(self)
        }

        fn lease_any_mut(&mut self) -> Option<&mut dyn Any> {
            Some(self)
        }

        fn read_resolved_host(
            &self,
            _access: vela_host::resolved::ResolvedHostAccess,
            _target: HostTargetInstance<'_>,
        ) -> vela_host::error::HostResult<HostValue> {
            Ok(HostValue::Unit)
        }
    }

    #[test]
    fn multi_lease_acquisition_rolls_back_on_conflict() {
        let mut host = LeaseHost;
        let mut args = CallArgs::new().with_host_mut("host", &mut host);
        let mut next = 1_u64 << 63;
        args.assign_direct_host_refs(&mut next);
        let root = match &args.entries[0] {
            CallArg::NamedHost {
                host_ref: Some(root),
                ..
            } => *root,
            _ => panic!("direct host argument should have an identity"),
        };

        let error = match args.take_host_leases(&[
            (root, HostLeaseKind::Exclusive),
            (root, HostLeaseKind::Exclusive),
        ]) {
            Ok(_) => panic!("duplicate exclusive request should fail atomically"),
            Err(error) => error,
        };
        assert!(matches!(
            error.kind,
            vela_host::error::HostErrorKind::HostObjectBusy { .. }
        ));

        let lease = args
            .take_host_lease(root, HostLeaseKind::Exclusive)
            .expect("failed acquisition should restore the first lease");
        drop(lease);
    }

    #[test]
    fn mutable_origin_shared_leases_coexist_and_restore_exclusive_access() {
        fn require_send<T: Send>(_: &T) {}

        let mut host = LeaseHost;
        let mut args = CallArgs::new().with_host_mut("host", &mut host);
        let mut next = 1_u64 << 63;
        args.assign_direct_host_refs(&mut next);
        let root = match &args.entries[0] {
            CallArg::NamedHost {
                host_ref: Some(root),
                ..
            } => *root,
            _ => panic!("direct host argument should have an identity"),
        };

        let first = args
            .take_host_lease(root, HostLeaseKind::Shared)
            .expect("first shared lease should be available");
        let second = args
            .take_host_lease(root, HostLeaseKind::Shared)
            .expect("second shared lease should coexist");
        require_send(&first);
        require_send(&second);
        assert!(!first.is_exclusive());
        assert!(!second.is_exclusive());

        let binding = args
            .direct_binding(root)
            .expect("mutable binding should remain registered");
        let super::HostArgBinding::Mutable { object } = binding else {
            panic!("binding should have mutable origin");
        };
        assert!(
            object.try_read().is_some(),
            "parent read access remains legal"
        );
        assert!(
            object.try_write().is_none(),
            "shared leases exclude mutation"
        );
        let conflict = match args.take_host_lease(root, HostLeaseKind::Exclusive) {
            Ok(_) => panic!("exclusive acquisition must conflict with shared leases"),
            Err(error) => error,
        };
        assert!(matches!(
            conflict.kind,
            vela_host::error::HostErrorKind::HostObjectBusy { .. }
        ));

        drop(first);
        drop(second);
        let exclusive = args
            .take_host_lease(root, HostLeaseKind::Exclusive)
            .expect("dropping all shared leases should restore exclusive access");
        require_send(&exclusive);
        assert!(exclusive.is_exclusive());
        drop(exclusive);
    }

    #[test]
    fn mixed_mutable_origin_acquisition_rolls_back_shared_state() {
        let mut host = LeaseHost;
        let mut args = CallArgs::new().with_host_mut("host", &mut host);
        let mut next = 1_u64 << 63;
        args.assign_direct_host_refs(&mut next);
        let root = match &args.entries[0] {
            CallArg::NamedHost {
                host_ref: Some(root),
                ..
            } => *root,
            _ => panic!("direct host argument should have an identity"),
        };

        let error = match args.take_host_leases(&[
            (root, HostLeaseKind::Shared),
            (root, HostLeaseKind::Exclusive),
        ]) {
            Ok(_) => panic!("shared followed by exclusive should fail atomically"),
            Err(error) => error,
        };
        assert!(matches!(
            error.kind,
            vela_host::error::HostErrorKind::HostObjectBusy { .. }
        ));

        let exclusive = args
            .take_host_lease(root, HostLeaseKind::Exclusive)
            .expect("rollback should restore the available state");
        drop(exclusive);
    }
}
