use std::collections::BTreeMap;

use crate::access::{FieldAccess, MethodAccess, MethodEffectSet};
use crate::candidates::ReflectCandidate;
use crate::error::ReflectErrorKind;
use crate::members::trait_by_name as trait_metadata_by_name;
use crate::permissions::{ReflectPermission, ReflectPermissionSet, ReflectPolicy};
use crate::registry::{
    FieldDesc, MethodDesc, TraitDesc, TypeDesc, TypeKey, TypeKind, TypeRegistry, VariantDesc,
};
use crate::types::type_by_name as type_metadata_by_name;
use crate::value::{
    ReflectContext, ReflectValue, fields, get, get_with_policy, implements, set, set_with_policy,
    type_of,
};
use vela_common::{SourceId, Span};
use vela_def::{FieldId, MethodId, TypeId, VariantId};
use vela_host::value::HostValue;

fn player_value() -> ReflectValue {
    ReflectValue::ScriptRecord {
        type_name: "Player".to_owned(),
        fields: BTreeMap::new(),
    }
}

fn registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register_trait(TraitDesc::new("Trackable"));
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .field(FieldDesc::new(FieldId::new(1), "id"))
            .field(FieldDesc::new(FieldId::new(2), "level").writable(true))
            .method(MethodDesc::new(MethodId::new(5), "grant_exp"))
            .trait_impl(TraitDesc::new("Damageable")),
    );
    registry
}

fn trait_name(name: &str) -> ReflectValue {
    ReflectValue::Host(HostValue::String(name.to_owned()))
}

mod calls;
mod fields;
mod implements;
