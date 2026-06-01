use std::sync::Arc;

use vela_reflect::registry::TypeRegistry;
use vela_reflect::{self as reflect};

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
        self.register_reflection_natives_with_policy(
            registry,
            reflect::permissions::ReflectPolicy::all(),
        );
    }

    pub fn register_reflection_natives_with_permissions(
        &mut self,
        registry: Arc<TypeRegistry>,
        permissions: reflect::permissions::ReflectPermissionSet,
    ) {
        self.register_reflection_natives_with_policy(
            registry,
            reflect::permissions::ReflectPolicy::new(permissions),
        );
    }

    pub fn register_reflection_natives_with_policy(
        &mut self,
        registry: Arc<TypeRegistry>,
        policy: reflect::permissions::ReflectPolicy,
    ) {
        self.register_type_registry(Arc::clone(&registry));
        let lookup_budget = Arc::new(reflect::permissions::ReflectLookupBudget::new(
            policy.lookup_limit(),
        ));

        policy::register(self, &policy, &lookup_budget);
        types::register(self, &registry, &policy, &lookup_budget);
        fields::register(self, &registry, &policy, &lookup_budget);
        modules::register(self, &registry, &policy, &lookup_budget);
        methods::register(self, &registry, &policy, &lookup_budget);
        traits::register(self, &registry, &policy, &lookup_budget);
        variants::register(self, &registry, &policy, &lookup_budget);
        let function_calls =
            values::ReflectedFunctionCalls::new(self.natives.clone(), self.host_natives.clone());
        values::register(self, &registry, &policy, &lookup_budget, function_calls);
    }
}
