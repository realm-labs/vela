use std::sync::Arc;

use vela_reflect::{self as reflect, TypeRegistry};

use crate::Vm;

mod common;
mod fields;
mod methods;
mod modules;
mod policy;
mod traits;
mod types;
mod values;
mod variants;

impl Vm {
    pub fn register_reflection_natives(&mut self, registry: Arc<TypeRegistry>) {
        self.register_reflection_natives_with_policy(registry, reflect::ReflectPolicy::all());
    }

    pub fn register_reflection_natives_with_permissions(
        &mut self,
        registry: Arc<TypeRegistry>,
        permissions: reflect::ReflectPermissionSet,
    ) {
        self.register_reflection_natives_with_policy(
            registry,
            reflect::ReflectPolicy::new(permissions),
        );
    }

    pub fn register_reflection_natives_with_policy(
        &mut self,
        registry: Arc<TypeRegistry>,
        policy: reflect::ReflectPolicy,
    ) {
        self.register_type_registry(Arc::clone(&registry));
        let lookup_budget = Arc::new(reflect::ReflectLookupBudget::new(policy.lookup_limit()));

        policy::register(self, &policy, &lookup_budget);
        types::register(self, &registry, &policy, &lookup_budget);
        fields::register(self, &registry, &policy, &lookup_budget);
        modules::register(self, &registry, &policy, &lookup_budget);
        methods::register(self, &registry, &policy, &lookup_budget);
        traits::register(self, &registry, &policy, &lookup_budget);
        variants::register(self, &registry, &policy, &lookup_budget);
        values::register(self, &registry, &policy, &lookup_budget);
    }
}
