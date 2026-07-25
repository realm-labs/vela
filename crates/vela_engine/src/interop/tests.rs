use std::collections::BTreeMap;

use vela_common::{
    CallableAsyncness, Capability, CollectionViewKind, CollectionViewMutation, HostObjectId,
    HostTypeId, InteropBindingContract, InteropRepresentation, InteropTypeId, TypeAbiFingerprint,
};
use vela_host::lease::HostLeaseKind;
use vela_host::path::HostRef;
use vela_vm::error::VmErrorKind;
use vela_vm::owned_value::OwnedValue;

use super::{
    BoundaryMode, CallableAccess, CallableContract, CallableIdentity, CallableKind,
    CallableLanguage, CallableOrigin, CallableParameter, CallableReturn, ErrorMode,
    HostLeaseParameterPlan, HostParamLeaseRequest, PreparedHostLeasePlan, ReturnMode,
    VelaValueBoundary, VelaValueKeyBoundary, preflight_host_parameter_leases,
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
        asyncness: CallableAsyncness::Sync,
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
fn callable_abi_includes_exact_type_binding_and_representation() {
    let mut btree = contract(EffectSet::host_read());
    btree.parameters[0] = btree.parameters[0]
        .clone()
        .with_binding(InteropBindingContract::new(
            InteropTypeId::new(11),
            InteropRepresentation::CollectionView(CollectionViewKind::Map),
            TypeAbiFingerprint::new(101),
        ));
    let mut hashed = btree.clone();
    hashed.parameters[0].binding = Some(InteropBindingContract::new(
        InteropTypeId::new(12),
        InteropRepresentation::CollectionView(CollectionViewKind::Map),
        TypeAbiFingerprint::new(102),
    ));
    let mut mutable = btree.clone();
    mutable.parameters[0].binding = Some(InteropBindingContract::new(
        InteropTypeId::new(11),
        InteropRepresentation::CollectionMut {
            kind: CollectionViewKind::Map,
            mutation: CollectionViewMutation::Growable,
        },
        TypeAbiFingerprint::new(101),
    ));

    assert_ne!(btree.abi_fingerprint(), hashed.abi_fingerprint());
    assert_ne!(btree.abi_fingerprint(), mutable.abi_fingerprint());
    assert_eq!(btree.abi_diff(&hashed)[0].field, "parameters");
    assert_eq!(btree.abi_diff(&mutable)[0].field, "parameters");
}

#[test]
fn callable_abi_distinguishes_fixed_and_growable_collection_hints() {
    let mut fixed = contract(EffectSet::host_write());
    fixed.parameters[0].ty = TypeHint::array_mut_of(TypeHint::i64(), CollectionViewMutation::Fixed);
    let mut growable = fixed.clone();
    growable.parameters[0].ty =
        TypeHint::array_mut_of(TypeHint::i64(), CollectionViewMutation::Growable);

    assert_ne!(fixed.abi_fingerprint(), growable.abi_fingerprint());
    assert_eq!(fixed.abi_diff(&growable)[0].field, "parameters");
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

fn host_request(
    parameter_identity: u64,
    parameter: &str,
    root: HostRef,
    mode: HostLeaseKind,
) -> HostParamLeaseRequest {
    let mut contract = contract(EffectSet::host_write());
    contract.parameters = vec![CallableParameter::new(
        parameter_identity,
        parameter,
        TypeHint::Any,
        if mode == HostLeaseKind::Shared {
            BoundaryMode::SharedHost
        } else {
            BoundaryMode::ExclusiveHost
        },
    )];
    HostParamLeaseRequest::from_argument(
        &contract,
        0,
        0,
        root.type_id,
        mode,
        &OwnedValue::HostRef(root),
    )
    .expect("matching host argument should form a request")
}

#[test]
fn host_request_reports_exact_type_mismatch_before_leasing() {
    let argument = OwnedValue::HostRef(HostRef::new(HostTypeId::new(2), HostObjectId::new(9), 0));
    let error = HostParamLeaseRequest::from_argument(
        &CallableContract {
            parameters: vec![CallableParameter::new(
                7,
                "player",
                TypeHint::Any,
                BoundaryMode::ExclusiveHost,
            )],
            ..contract(EffectSet::host_write())
        },
        0,
        0,
        HostTypeId::new(1),
        HostLeaseKind::Exclusive,
        &argument,
    )
    .expect_err("wrong exact host type must fail before lease acquisition");

    assert_eq!(
        error.kind(),
        VmErrorKind::HostArgumentTypeMismatch {
            callable: "game::grant_exp".to_owned(),
            parameter: "player".to_owned(),
            expected: HostTypeId::new(1),
            actual: HostTypeId::new(2),
        }
    );
}

#[test]
fn host_preflight_uses_canonical_identity_for_alias_matrix() {
    let root = HostRef::new(HostTypeId::new(1), HostObjectId::new(9), 3);
    let shared = [
        host_request(0, "first", root, HostLeaseKind::Shared),
        host_request(1, "second", root, HostLeaseKind::Shared),
    ];
    let prepared =
        preflight_host_parameter_leases(&shared).expect("shared aliases should pass preflight");
    assert_eq!(
        prepared.as_slice(),
        &[(root, HostLeaseKind::Shared), (root, HostLeaseKind::Shared)]
    );
    assert!(
        !prepared.spilled(),
        "ordinary host arities should remain inline"
    );

    let conflict = [
        host_request(0, "first", root, HostLeaseKind::Shared),
        host_request(1, "second", root, HostLeaseKind::Exclusive),
    ];
    assert!(matches!(
        preflight_host_parameter_leases(&conflict)
            .expect_err("mixed alias should fail")
            .kind(),
        VmErrorKind::AliasedMutableHostArguments { .. }
    ));
}

#[test]
fn prepared_host_plan_reuses_registration_metadata_and_stays_inline() {
    let first = HostRef::new(HostTypeId::new(1), HostObjectId::new(9), 3);
    let second = HostRef::new(HostTypeId::new(1), HostObjectId::new(10), 3);
    let contract = CallableContract {
        parameters: vec![
            CallableParameter::new(7, "first", TypeHint::Any, BoundaryMode::ExclusiveHost),
            CallableParameter::new(8, "second", TypeHint::Any, BoundaryMode::SharedHost),
        ],
        ..contract(EffectSet::host_write())
    };
    let plan = PreparedHostLeasePlan::new(
        contract,
        2,
        [
            HostLeaseParameterPlan::argument(0, 0, HostTypeId::new(1), HostLeaseKind::Exclusive),
            HostLeaseParameterPlan::argument(1, 1, HostTypeId::new(1), HostLeaseKind::Shared),
        ],
    );

    let prepared = plan
        .prepare(&[OwnedValue::HostRef(first), OwnedValue::HostRef(second)])
        .expect("distinct prepared host arguments should pass");

    assert_eq!(
        prepared.as_slice(),
        &[
            (first, HostLeaseKind::Exclusive),
            (second, HostLeaseKind::Shared)
        ]
    );
    assert!(!prepared.spilled());
    assert_eq!(plan.callable(), "game::grant_exp");
}

#[test]
fn prepared_method_plan_checks_receiver_alias_before_leasing() {
    let root = HostRef::new(HostTypeId::new(1), HostObjectId::new(9), 3);
    let contract = CallableContract {
        parameters: vec![
            CallableParameter::new(7, "self", TypeHint::Any, BoundaryMode::ExclusiveHost),
            CallableParameter::new(8, "other", TypeHint::Any, BoundaryMode::SharedHost),
        ],
        ..contract(EffectSet::host_write())
    };
    let plan = PreparedHostLeasePlan::new(
        contract,
        1,
        [
            HostLeaseParameterPlan::receiver(0, HostTypeId::new(1), HostLeaseKind::Exclusive),
            HostLeaseParameterPlan::argument(1, 0, HostTypeId::new(1), HostLeaseKind::Shared),
        ],
    );

    let error = plan
        .prepare_method(root, &[OwnedValue::HostRef(root)])
        .expect_err("receiver and shared argument must conflict");

    assert_eq!(
        error.kind(),
        VmErrorKind::AliasedMutableHostArguments {
            callable: "game::grant_exp".to_owned(),
            first_parameter: "self".to_owned(),
            second_parameter: "other".to_owned(),
        }
    );
}
