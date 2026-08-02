//! Rust-side monomorphized Host method families.
//!
//! Vela intentionally has neither script generics nor overload resolution. A
//! method family therefore publishes one ordinary script method whose selected
//! argument is `Any`, while Rust registers a closed-over adapter for every
//! concrete nominal Value type accepted by the embedding application.

use std::collections::BTreeMap;
use std::sync::Arc;

use vela_host::lease::HostLeaseKind;
use vela_host::object::ScriptHostObject;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::args::FromScriptArg;
use crate::builder::EngineBuilder;
use crate::interop::{VelaValueBoundary, catch_export_panic};
use crate::method::NativeMethodDesc;
use crate::native::TypeHint;
use crate::type_registration::VelaType;
use crate::typed::IntoNativeReturn;

type MethodInstance<T> =
    dyn Fn(&mut T, &OwnedValue) -> VmResult<OwnedValue> + Send + Sync + 'static;
type TypeInstaller = dyn Fn(EngineBuilder) -> EngineBuilder + Send + Sync + 'static;

struct NominalMethodInstance<T> {
    install_type: Arc<TypeInstaller>,
    invoke: Arc<MethodInstance<T>>,
}

/// One script-visible Host method backed by Rust monomorphized instances.
///
/// Each registered argument type must be a nominal Vela `Record` or `Enum`.
/// The stable Vela type path is used only to select an instance; the selected
/// adapter still performs the normal generated Rust decoding before invoking
/// the concrete function.
pub struct NominalHostMethodFamily<T> {
    desc: NativeMethodDesc,
    instances: BTreeMap<String, NominalMethodInstance<T>>,
}

impl<T> NominalHostMethodFamily<T>
where
    T: ScriptHostObject + Send + 'static,
{
    /// Creates a family for a descriptor with exactly one `Any` parameter.
    ///
    /// The descriptor remains an ordinary Host method ABI: Vela sees no
    /// overloads and no generic parameters.
    #[must_use]
    pub fn new(desc: NativeMethodDesc) -> Self {
        assert!(
            matches!(desc.params.as_slice(), [parameter] if parameter.hint == TypeHint::Any),
            "a nominal Host method family requires exactly one Any parameter"
        );
        assert!(
            desc.receiver == vela_common::ReceiverCapability::Exclusive,
            "a nominal Host method family currently requires an exclusive receiver"
        );
        Self {
            desc,
            instances: BTreeMap::new(),
        }
    }

    /// Adds one Rust monomorphized instance to this family.
    ///
    /// Registering the same concrete type more than once is idempotent. This
    /// lets a protocol registry remain the single source of truth even when a
    /// concrete event has multiple runtime handlers.
    pub fn register_instance<A, R, F>(&mut self, function: F)
    where
        A: FromScriptArg + VelaType + VelaValueBoundary + 'static,
        R: IntoNativeReturn,
        F: Fn(&mut T, A) -> R + Send + Sync + 'static,
    {
        let hint = A::vela_type_hint();
        let key = match hint {
            TypeHint::Record(key) | TypeHint::Enum(key) => key.name,
            _ => panic!(
                "nominal Host method instance `{}` must be a Vela Record or Enum",
                A::TYPE_NAME
            ),
        };
        assert_eq!(
            key,
            A::TYPE_NAME,
            "nominal Host method instance type hint and decoder identity differ"
        );
        self.instances
            .entry(key)
            .or_insert_with(|| NominalMethodInstance {
                install_type: Arc::new(A::register),
                invoke: Arc::new(move |receiver, value| {
                    let argument = A::from_script_arg(value)?;
                    function(receiver, argument).into_native_return()
                }),
            });
    }

    /// Installs all concrete Value bindings and the single script-visible
    /// method into an Engine builder.
    #[must_use]
    pub fn install(self, mut builder: EngineBuilder) -> EngineBuilder {
        for instance in self.instances.values() {
            builder = (instance.install_type)(builder);
        }

        let method_id = self.desc.id;
        let callable = self.desc.name.clone();
        let instances = Arc::new(self.instances);
        builder.register_native_method_fn(self.desc, move |receiver, args, host| {
            let [argument] = args else {
                return Err(VmError::new(VmErrorKind::ArityMismatch {
                    name: callable.clone(),
                    expected: 1,
                    actual: args.len(),
                }));
            };
            let nominal_type = nominal_type_name(argument).ok_or_else(|| {
                VmError::new(VmErrorKind::TypeMismatch {
                    operation: "nominal Host method family argument",
                })
            })?;
            let instance = instances.get(nominal_type).ok_or_else(|| {
                VmError::new(VmErrorKind::TypeContractViolation {
                    expected: "registered nominal method instance".to_owned(),
                    actual: nominal_type.to_owned(),
                    debug_name: callable.clone(),
                })
            })?;

            let scoped_receiver =
                crate::host_call::retain_registered_host_method_receiver(receiver, host)?;
            if !receiver.segments.is_empty() && scoped_receiver.is_none() {
                return crate::host_call::call_registered_host_method_through_adapter(
                    receiver, args, method_id, host,
                );
            }
            let receiver_root = scoped_receiver.unwrap_or(receiver.root);
            let requests = [(receiver_root, HostLeaseKind::Exclusive)];
            let mut invocation_result = None;
            let lease_result =
                host.adapter
                    .with_host_leases(&requests, &mut |leases, _leased_adapter| {
                        let receiver = leases
                            .first_mut()
                            .and_then(|lease| lease.object_mut())
                            .and_then(|object| object.lease_any_mut())
                            .and_then(|object| object.downcast_mut::<T>())
                            .ok_or_else(|| {
                                vela_host::lease::host_lease_unsupported(receiver_root)
                            })?;
                        invocation_result = Some(catch_export_panic(&callable, || {
                            (instance.invoke)(receiver, argument)
                        }));
                        Ok(())
                    });
            let result = match lease_result {
                Ok(()) => invocation_result.expect("host lease callback must run exactly once"),
                Err(error) => Err(error.into()),
            };
            if let Some(scoped_receiver) = scoped_receiver {
                if let Err(error) = host.adapter.release_scoped_host(scoped_receiver) {
                    if result.is_ok() {
                        return Err(error.into());
                    }
                }
            }
            result
        })
    }

    /// Erases the internal monomorphized family into the ordinary typed method
    /// registration object consumed by application bindings.
    #[must_use]
    pub fn into_registration(self) -> crate::registration::MethodsRegistration<T> {
        crate::registration::MethodsRegistration::from_installer(move |builder| {
            self.install(builder)
        })
    }
}

fn nominal_type_name(value: &OwnedValue) -> Option<&str> {
    match value {
        OwnedValue::Record { type_name, .. } => Some(type_name),
        OwnedValue::Enum { enum_name, .. } => Some(enum_name),
        _ => None,
    }
}
