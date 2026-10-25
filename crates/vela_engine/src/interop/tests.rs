use std::collections::BTreeMap;

use vela_common::Capability;

use super::{
    BoundaryMode, CallableAccess, CallableContract, CallableIdentity, CallableKind,
    CallableLanguage, CallableOrigin, CallableParameter, CallableReturn, ErrorMode, ReturnMode,
    VelaValueBoundary, VelaValueKeyBoundary,
};
use crate::native::{EffectSet, TypeHint};

fn contract(effects: EffectSet) -> CallableContract {
    CallableContract {
        identity: CallableIdentity::new(CallableKind::RustFunction, 42),
        public_path: "game::grant_exp".to_owned(),
        parameters: vec![CallableParameter::new(
            7,
            "amount",
            TypeHint::i64(),
            BoundaryMode::Value,
        )],
        returns: CallableReturn::new(
            TypeHint::unit(),
            ReturnMode::OwnedValue,
            ErrorMode::RuntimeResult,
        ),
        effects,
        access: CallableAccess::default(),
        docs: Some("not ABI".to_owned()),
        attrs: BTreeMap::new(),
        origin: CallableOrigin {
            language: CallableLanguage::Rust,
            source_span: None,
        },
    }
}

#[test]
fn callable_fingerprint_is_deterministic_and_ignores_docs() {
    let first = contract(EffectSet::host_write());
    let mut second = first.clone();
    second.docs = Some("new docs".to_owned());

    assert_eq!(first.abi_fingerprint(), second.abi_fingerprint());
    assert!(first.abi_diff(&second).is_empty());
}

#[test]
fn redundant_effect_union_does_not_change_fingerprint() {
    let inferred = contract(EffectSet::host_write());
    let redundant = contract(EffectSet::host_write().union(EffectSet::host_read()));

    assert_eq!(inferred.effects, redundant.effects);
    assert_eq!(inferred.abi_fingerprint(), redundant.abi_fingerprint());
}

#[test]
fn effect_projection_is_canonical_and_excludes_redundant_host_read() {
    let contract = contract(EffectSet::host_write().union(EffectSet::random()));
    let capabilities = contract.required_capabilities();

    assert!(capabilities.contains(Capability::HostWrite));
    assert!(capabilities.contains(Capability::Random));
    assert!(!capabilities.contains(Capability::HostRead));
}

#[test]
fn abi_diff_names_changed_semantic_fields() {
    let expected = contract(EffectSet::host_read());
    let actual = contract(EffectSet::host_write());
    let differences = expected.abi_diff(&actual);

    assert_eq!(differences.len(), 1);
    assert_eq!(differences[0].field, "effects");
    assert!(
        differences[0]
            .to_string()
            .contains("callable ABI field `effects` changed")
    );
}

#[test]
fn protocol_identity_depends_on_public_vela_path_not_rust_type_identity() {
    let first = super::VelaProtocolIdentity::new("game::Damageable");
    let second = super::VelaProtocolIdentity::new("game::Damageable");
    let renamed = super::VelaProtocolIdentity::new("game::Target");

    assert_eq!(first, second);
    assert_ne!(first.stable, renamed.stable);
}

#[test]
fn byte_vectors_have_bytes_facts_and_supported_values_prove_stable_keys() {
    fn assert_stable_key<T: VelaValueKeyBoundary>() {}

    assert_eq!(Vec::<u8>::vela_type_hint(), TypeHint::bytes());
    assert_eq!(
        Vec::<i64>::vela_type_hint(),
        TypeHint::array_of(TypeHint::i64())
    );
    assert_stable_key::<i64>();
    assert_stable_key::<String>();
    assert_stable_key::<Vec<u8>>();
}
