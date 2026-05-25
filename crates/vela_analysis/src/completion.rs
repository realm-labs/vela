use std::collections::BTreeSet;

use vela_hir::{DeclarationKind, HirDeclId, ModuleGraph};

use crate::{
    AnalysisFacts, RegistryFacts, TypeFact, stdlib_function_completion_facts, stdlib_method_facts,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionKind {
    Binding,
    Const,
    Field,
    Method,
    Module,
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
    let mut completions = match receiver {
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
    };
    completions.extend(stdlib_method_completions(receiver));
    completions
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

pub fn declaration_completions(graph: &ModuleGraph, facts: &AnalysisFacts) -> Vec<CompletionItem> {
    graph
        .declarations()
        .filter_map(|declaration| {
            let kind = completion_kind_for_declaration(declaration.kind)?;
            let fact = facts
                .declaration(declaration.id)
                .cloned()
                .unwrap_or(TypeFact::Unknown);
            Some(CompletionItem::new(
                qualified_declaration_label(graph, declaration.id),
                kind,
                fact,
            ))
        })
        .collect()
}

pub fn module_completions(graph: &ModuleGraph) -> Vec<CompletionItem> {
    module_completion_labels(graph)
        .into_iter()
        .map(|label| {
            CompletionItem::new(
                label.clone(),
                CompletionKind::Module,
                TypeFact::module(label),
            )
        })
        .collect()
}

pub fn local_completions(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    declaration: HirDeclId,
) -> Vec<CompletionItem> {
    let Some(bindings) = graph.bindings(declaration) else {
        return Vec::new();
    };
    bindings
        .locals()
        .map(|local| {
            let fact = facts.local(local.id).cloned().unwrap_or(TypeFact::Unknown);
            CompletionItem::new(local.name.clone(), CompletionKind::Binding, fact)
        })
        .collect()
}

fn module_completion_labels(graph: &ModuleGraph) -> BTreeSet<String> {
    let mut modules = BTreeSet::new();
    for declaration in graph.declarations() {
        let Some(module_path) = graph.module_path(declaration.module) else {
            continue;
        };
        let segments = module_path.segments();
        for len in 1..=segments.len() {
            modules.insert(segments[..len].join("."));
        }
    }
    modules
}

fn completion_kind_for_declaration(kind: DeclarationKind) -> Option<CompletionKind> {
    match kind {
        DeclarationKind::Const => Some(CompletionKind::Const),
        DeclarationKind::Function => Some(CompletionKind::Function),
        DeclarationKind::Struct | DeclarationKind::Enum => Some(CompletionKind::Type),
        DeclarationKind::Trait => Some(CompletionKind::Trait),
        DeclarationKind::Impl => None,
    }
}

fn qualified_declaration_label(graph: &ModuleGraph, declaration: HirDeclId) -> String {
    let Some(declaration) = graph.declaration(declaration) else {
        return String::new();
    };
    let Some(module_path) = graph.module_path(declaration.module) else {
        return declaration.name.clone();
    };
    let module = module_path.join();
    if module.is_empty() {
        declaration.name.clone()
    } else {
        format!("{module}.{}", declaration.name)
    }
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
    let mut completions = facts
        .functions()
        .map(|function| CompletionItem::new(function.name, CompletionKind::Function, function.fact))
        .collect::<Vec<_>>();
    completions.extend(stdlib_function_completions());
    completions
}

fn stdlib_method_completions(receiver: &TypeFact) -> Vec<CompletionItem> {
    stdlib_method_facts(receiver, None)
        .into_iter()
        .map(|fact| {
            CompletionItem::new(
                fact.method,
                CompletionKind::Method,
                TypeFact::function(fact.params, fact.returns),
            )
        })
        .collect()
}

fn stdlib_function_completions() -> Vec<CompletionItem> {
    stdlib_function_completion_facts()
        .into_iter()
        .map(|fact| {
            CompletionItem::new(
                fact.name,
                CompletionKind::Function,
                TypeFact::function(fact.params, fact.returns),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use vela_common::{FieldId, FunctionId, HostMethodId, MethodId, SourceId, TypeId, VariantId};
    use vela_hir::{ModulePath, ModuleSource};
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

    #[test]
    fn receiver_completions_include_stdlib_collection_and_string_methods() {
        let facts = registry_facts();

        let map = member_completions(&facts, &TypeFact::map(TypeFact::String, TypeFact::Int));
        assert!(map.contains(&CompletionItem::new(
            "get",
            CompletionKind::Method,
            TypeFact::function(vec![TypeFact::String], TypeFact::option(TypeFact::Int)),
        )));
        assert!(map.contains(&CompletionItem::new(
            "filter",
            CompletionKind::Method,
            TypeFact::function(
                vec![TypeFact::function(
                    vec![TypeFact::String, TypeFact::Int],
                    TypeFact::Bool,
                )],
                TypeFact::map(TypeFact::String, TypeFact::Int),
            ),
        )));
        let array = member_completions(&facts, &TypeFact::array(TypeFact::String));
        assert!(array.contains(&CompletionItem::new(
            "first",
            CompletionKind::Method,
            TypeFact::function(Vec::new(), TypeFact::option(TypeFact::String)),
        )));
        assert!(array.contains(&CompletionItem::new(
            "last",
            CompletionKind::Method,
            TypeFact::function(Vec::new(), TypeFact::option(TypeFact::String)),
        )));
        assert!(array.contains(&CompletionItem::new(
            "join",
            CompletionKind::Method,
            TypeFact::function(vec![TypeFact::String], TypeFact::String),
        )));

        let string = member_completions(&facts, &TypeFact::String);
        assert!(string.contains(&CompletionItem::new(
            "find",
            CompletionKind::Method,
            TypeFact::function(vec![TypeFact::String], TypeFact::option(TypeFact::Int)),
        )));
        assert!(string.contains(&CompletionItem::new(
            "strip_prefix",
            CompletionKind::Method,
            TypeFact::function(vec![TypeFact::String], TypeFact::option(TypeFact::String)),
        )));
        assert!(string.contains(&CompletionItem::new(
            "strip_suffix",
            CompletionKind::Method,
            TypeFact::function(vec![TypeFact::String], TypeFact::option(TypeFact::String)),
        )));
        assert!(string.contains(&CompletionItem::new(
            "replace",
            CompletionKind::Method,
            TypeFact::function(vec![TypeFact::String, TypeFact::String], TypeFact::String),
        )));
        assert!(string.contains(&CompletionItem::new(
            "slice",
            CompletionKind::Method,
            TypeFact::function(vec![TypeFact::Int, TypeFact::Int], TypeFact::String),
        )));
        assert!(string.contains(&CompletionItem::new(
            "split",
            CompletionKind::Method,
            TypeFact::function(vec![TypeFact::String], TypeFact::array(TypeFact::String)),
        )));
    }

    #[test]
    fn global_completions_include_stdlib_functions() {
        let facts = registry_facts();
        let completions = global_completions(&facts);
        let number = TypeFact::Union(vec![TypeFact::Int, TypeFact::Float]);

        assert!(completions.contains(&CompletionItem::new(
            "option.unwrap_or",
            CompletionKind::Function,
            TypeFact::function(
                vec![TypeFact::option(TypeFact::Any), TypeFact::Any],
                TypeFact::Any
            ),
        )));
        assert!(completions.contains(&CompletionItem::new(
            "math.random",
            CompletionKind::Function,
            TypeFact::function(vec![TypeFact::Int, TypeFact::Int], TypeFact::Int),
        )));
        assert!(completions.contains(&CompletionItem::new(
            "math.lerp",
            CompletionKind::Function,
            TypeFact::function(
                vec![number.clone(), number.clone(), number],
                TypeFact::Float
            ),
        )));
        assert!(completions.contains(&CompletionItem::new(
            "math.round",
            CompletionKind::Function,
            TypeFact::function(
                vec![TypeFact::Union(vec![TypeFact::Int, TypeFact::Float])],
                TypeFact::Int
            ),
        )));
    }

    #[test]
    fn local_completions_include_function_scope_bindings() {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game"),
            r#"
            struct Player { level: int }
            fn grant(player: Player, amount: int) -> bool {
                let rewards: map = {};
                let inferred = 10;
                for reward in [] {
                    let amount: string = reward;
                }
                return amount > 0;
            }
            "#,
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);
        let declaration = graph
            .declarations()
            .find(|declaration| declaration.name == "grant")
            .expect("grant declaration")
            .id;
        let facts = AnalysisFacts::from_module_graph(&graph);

        let completions = local_completions(&graph, &facts, declaration);

        assert!(completions.contains(&CompletionItem::new(
            "player",
            CompletionKind::Binding,
            TypeFact::record("game.Player"),
        )));
        assert!(completions.contains(&CompletionItem::new(
            "rewards",
            CompletionKind::Binding,
            TypeFact::map(TypeFact::Unknown, TypeFact::Unknown),
        )));
        assert!(completions.contains(&CompletionItem::new(
            "inferred",
            CompletionKind::Binding,
            TypeFact::Unknown,
        )));
        assert!(completions.contains(&CompletionItem::new(
            "amount",
            CompletionKind::Binding,
            TypeFact::Int,
        )));
        assert!(completions.contains(&CompletionItem::new(
            "amount",
            CompletionKind::Binding,
            TypeFact::String,
        )));
    }

    #[test]
    fn declaration_completions_include_script_declarations() {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.player"),
            r#"
            pub struct Player { level: int }
            pub enum QuestState { Active { quest_id: string }, Done }
            pub trait Damageable {
                fn damage(self, amount: int) -> bool;
            }
            pub const START_LEVEL: int = 1
            pub fn grant(player: Player, amount: int) -> bool {
                return amount > 0;
            }
            impl Damageable for Player {
                fn damage(self, amount: int) -> bool {
                    return amount > 0;
                }
            }
            "#,
        ));
        graph.add_source(ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_dotted("game.reward"),
            r#"
            pub fn grant(amount: int) -> int {
                return amount + 1;
            }
            "#,
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);
        let facts = AnalysisFacts::from_module_graph(&graph);

        let completions = declaration_completions(&graph, &facts);

        assert!(completions.contains(&CompletionItem::new(
            "game.player.Player",
            CompletionKind::Type,
            TypeFact::record("game.player.Player"),
        )));
        assert!(completions.contains(&CompletionItem::new(
            "game.player.QuestState",
            CompletionKind::Type,
            TypeFact::enum_type("game.player.QuestState", None::<String>),
        )));
        assert!(completions.contains(&CompletionItem::new(
            "game.player.Damageable",
            CompletionKind::Trait,
            TypeFact::trait_type("game.player.Damageable"),
        )));
        assert!(completions.contains(&CompletionItem::new(
            "game.player.START_LEVEL",
            CompletionKind::Const,
            TypeFact::Int,
        )));
        assert!(completions.contains(&CompletionItem::new(
            "game.player.grant",
            CompletionKind::Function,
            TypeFact::function(
                vec![TypeFact::record("game.player.Player"), TypeFact::Int],
                TypeFact::Bool,
            ),
        )));
        assert!(completions.contains(&CompletionItem::new(
            "game.reward.grant",
            CompletionKind::Function,
            TypeFact::function(vec![TypeFact::Int], TypeFact::Int),
        )));
        assert!(
            completions
                .iter()
                .all(|completion| completion.label != "game.player.Damageable.Player")
        );
    }

    #[test]
    fn module_completions_include_module_paths_and_prefixes() {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.player"),
            "pub fn level() { return 1; }",
        ));
        graph.add_source(ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_dotted("game.reward"),
            "pub fn grant() { return 2; }",
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);

        let completions = module_completions(&graph);

        assert_eq!(
            completions,
            vec![
                CompletionItem::new("game", CompletionKind::Module, TypeFact::module("game")),
                CompletionItem::new(
                    "game.player",
                    CompletionKind::Module,
                    TypeFact::module("game.player"),
                ),
                CompletionItem::new(
                    "game.reward",
                    CompletionKind::Module,
                    TypeFact::module("game.reward"),
                ),
            ]
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
