use vela_common::{InteropTypeId, TypeAbiFingerprint, TypeBindingRegistryChecksum};
use vela_def::{DefPath, TypeId};
use vela_reflect::registry::{TypeDesc, TypeKey, TypeRegistry};
use vela_reflect::type_binding::TypeBindingDesc;
use vela_registry::{DefinitionRegistry, TypeBindingDef, TypeDef};

use super::{RegistryFacts, RegistryTypeBindingFact};

#[test]
fn owned_value_binding_identity_abi_and_checksum_survive_both_projections() {
    let id = TypeId::new(u128::MAX);
    let interop = InteropTypeId::from_type_id(id);
    let abi = TypeAbiFingerprint::new(17);
    let checksum = TypeBindingRegistryChecksum::new(23);
    let mut definitions = DefinitionRegistry::new();
    definitions
        .register_type(
            TypeDef::new(DefPath::ty("host", ["value"], "Row"))
                .with_id(id)
                .binding(TypeBindingDef::new(interop, abi)),
        )
        .expect("valid owned binding metadata");
    definitions.seal_type_bindings(checksum);
    let compile = RegistryFacts::from_compile_view(definitions.compile_view())
        .expect("valid owned binding metadata");

    let key = TypeKey::new(id, "value::Row");
    let mut reflection = TypeRegistry::new();
    reflection.register(TypeDesc::new(key.clone()));
    reflection.install_type_bindings([TypeBindingDesc::new(interop, key, abi)], checksum);
    let reflected = RegistryFacts::from_registry(&reflection);
    let expected = RegistryTypeBindingFact {
        id: interop,
        abi_fingerprint: abi,
    };
    for facts in [&compile, &reflected] {
        assert_eq!(facts.type_binding_fact("value::Row"), Some(&expected));
        assert_eq!(facts.type_binding_checksum(), Some(checksum));
        assert_eq!(
            facts
                .type_target_fact("value::Row")
                .expect("valid owned binding metadata")
                .semantic,
            id
        );
    }
    assert_ne!(
        expected,
        RegistryTypeBindingFact {
            id: interop,
            abi_fingerprint: TypeAbiFingerprint::new(18),
        }
    );
}

#[test]
fn shared_schemas_export_no_borrowed_object_or_path_facts() {
    let common = include_str!("../../../vela_common/src/interop_type.rs");
    let ids = include_str!("../../../vela_common/src/lib.rs");
    let definitions = include_str!("../../../vela_def/src/lib.rs");
    let registry = include_str!("../../../vela_registry/src/defs.rs");
    let analysis = include_str!("../registry.rs");
    let targets = include_str!("../semantic_facts/targets.rs");
    for removed in [
        "CollectionView",
        "HostCollectionIteration",
        "HostConstructionLifetime",
        "HostConstructorBinding",
        "InteropRepresentation",
        "StoragePolicy",
        "ReceiverCapabil",
    ] {
        assert!(!common.contains(removed), "common schema retains {removed}");
    }
    for removed in ["HostMethodId", "HostObjectId", "HostTypeId"] {
        assert!(!ids.contains(removed), "common exports retain {removed}");
    }
    for removed in ["host_release_function_id", "host_try_release_function_id"] {
        assert!(
            !definitions.contains(removed),
            "definition IDs retain {removed}"
        );
    }
    for schema in [registry, analysis] {
        for removed in [
            "ScopedResource",
            "host_runtime",
            "collection_views",
            "host_constructors",
            "constructor_ids",
            "IndexCapability",
            "ReceiverCapability",
        ] {
            assert!(!schema.contains(removed), "schema retains {removed}");
        }
    }
    for removed in ["HostPath", "HostField", "HostProperty", "HostMethod"] {
        assert!(
            !targets.contains(removed),
            "semantic targets retain {removed}"
        );
    }
}
