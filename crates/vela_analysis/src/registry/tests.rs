#[cfg(test)]
mod tests {
    use vela_common::{HostMethodId, HostTypeId, SourceId, Span};
    use vela_def::{FieldId, FunctionId, MethodId, TypeId, VariantId};
    use vela_reflect::access::{MethodAccess, MethodEffectSet};
    use vela_reflect::modules::{FunctionDesc, FunctionParamDesc, ModuleDesc};
    use vela_reflect::registry::{
        FieldDesc, MethodDesc, MethodParamDesc, TraitDesc, TraitMethodDesc, TypeDesc, TypeKey,
        TypeKind, TypeRegistry, VariantDesc,
    };

    use super::*;

    #[test]
    fn registry_facts_cover_types_fields_methods_functions_and_modules() {
        let player = TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .host_type(HostTypeId::new(1))
            .docs("Player host object.")
            .field(
                FieldDesc::new(FieldId::new(1), "level")
                    .type_hint("i64")
                    .docs("Current player level."),
            )
            .field(FieldDesc::new(FieldId::new(2), "inventory").type_hint("Inventory"))
            .method(
                MethodDesc::new(HostMethodId::new(1), "grant_exp")
                    .param(
                        MethodParamDesc::new("amount")
                            .type_hint("i64")
                            .defaulted(true),
                    )
                    .return_type("bool")
                    .docs("Grant player experience.")
                    .effects(MethodEffectSet::host_write())
                    .access(MethodAccess::new().require_permission("player.reward")),
            )
            .trait_impl(
                TraitDesc::new("Damageable")
                    .docs("Can receive damage.")
                    .method(
                        TraitMethodDesc::new(MethodId::new(1), "damage")
                            .param(MethodParamDesc::new("amount").type_hint("i64"))
                            .return_type("bool")
                            .docs("Apply damage."),
                    ),
            );
        let inventory = TypeDesc::new(TypeKey::new(TypeId::new(2), "Inventory"))
            .kind(TypeKind::ScriptStruct)
            .field(FieldDesc::new(FieldId::new(1), "items").type_hint("Map"));
        let quest = TypeDesc::new(TypeKey::new(TypeId::new(3), "QuestState"))
            .kind(TypeKind::ScriptEnum)
            .variant(
                VariantDesc::new(VariantId::new(1), "Active")
                    .docs("Active quest state.")
                    .field(
                        FieldDesc::new(FieldId::new(1), "quest_id")
                            .type_hint("String")
                            .docs("Active quest id."),
                    ),
            );

        let mut registry = TypeRegistry::new();
        registry.register(player);
        registry.register(inventory);
        registry.register(quest);
        registry.register_module(
            ModuleDesc::new("game::reward")
                .docs("Reward module.")
                .source_span(Span::new(SourceId::new(7), 10, 20)),
        );
        registry.register_function(
            FunctionDesc::new(FunctionId::new(1), "game::reward::grant")
                .param(FunctionParamDesc::new("player").type_hint("Player"))
                .param(
                    FunctionParamDesc::new("amount")
                        .type_hint("i64")
                        .defaulted(true),
                )
                .return_type("bool")
                .docs("Grant reward from a script module."),
        );

        let facts = RegistryFacts::from_registry(&registry);

        assert_eq!(facts.type_fact("Player"), Some(&TypeFact::host("Player")));
        assert_eq!(
            facts.type_target_fact("Player"),
            Some(&RegistryTypeTargetFact::new(
                "Player",
                TypeId::new(1),
                Some(HostTypeId::new(1)),
            ))
        );
        assert_eq!(
            facts.type_fact("Inventory"),
            Some(&TypeFact::record("Inventory"))
        );
        assert_eq!(
            facts.type_fact("QuestState"),
            Some(&TypeFact::enum_type("QuestState", None::<String>))
        );
        assert_eq!(facts.field_fact("Player", "level"), Some(&TypeFact::I64));
        assert!(matches!(
            facts.field_target_fact("Player", "level"),
            Some(target)
                if target.owner == TypeId::new(1)
                    && target.semantic == FieldId::new(1)
                    && target.host_runtime == Some(FieldId::new(1))
                    && !target.variant_field
                    && target.access.readable
                    && !target.access.writable
        ));
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
        assert!(matches!(
            facts.field_target_fact("QuestState::Active", "quest_id"),
            Some(target)
                if target.owner == TypeId::new(3)
                    && target.semantic == FieldId::new(1)
                    && target.host_runtime.is_none()
                    && target.variant_field
        ));
        assert_eq!(
            facts.method_fact("Player", "grant_exp"),
            Some(&TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL))
        );
        let method_signature = facts
            .method_signature_fact("Player", "grant_exp")
            .expect("method signature");
        assert_eq!(method_signature.parameters[0].name, "amount");
        assert_eq!(method_signature.parameters[0].type_fact, TypeFact::I64);
        assert_eq!(
            method_signature.parameters[0].requirement,
            CallableParameterRequirementFact::Defaulted
        );
        assert_eq!(method_signature.parameters[0].declaration_span, None);
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
        let function_signature = facts
            .function_signature_fact("game::reward::grant")
            .expect("function signature");
        assert_eq!(
            function_signature
                .parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>(),
            ["player", "amount"]
        );
        assert_eq!(
            function_signature.parameters[0].requirement,
            CallableParameterRequirementFact::Required
        );
        assert_eq!(
            function_signature.parameters[1].requirement,
            CallableParameterRequirementFact::Defaulted
        );
        assert!(
            function_signature
                .parameters
                .iter()
                .all(|parameter| parameter.declaration_span.is_none())
        );
        assert_eq!(function_signature.returns, TypeFact::BOOL);
        assert!(
            facts
                .function_access_fact("game::reward::grant")
                .is_some_and(|access| {
                    access.public && access.reflect_visible && !access.reflect_callable
                })
        );
        assert_eq!(
            facts.module_fact("game::reward"),
            Some(&TypeFact::module("game::reward"))
        );
        assert_eq!(facts.module_docs("game::reward"), Some("Reward module."));
        assert_eq!(
            facts.module_source_span("game::reward"),
            Some(Span::new(SourceId::new(7), 10, 20))
        );
        assert_eq!(
            facts.trait_method_fact("Damageable", "damage"),
            Some(&TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL))
        );
        assert_eq!(facts.type_docs("Player"), Some("Player host object."));
        assert_eq!(
            facts.field_docs("Player", "level"),
            Some("Current player level.")
        );
        assert_eq!(
            facts.method_docs("Player", "grant_exp"),
            Some("Grant player experience.")
        );
        assert_eq!(facts.trait_docs("Damageable"), Some("Can receive damage."));
        assert_eq!(
            facts.trait_method_docs("Damageable", "damage"),
            Some("Apply damage.")
        );
        assert_eq!(
            facts.variant_docs("QuestState", "Active"),
            Some("Active quest state.")
        );
        assert_eq!(
            facts.field_docs("QuestState::Active", "quest_id"),
            Some("Active quest id.")
        );
        assert_eq!(
            facts.function_docs("game::reward::grant"),
            Some("Grant reward from a script module.")
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
    fn registry_facts_parse_structural_tuple_hints_from_descriptors() {
        let player = TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .field(FieldDesc::new(FieldId::new(1), "split").type_hint("Option<(String, String)>"))
            .field(
                FieldDesc::new(FieldId::new(2), "outcome").type_hint("Result<(String, i64), ()>"),
            )
            .method(
                MethodDesc::new(HostMethodId::new(1), "join")
                    .param(MethodParamDesc::new("parts").type_hint("(String, String)"))
                    .return_type("Option<(String, String)>"),
            );

        let mut registry = TypeRegistry::new();
        registry.register(player);
        registry.register_function(
            FunctionDesc::new(FunctionId::new(1), "game::reward::split")
                .return_type("Result<Option<(String, i64)>, ()>"),
        );

        let facts = RegistryFacts::from_registry(&registry);

        assert_eq!(
            facts.field_fact("Player", "split"),
            Some(&TypeFact::option(TypeFact::tuple([
                TypeFact::STRING,
                TypeFact::STRING
            ])))
        );
        assert_eq!(
            facts.field_fact("Player", "outcome"),
            Some(&TypeFact::result(
                TypeFact::tuple([TypeFact::STRING, TypeFact::I64]),
                TypeFact::UNIT,
            ))
        );
        assert_eq!(
            facts.method_fact("Player", "join"),
            Some(&TypeFact::function(
                vec![TypeFact::tuple([TypeFact::STRING, TypeFact::STRING])],
                TypeFact::option(TypeFact::tuple([TypeFact::STRING, TypeFact::STRING])),
            ))
        );
        assert_eq!(
            facts.function_fact("game::reward::split"),
            Some(&TypeFact::function(
                Vec::new(),
                TypeFact::result(
                    TypeFact::option(TypeFact::tuple([TypeFact::STRING, TypeFact::I64])),
                    TypeFact::UNIT,
                ),
            ))
        );
    }

    #[test]
    fn owner_scoped_member_indexes_include_qualified_short_names() {
        let mut facts = RegistryFacts::default();
        facts.insert_field("game::Player", "level", TypeFact::I64);
        facts.insert_variant(
            "game::QuestState",
            "Active",
            TypeFact::enum_type("game::QuestState", Some("Active")),
        );

        assert_eq!(
            facts
                .fields_for_owner_or_short_name("game::Player")
                .into_iter()
                .map(|field| (field.owner, field.name, field.fact))
                .collect::<Vec<_>>(),
            vec![("game::Player".to_owned(), "level".to_owned(), TypeFact::I64)]
        );
        assert_eq!(
            facts
                .fields_for_owner_or_short_name("Player")
                .into_iter()
                .map(|field| (field.owner, field.name, field.fact))
                .collect::<Vec<_>>(),
            vec![("game::Player".to_owned(), "level".to_owned(), TypeFact::I64)]
        );
        assert_eq!(
            facts
                .variants_for_owner_or_short_name("QuestState")
                .into_iter()
                .map(|variant| (variant.owner, variant.name, variant.fact))
                .collect::<Vec<_>>(),
            vec![(
                "game::QuestState".to_owned(),
                "Active".to_owned(),
                TypeFact::enum_type("game::QuestState", Some("Active"))
            )]
        );
    }

    #[test]
    fn registry_facts_cover_builtin_type_kinds_without_generics() {
        let mut registry = TypeRegistry::new();
        for (id, name, kind) in [
            (10, "()", TypeKind::Unit),
            (11, "bool", TypeKind::Bool),
            (12, "i64", TypeKind::I64),
            (13, "f64", TypeKind::F64),
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

        assert_eq!(facts.type_fact("()"), Some(&TypeFact::UNIT));
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
        assert_eq!(facts.type_fact("closure"), Some(&TypeFact::Closure));
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

    #[test]
    fn compile_view_facts_preserve_targets_signatures_effects_and_type_hints() {
        use vela_def::DefPath;
        use vela_registry::{
            DefinitionRegistry, EffectSet, FieldAccessDef, FieldDef, FunctionAccessDef,
            FunctionDef, FunctionSignature, IndexCapabilityDef, MethodAccessDef, MethodDef,
            ParamDef, TypeDef, TypeHintDef,
        };

        let mut registry = DefinitionRegistry::new();
        let player = registry
            .register_type(
                TypeDef::new(DefPath::ty("host", ["game"], "Player"))
                    .host_runtime_id(7)
                    .index_capability(
                        IndexCapabilityDef::new()
                            .readable(true)
                            .writable(false)
                            .addable(true)
                            .removable(false)
                            .key_type("String")
                            .value_type(TypeHintDef::new(["game", "Reward"])),
                    ),
            )
            .expect("Player registration");
        registry
            .register_type(TypeDef::new(DefPath::ty("script", ["game"], "Reward")))
            .expect("Reward registration");
        registry
            .register_field(
                FieldDef::new(DefPath::field("host", ["game"], "Player", "level"), player)
                    .access(
                        FieldAccessDef::new()
                            .readable(false)
                            .writable(true)
                            .reflect_readable(true)
                            .reflect_writable(false)
                            .require_permission("player.inspect"),
                    )
                    .host_runtime_id(81)
                    .declaration_order(3)
                    .defaulted(true)
                    .type_hint(Some("i64")),
            )
            .expect("level registration");
        registry
            .register_field(
                FieldDef::new(
                    DefPath::field("host", ["game"], "Player", "rewards"),
                    player,
                )
                .type_hint(Some(
                    TypeHintDef::named("Array").with_args([TypeHintDef::new(["game", "Reward"])]),
                )),
            )
            .expect("rewards registration");
        registry
            .register_method(
                MethodDef::new(
                    DefPath::method("host", ["game"], "Player", "save"),
                    player,
                    FunctionSignature::new([ParamDef::new("amount", Some("i64"))], Some("bool")),
                )
                .effects(EffectSet {
                    host_read: true,
                    host_write: true,
                    reflection_write: true,
                    ..EffectSet::default()
                })
                .access(
                    MethodAccessDef::new()
                        .public(false)
                        .reflect_callable(false)
                        .require_permission("player.admin"),
                ),
            )
            .expect("save registration");
        registry
            .register_function(
                FunctionDef::new(
                    DefPath::function("host", ["game"], "grant"),
                    FunctionSignature::new(
                        [ParamDef::new(
                            "player",
                            Some(TypeHintDef::new(["game", "Player"])),
                        )],
                        Some(
                            TypeHintDef::named("Result").with_args([
                                TypeHintDef::named("bool"),
                                TypeHintDef::named("String"),
                            ]),
                        ),
                    ),
                )
                .effects(EffectSet {
                    host_read: true,
                    event_emit: true,
                    reflection_write: true,
                    ..EffectSet::default()
                })
                .access(
                    FunctionAccessDef::new()
                        .public(false)
                        .reflect_visible(true)
                        .reflect_callable(true),
                ),
            )
            .expect("grant registration");

        let facts = RegistryFacts::from_compile_view(registry.compile_view());

        assert_eq!(
            facts.type_fact("game::Player"),
            Some(&TypeFact::host("game::Player"))
        );
        assert_eq!(
            facts.type_fact("Player"),
            Some(&TypeFact::host("game::Player"))
        );
        assert!(matches!(
            facts.type_target_fact("game::Player"),
            Some(target)
                if target.semantic == player
                    && target.host_runtime == Some(HostTypeId::new(7))
        ));
        assert_eq!(
            facts.type_fact("game::Reward"),
            Some(&TypeFact::record("game::Reward"))
        );
        assert!(matches!(
            facts.index_capability_fact("game::Player"),
            Some(capability)
                if capability.readable
                    && !capability.writable
                    && capability.addable
                    && !capability.removable
                    && capability.key == TypeFact::STRING
                    && capability.value == TypeFact::record("game::Reward")
        ));
        assert!(matches!(
            facts.index_capability_fact("Player"),
            Some(capability)
                if capability.owner == "Player"
                    && capability.key == TypeFact::STRING
                    && capability.value == TypeFact::record("game::Reward")
        ));
        assert_eq!(
            facts.field_fact("game::Player", "level"),
            Some(&TypeFact::I64)
        );
        assert_eq!(
            facts.field_fact("game::Player", "rewards"),
            Some(&TypeFact::array(TypeFact::record("game::Reward")))
        );
        assert!(
            facts
                .field_access_fact("game::Player", "level")
                .is_some_and(|access| {
                    !access.readable
                        && access.writable
                        && access.reflect_readable
                        && !access.reflect_writable
                        && access.required_permissions == ["player.inspect"]
                })
        );
        assert!(matches!(
            facts.field_target_fact("game::Player", "level"),
            Some(target)
                if target.owner == player
                    && target.host_runtime == Some(FieldId::new(81))
                    && !target.variant_field
                    && target.declaration_order == 3
                    && target.has_default
        ));
        assert_eq!(
            facts.method_fact("game::Player", "save"),
            Some(&TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL))
        );
        let method_signature = facts
            .method_signature_fact("game::Player", "save")
            .expect("compile-view method signature");
        assert_eq!(method_signature.parameters[0].name, "amount");
        assert_eq!(method_signature.parameters[0].type_fact, TypeFact::I64);
        assert_eq!(
            method_signature.parameters[0].requirement,
            CallableParameterRequirementFact::Required
        );
        assert_eq!(method_signature.parameters[0].declaration_span, None);
        assert!(
            facts
                .method_effect_fact("game::Player", "save")
                .is_some_and(|effect| effect.writes_host && effect.writes_reflection)
        );
        assert!(
            facts
                .method_access_fact("game::Player", "save")
                .is_some_and(|access| {
                    !access.public
                        && !access.reflect_callable
                        && access.required_permissions == ["player.admin"]
                })
        );
        assert_eq!(
            facts.function_fact("game::grant"),
            Some(&TypeFact::function(
                vec![TypeFact::host("game::Player")],
                TypeFact::result(TypeFact::BOOL, TypeFact::STRING),
            ))
        );
        let function_signature = facts
            .function_signature_fact("game::grant")
            .expect("compile-view function signature");
        assert_eq!(function_signature.parameters[0].name, "player");
        assert_eq!(
            function_signature.parameters[0].type_fact,
            TypeFact::host("game::Player")
        );
        assert_eq!(
            function_signature.parameters[0].requirement,
            CallableParameterRequirementFact::Required
        );
        assert_eq!(function_signature.parameters[0].declaration_span, None);
        assert_eq!(
            function_signature.returns,
            TypeFact::result(TypeFact::BOOL, TypeFact::STRING)
        );
        assert_eq!(facts.function_origin("game::grant"), Some(DeclOrigin::Host));
        assert!(
            facts
                .function_effect_fact("game::grant")
                .is_some_and(|effect| {
                    effect.reads_host && effect.emits_events && effect.writes_reflection
                })
        );
        assert!(
            facts
                .function_access_fact("game::grant")
                .is_some_and(|access| {
                    !access.public && access.reflect_visible && access.reflect_callable
                })
        );
    }

    #[test]
    fn compile_view_facts_feed_analysis_targets_without_reflection_descriptors() {
        use vela_def::DefPath;
        use vela_hir::body::HirExprKind;
        use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};
        use vela_registry::{
            DefinitionRegistry, EffectSet, FieldDef, FunctionDef, FunctionSignature, MethodDef,
            ParamDef, TypeDef, TypeHintDef,
        };

        let mut registry = DefinitionRegistry::new();
        let player = registry
            .register_type(TypeDef::new(DefPath::ty("host", ["game"], "Player")))
            .expect("Player registration");
        registry
            .register_field(
                FieldDef::new(DefPath::field("host", ["game"], "Player", "level"), player)
                    .type_hint(Some("i64")),
            )
            .expect("level registration");
        registry
            .register_method(
                MethodDef::new(
                    DefPath::method("host", ["game"], "Player", "save"),
                    player,
                    FunctionSignature::new([ParamDef::new("amount", Some("i64"))], Some("bool")),
                )
                .effects(EffectSet {
                    host_write: true,
                    ..EffectSet::default()
                }),
            )
            .expect("save registration");
        registry
            .register_function(
                FunctionDef::new(
                    DefPath::function("host", ["game"], "grant"),
                    FunctionSignature::new(
                        [ParamDef::new(
                            "player",
                            Some(TypeHintDef::new(["game", "Player"])),
                        )],
                        Some("bool"),
                    ),
                )
                .effects(EffectSet {
                    host_read: true,
                    ..EffectSet::default()
                }),
            )
            .expect("grant registration");

        let schema = RegistryFacts::from_compile_view(registry.compile_view());
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(91),
            ModulePath::from_qualified("test"),
            r#"
            fn main(player: game::Player) -> bool {
                game::grant(player)
                player.save(1)
                return player.level > 0
            }
            "#,
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);

        let analysis = crate::facts::AnalysisFacts::from_module_graph_and_schema(&graph, &schema);
        let body = graph
            .bodies()
            .find(|body| matches!(body.owner, vela_hir::body::HirBodyOwner::Declaration(_)))
            .expect("main body");

        assert!(body.expressions.values().any(|expression| {
            matches!(
                analysis.call_target(expression.id),
                Some(crate::semantic_facts::CallTargetFact::NativeFunction { path })
                    if path == "game::grant"
            )
        }));
        assert!(body.expressions.values().any(|expression| {
            matches!(
                analysis.call_target(expression.id),
                Some(crate::semantic_facts::CallTargetFact::HostMethod { owner, name })
                    if owner == "game::Player" && name == "save"
            ) && analysis
                .effect(expression.id)
                .is_some_and(|effect| effect.writes_host)
        }));
        let level = body
            .expressions
            .values()
            .find(|expression| {
                matches!(&expression.kind, HirExprKind::Field(field) if field.name == "level")
            })
            .expect("level field");
        assert_eq!(analysis.expression(level.id), Some(&TypeFact::I64));
        assert!(matches!(
            analysis.member_target(level.id),
            Some(crate::semantic_facts::MemberTargetFact::HostField(target))
                if target.owner_name == "game::Player" && target.name == "level"
                    && target.owner == player
        ));
    }
}
