//! Explicit registration objects used by embedding applications.

use std::any::TypeId;
use std::collections::HashSet;
use std::marker::PhantomData;

use crate::builder::EngineBuilder;
use crate::interop::CallableRegistration;
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
