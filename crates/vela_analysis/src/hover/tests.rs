use vela_common::{HostMethodId, SourceId, Span};
use vela_def::{FieldId, FunctionId, TypeId, VariantId};
use vela_reflect::access::{FieldAccess, FunctionEffectSet, MethodEffectSet};
use vela_reflect::modules::{DeclOrigin, FunctionDesc, FunctionParamDesc, ModuleDesc};
use vela_reflect::registry::{
    FieldDesc, MethodDesc, MethodParamDesc, TraitDesc, TraitMethodDesc, TypeDesc, TypeKey,
    TypeKind, TypeRegistry, VariantDesc,
};

use super::*;

#[test]
fn hovers_types_fields_methods_and_variants() {
    let registry = hover_registry();

    let type_info = type_hover(&registry, "Player").expect("type hover");
    assert_eq!(type_info.kind, HoverKind::Type);
    assert_eq!(type_info.fact, TypeFact::host("Player"));
    assert_eq!(type_info.docs.as_deref(), Some("host player state"));
    assert_eq!(
        type_info.attrs,
        vec![("role".to_owned(), "actor".to_owned())]
    );

    let field_info = field_hover(&registry, "Player", "level").expect("field hover");
    assert_eq!(field_info.kind, HoverKind::Field);
    assert_eq!(field_info.fact, TypeFact::I64);
    assert!(
        field_info
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("level.write"))
    );

    let method_info = method_hover(&registry, "Player", "grant").expect("method hover");
    assert_eq!(method_info.kind, HoverKind::Method);
    assert_eq!(
        method_info.fact,
        TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL)
    );
    assert!(
        method_info
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("writes_host"))
    );

    let variant_info = variant_hover(&registry, "QuestState", "Active").expect("variant hover");
    assert_eq!(variant_info.kind, HoverKind::Variant);
    assert_eq!(
        variant_info.fact,
        TypeFact::enum_type("QuestState", Some("Active"))
    );

    let variant_field_info =
        field_hover(&registry, "QuestState::Active", "quest_id").expect("variant field hover");
    assert_eq!(variant_field_info.fact, TypeFact::STRING);
}

#[test]
fn hovers_builtin_type_kinds_without_generics() {
    let registry = hover_registry();

    let array_info = type_hover(&registry, "array").expect("array hover");
    assert_eq!(array_info.kind, HoverKind::Type);
    assert_eq!(array_info.fact, TypeFact::array(TypeFact::Any));
    assert_eq!(array_info.detail.as_deref(), Some("kind: array"));

    let map_info = type_hover(&registry, "map").expect("map hover");
    assert_eq!(map_info.fact, TypeFact::map(TypeFact::Any, TypeFact::Any));
    assert_eq!(map_info.detail.as_deref(), Some("kind: map"));

    let closure_info = type_hover(&registry, "closure").expect("closure hover");
    assert_eq!(
        closure_info.fact,
        TypeFact::function(Vec::new(), TypeFact::Any)
    );
    assert_eq!(closure_info.detail.as_deref(), Some("kind: closure"));
}

#[test]
fn hovers_functions_traits_and_modules() {
    let registry = hover_registry();

    let function_info = function_hover(&registry, "grant_reward").expect("function hover");
    assert_eq!(function_info.kind, HoverKind::Function);
    assert_eq!(
        function_info.fact,
        TypeFact::function(vec![TypeFact::host("Player")], TypeFact::BOOL)
    );
    assert!(
        function_info
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("script"))
    );
    assert!(
        function_info
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("emits_events"))
    );

    let trait_info = trait_hover(&registry, "Damageable").expect("trait hover");
    assert_eq!(trait_info.fact, TypeFact::trait_type("Damageable"));
    assert_eq!(trait_info.docs.as_deref(), Some("can receive damage"));

    let trait_method_info =
        trait_method_hover(&registry, "Damageable", "damage").expect("trait method hover");
    assert_eq!(
        trait_method_info.fact,
        TypeFact::function(vec![TypeFact::I64], TypeFact::NULL)
    );
    assert_eq!(trait_method_info.detail.as_deref(), Some("defaulted: true"));

    let module_info = module_hover(&registry, "game::rewards").expect("module hover");
    assert_eq!(module_info.kind, HoverKind::Module);
    assert_eq!(module_info.fact, TypeFact::module("game::rewards"));
    assert_eq!(
        module_info.source_span,
        Some(Span::new(SourceId::new(1), 10, 22))
    );
}

fn hover_registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
            .docs("host player state")
            .attr("role", "actor")
            .field(
                FieldDesc::new(FieldId::new(1), "level")
                    .type_hint("i64")
                    .writable(true)
                    .access(
                        FieldAccess::new()
                            .writable(true)
                            .require_permission("level.write"),
                    ),
            )
            .method(
                MethodDesc::new(HostMethodId::new(1), "grant")
                    .param(MethodParamDesc::new("amount").type_hint("i64"))
                    .return_type("bool")
                    .effects(MethodEffectSet::host_write()),
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
    registry.register(TypeDesc::new(TypeKey::new(TypeId::new(3), "array")).kind(TypeKind::Array));
    registry.register(TypeDesc::new(TypeKey::new(TypeId::new(4), "map")).kind(TypeKind::Map));
    registry
        .register(TypeDesc::new(TypeKey::new(TypeId::new(5), "closure")).kind(TypeKind::Closure));
    registry.register_trait(
        TraitDesc::new("Damageable")
            .docs("can receive damage")
            .method(
                TraitMethodDesc::new(vela_def::MethodId::new(1), "damage")
                    .defaulted(true)
                    .param(MethodParamDesc::new("amount").type_hint("i64"))
                    .return_type("null"),
            ),
    );
    registry.register_module(ModuleDesc::new("game::rewards").source_span(Span::new(
        SourceId::new(1),
        10,
        22,
    )));
    registry.register_function(
        FunctionDesc::new(FunctionId::new(1), "grant_reward")
            .module("game::rewards")
            .origin(DeclOrigin::Script)
            .param(FunctionParamDesc::new("player").type_hint("Player"))
            .return_type("bool")
            .effects(FunctionEffectSet::event_emit()),
    );
    registry
}
