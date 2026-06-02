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
    ReflectContext, ReflectValue, call, call_with_policy, fields, get, get_with_policy, implements,
    set, set_with_policy, type_of,
};
use vela_common::{
    FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, Span, TypeId, VariantId,
};
use vela_host::mock::MockStateAdapter;
use vela_host::patch::PatchOp;
use vela_host::path::{HostPath, HostRef};
use vela_host::tx::{HostObjectSnapshot, PatchTx};
use vela_host::value::HostValue;

fn player_ref() -> HostRef {
    HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3)
}

fn registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.register_trait(TraitDesc::new("Trackable"));
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
            .host_type(HostTypeId::new(1))
            .field(FieldDesc::new(FieldId::new(1), "id"))
            .field(FieldDesc::new(FieldId::new(2), "level").writable(true))
            .method(MethodDesc::new(HostMethodId::new(5), "grant_exp"))
            .trait_impl(TraitDesc::new("Damageable")),
    );
    registry
}

fn adapter_with_level(value: HostValue) -> MockStateAdapter {
    let mut adapter = MockStateAdapter::new();
    adapter.insert_value(HostPath::new(player_ref()).field(FieldId::new(2)), value);
    adapter
}

fn trait_name(name: &str) -> ReflectValue {
    ReflectValue::Host(HostValue::String(name.to_owned()))
}

mod calls;
mod fields;
mod implements;
