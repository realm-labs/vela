use std::collections::BTreeMap;

use vela_common::PrimitiveTag;
use vela_reflect::access::{FunctionEffectSet, MethodAccess, MethodEffectSet};
use vela_reflect::modules::FunctionDesc;
use vela_reflect::registry::{
    FieldDesc, MethodDesc, TraitMethodDesc, TypeDesc, TypeKind, TypeRegistry,
};

use crate::type_fact::TypeFact;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryMemberFact {
    pub owner: String,
    pub name: String,
    pub fact: TypeFact,
}

impl RegistryMemberFact {
    fn new(owner: impl Into<String>, name: impl Into<String>, fact: TypeFact) -> Self {
        Self {
            owner: owner.into(),
            name: name.into(),
            fact,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryFieldAccessFact {
    pub owner: String,
    pub name: String,
    pub readable: bool,
    pub writable: bool,
    pub reflect_readable: bool,
    pub reflect_writable: bool,
    pub required_permissions: Vec<String>,
}

impl RegistryFieldAccessFact {
    fn new(owner: impl Into<String>, name: impl Into<String>, field: &FieldDesc) -> Self {
        Self {
            owner: owner.into(),
            name: name.into(),
            readable: field.access.readable,
            writable: field.access.writable,
            reflect_readable: field.access.reflect_readable,
            reflect_writable: field.access.reflect_writable,
            required_permissions: field.access.required_permissions().to_vec(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryMethodAccessFact {
    pub owner: String,
    pub name: String,
    pub public: bool,
    pub reflect_callable: bool,
    pub required_permissions: Vec<String>,
}

impl RegistryMethodAccessFact {
    fn new(owner: impl Into<String>, name: impl Into<String>, access: &MethodAccess) -> Self {
        Self {
            owner: owner.into(),
            name: name.into(),
            public: access.public,
            reflect_callable: access.reflect_callable,
            required_permissions: access.required_permissions().to_vec(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryFunctionFact {
    pub name: String,
    pub fact: TypeFact,
}

impl RegistryFunctionFact {
    fn new(name: impl Into<String>, fact: TypeFact) -> Self {
        Self {
            name: name.into(),
            fact,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RegistryEffectFact {
    pub reads_host: bool,
    pub writes_host: bool,
    pub emits_events: bool,
    pub reads_time: bool,
    pub uses_random: bool,
    pub reads_io: bool,
    pub writes_io: bool,
    pub reads_reflection: bool,
    pub writes_reflection: bool,
    pub calls_reflection: bool,
}

impl RegistryEffectFact {
    #[must_use]
    pub const fn pure() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: false,
            reads_time: false,
            uses_random: false,
            reads_io: false,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    #[must_use]
    pub const fn host_read() -> Self {
        Self {
            reads_host: true,
            writes_host: false,
            emits_events: false,
            reads_time: false,
            uses_random: false,
            reads_io: false,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    #[must_use]
    pub const fn host_write() -> Self {
        Self {
            reads_host: true,
            writes_host: true,
            emits_events: false,
            reads_time: false,
            uses_random: false,
            reads_io: false,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    #[must_use]
    pub const fn event_emit() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: true,
            reads_time: false,
            uses_random: false,
            reads_io: false,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    #[must_use]
    pub fn denied_by(&self, allowed: &Self) -> Vec<&'static str> {
        self.effect_flags()
            .into_iter()
            .zip(allowed.effect_flags())
            .filter_map(|((name, required), (_, allowed))| (required && !allowed).then_some(name))
            .collect()
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        let effects = self
            .effect_flags()
            .into_iter()
            .filter_map(|(name, enabled)| enabled.then_some(name))
            .collect::<Vec<_>>();
        if effects.is_empty() {
            "pure".to_owned()
        } else {
            effects.join(", ")
        }
    }

    fn effect_flags(&self) -> [(&'static str, bool); 10] {
        [
            ("reads_host", self.reads_host && !self.writes_host),
            ("writes_host", self.writes_host),
            ("emits_events", self.emits_events),
            ("reads_time", self.reads_time),
            ("uses_random", self.uses_random),
            ("reads_io", self.reads_io),
            ("writes_io", self.writes_io),
            ("reads_reflection", self.reads_reflection),
            ("writes_reflection", self.writes_reflection),
            ("calls_reflection", self.calls_reflection),
        ]
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RegistryFacts {
    types: BTreeMap<String, TypeFact>,
    traits: BTreeMap<String, TypeFact>,
    fields: BTreeMap<(String, String), TypeFact>,
    field_access: BTreeMap<(String, String), RegistryFieldAccessFact>,
    variants: BTreeMap<(String, String), TypeFact>,
    methods: BTreeMap<(String, String), TypeFact>,
    trait_methods: BTreeMap<(String, String), TypeFact>,
    functions: BTreeMap<String, TypeFact>,
    method_effects: BTreeMap<(String, String), RegistryEffectFact>,
    method_access: BTreeMap<(String, String), RegistryMethodAccessFact>,
    trait_method_effects: BTreeMap<(String, String), RegistryEffectFact>,
    function_effects: BTreeMap<String, RegistryEffectFact>,
}

impl RegistryFacts {
    #[must_use]
    pub fn from_registry(registry: &TypeRegistry) -> Self {
        let mut facts = Self::default();

        for desc in registry.types() {
            let type_fact = type_desc_fact(desc);
            facts.types.insert(desc.key.name.clone(), type_fact.clone());

            for field in &desc.fields {
                let key = (desc.key.name.clone(), field.name.clone());
                facts.fields.insert(
                    key.clone(),
                    field
                        .type_hint
                        .as_deref()
                        .map_or(TypeFact::Unknown, |hint| registry_hint_fact(registry, hint)),
                );
                facts.field_access.insert(
                    key,
                    RegistryFieldAccessFact::new(&desc.key.name, &field.name, field),
                );
            }

            for method in &desc.methods {
                let key = (desc.key.name.clone(), method.name.clone());
                facts
                    .methods
                    .insert(key.clone(), method_desc_fact(registry, method));
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
            }

            for variant in &desc.variants {
                facts.variants.insert(
                    (desc.key.name.clone(), variant.name.clone()),
                    TypeFact::enum_type(&desc.key.name, Some(&variant.name)),
                );
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
                    facts
                        .field_access
                        .insert(key, RegistryFieldAccessFact::new(owner, &field.name, field));
                }
            }
        }

        for function in registry.functions() {
            facts.functions.insert(
                function.name.clone(),
                function_desc_fact(registry, function),
            );
            facts.function_effects.insert(
                function.name.clone(),
                function_effect_fact(&function.effects),
            );
        }

        for trait_desc in registry.traits() {
            facts
                .traits
                .entry(trait_desc.name.clone())
                .or_insert_with(|| TypeFact::trait_type(&trait_desc.name));
            for method in &trait_desc.methods {
                facts.trait_methods.insert(
                    (trait_desc.name.clone(), method.name.clone()),
                    trait_method_desc_fact(registry, method),
                );
                facts.trait_method_effects.insert(
                    (trait_desc.name.clone(), method.name.clone()),
                    RegistryEffectFact::pure(),
                );
            }
        }

        collect_trait_methods(registry, &mut facts);

        facts
    }

    #[must_use]
    pub fn type_fact(&self, name: &str) -> Option<&TypeFact> {
        self.types.get(name)
    }

    pub fn types(&self) -> impl Iterator<Item = (&str, &TypeFact)> {
        self.types.iter().map(|(name, fact)| (name.as_str(), fact))
    }

    #[must_use]
    pub fn trait_fact(&self, name: &str) -> Option<&TypeFact> {
        self.traits.get(name)
    }

    pub fn traits(&self) -> impl Iterator<Item = (&str, &TypeFact)> {
        self.traits.iter().map(|(name, fact)| (name.as_str(), fact))
    }

    #[must_use]
    pub fn field_fact(&self, owner: &str, field: &str) -> Option<&TypeFact> {
        self.fields.get(&(owner.to_owned(), field.to_owned()))
    }

    #[must_use]
    pub fn field_access_fact(&self, owner: &str, field: &str) -> Option<&RegistryFieldAccessFact> {
        self.field_access.get(&(owner.to_owned(), field.to_owned()))
    }

    pub fn fields(&self) -> impl Iterator<Item = RegistryMemberFact> + '_ {
        self.fields
            .iter()
            .map(|((owner, name), fact)| RegistryMemberFact::new(owner, name, fact.clone()))
    }

    #[must_use]
    pub fn variant_fact(&self, owner: &str, variant: &str) -> Option<&TypeFact> {
        self.variants.get(&(owner.to_owned(), variant.to_owned()))
    }

    pub fn variant_names(&self, owner: &str) -> Vec<String> {
        self.variants
            .keys()
            .filter_map(|(variant_owner, variant)| {
                if variant_owner == owner {
                    Some(variant.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn variants(&self) -> impl Iterator<Item = RegistryMemberFact> + '_ {
        self.variants
            .iter()
            .map(|((owner, name), fact)| RegistryMemberFact::new(owner, name, fact.clone()))
    }

    #[must_use]
    pub fn method_fact(&self, owner: &str, method: &str) -> Option<&TypeFact> {
        self.methods.get(&(owner.to_owned(), method.to_owned()))
    }

    #[must_use]
    pub fn method_effect_fact(&self, owner: &str, method: &str) -> Option<&RegistryEffectFact> {
        self.method_effects
            .get(&(owner.to_owned(), method.to_owned()))
    }

    #[must_use]
    pub fn method_access_fact(
        &self,
        owner: &str,
        method: &str,
    ) -> Option<&RegistryMethodAccessFact> {
        self.method_access
            .get(&(owner.to_owned(), method.to_owned()))
    }

    pub fn methods(&self) -> impl Iterator<Item = RegistryMemberFact> + '_ {
        self.methods
            .iter()
            .map(|((owner, name), fact)| RegistryMemberFact::new(owner, name, fact.clone()))
    }

    #[must_use]
    pub fn trait_method_fact(&self, trait_name: &str, method: &str) -> Option<&TypeFact> {
        self.trait_methods
            .get(&(trait_name.to_owned(), method.to_owned()))
    }

    #[must_use]
    pub fn trait_method_effect_fact(
        &self,
        trait_name: &str,
        method: &str,
    ) -> Option<&RegistryEffectFact> {
        self.trait_method_effects
            .get(&(trait_name.to_owned(), method.to_owned()))
    }

    pub fn trait_methods(&self) -> impl Iterator<Item = RegistryMemberFact> + '_ {
        self.trait_methods
            .iter()
            .map(|((owner, name), fact)| RegistryMemberFact::new(owner, name, fact.clone()))
    }

    #[must_use]
    pub fn function_fact(&self, name: &str) -> Option<&TypeFact> {
        self.functions.get(name)
    }

    #[must_use]
    pub fn function_effect_fact(&self, name: &str) -> Option<&RegistryEffectFact> {
        self.function_effects.get(name)
    }

    pub fn functions(&self) -> impl Iterator<Item = RegistryFunctionFact> + '_ {
        self.functions
            .iter()
            .map(|(name, fact)| RegistryFunctionFact::new(name, fact.clone()))
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

fn type_desc_fact(desc: &TypeDesc) -> TypeFact {
    if let Some(tag) = PrimitiveTag::from_name(&desc.key.name) {
        return TypeFact::primitive(tag);
    }

    match desc.kind {
        TypeKind::Null => TypeFact::NULL,
        TypeKind::Bool => TypeFact::BOOL,
        TypeKind::Int => TypeFact::I64,
        TypeKind::Float => TypeFact::F64,
        TypeKind::String => TypeFact::STRING,
        TypeKind::Bytes => TypeFact::BYTES,
        TypeKind::Array => TypeFact::array(TypeFact::Any),
        TypeKind::Map => TypeFact::map(TypeFact::Any, TypeFact::Any),
        TypeKind::Set => TypeFact::set(TypeFact::Any),
        TypeKind::Range => TypeFact::Range,
        TypeKind::Function | TypeKind::Closure => TypeFact::function(Vec::new(), TypeFact::Any),
        TypeKind::Host => TypeFact::host(&desc.key.name),
        TypeKind::ScriptStruct => TypeFact::record(&desc.key.name),
        TypeKind::ScriptEnum => TypeFact::enum_type(&desc.key.name, None::<String>),
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

fn registry_hint_fact(registry: &TypeRegistry, hint: &str) -> TypeFact {
    if let Some(tag) = PrimitiveTag::from_name(hint) {
        return TypeFact::primitive(tag);
    }

    match hint {
        "any" => TypeFact::Any,
        "array" => TypeFact::array(TypeFact::Unknown),
        "map" => TypeFact::map(TypeFact::Unknown, TypeFact::Unknown),
        "set" => TypeFact::set(TypeFact::Unknown),
        "function" => TypeFact::function(Vec::new(), TypeFact::Unknown),
        "Option" => TypeFact::option(TypeFact::Unknown),
        "Result" => TypeFact::result(TypeFact::Unknown, TypeFact::Unknown),
        name => registry
            .type_by_name(name)
            .map_or_else(|| trait_or_unknown(registry, name), type_desc_fact),
    }
}

fn trait_or_unknown(registry: &TypeRegistry, name: &str) -> TypeFact {
    if registry.trait_by_name(name).is_some()
        || registry
            .types()
            .flat_map(|type_desc| type_desc.traits.iter())
            .any(|trait_desc| trait_desc.name == name)
    {
        TypeFact::trait_type(name)
    } else {
        TypeFact::Unknown
    }
}

fn collect_trait_methods(registry: &TypeRegistry, facts: &mut RegistryFacts) {
    for type_desc in registry.types() {
        for trait_desc in &type_desc.traits {
            for method in &trait_desc.methods {
                facts.trait_methods.insert(
                    (trait_desc.name.clone(), method.name.clone()),
                    trait_method_desc_fact(registry, method),
                );
                facts.trait_method_effects.insert(
                    (trait_desc.name.clone(), method.name.clone()),
                    RegistryEffectFact::pure(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use vela_common::{HostMethodId, HostTypeId};
    use vela_def::{FieldId, FunctionId, MethodId, TypeId, VariantId};
    use vela_reflect::access::{MethodAccess, MethodEffectSet};
    use vela_reflect::modules::{FunctionDesc, FunctionParamDesc};
    use vela_reflect::registry::{
        FieldDesc, MethodDesc, MethodParamDesc, TraitDesc, TraitMethodDesc, TypeDesc, TypeKey,
        TypeKind, TypeRegistry, VariantDesc,
    };

    use super::*;

    #[test]
    fn registry_facts_cover_types_fields_methods_and_functions() {
        let player = TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .host_type(HostTypeId::new(1))
            .field(FieldDesc::new(FieldId::new(1), "level").type_hint("i64"))
            .field(FieldDesc::new(FieldId::new(2), "inventory").type_hint("Inventory"))
            .method(
                MethodDesc::new(HostMethodId::new(1), "grant_exp")
                    .param(MethodParamDesc::new("amount").type_hint("i64"))
                    .return_type("bool")
                    .effects(MethodEffectSet::host_write())
                    .access(MethodAccess::new().require_permission("player.reward")),
            )
            .trait_impl(
                TraitDesc::new("Damageable").method(
                    TraitMethodDesc::new(MethodId::new(1), "damage")
                        .param(MethodParamDesc::new("amount").type_hint("i64"))
                        .return_type("bool"),
                ),
            );
        let inventory = TypeDesc::new(TypeKey::new(TypeId::new(2), "Inventory"))
            .kind(TypeKind::ScriptStruct)
            .field(FieldDesc::new(FieldId::new(1), "items").type_hint("map"));
        let quest = TypeDesc::new(TypeKey::new(TypeId::new(3), "QuestState"))
            .kind(TypeKind::ScriptEnum)
            .variant(
                VariantDesc::new(VariantId::new(1), "Active")
                    .field(FieldDesc::new(FieldId::new(1), "quest_id").type_hint("string")),
            );

        let mut registry = TypeRegistry::new();
        registry.register(player);
        registry.register(inventory);
        registry.register(quest);
        registry.register_function(
            FunctionDesc::new(FunctionId::new(1), "game::reward::grant")
                .param(FunctionParamDesc::new("player").type_hint("Player"))
                .param(FunctionParamDesc::new("amount").type_hint("i64"))
                .return_type("bool"),
        );

        let facts = RegistryFacts::from_registry(&registry);

        assert_eq!(facts.type_fact("Player"), Some(&TypeFact::host("Player")));
        assert_eq!(
            facts.type_fact("Inventory"),
            Some(&TypeFact::record("Inventory"))
        );
        assert_eq!(
            facts.type_fact("QuestState"),
            Some(&TypeFact::enum_type("QuestState", None::<String>))
        );
        assert_eq!(facts.field_fact("Player", "level"), Some(&TypeFact::I64));
        assert_eq!(
            facts.field_fact("Player", "inventory"),
            Some(&TypeFact::record("Inventory"))
        );
        assert!(
            facts
                .field_access_fact("Player", "level")
                .is_some_and(|access| !access.writable && access.readable)
        );
        assert_eq!(
            facts.field_fact("QuestState::Active", "quest_id"),
            Some(&TypeFact::STRING)
        );
        assert_eq!(
            facts.method_fact("Player", "grant_exp"),
            Some(&TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL))
        );
        assert_eq!(
            facts.method_effect_fact("Player", "grant_exp"),
            Some(&RegistryEffectFact::host_write())
        );
        assert!(
            facts
                .method_access_fact("Player", "grant_exp")
                .is_some_and(|access| access.reflect_callable
                    && access.required_permissions == vec!["player.reward".to_owned()])
        );
        assert_eq!(
            facts.function_fact("game::reward::grant"),
            Some(&TypeFact::function(
                vec![TypeFact::host("Player"), TypeFact::I64],
                TypeFact::BOOL,
            ))
        );
        assert_eq!(
            facts.trait_method_fact("Damageable", "damage"),
            Some(&TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL))
        );
    }

    #[test]
    fn unknown_registry_hints_degrade_without_blocking_analysis() {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .field(FieldDesc::new(FieldId::new(1), "mystery").type_hint("MissingType")),
        );

        let facts = RegistryFacts::from_registry(&registry);

        assert_eq!(
            facts.field_fact("Player", "mystery"),
            Some(&TypeFact::Unknown)
        );
    }

    #[test]
    fn registry_facts_cover_builtin_type_kinds_without_generics() {
        let mut registry = TypeRegistry::new();
        for (id, name, kind) in [
            (10, "null", TypeKind::Null),
            (11, "bool", TypeKind::Bool),
            (12, "i64", TypeKind::Int),
            (13, "f64", TypeKind::Float),
            (14, "string", TypeKind::String),
            (15, "array", TypeKind::Array),
            (16, "map", TypeKind::Map),
            (17, "set", TypeKind::Set),
            (18, "range", TypeKind::Range),
            (19, "function", TypeKind::Function),
            (20, "closure", TypeKind::Closure),
        ] {
            registry.register(TypeDesc::new(TypeKey::new(TypeId::new(id), name)).kind(kind));
        }

        let facts = RegistryFacts::from_registry(&registry);

        assert_eq!(facts.type_fact("null"), Some(&TypeFact::NULL));
        assert_eq!(facts.type_fact("bool"), Some(&TypeFact::BOOL));
        assert_eq!(facts.type_fact("i64"), Some(&TypeFact::I64));
        assert_eq!(facts.type_fact("f64"), Some(&TypeFact::F64));
        assert_eq!(facts.type_fact("string"), Some(&TypeFact::STRING));
        assert_eq!(
            facts.type_fact("array"),
            Some(&TypeFact::array(TypeFact::Any))
        );
        assert_eq!(
            facts.type_fact("map"),
            Some(&TypeFact::map(TypeFact::Any, TypeFact::Any))
        );
        assert_eq!(facts.type_fact("set"), Some(&TypeFact::set(TypeFact::Any)));
        assert_eq!(facts.type_fact("range"), Some(&TypeFact::Range));
        assert_eq!(
            facts.type_fact("function"),
            Some(&TypeFact::function(Vec::new(), TypeFact::Any))
        );
        assert_eq!(
            facts.type_fact("closure"),
            Some(&TypeFact::function(Vec::new(), TypeFact::Any))
        );
    }

    #[test]
    fn registry_facts_cover_registered_trait_methods() {
        let mut registry = TypeRegistry::new();
        registry.register_trait(
            TraitDesc::new("Rewardable").method(
                TraitMethodDesc::new(MethodId::new(9), "reward")
                    .param(MethodParamDesc::new("amount").type_hint("i64"))
                    .return_type("Result"),
            ),
        );

        let facts = RegistryFacts::from_registry(&registry);

        assert_eq!(
            facts.trait_fact("Rewardable"),
            Some(&TypeFact::trait_type("Rewardable"))
        );
        assert_eq!(
            facts.trait_method_fact("Rewardable", "reward"),
            Some(&TypeFact::function(
                vec![TypeFact::I64],
                TypeFact::result(TypeFact::Unknown, TypeFact::Unknown),
            ))
        );
    }
}
