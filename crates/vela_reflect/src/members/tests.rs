use std::collections::BTreeMap;

use super::*;
use vela_common::{HostMethodId, HostObjectId, HostTypeId, SourceId, Span};
use vela_def::{FieldId, MethodId, TypeId, VariantId};
use vela_host::path::HostRef;

use crate::permissions::ReflectPolicy;
use crate::registry::{
    FieldDesc, MethodDesc, MethodParamDesc, TraitDesc, TraitMethodDesc, TypeKey, TypeKind,
    VariantDesc,
};

fn player_ref() -> HostRef {
    HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3)
}

fn registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .docs("A player host object.")
            .attr("domain", "gameplay")
            .field(FieldDesc::new(FieldId::new(1), "id"))
            .field(
                FieldDesc::new(FieldId::new(2), "level")
                    .writable(true)
                    .type_hint("int")
                    .source_span(Span::new(SourceId::new(8), 50, 55))
                    .docs("Current level.")
                    .attr("unit", "level"),
            )
            .method(
                MethodDesc::new(HostMethodId::new(5), "grant_exp")
                    .param(MethodParamDesc::new("amount").type_hint("int"))
                    .return_type("bool")
                    .source_span(Span::new(SourceId::new(8), 60, 80))
                    .effects(crate::access::MethodEffectSet::host_write())
                    .access(
                        crate::access::MethodAccess::new()
                            .reflect_callable(true)
                            .require_permission("player.grant_exp"),
                    )
                    .docs("Grant experience.")
                    .attr("effect", "write"),
            )
            .trait_impl(
                TraitDesc::new("Damageable")
                    .source_span(Span::new(SourceId::new(8), 20, 40))
                    .docs("Can take damage.")
                    .attr("combat", "true")
                    .method(
                        TraitMethodDesc::new(MethodId::new(9), "damage")
                            .param(MethodParamDesc::new("amount").type_hint("int"))
                            .return_type("int")
                            .defaulted(true)
                            .docs("Apply damage.")
                            .attr("default", "true"),
                    ),
            ),
    );
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(200), "QuestProgress"))
            .kind(TypeKind::ScriptEnum)
            .variant(
                VariantDesc::new(VariantId::new(11), "Active")
                    .source_span(Span::new(SourceId::new(8), 90, 100))
                    .docs("Quest is active.")
                    .attr("state", "open")
                    .field(FieldDesc::new(FieldId::new(12), "count")),
            )
            .variant(VariantDesc::new(VariantId::new(13), "Finished")),
    );
    registry
}

mod candidates;
mod metadata;
mod policy;
