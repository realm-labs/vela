use vela_common::{FieldId, HostTypeId, TraitId, TypeId};
use vela_reflect::registry::{FieldDesc, TraitDesc, TypeDesc, TypeKey};

mod context;
mod host_methods;
mod native;
mod random;
mod reflection;
mod source_reload;
mod typed_host;
mod typed_native;
mod validation;

fn player_type(type_id: TypeId, host_type_id: HostTypeId) -> TypeDesc {
    TypeDesc::new(TypeKey::new(type_id, "Player"))
        .host_type(host_type_id)
        .field(FieldDesc::new(FieldId::new(1), "level").writable(true))
}

fn trait_desc_with_id(id: TraitId, name: &str) -> TraitDesc {
    let mut desc = TraitDesc::new(name);
    desc.id = id;
    desc
}
