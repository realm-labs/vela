use vela_reflect::access::{FunctionEffectSet, MethodEffectSet};
use vela_reflect::modules::FunctionDesc;
use vela_reflect::registry::{MethodDesc, TraitMethodDesc, TypeKind, TypeRegistry};

use super::{
    RegistryEffectFact, RegistryFacts, RegistryFieldAccessFact, RegistryFieldTargetFact,
    RegistryIndexCapabilityFact, RegistryMethodAccessFact, RegistryModuleFact,
    RegistryTypeTargetFact, registry_hint_fact, type_desc_fact,
};
use crate::type_fact::TypeFact;

impl RegistryFacts {
    #[must_use]
    pub fn from_registry(registry: &TypeRegistry) -> Self {
        let mut facts = Self::default();

        for desc in registry.types() {
            let type_fact = type_desc_fact(desc);
            facts.types.insert(desc.key.name.clone(), type_fact.clone());
            facts.type_targets.insert(
                desc.key.name.clone(),
                RegistryTypeTargetFact::new(&desc.key.name, desc.key.id, desc.host_type_id),
            );
            if let Some(docs) = &desc.docs {
                facts.type_docs.insert(desc.key.name.clone(), docs.clone());
            }
            if let Some(capability) = &desc.index_capability {
                facts.index_capabilities.insert(
                    desc.key.name.clone(),
                    RegistryIndexCapabilityFact::new(&desc.key.name, capability, registry),
                );
            }

            for field in &desc.fields {
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
                    ),
                );
            }

            for method in &desc.methods {
                let key = (desc.key.name.clone(), method.name.clone());
                facts
                    .methods
                    .insert(key.clone(), method_desc_fact(registry, method));
                if let Some(docs) = &method.docs {
                    facts.method_docs.insert(key.clone(), docs.clone());
                }
                facts
                    .method_effects
                    .insert(key.clone(), method_effect_fact(&method.effects));
                facts.method_access.insert(
                    key,
                    RegistryMethodAccessFact::new(&desc.key.name, &method.name, &method.access),
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
                for field in &variant.fields {
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
                        ),
                    );
                }
            }
        }

        for function in registry.functions() {
            facts.functions.insert(
                function.name.clone(),
                function_desc_fact(registry, function),
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

fn collect_trait_methods(registry: &TypeRegistry, facts: &mut RegistryFacts) {
    for type_desc in registry.types() {
        for trait_desc in &type_desc.traits {
            for method in &trait_desc.methods {
                let key = (trait_desc.name.clone(), method.name.clone());
                facts
                    .trait_methods
                    .insert(key.clone(), trait_method_desc_fact(registry, method));
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
