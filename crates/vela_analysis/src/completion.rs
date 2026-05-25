use crate::{RegistryFacts, TypeFact};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionKind {
    Field,
    Method,
    Variant,
    Function,
    Type,
    Trait,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub fact: TypeFact,
}

impl CompletionItem {
    fn new(label: impl Into<String>, kind: CompletionKind, fact: TypeFact) -> Self {
        Self {
            label: label.into(),
            kind,
            fact,
        }
    }
}

pub fn member_completions(facts: &RegistryFacts, receiver: &TypeFact) -> Vec<CompletionItem> {
    match receiver {
        TypeFact::Host { name } | TypeFact::Record { name } => {
            owner_member_completions(facts, name)
        }
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => owner_field_completions(facts, &format!("{name}.{variant}")),
        TypeFact::Enum {
            name,
            variant: None,
        } => variant_completions(facts, name),
        TypeFact::Trait { name } => trait_method_completions(facts, name),
        _ => Vec::new(),
    }
}

pub fn global_completions(facts: &RegistryFacts) -> Vec<CompletionItem> {
    let mut completions = type_completions(facts);
    completions.extend(function_completions(facts));
    completions
}

pub fn type_completions(facts: &RegistryFacts) -> Vec<CompletionItem> {
    let mut completions = facts
        .types()
        .map(|(name, fact)| CompletionItem::new(name, CompletionKind::Type, fact.clone()))
        .collect::<Vec<_>>();
    completions.extend(
        facts
            .traits()
            .map(|(name, fact)| CompletionItem::new(name, CompletionKind::Trait, fact.clone())),
    );
    completions
}

fn owner_member_completions(facts: &RegistryFacts, owner: &str) -> Vec<CompletionItem> {
    let mut completions = owner_field_completions(facts, owner);
    completions.extend(
        facts
            .methods()
            .filter(|method| method.owner == owner)
            .map(|method| CompletionItem::new(method.name, CompletionKind::Method, method.fact)),
    );
    completions
}

fn owner_field_completions(facts: &RegistryFacts, owner: &str) -> Vec<CompletionItem> {
    facts
        .fields()
        .filter(|field| field.owner == owner)
        .map(|field| CompletionItem::new(field.name, CompletionKind::Field, field.fact))
        .collect()
}

fn variant_completions(facts: &RegistryFacts, owner: &str) -> Vec<CompletionItem> {
    facts
        .variants()
        .filter(|variant| variant.owner == owner)
        .map(|variant| CompletionItem::new(variant.name, CompletionKind::Variant, variant.fact))
        .collect()
}

fn trait_method_completions(facts: &RegistryFacts, owner: &str) -> Vec<CompletionItem> {
    facts
        .trait_methods()
        .filter(|method| method.owner == owner)
        .map(|method| CompletionItem::new(method.name, CompletionKind::Method, method.fact))
        .collect()
}

fn function_completions(facts: &RegistryFacts) -> Vec<CompletionItem> {
    facts
        .functions()
        .map(|function| CompletionItem::new(function.name, CompletionKind::Function, function.fact))
        .collect()
}

#[cfg(test)]
mod tests {
    use vela_common::{FieldId, FunctionId, HostMethodId, MethodId, TypeId, VariantId};
    use vela_reflect::{
        FieldDesc, FunctionDesc, MethodDesc, MethodParamDesc, TraitDesc, TraitMethodDesc, TypeDesc,
        TypeKey, TypeKind, TypeRegistry, VariantDesc,
    };

    use super::*;

    #[test]
    fn receiver_completions_include_fields_and_methods_for_host_or_record_facts() {
        let facts = registry_facts();

        let completions = member_completions(&facts, &TypeFact::host("Player"));

        assert!(completions.contains(&CompletionItem::new(
            "level",
            CompletionKind::Field,
            TypeFact::Int,
        )));
        assert!(completions.contains(&CompletionItem::new(
            "grant_exp",
            CompletionKind::Method,
            TypeFact::function(vec![TypeFact::Int], TypeFact::Bool),
        )));
    }

    #[test]
    fn enum_completions_include_variants_and_variant_fields() {
        let facts = registry_facts();

        let variants =
            member_completions(&facts, &TypeFact::enum_type("QuestState", None::<String>));
        assert_eq!(
            variants,
            vec![CompletionItem::new(
                "Active",
                CompletionKind::Variant,
                TypeFact::enum_type("QuestState", Some("Active")),
            )]
        );

        let fields = member_completions(&facts, &TypeFact::enum_type("QuestState", Some("Active")));
        assert_eq!(
            fields,
            vec![CompletionItem::new(
                "quest_id",
                CompletionKind::Field,
                TypeFact::String,
            )]
        );
    }

    #[test]
    fn global_completions_include_types_traits_and_functions() {
        let facts = registry_facts();
        let completions = global_completions(&facts);

        assert!(completions.contains(&CompletionItem::new(
            "Player",
            CompletionKind::Type,
            TypeFact::host("Player"),
        )));
        assert!(completions.contains(&CompletionItem::new(
            "Damageable",
            CompletionKind::Trait,
            TypeFact::trait_type("Damageable"),
        )));
        assert!(completions.contains(&CompletionItem::new(
            "game.reward.grant",
            CompletionKind::Function,
            TypeFact::function(vec![TypeFact::host("Player")], TypeFact::Bool),
        )));
    }

    #[test]
    fn trait_receiver_completions_include_trait_methods() {
        let facts = registry_facts();

        let completions = member_completions(&facts, &TypeFact::trait_type("Damageable"));

        assert_eq!(
            completions,
            vec![CompletionItem::new(
                "damage",
                CompletionKind::Method,
                TypeFact::function(vec![TypeFact::Int], TypeFact::Bool),
            )]
        );
    }

    fn registry_facts() -> RegistryFacts {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .field(FieldDesc::new(FieldId::new(1), "level").type_hint("int"))
                .method(
                    MethodDesc::new(HostMethodId::new(1), "grant_exp")
                        .param(MethodParamDesc::new("amount").type_hint("int"))
                        .return_type("bool"),
                ),
        );
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(2), "QuestState"))
                .kind(TypeKind::ScriptEnum)
                .variant(
                    VariantDesc::new(VariantId::new(1), "Active")
                        .field(FieldDesc::new(FieldId::new(2), "quest_id").type_hint("string")),
                ),
        );
        registry.register_trait(
            TraitDesc::new("Damageable").method(
                TraitMethodDesc::new(MethodId::new(1), "damage")
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("bool"),
            ),
        );
        registry.register_function(
            FunctionDesc::new(FunctionId::new(1), "game.reward.grant")
                .param(vela_reflect::FunctionParamDesc::new("player").type_hint("Player"))
                .return_type("bool"),
        );
        RegistryFacts::from_registry(&registry)
    }
}
