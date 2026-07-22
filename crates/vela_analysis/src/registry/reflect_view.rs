use vela_reflect::access::{FunctionEffectSet, MethodEffectSet};
use vela_reflect::modules::FunctionDesc;
use vela_reflect::registry::{MethodDesc, TraitMethodDesc, TypeKind, TypeRegistry};

use super::{
    CallableParameterFact, CallableParameterRequirementFact, CallableSignatureFact,
    RegistryEffectFact, RegistryFacts, RegistryFieldAccessFact, RegistryFieldTargetFact,
    RegistryFunctionAccessFact, RegistryIndexCapabilityFact, RegistryMethodAccessFact,
    RegistryModuleFact, RegistryTypeBindingFact, RegistryTypeTargetFact, registry_hint_fact,
    type_desc_fact,
};
use crate::type_fact::TypeFact;

impl RegistryFacts {
    #[must_use]
    pub fn from_registry(registry: &TypeRegistry) -> Self {
        let mut facts = Self::default();

        if let Some(snapshot) = registry.type_binding_snapshot() {
            facts.set_type_binding_checksum(snapshot.checksum());
        }

        for desc in registry.types() {
            let type_fact = type_desc_fact(registry, desc);
            facts.types.insert(desc.key.name.clone(), type_fact.clone());
            facts.type_targets.insert(
                desc.key.name.clone(),
                RegistryTypeTargetFact::new(&desc.key.name, desc.key.id, desc.host_type_id),
            );
            if let Some(binding) = registry.type_binding_for_key(&desc.key) {
                facts.insert_type_binding(
                    &desc.key.name,
                    RegistryTypeBindingFact {
                        id: binding.id,
                        storage: binding.storage,
                        capabilities: binding.capabilities,
                        collection_views: binding.collection_views,
                        constructor_ids: binding.constructor_ids.clone(),
                        abi_fingerprint: binding.abi_fingerprint,
                    },
                );
            }
            if let Some(docs) = &desc.docs {
                facts.type_docs.insert(desc.key.name.clone(), docs.clone());
            }
            if let Some(capability) = &desc.index_capability {
                facts.index_capabilities.insert(
                    desc.key.name.clone(),
                    RegistryIndexCapabilityFact::new(&desc.key.name, capability, registry),
                );
            }

            for (declaration_order, field) in desc.fields.iter().enumerate() {
                let key = (desc.key.name.clone(), field.name.clone());
                facts.fields.insert(
                    key.clone(),
                    field
                        .type_hint
                        .as_deref()
                        .map_or(TypeFact::Unknown, |hint| registry_hint_fact(registry, hint)),
                );
                if let Some(docs) = &field.docs {
                    facts.field_docs.insert(key.clone(), docs.clone());
                }
                let access = RegistryFieldAccessFact::new(&desc.key.name, &field.name, field);
                facts.field_access.insert(key.clone(), access.clone());
                facts.field_targets.insert(
                    key,
                    RegistryFieldTargetFact::new(
                        desc.key.id,
                        &desc.key.name,
                        &field.name,
                        field.id,
                        matches!(desc.kind, TypeKind::Host).then_some(field.id),
                        false,
                        access,
                    )
                    .declaration_order(declaration_order_u32(declaration_order))
                    .defaulted(field.has_default),
                );
            }

            for method in &desc.methods {
                let key = (desc.key.name.clone(), method.name.clone());
                facts
                    .methods
                    .insert(key.clone(), method_desc_fact(registry, method));
                facts
                    .method_signatures
                    .insert(key.clone(), method_desc_signature_fact(registry, method));
                if let Some(docs) = &method.docs {
                    facts.method_docs.insert(key.clone(), docs.clone());
                }
                facts
                    .method_effects
                    .insert(key.clone(), method_effect_fact(&method.effects));
                facts.method_access.insert(
                    key,
                    RegistryMethodAccessFact::new(
                        &desc.key.name,
                        &method.name,
                        method.receiver,
                        &method.access,
                    ),
                );
            }

            for trait_desc in &desc.traits {
                facts
                    .traits
                    .entry(trait_desc.name.clone())
                    .or_insert_with(|| TypeFact::trait_type(&trait_desc.name));
                if let Some(docs) = &trait_desc.docs {
                    facts
                        .trait_docs
                        .entry(trait_desc.name.clone())
                        .or_insert_with(|| docs.clone());
                }
            }

            for variant in &desc.variants {
                let variant_key = (desc.key.name.clone(), variant.name.clone());
                facts.variants.insert(
                    variant_key.clone(),
                    TypeFact::enum_type(&desc.key.name, Some(&variant.name)),
                );
                if let Some(docs) = &variant.docs {
                    facts.variant_docs.insert(variant_key, docs.clone());
                }
                for (declaration_order, field) in variant.fields.iter().enumerate() {
                    let owner = format!("{}::{}", desc.key.name, variant.name);
                    let key = (owner.clone(), field.name.clone());
                    facts.fields.insert(
                        key.clone(),
                        field
                            .type_hint
                            .as_deref()
                            .map_or(TypeFact::Unknown, |hint| registry_hint_fact(registry, hint)),
                    );
                    if let Some(docs) = &field.docs {
                        facts.field_docs.insert(key.clone(), docs.clone());
                    }
                    let access = RegistryFieldAccessFact::new(&owner, &field.name, field);
                    facts.field_access.insert(key.clone(), access.clone());
                    facts.field_targets.insert(
                        key,
                        RegistryFieldTargetFact::new(
                            desc.key.id,
                            owner,
                            &field.name,
                            field.id,
                            matches!(desc.kind, TypeKind::Host).then_some(field.id),
                            true,
                            access,
                        )
                        .declaration_order(declaration_order_u32(declaration_order))
                        .defaulted(field.has_default),
                    );
                }
            }
        }

        for function in registry.functions() {
            facts.functions.insert(
                function.name.clone(),
                function_desc_fact(registry, function),
            );
            facts.function_signatures.insert(
                function.name.clone(),
                function_desc_signature_fact(registry, function),
            );
            facts
                .function_origins
                .insert(function.name.clone(), function.origin);
            if let Some(docs) = &function.docs {
                facts
                    .function_docs
                    .insert(function.name.clone(), docs.clone());
            }
            facts.function_effects.insert(
                function.name.clone(),
                function_effect_fact(&function.effects),
            );
            facts.function_access.insert(
                function.name.clone(),
                RegistryFunctionAccessFact::new(&function.name, &function.access),
            );
        }

        for module in registry.modules() {
            facts
                .modules
                .insert(module.name.clone(), RegistryModuleFact::new(module));
        }

        for trait_desc in registry.traits() {
            facts
                .traits
                .entry(trait_desc.name.clone())
                .or_insert_with(|| TypeFact::trait_type(&trait_desc.name));
            if let Some(docs) = &trait_desc.docs {
                facts
                    .trait_docs
                    .entry(trait_desc.name.clone())
                    .or_insert_with(|| docs.clone());
            }
            for method in &trait_desc.methods {
                let key = (trait_desc.name.clone(), method.name.clone());
                facts
                    .trait_methods
                    .insert(key.clone(), trait_method_desc_fact(registry, method));
                facts.trait_method_signatures.insert(
                    key.clone(),
                    trait_method_desc_signature_fact(registry, method),
                );
                facts
                    .trait_method_effects
                    .insert(key.clone(), RegistryEffectFact::pure());
                if let Some(docs) = &method.docs {
                    facts.trait_method_docs.insert(key, docs.clone());
                }
            }
        }

        collect_trait_methods(registry, &mut facts);
        facts.rebuild_owner_indexes();

        facts
    }
}

fn function_effect_fact(effects: &FunctionEffectSet) -> RegistryEffectFact {
    RegistryEffectFact {
        reads_host: effects.reads_host,
        writes_host: effects.writes_host,
        emits_events: effects.emits_events,
        reads_time: effects.reads_time,
        uses_random: effects.uses_random,
        reads_io: effects.reads_io,
        writes_io: effects.writes_io,
        reads_reflection: effects.reads_reflection,
        writes_reflection: effects.writes_reflection,
        calls_reflection: effects.calls_reflection,
    }
}

fn method_effect_fact(effects: &MethodEffectSet) -> RegistryEffectFact {
    RegistryEffectFact {
        reads_host: effects.reads_host,
        writes_host: effects.writes_host,
        emits_events: effects.emits_events,
        reads_time: effects.reads_time,
        uses_random: effects.uses_random,
        reads_io: effects.reads_io,
        writes_io: effects.writes_io,
        reads_reflection: effects.reads_reflection,
        writes_reflection: effects.writes_reflection,
        calls_reflection: effects.calls_reflection,
    }
}

fn function_desc_fact(registry: &TypeRegistry, desc: &FunctionDesc) -> TypeFact {
    let params = desc
        .params
        .iter()
        .map(|param| {
            param
                .type_hint
                .as_deref()
                .map_or(TypeFact::Unknown, |hint| registry_hint_fact(registry, hint))
        })
        .collect();
    let returns = desc
        .return_type
        .as_deref()
        .map_or(TypeFact::Unknown, |hint| registry_hint_fact(registry, hint));
    TypeFact::function(params, returns)
}

fn function_desc_signature_fact(
    registry: &TypeRegistry,
    desc: &FunctionDesc,
) -> CallableSignatureFact {
    let parameters = desc.params.iter().map(|parameter| {
        reflected_parameter_fact(
            &parameter.name,
            parameter.type_hint.as_deref(),
            parameter.has_default,
            registry,
        )
    });
    CallableSignatureFact::new(
        parameters,
        desc.return_type
            .as_deref()
            .map_or(TypeFact::Unknown, |hint| registry_hint_fact(registry, hint)),
    )
    .asyncness(desc.asyncness)
}

fn method_desc_fact(registry: &TypeRegistry, desc: &MethodDesc) -> TypeFact {
    let params = desc
        .params
        .iter()
        .map(|param| {
            param
                .type_hint
                .as_deref()
                .map_or(TypeFact::Unknown, |hint| registry_hint_fact(registry, hint))
        })
        .collect();
    let returns = desc
        .return_type
        .as_deref()
        .map_or(TypeFact::Unknown, |hint| registry_hint_fact(registry, hint));
    TypeFact::function(params, returns)
}

fn method_desc_signature_fact(registry: &TypeRegistry, desc: &MethodDesc) -> CallableSignatureFact {
    let parameters = desc.params.iter().map(|parameter| {
        reflected_parameter_fact(
            &parameter.name,
            parameter.type_hint.as_deref(),
            parameter.has_default,
            registry,
        )
    });
    CallableSignatureFact::new(
        parameters,
        desc.return_type
            .as_deref()
            .map_or(TypeFact::Unknown, |hint| registry_hint_fact(registry, hint)),
    )
    .asyncness(desc.asyncness)
}

fn trait_method_desc_fact(registry: &TypeRegistry, desc: &TraitMethodDesc) -> TypeFact {
    let params = desc
        .params
        .iter()
        .map(|param| {
            param
                .type_hint
                .as_deref()
                .map_or(TypeFact::Unknown, |hint| registry_hint_fact(registry, hint))
        })
        .collect();
    let returns = desc
        .return_type
        .as_deref()
        .map_or(TypeFact::Unknown, |hint| registry_hint_fact(registry, hint));
    TypeFact::function(params, returns)
}

fn trait_method_desc_signature_fact(
    registry: &TypeRegistry,
    desc: &TraitMethodDesc,
) -> CallableSignatureFact {
    let parameters = desc.params.iter().map(|parameter| {
        reflected_parameter_fact(
            &parameter.name,
            parameter.type_hint.as_deref(),
            parameter.has_default,
            registry,
        )
    });
    CallableSignatureFact::new(
        parameters,
        desc.return_type
            .as_deref()
            .map_or(TypeFact::Unknown, |hint| registry_hint_fact(registry, hint)),
    )
    .asyncness(desc.asyncness)
}

fn reflected_parameter_fact(
    name: &str,
    type_hint: Option<&str>,
    has_default: bool,
    registry: &TypeRegistry,
) -> CallableParameterFact {
    CallableParameterFact::new(
        name,
        type_hint.map_or(TypeFact::Unknown, |hint| registry_hint_fact(registry, hint)),
        if has_default {
            CallableParameterRequirementFact::Defaulted
        } else {
            CallableParameterRequirementFact::Required
        },
    )
}

fn collect_trait_methods(registry: &TypeRegistry, facts: &mut RegistryFacts) {
    for type_desc in registry.types() {
        for trait_desc in &type_desc.traits {
            for method in &trait_desc.methods {
                let key = (trait_desc.name.clone(), method.name.clone());
                facts
                    .trait_methods
                    .insert(key.clone(), trait_method_desc_fact(registry, method));
                facts.trait_method_signatures.insert(
                    key.clone(),
                    trait_method_desc_signature_fact(registry, method),
                );
                facts
                    .trait_method_effects
                    .insert(key.clone(), RegistryEffectFact::pure());
                if let Some(docs) = &method.docs {
                    facts.trait_method_docs.insert(key, docs.clone());
                }
            }
        }
    }
}

fn declaration_order_u32(index: usize) -> u32 {
    u32::try_from(index).expect("reflection field declaration order exceeds u32::MAX")
}
