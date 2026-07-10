use std::collections::BTreeMap;

use vela_common::{HostTypeId, PrimitiveTag};
use vela_def::{DefPath, FieldId, TypeId, VariantId};
use vela_reflect::modules::DeclOrigin;
use vela_registry::{
    Def, EffectSet, FunctionSignature, RegistryCompileView, TypeDef, TypeHintDef, TypeKindDef,
};

use super::{
    RegistryEffectFact, RegistryFacts, RegistryFieldAccessFact, RegistryFieldTargetFact,
    RegistryFunctionAccessFact, RegistryIndexCapabilityFact, RegistryMethodAccessFact,
    RegistryTypeTargetFact,
};
use crate::type_fact::TypeFact;

impl RegistryFacts {
    /// Builds analysis schema facts from the same backend-neutral definition
    /// metadata used by production compilation.
    #[must_use]
    pub fn from_compile_view(registry: RegistryCompileView<'_>) -> Self {
        CompileViewFacts::new(registry).build()
    }
}

struct CompileViewFacts<'registry> {
    registry: RegistryCompileView<'registry>,
    type_names: BTreeMap<TypeId, String>,
    type_facts: BTreeMap<String, TypeFact>,
    type_targets: BTreeMap<String, (TypeId, Option<u128>)>,
    variant_names: BTreeMap<VariantId, String>,
}

impl<'registry> CompileViewFacts<'registry> {
    fn new(registry: RegistryCompileView<'registry>) -> Self {
        let variant_names = registry
            .definitions()
            .filter_map(|definition| match definition {
                Def::Variant(variant) => Some((variant.id, variant.path.name.clone())),
                _ => None,
            })
            .collect();
        Self {
            registry,
            type_names: BTreeMap::new(),
            type_facts: BTreeMap::new(),
            type_targets: BTreeMap::new(),
            variant_names,
        }
    }

    fn build(mut self) -> RegistryFacts {
        self.collect_types();
        let mut facts = RegistryFacts::default();
        for (name, fact) in &self.type_facts {
            facts.insert_type(name, fact.clone());
        }
        for (name, (semantic, host_runtime)) in &self.type_targets {
            facts.insert_type_target(RegistryTypeTargetFact::new(
                name,
                *semantic,
                host_runtime.map(host_type_id),
            ));
        }
        for definition in self.registry.definitions() {
            let Def::Type(definition) = definition else {
                continue;
            };
            let Some(capability) = &definition.index_capability else {
                continue;
            };
            let key = capability
                .key_type
                .as_ref()
                .map_or(TypeFact::Unknown, |hint| self.type_hint_fact(hint));
            let value = capability
                .value_type
                .as_ref()
                .map_or(TypeFact::Unknown, |hint| self.type_hint_fact(hint));
            for owner in self
                .type_targets
                .iter()
                .filter_map(|(name, target)| (target.0 == definition.id).then_some(name))
            {
                facts.insert_index_capability(RegistryIndexCapabilityFact {
                    owner: owner.clone(),
                    readable: capability.readable,
                    writable: capability.writable,
                    addable: capability.addable,
                    removable: capability.removable,
                    key: key.clone(),
                    value: value.clone(),
                });
            }
        }

        for definition in self.registry.definitions() {
            match definition {
                Def::Field(field) => self.insert_field(&mut facts, field),
                Def::Method(method) => {
                    let Some(owner) = self.type_names.get(&method.owner) else {
                        continue;
                    };
                    facts.insert_method(
                        owner,
                        &method.path.name,
                        self.signature_fact(&method.signature),
                    );
                    facts.insert_method_effect(
                        owner,
                        &method.path.name,
                        definition_effect_fact(method.effects),
                    );
                    facts.insert_method_access(RegistryMethodAccessFact {
                        owner: owner.clone(),
                        name: method.path.name.clone(),
                        public: method.access.public,
                        reflect_callable: method.access.reflect_callable,
                        required_permissions: method.access.required_permissions().to_vec(),
                    });
                }
                Def::Function(function) => {
                    let name = source_name(&function.path);
                    facts.insert_function(&name, self.signature_fact(&function.signature));
                    facts.insert_function_effect(&name, definition_effect_fact(function.effects));
                    facts.insert_function_access(RegistryFunctionAccessFact {
                        name: name.clone(),
                        public: function.access.public,
                        reflect_visible: function.access.reflect_visible,
                        reflect_callable: function.access.reflect_callable,
                    });
                    facts.insert_function_origin(
                        name,
                        if function.path.package == "host" {
                            DeclOrigin::Host
                        } else {
                            DeclOrigin::Script
                        },
                    );
                }
                Def::Variant(variant) => {
                    let Some(owner) = self.type_names.get(&variant.owner) else {
                        continue;
                    };
                    facts.insert_variant(
                        owner,
                        &variant.path.name,
                        TypeFact::enum_type(owner, Some(&variant.path.name)),
                    );
                }
                Def::Trait(trait_def) => {
                    let name = source_name(&trait_def.path);
                    facts.insert_trait(&name, TypeFact::trait_type(&name));
                }
                Def::Type(_) => {}
            }
        }
        facts
    }

    fn collect_types(&mut self) {
        let types = self
            .registry
            .definitions()
            .filter_map(|definition| match definition {
                Def::Type(definition) => Some(definition),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut short_name_counts = BTreeMap::<String, usize>::new();
        for definition in &types {
            *short_name_counts
                .entry(definition.path.name.clone())
                .or_default() += 1;
        }
        for definition in types {
            let name = source_name(&definition.path);
            let fact = registered_type_fact(definition, &name);
            self.type_names.insert(definition.id, name.clone());
            self.type_facts.insert(name.clone(), fact.clone());
            self.type_targets
                .insert(name.clone(), (definition.id, definition.host_runtime_id));
            if name != definition.path.name
                && short_name_counts.get(&definition.path.name) == Some(&1)
            {
                self.type_facts.insert(definition.path.name.clone(), fact);
                self.type_targets.insert(
                    definition.path.name.clone(),
                    (definition.id, definition.host_runtime_id),
                );
            }
        }
    }

    fn insert_field(&self, facts: &mut RegistryFacts, field: &vela_registry::FieldDef) {
        let Some(type_owner) = self.type_names.get(&field.owner) else {
            return;
        };
        let owner = if let Some(variant) = field.variant {
            let Some(variant) = self.variant_names.get(&variant) else {
                return;
            };
            format!("{type_owner}::{variant}")
        } else {
            type_owner.clone()
        };
        let fact = field
            .type_hint
            .as_ref()
            .map_or(TypeFact::Unknown, |hint| self.type_hint_fact(hint));
        facts.insert_field(&owner, &field.path.name, fact);
        let access = RegistryFieldAccessFact {
            owner: owner.clone(),
            name: field.path.name.clone(),
            readable: field.access.readable,
            writable: field.access.writable,
            reflect_readable: field.access.reflect_readable,
            reflect_writable: field.access.reflect_writable,
            required_permissions: field.access.required_permissions().to_vec(),
        };
        facts.insert_field_access(access.clone());
        facts.insert_field_target(
            RegistryFieldTargetFact::new(
                field.owner,
                owner,
                &field.path.name,
                field.id,
                field.host_runtime_id.map(FieldId::new),
                field.variant.is_some(),
                access,
            )
            .declaration_order(field.declaration_order)
            .defaulted(field.has_default),
        );
    }

    fn signature_fact(&self, signature: &FunctionSignature) -> TypeFact {
        let params = signature
            .params
            .iter()
            .map(|param| {
                param
                    .type_hint
                    .as_ref()
                    .map_or(TypeFact::Unknown, |hint| self.type_hint_fact(hint))
            })
            .collect();
        let returns = signature
            .return_type
            .as_ref()
            .map_or(TypeFact::Unknown, |hint| self.type_hint_fact(hint));
        TypeFact::function(params, returns)
    }

    fn type_hint_fact(&self, hint: &TypeHintDef) -> TypeFact {
        let name = hint.path.join("::");
        match (name.as_str(), hint.args.as_slice()) {
            ("()", []) => TypeFact::UNIT,
            ("()", elements) if elements.len() >= 2 => {
                TypeFact::tuple(elements.iter().map(|element| self.type_hint_fact(element)))
            }
            ("Any", []) => TypeFact::Any,
            ("String", []) => TypeFact::STRING,
            ("Bytes", []) => TypeFact::BYTES,
            ("Array", []) => TypeFact::array(TypeFact::Unknown),
            ("Array", [element]) => TypeFact::array(self.type_hint_fact(element)),
            ("Map", []) => TypeFact::map(TypeFact::Unknown, TypeFact::Unknown),
            ("Map", [key, value]) => {
                TypeFact::map(self.type_hint_fact(key), self.type_hint_fact(value))
            }
            ("Set", []) => TypeFact::set(TypeFact::Unknown),
            ("Set", [element]) => TypeFact::set(self.type_hint_fact(element)),
            ("Iterator", []) => TypeFact::iterator(TypeFact::Unknown),
            ("Iterator", [item]) => TypeFact::iterator(self.type_hint_fact(item)),
            ("Function", []) => TypeFact::function(Vec::new(), TypeFact::Unknown),
            ("Option", []) => TypeFact::option(TypeFact::Unknown),
            ("Option", [some]) => TypeFact::option(self.type_hint_fact(some)),
            ("Result", []) => TypeFact::result(TypeFact::Unknown, TypeFact::Unknown),
            ("Result", [ok, err]) => {
                TypeFact::result(self.type_hint_fact(ok), self.type_hint_fact(err))
            }
            (name, []) => PrimitiveTag::from_name(name)
                .map(TypeFact::primitive)
                .or_else(|| self.type_facts.get(name).cloned())
                .unwrap_or(TypeFact::Unknown),
            _ => TypeFact::Unknown,
        }
    }
}

fn source_name(path: &DefPath) -> String {
    path.module
        .iter()
        .chain(std::iter::once(&path.name))
        .cloned()
        .collect::<Vec<_>>()
        .join("::")
}

fn registered_type_fact(definition: &TypeDef, name: &str) -> TypeFact {
    if let Some(primitive) = definition.primitive {
        return TypeFact::primitive(primitive);
    }
    if definition.path.package == "std" {
        match definition.path.name.as_str() {
            "Option" => return TypeFact::option(TypeFact::Any),
            "Result" => return TypeFact::result(TypeFact::Any, TypeFact::Any),
            _ => {}
        }
    }
    match definition.kind {
        TypeKindDef::Unit => TypeFact::UNIT,
        TypeKindDef::Bool => TypeFact::BOOL,
        TypeKindDef::I8 => TypeFact::I8,
        TypeKindDef::I16 => TypeFact::I16,
        TypeKindDef::I32 => TypeFact::I32,
        TypeKindDef::I64 => TypeFact::I64,
        TypeKindDef::U8 => TypeFact::U8,
        TypeKindDef::U16 => TypeFact::U16,
        TypeKindDef::U32 => TypeFact::U32,
        TypeKindDef::U64 => TypeFact::U64,
        TypeKindDef::F32 => TypeFact::F32,
        TypeKindDef::F64 => TypeFact::F64,
        TypeKindDef::Char => TypeFact::CHAR,
        TypeKindDef::String => TypeFact::STRING,
        TypeKindDef::Bytes => TypeFact::BYTES,
        TypeKindDef::Array => TypeFact::array(TypeFact::Any),
        TypeKindDef::Map => TypeFact::map(TypeFact::Any, TypeFact::Any),
        TypeKindDef::Set => TypeFact::set(TypeFact::Any),
        TypeKindDef::Iterator => TypeFact::iterator(TypeFact::Any),
        TypeKindDef::Range => TypeFact::Range,
        TypeKindDef::Function | TypeKindDef::Closure => {
            TypeFact::function(Vec::new(), TypeFact::Any)
        }
        TypeKindDef::Host => TypeFact::host(name),
        TypeKindDef::ScriptStruct => TypeFact::record(name),
        TypeKindDef::ScriptEnum => TypeFact::enum_type(name, None::<String>),
    }
}

fn definition_effect_fact(effects: EffectSet) -> RegistryEffectFact {
    RegistryEffectFact {
        reads_host: effects.host_read,
        writes_host: effects.host_write,
        emits_events: effects.event_emit,
        reads_time: effects.time,
        uses_random: effects.random,
        reads_io: effects.io_read,
        writes_io: effects.io_write,
        reads_reflection: effects.reflection_read,
        writes_reflection: effects.reflection_write,
        calls_reflection: effects.reflection_call,
    }
}

fn host_type_id(value: u128) -> HostTypeId {
    HostTypeId::new(u64::try_from(value).expect("host type runtime id exceeds u64::MAX"))
}
