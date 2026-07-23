use vela_common::{
    CollectionViewCapabilities, CollectionViewKind, CollectionViewMutation, InteropRepresentation,
};

use super::standard_type_binding;

#[test]
fn byte_binding_preserves_owned_identity_across_growable_views() {
    let bytes = standard_type_binding::<Vec<u8>>();
    assert_eq!(
        bytes.collection_views(),
        Some(CollectionViewCapabilities::mutable(
            CollectionViewKind::Array,
            CollectionViewMutation::Growable,
        ))
    );

    let owned = bytes
        .interop_contract(InteropRepresentation::Owned)
        .expect("owned Bytes representation");
    let shared = bytes
        .interop_contract(InteropRepresentation::CollectionView(
            CollectionViewKind::Array,
        ))
        .expect("borrowed byte view");
    assert_eq!(owned.type_id, shared.type_id);
    assert_eq!(owned.abi_fingerprint, shared.abi_fingerprint);
}
