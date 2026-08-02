//! Explicit registration objects used by embedding applications.

use std::any::TypeId;
use std::collections::HashSet;
use std::marker::PhantomData;

use vela_common::{HostMethodId, HostTypeId, ReceiverCapability, stable_id};
use vela_host::lease::HostLeaseKind;
use vela_reflect::registry::{SchemaHash, TypeDesc, TypeKey, TypeKind};
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

use crate::builder::EngineBuilder;
use crate::interop::{CallableRegistration, catch_export_panic};
use crate::method::NativeMethodDesc;
use crate::type_binding::TypeBinding;
use crate::type_registration::VelaType;

type Installer = Box<dyn FnOnce(EngineBuilder) -> EngineBuilder + 'static>;

/// The complete Vela surface for one concrete Rust type.
pub struct TypeRegistration<T> {
    installer: Installer,
    marker: PhantomData<fn() -> T>,
}

impl<T> TypeRegistration<T>
where
    T: VelaType,
{
    /// Uses the registration generated for `T` (including its dependencies).
    #[must_use]
    pub fn of() -> Self {
        Self::from_installer(T::register)
    }
}

impl<T: 'static> TypeRegistration<T> {
    /// Creates a registration from a manually constructed type binding.
    #[must_use]
    pub fn binding(binding: TypeBinding<T>) -> Self {
        Self::from_installer(move |builder| builder.install_type_binding(binding))
    }

    /// Registers an arbitrary concrete Rust type as an opaque Host type.
    ///
    /// The Rust type does not need to implement a Vela trait or be wrapped in
    /// an application-owned newtype. Its script surface consists only of the
    /// methods explicitly attached through [`MethodRegistration`].
    #[must_use]
    pub fn host(path: &str) -> Self {
        Self::binding(TypeBinding::host(registered_host_type_desc(path)))
    }
}

impl<T> TypeRegistration<T> {
    #[doc(hidden)]
    #[must_use]
    pub fn from_installer(
        installer: impl FnOnce(EngineBuilder) -> EngineBuilder + 'static,
    ) -> Self {
        Self {
            installer: Box::new(installer),
            marker: PhantomData,
        }
    }
}

/// All ordinary methods owned by one concrete Rust type.
pub struct MethodsRegistration<T> {
    source: MethodsSource,
    marker: PhantomData<fn() -> T>,
}

/// One method owned by one concrete Rust type.
pub struct MethodRegistration<T> {
    methods: MethodsRegistration<T>,
}

impl<T> MethodRegistration<T> {
    #[must_use]
    pub fn new(
        contract: crate::interop::CallableContract,
        installer: impl Fn(EngineBuilder) -> EngineBuilder + Send + Sync + 'static,
    ) -> Self {
        Self {
            methods: MethodsRegistration::new(vec![contract], installer),
        }
    }

    /// The callable contract contributed by this method.
    #[must_use]
    pub fn contract(&self) -> Option<&crate::interop::CallableContract> {
        self.methods.contracts().first()
    }

    /// Registers one shared-receiver method for an explicitly registered Host
    /// type. Arguments and the return value use Vela's owned dynamic values.
    #[must_use]
    pub fn shared(
        mut desc: NativeMethodDesc,
        function: impl Fn(&T, &[OwnedValue]) -> VmResult<OwnedValue> + Send + Sync + 'static,
    ) -> Self
    where
        T: Send + Sync + 'static,
    {
        desc.receiver = ReceiverCapability::Shared;
        registered_host_method(
            desc,
            HostLeaseKind::Shared,
            move |object, args, receiver_root| {
                let receiver = object
                    .lease_any()
                    .and_then(|object| object.downcast_ref::<T>())
                    .ok_or_else(|| vela_host::lease::host_lease_unsupported(receiver_root))?;
                function(receiver, args)
            },
        )
    }

    /// Registers one exclusive-receiver method for an explicitly registered
    /// Host type. Arguments and the return value use Vela's owned dynamic
    /// values.
    #[must_use]
    pub fn exclusive(
        mut desc: NativeMethodDesc,
        function: impl Fn(&mut T, &[OwnedValue]) -> VmResult<OwnedValue> + Send + Sync + 'static,
    ) -> Self
    where
        T: Send + Sync + 'static,
    {
        desc.receiver = ReceiverCapability::Exclusive;
        let method_id = desc.id;
        let callable = desc.name.clone();
        Self::from_installer(move |builder| {
            builder.register_native_method_fn(desc, move |receiver, args, host| {
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
                            invocation_result =
                                Some(catch_export_panic(&callable, || function(receiver, args)));
                            Ok(())
                        });
                let result = match lease_result {
                    Ok(()) => invocation_result.expect("host lease callback must run exactly once"),
                    Err(error) => Err(error.into()),
                };
                release_registered_receiver(scoped_receiver, result, host)
            })
        })
    }

    #[doc(hidden)]
    #[must_use]
    pub fn from_installer(
        installer: impl FnOnce(EngineBuilder) -> EngineBuilder + 'static,
    ) -> Self {
        Self {
            methods: MethodsRegistration::from_installer(installer),
        }
    }
}

fn registered_host_method<T>(
    desc: NativeMethodDesc,
    lease_kind: HostLeaseKind,
    function: impl Fn(
        &dyn vela_host::object::ScriptHostObject,
        &[OwnedValue],
        vela_host::path::HostRef,
    ) -> VmResult<OwnedValue>
    + Send
    + Sync
    + 'static,
) -> MethodRegistration<T>
where
    T: Send + Sync + 'static,
{
    let method_id = desc.id;
    let callable = desc.name.clone();
    MethodRegistration::from_installer(move |builder| {
        builder.register_native_method_fn(desc, move |receiver, args, host| {
            let scoped_receiver =
                crate::host_call::retain_registered_host_method_receiver(receiver, host)?;
            if !receiver.segments.is_empty() && scoped_receiver.is_none() {
                return crate::host_call::call_registered_host_method_through_adapter(
                    receiver, args, method_id, host,
                );
            }
            let receiver_root = scoped_receiver.unwrap_or(receiver.root);
            let requests = [(receiver_root, lease_kind)];
            let mut invocation_result = None;
            let lease_result =
                host.adapter
                    .with_host_leases(&requests, &mut |leases, _leased_adapter| {
                        let receiver = leases.first().expect("one receiver lease").object();
                        invocation_result = Some(catch_export_panic(&callable, || {
                            function(receiver, args, receiver_root)
                        }));
                        Ok(())
                    });
            let result = match lease_result {
                Ok(()) => invocation_result.expect("host lease callback must run exactly once"),
                Err(error) => Err(error.into()),
            };
            release_registered_receiver(scoped_receiver, result, host)
        })
    })
}

pub(crate) fn release_registered_receiver(
    scoped_receiver: Option<vela_host::path::HostRef>,
    result: VmResult<OwnedValue>,
    host: &mut vela_vm::HostExecution<'_>,
) -> VmResult<OwnedValue> {
    if let Some(scoped_receiver) = scoped_receiver
        && let Err(error) = host.adapter.release_scoped_host(scoped_receiver)
        && result.is_ok()
    {
        return Err(error.into());
    }
    result
}

/// Builds the stable reflection identity used by [`TypeRegistration::host`]
/// and by handwritten [`NativeMethodDesc`] owners.
#[must_use]
pub fn registered_host_type_desc(path: &str) -> TypeDesc {
    let (module, name) = path
        .rsplit_once("::")
        .filter(|(module, name)| !module.is_empty() && !name.is_empty())
        .unwrap_or_else(|| panic!("registered Host path must include a module and type name"));
    TypeDesc::new(TypeKey::new(
        vela_def::TypeId::new(u128::from(stable_id("host_type", "", path))),
        name,
    ))
    .kind(TypeKind::Host)
    .schema_hash(SchemaHash::new(stable_id(
        "registered_host_schema_v1",
        "",
        path,
    )))
    .host_type(HostTypeId::new(stable_id("host_ref_type", "", path)))
    .attr("module", module)
}

/// Creates a method descriptor with the same stable owner and method identity
/// as generated Host methods.
#[must_use]
pub fn registered_host_method_desc(path: &str, name: &str) -> NativeMethodDesc {
    NativeMethodDesc::new(
        registered_host_type_desc(path).key,
        HostMethodId::new(u128::from(stable_id("host_method", path, name))),
        name,
    )
}

enum MethodsSource {
    Bundle(CallableRegistration),
    Installer(Installer),
}

impl<T> MethodsRegistration<T> {
    #[must_use]
    pub fn new(
        contracts: Vec<crate::interop::CallableContract>,
        installer: impl Fn(EngineBuilder) -> EngineBuilder + Send + Sync + 'static,
    ) -> Self {
        Self::from_bundle(CallableRegistration::new(contracts, installer))
    }

    #[must_use]
    pub fn with_protocols(
        contracts: Vec<crate::interop::CallableContract>,
        protocols: Vec<crate::interop::VelaProtocolContract>,
        installer: impl Fn(EngineBuilder) -> EngineBuilder + Send + Sync + 'static,
    ) -> Self {
        Self::from_bundle(CallableRegistration::with_protocols(
            contracts, protocols, installer,
        ))
    }

    fn from_bundle(bundle: CallableRegistration) -> Self {
        Self {
            source: MethodsSource::Bundle(bundle),
            marker: PhantomData,
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub fn from_installer(
        installer: impl FnOnce(EngineBuilder) -> EngineBuilder + 'static,
    ) -> Self {
        Self {
            source: MethodsSource::Installer(Box::new(installer)),
            marker: PhantomData,
        }
    }

    /// Callable contracts contributed by this method set.
    #[must_use]
    pub fn contracts(&self) -> &[crate::interop::CallableContract] {
        match &self.source {
            MethodsSource::Bundle(bundle) => bundle.contracts(),
            MethodsSource::Installer(_) => &[],
        }
    }

    /// Protocol contracts contributed by this method set.
    #[must_use]
    pub fn protocols(&self) -> &[crate::interop::VelaProtocolContract] {
        match &self.source {
            MethodsSource::Bundle(bundle) => bundle.protocols(),
            MethodsSource::Installer(_) => &[],
        }
    }

    fn install(self, builder: EngineBuilder) -> EngineBuilder {
        match self.source {
            MethodsSource::Bundle(bundle) => bundle.install(builder),
            MethodsSource::Installer(installer) => installer(builder),
        }
    }
}

/// All free functions owned by one Vela module.
pub struct ModuleRegistration {
    bundle: CallableRegistration,
}

impl ModuleRegistration {
    #[must_use]
    pub fn new(
        contracts: Vec<crate::interop::CallableContract>,
        installer: impl Fn(EngineBuilder) -> EngineBuilder + Send + Sync + 'static,
    ) -> Self {
        Self::from_bundle(CallableRegistration::new(contracts, installer))
    }

    fn from_bundle(bundle: CallableRegistration) -> Self {
        Self { bundle }
    }

    /// Callable contracts contributed by this module.
    #[must_use]
    pub fn contracts(&self) -> &[crate::interop::CallableContract] {
        self.bundle.contracts()
    }
}

/// One explicit, application-owned set of Vela bindings.
///
/// Registration has no inventory or ambient discovery. Exact repeated type
/// registrations compose across binding modules; Engine sealing still rejects
/// incompatible bindings for the same Rust type. Method sets are attached
/// through the typed handle returned by [`Self::register_type`].
#[derive(Default)]
pub struct VelaBindings {
    registered_types: HashSet<TypeId>,
    installers: Vec<Installer>,
}

impl VelaBindings {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one concrete Rust type and returns its typed method handle.
    pub fn register_type<T>(&mut self, registration: TypeRegistration<T>) -> RegisteredType<'_, T>
    where
        T: 'static,
    {
        self.registered_types.insert(TypeId::of::<T>());
        self.installers.push(registration.installer);
        RegisteredType {
            bindings: self,
            marker: PhantomData,
        }
    }

    /// Returns the method-registration handle for an already registered type.
    pub fn type_mut<T>(&mut self) -> RegisteredType<'_, T>
    where
        T: 'static,
    {
        assert!(
            self.registered_types.contains(&TypeId::of::<T>()),
            "Vela methods cannot be registered before their owner type `{}`",
            std::any::type_name::<T>()
        );
        RegisteredType {
            bindings: self,
            marker: PhantomData,
        }
    }

    /// Registers the free functions exported by one module.
    pub fn register_module(&mut self, registration: ModuleRegistration) -> &mut Self {
        self.installers.push(Box::new(move |builder| {
            registration.bundle.install(builder)
        }));
        self
    }

    pub(crate) fn install(self, mut builder: EngineBuilder) -> EngineBuilder {
        for installer in self.installers {
            builder = installer(builder);
        }
        builder
    }
}

/// Typed handle used to attach methods to an already registered owner type.
pub struct RegisteredType<'a, T> {
    bindings: &'a mut VelaBindings,
    marker: PhantomData<fn() -> T>,
}

impl<T: 'static> RegisteredType<'_, T> {
    /// Attaches one generated or manually constructed method set to `T`.
    pub fn register_methods(self, registration: MethodsRegistration<T>) -> Self {
        self.bindings
            .installers
            .push(Box::new(move |builder| registration.install(builder)));
        self
    }

    /// Attaches one generated or manually constructed method to `T`.
    pub fn register_method(self, registration: MethodRegistration<T>) -> Self {
        self.register_methods(registration.methods)
    }
}

#[doc(hidden)]
pub trait InstallRegistration {
    fn install_into(self, builder: EngineBuilder) -> EngineBuilder;
}

impl<T> InstallRegistration for MethodsRegistration<T> {
    fn install_into(self, builder: EngineBuilder) -> EngineBuilder {
        self.install(builder)
    }
}

#[doc(hidden)]
pub mod __private {
    pub use crate::method_family::NominalHostMethodFamily;
}

impl InstallRegistration for ModuleRegistration {
    fn install_into(self, builder: EngineBuilder) -> EngineBuilder {
        self.bundle.install(builder)
    }
}
