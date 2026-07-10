use std::collections::{BTreeMap, BTreeSet};

use vela_common::PrimitiveTag;
use vela_def::{DefPath, TypeId};
use vela_reflect::modules::DeclOrigin;
use vela_registry::{Def, EffectSet, FunctionSignature, RegistryCompileView, TypeDef, TypeHintDef};

use super::{RegistryEffectFact, RegistryFacts, RegistryFieldAccessFact};
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
    enum_types: BTreeSet<TypeId>,
}

impl<'registry> CompileViewFacts<'registry> {
    fn new(registry: RegistryCompileView<'registry>) -> Self {
        let enum_types = registry
            .definitions()
            .filter_map(|definition| match definition {
                Def::Variant(variant) => Some(variant.owner),
                _ => None,
            })
            .collect();
        Self {
            registry,
            type_names: BTreeMap::new(),
            type_facts: BTreeMap::new(),
            enum_types,
        }
    }

    fn build(mut self) -> RegistryFacts {
        self.collect_types();
        let mut facts = RegistryFacts::default();
        for (name, fact) in &self.type_facts {
            facts.insert_type(name, fact.clone());
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
                }
                Def::Function(function) => {
                    let name = source_name(&function.path);
                    facts.insert_function(&name, self.signature_fact(&function.signature));
                    facts.insert_function_effect(&name, definition_effect_fact(function.effects));
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
            let fact =
                registered_type_fact(definition, &name, self.enum_types.contains(&definition.id));
            self.type_names.insert(definition.id, name.clone());
            self.type_facts.insert(name.clone(), fact.clone());
            if name != definition.path.name
                && short_name_counts.get(&definition.path.name) == Some(&1)
            {
                self.type_facts.insert(definition.path.name.clone(), fact);
            }
        }
    }

    fn insert_field(&self, facts: &mut RegistryFacts, field: &vela_registry::FieldDef) {
        let Some(type_owner) = self.type_names.get(&field.owner) else {
            return;
        };
        let owner = if field.variant_field {
            field
                .path
                .owner
                .as_deref()
                .and_then(|owner| owner.rsplit("::").next())
                .map_or_else(
                    || type_owner.clone(),
                    |variant| format!("{type_owner}::{variant}"),
                )
        } else {
            type_owner.clone()
        };
        let fact = field
            .type_hint
            .as_ref()
            .map_or(TypeFact::Unknown, |hint| self.type_hint_fact(hint));
        facts.insert_field(&owner, &field.path.name, fact);
        facts.insert_field_access(RegistryFieldAccessFact {
            owner,
            name: field.path.name.clone(),
            readable: true,
            writable: field.writable,
            reflect_readable: false,
            reflect_writable: false,
            required_permissions: Vec::new(),
        });
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

fn registered_type_fact(definition: &TypeDef, name: &str, is_enum: bool) -> TypeFact {
    if let Some(primitive) = definition.primitive {
        return TypeFact::primitive(primitive);
    }
    if definition.path.package == "host" || definition.host_runtime_id.is_some() {
        return TypeFact::host(name);
    }
    match definition.path.name.as_str() {
        "Any" => TypeFact::Any,
        "Array" => TypeFact::array(TypeFact::Any),
        "Map" => TypeFact::map(TypeFact::Any, TypeFact::Any),
        "Set" => TypeFact::set(TypeFact::Any),
        "Iterator" => TypeFact::iterator(TypeFact::Any),
        "Range" => TypeFact::Range,
        "Function" | "Closure" => TypeFact::function(Vec::new(), TypeFact::Any),
        "Option" => TypeFact::option(TypeFact::Any),
        "Result" => TypeFact::result(TypeFact::Any, TypeFact::Any),
        _ if is_enum => TypeFact::enum_type(name, None::<String>),
        _ => TypeFact::record(name),
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
        writes_reflection: false,
        calls_reflection: effects.reflection_call,
    }
}
