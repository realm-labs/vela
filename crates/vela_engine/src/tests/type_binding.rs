use vela_analysis::registry::RegistryFacts;
use vela_common::{
    HostMethodId, HostTypeId, InteropTypeId, ReceiverCapabilities, ReceiverCapability,
    StoragePolicy,
};
use vela_def::{FieldId, TypeId};
use vela_reflect::registry::{FieldDesc, TypeDesc, TypeKey, TypeKind};

use crate::engine::Engine;
use crate::error::EngineErrorKind;
use crate::method::NativeMethodDesc;
use crate::native::{EffectSet, TypeHint};
use crate::type_binding::TypeBinding;

struct ExternalHost;
struct ExternalValue;

fn host_desc(id: u128, host_id: u64) -> TypeDesc {
    TypeDesc::new(TypeKey::new(TypeId::new(id), "host::ExternalHost"))
        .host_type(HostTypeId::new(host_id))
        .field(
            FieldDesc::new(FieldId::new(11), "value")
                .type_hint("i64")
                .writable(true),
        )
}

fn value_desc(id: u128, field_name: &str) -> TypeDesc {
    TypeDesc::new(TypeKey::new(TypeId::new(id), "host::ExternalValue"))
        .kind(TypeKind::ScriptStruct)
        .field(FieldDesc::new(FieldId::new(21), field_name).type_hint("i64"))
}

#[test]
fn unified_type_binding_is_sealed_into_engine_and_reflection_registry() {
    let owner = host_desc(101, 201).key;
    let engine = Engine::builder()
        .register_rust_type::<ExternalHost>(
            TypeBinding::host(host_desc(101, 201)).method_desc(
                NativeMethodDesc::new(owner.clone(), HostMethodId::new(301), "read")
                    .returns(TypeHint::i64())
                    .effects(EffectSet::host_read()),
            ),
        )
        .register_rust_type::<ExternalValue>(TypeBinding::value(value_desc(102, "amount")))
        .build()
        .expect("unified type bindings should seal");

    let bindings = engine.type_bindings();
    let host = bindings
        .get_for::<ExternalHost>()
        .expect("Rust host type binding");
    assert_eq!(host.id, InteropTypeId::new(101));
    assert_eq!(host.storage, StoragePolicy::Host);
    assert!(host.capabilities.contains(ReceiverCapability::Owned));
    assert!(host.capabilities.contains(ReceiverCapability::Shared));
    assert!(host.capabilities.contains(ReceiverCapability::Exclusive));
    assert!(!host.capabilities.contains(ReceiverCapability::Construct));

    let reflected_registry = engine.registry();
    assert_eq!(
        reflected_registry
            .type_binding_for_key(&owner)
            .expect("reflection uses the sealed binding facts"),
        host
    );
    assert_eq!(
        reflected_registry
            .type_binding_snapshot()
            .expect("sealed binding snapshot")
            .checksum(),
        bindings.checksum()
    );
    assert!(
        reflected_registry
            .type_by_name("host::ExternalHost")
            .expect("type metadata")
            .methods
            .iter()
            .any(|method| method.name == "read")
    );

    let value = bindings
        .get_for::<ExternalValue>()
        .expect("Rust value type binding");
    assert_eq!(value.storage, StoragePolicy::Value);
    assert_eq!(reflected_registry.type_bindings().count(), 2);

    let compiler_facts = RegistryFacts::from_compile_view(engine.compiler_registry())
        .expect("compiler binding facts");
    assert_eq!(
        compiler_facts
            .type_binding_fact("host::ExternalHost")
            .expect("compiler consumes the same binding")
            .abi_fingerprint,
        host.abi_fingerprint
    );
    assert_eq!(
        compiler_facts.type_binding_checksum(),
        Some(bindings.checksum())
    );
}

#[test]
fn type_binding_fingerprint_ignores_docs_but_tracks_structural_abi() {
    let first = Engine::builder()
        .register_rust_type::<ExternalValue>(TypeBinding::value(
            value_desc(102, "amount").docs("first docs"),
        ))
        .build()
        .expect("first binding");
    let second = Engine::builder()
        .register_rust_type::<ExternalValue>(TypeBinding::value(
            value_desc(102, "amount").docs("different docs"),
        ))
        .build()
        .expect("second binding");
    let changed = Engine::builder()
        .register_rust_type::<ExternalValue>(TypeBinding::value(value_desc(102, "total")))
        .build()
        .expect("changed binding");

    let first = first.type_bindings();
    let second = second.type_bindings();
    let changed = changed.type_bindings();
    assert_eq!(
        first
            .get_for::<ExternalValue>()
            .expect("first value binding")
            .abi_fingerprint,
        second
            .get_for::<ExternalValue>()
            .expect("second value binding")
            .abi_fingerprint
    );
    assert_eq!(first.checksum(), second.checksum());
    assert_ne!(
        first
            .get_for::<ExternalValue>()
            .expect("first value binding")
            .abi_fingerprint,
        changed
            .get_for::<ExternalValue>()
            .expect("changed value binding")
            .abi_fingerprint
    );
    assert_ne!(first.checksum(), changed.checksum());
}

#[test]
fn type_binding_rejects_ambiguous_storage_and_receiver_capabilities() {
    let value_with_host_storage = Engine::builder()
        .register_rust_type::<ExternalValue>(TypeBinding::value(host_desc(101, 201)))
        .build();
    assert!(matches!(
        value_with_host_storage,
        Err(error)
            if error.kind == EngineErrorKind::InvalidTypeBindingStorage {
                name: "host::ExternalHost".to_owned(),
                storage: "value".to_owned(),
            }
    ));

    let exclusive_without_shared = Engine::builder()
        .register_rust_type::<ExternalHost>(
            TypeBinding::host(host_desc(101, 201)).receiver_capabilities(
                ReceiverCapabilities::OWNED.with(ReceiverCapability::Exclusive),
            ),
        )
        .build();
    assert!(matches!(
        exclusive_without_shared,
        Err(error)
            if error.kind == EngineErrorKind::InvalidTypeBindingCapabilities {
                name: "host::ExternalHost".to_owned(),
                bits: 5,
            }
    ));
}

#[test]
fn one_rust_type_cannot_map_to_two_interop_identities() {
    let result = Engine::builder()
        .register_rust_type::<ExternalValue>(TypeBinding::value(value_desc(102, "amount")))
        .register_rust_type::<ExternalValue>(TypeBinding::value(
            TypeDesc::new(TypeKey::new(TypeId::new(103), "host::OtherValue"))
                .kind(TypeKind::ScriptStruct),
        ))
        .build();
    assert!(matches!(
        result,
        Err(error)
            if error.kind == EngineErrorKind::DuplicateRustTypeBinding {
                first: 102,
                second: 103,
            }
    ));
}
