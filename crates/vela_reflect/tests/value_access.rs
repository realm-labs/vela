use std::collections::BTreeMap;

use vela_def::{FieldId, TypeId};
use vela_host::value::HostValue;
use vela_reflect::access::FieldAccess;
use vela_reflect::error::ReflectErrorKind;
use vela_reflect::permissions::{ReflectPermission, ReflectPolicy};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey, TypeRegistry};
use vela_reflect::value::{ReflectContext, ReflectValue, get, set_with_policy};

#[test]
fn owned_record_updates_are_explicit_copies_not_rust_write_through() {
    let registry = TypeRegistry::new();
    let mut context = ReflectContext {
        registry: &registry,
    };
    let original = ReflectValue::ScriptRecord {
        type_name: "Player".to_owned(),
        fields: BTreeMap::from([("level".to_owned(), ReflectValue::Host(HostValue::i64(9)))]),
    };
    let updated = set_with_policy(
        &mut context,
        &original,
        "level",
        ReflectValue::Host(HostValue::i64(12)),
        &ReflectPolicy::all(),
    )
    .expect("owned update");
    assert_eq!(
        get(&mut context, &original, "level").expect("owned field remains readable"),
        HostValue::i64(9)
    );
    assert_eq!(
        get(&mut context, &updated, "level").expect("owned field remains readable"),
        HostValue::i64(12)
    );
    assert_eq!(
        get(
            &mut context,
            &ReflectValue::Host(HostValue::i64(9)),
            "level"
        )
        .expect_err("invalid or denied field operation")
        .kind,
        ReflectErrorKind::InvalidTarget
    );
}

#[test]
fn owned_record_writes_still_enforce_schema_policy() {
    let mut registry = TypeRegistry::new();
    registry.register(
        TypeDesc::new(TypeKey::new(TypeId::new(100), "Player")).field(
            FieldDesc::new(FieldId::new(1), "id")
                .access(FieldAccess::new().reflect_writable(false)),
        ),
    );
    let original = ReflectValue::ScriptRecord {
        type_name: "Player".to_owned(),
        fields: BTreeMap::from([("id".to_owned(), ReflectValue::Host(HostValue::i64(7)))]),
    };
    let mut context = ReflectContext {
        registry: &registry,
    };
    assert!(matches!(
        set_with_policy(
            &mut context,
            &original,
            "id",
            ReflectValue::Host(HostValue::i64(8)),
            &ReflectPolicy::all()
        )
        .expect_err("invalid or denied field operation")
        .kind,
        ReflectErrorKind::FieldNotReflectWritable { .. }
    ));
    assert_eq!(
        get(&mut context, &original, "id").expect("owned field remains readable"),
        HostValue::i64(7)
    );
}

#[test]
fn reflection_boundary_has_no_object_or_adapter_escape_surface() {
    let value_api = include_str!("../src/value.rs");
    let registry_api = include_str!("../src/registry.rs");
    let binding_api = include_str!("../src/type_binding.rs");
    for removed in [
        "HostRef",
        "HostPath",
        "HostTarget",
        "ScriptStateAdapter",
        "HostAccess",
        "resolve_host_method",
        "pub fn call(",
        "pub fn call_with_policy(",
    ] {
        assert!(!value_api.contains(removed), "value API retains {removed}");
    }
    for removed in [
        "host_type_id",
        "host_keys",
        "type_of_host",
        "HostIndexCapability",
        "HostMethodId",
    ] {
        assert!(
            !registry_api.contains(removed),
            "registry retains {removed}"
        );
    }
    for removed in [
        "storage",
        "capabilities",
        "collection_views",
        "constructor",
        "representation",
    ] {
        assert!(!binding_api.contains(removed), "binding retains {removed}");
    }
    assert!(ReflectPermission::from_name("reflect::inspect_host_path").is_none());
}
