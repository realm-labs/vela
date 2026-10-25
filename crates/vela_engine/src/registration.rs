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
/// incompatible bindings for the same Rust type.
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

    /// Registers one concrete owned Rust value type.
    pub fn register_type<T>(&mut self, registration: TypeRegistration<T>) -> &mut Self
    where
        T: 'static,
    {
        if self.registered_types.insert(TypeId::of::<T>()) {
            self.installers.push(registration.installer);
        }
        self
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

#[doc(hidden)]
pub trait InstallRegistration {
    fn install_into(self, builder: EngineBuilder) -> EngineBuilder;
}

impl InstallRegistration for ModuleRegistration {
    fn install_into(self, builder: EngineBuilder) -> EngineBuilder {
        self.bundle.install(builder)
    }
}

#[cfg(test)]
mod tests {
    use super::{TypeRegistration, VelaBindings};

    #[test]
    fn repeated_concrete_type_registration_installs_once() {
        let mut bindings = VelaBindings::new();
        bindings.register_type(TypeRegistration::<i64>::from_installer(|builder| builder));
        bindings.register_type(TypeRegistration::<i64>::from_installer(|builder| builder));

        assert_eq!(bindings.installers.len(), 1);
    }
}
